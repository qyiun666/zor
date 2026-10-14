use std::collections::HashMap;
use crate::diag::Diag;
use crate::parser::*;

/// R2 所有权状态：类型 + 移动/借用位（块尾失活：check_block 出块时清零借方位）
#[derive(Clone, Debug)]
pub struct Var {
    pub ty: Ty,
    pub moved: bool,
    pub unboxed: bool,
    pub partial_moved: bool,
    pub moved_fields: Vec<String>,
    pub shared: u32,
    pub mut_active: bool,
}

impl Var {
    fn fresh(ty: Ty) -> Self {
        Var { ty, moved: false, unboxed: false, partial_moved: false, moved_fields: vec![], shared: 0, mut_active: false }
    }
}

pub struct Ck<'a> {
    prog: &'a Prog,
    diags: Vec<Diag>,
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, HashMap<String, Vec<Ty>>>,
    fns: HashMap<String, (Vec<Ty>, Ty)>,
}

fn int_bits(p: &str) -> Option<u32> {
    Some(match p { "i8" => 8, "i16" => 16, "i32" => 32, "i64" => 64, "u8" => 8, "u16" => 16, "u32" => 32, "u64" => 64, "byte" => 8, _ => return None })
}

impl<'a> Ck<'a> {
    pub fn new(prog: &'a Prog) -> Self {
        Ck { prog, diags: vec![], structs: HashMap::new(), enums: HashMap::new(), fns: HashMap::new() }
    }

    pub fn run(mut self) -> Vec<Diag> {
        // 1. 注册具名类型/函数
        for s in &self.prog.structs {
            self.structs.insert(s.name.clone(), vec![]);
        }
        for e in &self.prog.enums {
            self.enums.insert(e.name.clone(), HashMap::new());
        }
        for f in &self.prog.fns {
            self.fns.insert(f.name.clone(), (vec![], Ty::unit()));
        }
        // 2. 解析声明签名
        for s in &self.prog.structs {
            let fields = s.fields.iter().map(|(n, t, l, c)| (n.clone(), self.resolve(t.clone(), *l, *c, true))).collect::<Vec<_>>();
            self.structs.insert(s.name.clone(), fields);
        }
        for e in &self.prog.enums {
            let mut vs = HashMap::new();
            for (vn, payload, vl, vc) in &e.variants {
                let pl = payload.iter().map(|(t, l, c)| self.resolve(t.clone(), *l, *c, true)).collect::<Vec<_>>();
                let _ = (vl, vc);
                vs.insert(vn.clone(), pl);
            }
            self.enums.insert(e.name.clone(), vs);
        }
        for f in &self.prog.fns {
            let ps = f.params.iter().map(|(_, t, l, c)| self.resolve(t.clone(), *l, *c, false)).collect::<Vec<_>>();
            let ret = match &f.ret {
                Some((t, l, c)) => {
                    // 返回位引用=E1502（parse_ty(TyPos::Return) 已报，此处兜底）
                    let r = self.resolve(t.clone(), *l, *c, true);
                    if matches!(r, Ty::Ref(..)) { self.d("E1502", *l, *c, "返回类型不得含引用（R2 §4）"); Ty::Bad } else { r }
                }
                None => Ty::unit(),
            };
            self.fns.insert(f.name.clone(), (ps, ret));
        }
        // 3. enum/struct 递归检查（E1401）
        self.check_recursion();
        // 4. 函数体检查
        let fns = &self.prog.fns;
        for fd in fns {
            let key = fd.name.clone();
            let ret = self.fns.get(&key).map(|(_, r)| r.clone()).unwrap_or(Ty::unit());
            let ps = self.fns.get(&key).map(|(ps, _)| ps.clone()).unwrap_or_default();
            let mut env = HashMap::new();
            for ((pn, _, _, _), pt) in fd.params.iter().zip(ps.iter()) {
                env.insert(pn.clone(), Var::fresh(pt.clone()));
            }
            self.check_block(&fd.body, Some(&ret), &mut env);
        }
        self.diags
    }

    fn d(&mut self, code: &str, l: usize, c: usize, msg: impl Into<String>) {
        self.diags.push(Diag::new(code, l, c, msg));
    }

    fn known_named(&self, n: &str) -> bool {
        self.structs.contains_key(n) || self.enums.contains_key(n)
    }

    /// R1 §5 Copy 集结构判定：prim/unit Copy；数组/元组/struct/enum 成员全 Copy 才 Copy；
    /// Box 非 Copy；&T Copy、&mut T unique-move 非 Copy。Bad 宽松放行。
    fn is_copy(&self, t: &Ty) -> bool {
        match t {
            Ty::Prim(_) => true,
            Ty::BoxTy(_) => false,
            Ty::Ref(_, m) => !*m,
            Ty::Arr(i, _) => self.is_copy(i),
            Ty::Tuple(ts) => ts.iter().all(|x| self.is_copy(x)),
            Ty::Named(n) => {
                if let Some(fs) = self.structs.get(n) {
                    return fs.iter().all(|(_, t)| self.is_copy(t));
                }
                if let Some(vs) = self.enums.get(n) {
                    return vs.values().all(|pl| pl.iter().all(|x| self.is_copy(x)));
                }
                false
            }
            Ty::Bad => true,
        }
    }

    /// 读检查：unboxed→E2301（先于 moved，unbox 语义即终结）；moved→E2101；whole 且 partial→E2102
    fn read_check(&mut self, n: &str, l: usize, c: usize, v: &Var, whole: bool) {
        if v.unboxed {
            self.d("E2301", l, c, format!("'{}' 已 unbox（释放/交出后再使用，R2 §5/§8）", n));
        } else if v.moved {
            self.d("E2101", l, c, format!("'{}' 已移出（移动后使用，R2 §1）", n));
        } else if whole && v.partial_moved {
            self.d("E2102", l, c, format!("'{}' 已部分移动，不可整体使用（R2 §1；仅可访问其余字段）", n));
        }
    }

    /// 移动源登记：e 属主转移入新位置（绑定/实参/返回/赋值/容器槽位）
    fn note_move_src(&mut self, e: &Ex, ty: &Ty, env: &mut HashMap<String, Var>) {
        if self.is_copy(ty) { return; }
        match e {
            Ex::Path(n, _, _) => {
                if let Some(v) = env.get_mut(n) { v.moved = true; }
            }
            Ex::Field(base, fname, _, _) => {
                if let Ex::Path(n, _, _) = base.as_ref() {
                    if let Some(v) = env.get_mut(n) {
                        v.partial_moved = true;
                        if !v.moved_fields.iter().any(|f| f == fname) { v.moved_fields.push(fname.clone()); }
                    }
                }
            }
            Ex::Box(inner, _, _) => {
                let inner_ty = if let Ty::BoxTy(i) = ty { (**i).clone() } else { ty.clone() };
                self.note_move_src(inner, &inner_ty, env);
            }
            Ex::Tup(es, _, _) => {
                for (i, x) in es.iter().enumerate() {
                    let t = if let Ty::Tuple(ts) = ty { ts.get(i).cloned().unwrap_or(Ty::Bad) } else { Ty::Bad };
                    self.note_move_src(x, &t, env);
                }
            }
            _ => {}
        }
    }

    /// 引用位置容器检查兜底（parse_ty 已拦容器，此处处理 Bad/嵌套）
    fn resolve(&mut self, t: Ty, l: usize, c: usize, in_container: bool) -> Ty {
        match t {
            Ty::Named(n) => {
                if self.known_named(&n) { Ty::Named(n) } else {
                    self.d("E1101", l, c, format!("unknown type name '{}'", n));
                    Ty::Bad
                }
            }
            Ty::Arr(inner, k) => {
                let i = self.resolve(*inner, l, c, true);
                if matches!(i, Ty::Ref(..)) { self.d("E1501", l, c, "数组元素不得为引用类型"); }
                Ty::Arr(Box::new(i), k)
            }
            Ty::BoxTy(inner) => {
                let i = self.resolve(*inner, l, c, true); // Box 内禁引用（R1 §3 容器位）
                Ty::BoxTy(Box::new(i))
            }
            Ty::Tuple(ts) => Ty::Tuple(ts.into_iter().map(|x| self.resolve(x, l, c, in_container)).collect()),
            Ty::Ref(inner, m) => {
                if in_container { self.d("E1501", l, c, "引用类型不得出现在容器位置"); return Ty::Bad; }
                Ty::Ref(Box::new(self.resolve(*inner, l, c, false)), m)
            }
            other => other,
        }
    }

    fn edges_of(&self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(fs) = self.structs.get(name) {
            for (_, t) in fs { collect_named(t, &mut out); }
        }
        if let Some(vs) = self.enums.get(name) {
            for pl in vs.values() { for t in pl { collect_named(t, &mut out); } }
        }
        out
    }

    fn check_recursion(&mut self) {
        // 每个具名类型做 DFS，检测能否回到自身（传递含直接递归）；Box 间址断环（R2 §7 链表可表达）
        let names: Vec<String> = {
            let mut v: Vec<String> = self.structs.keys().cloned().collect();
            v.extend(self.enums.keys().cloned());
            v.sort();
            v
        };
        let mut cyc: HashMap<String, Vec<String>> = HashMap::new();
        for n in &names {
            let mut stack = vec![(n.clone(), self.edges_of(n), 0usize)];
            let mut visited: HashMap<String, bool> = HashMap::new();
            let mut path = vec![n.clone()];
            if let Some(found) = dfs_cycle(&mut stack, &mut path, &mut visited, self, n.clone()) {
                cyc.insert(n.clone(), found);
            }
        }
        for (n, path) in &cyc {
            let is_enum = self.enums.contains_key(n);
            let (l, c) = if is_enum {
                // 报在回边变体处：载荷引用路径下一跳的变体
                let next = &path[1];
                self.enums.get(n).and_then(|vs| {
                    vs.iter().find(|(_, pl)| pl.iter().any(|t| named_is(t, next))).map(|(vn, _)| variant_span(self.prog, n, vn)).flatten()
                }).unwrap_or((0, 0))
            } else {
                (0, 0)
            };
            if is_enum {
                self.d("E1401", l, c, format!("enum '{}' 变体载荷递归引用自身（0.1.x 禁止，传递路径 {:?}）", n, path));
            } else {
                self.d("E1401", 0, 0, format!("struct '{}' 按值循环依赖（大小不可确定）", n));
            }
        }
    }

    // ———— 语句/块检查 ————

    fn check_block(&mut self, blk: &Blk, expect: Option<&Ty>, env: &mut HashMap<String, Var>) {
        let mut env2 = env.clone();
        for st in &blk.stmts { self.check_stmt(st, &mut env2); }
        if let Some(tail) = &blk.tail {
            let t = self.check_expr(tail, expect, &mut env2);
            if let Some(e) = expect {
                if !compatible(e, &t) { self.mismatch(t_span(tail), e, &t); }
            }
            // 返回=移动点（R2 §1）
            self.note_move_src(tail.as_ref(), &t, &mut env2);
        } else if let Some(e) = expect {
            if e != &Ty::unit() && e != &Ty::Bad {
                self.d("E1601", blk.span.0, blk.span.1, format!("函数体需尾表达式返回 '{}'", e.display()));
            }
        }
        // 块尾失活（R2 §2：0.1.x 不做流敏感收窄）：借方位出块清零；移动/unbox 状态外传（保守合并）
        for (_, v) in env2.iter_mut() { v.shared = 0; v.mut_active = false; }
        *env = env2;
    }

    fn mismatch(&mut self, (l, c): (usize, usize), expect: &Ty, got: &Ty) {
        if got == &Ty::Bad || expect == &Ty::Bad { return; }
        self.d("E1201", l, c, format!("类型不匹配：期望 '{}'，实际 '{}'", expect.display(), got.display()));
    }

    fn check_stmt(&mut self, st: &St, env: &mut HashMap<String, Var>) {
        match st {
            St::Let(name, ann, init, l, c) => {
                let vt = if let Some(a) = ann {
                    let r = a.clone();
                    let got = self.check_expr(init, Some(&r), env);
                    if !compatible(&r, &got) && got != Ty::Bad { self.mismatch(t_span(init), &r, &got); }
                    r
                } else {
                    self.check_expr(init, None, env)
                };
                if matches!(ann, Some(Ty::Ref(..))) { self.d("E1501", *l, *c, "引用类型只能出现在函数参数/局部借用，不得作为 let 标注持久化"); }
                self.note_move_src(init, &vt, env);
                // 持久借用记账：句柄绑定 `let h = &v / &mut v`（活跃至所在块尾，R2 §2）
                if let Ex::Ref(inner, m, _, _) = init {
                    if let Ex::Path(n, _, _) = inner.as_ref() {
                        if let Some(v) = env.get_mut(n) {
                            if *m { v.mut_active = true; } else { v.shared += 1; }
                        }
                    }
                }
                env.insert(name.clone(), Var::fresh(vt));
            }
            St::LetDecl(name, _, l, c) => {
                self.d("E1601", *l, *c, format!("let '{}' 缺少初始化（0.1.x 不支持无初始值绑定）", name));
            }
            St::Assign(lhs, rhs, _l, _c) => {
                let rt = self.check_expr(rhs, None, env);
                self.note_move_src(rhs, &rt, env);
                let lt = match lhs {
                    Ex::Path(n, ll, lc) => {
                        // 重赋整值：恢复/覆盖所有权状态（R2 §1 再赋整值恢复），不做读检查
                        match env.get_mut(n) {
                            Some(v) => { let t = v.ty.clone(); *v = Var::fresh(rt.clone()); t }
                            None => { self.d("E1101", *ll, *lc, format!("unknown identifier '{}'", n)); Ty::Bad }
                        }
                    }
                    Ex::Deref(base, dl, dc) => {
                        let bt = self.check_expr(base, None, env);
                        match bt {
                            Ty::BoxTy(i) => *i,
                            Ty::Ref(i, true) => *i,
                            Ty::Ref(i, false) => { self.d("E2201", *dl, *dc, "共享借用期间不可经 * 写入（R2 §2）"); *i }
                            Ty::Bad => Ty::Bad,
                            other => { self.d("E1201", *dl, *dc, format!("不可解引用赋值于 '{}'", other.display())); Ty::Bad }
                        }
                    }
                    Ex::Field(base, fname, fl, fc) => {
                        let ft = self.field_place_ty(base, fname, *fl, *fc, env);
                        // 写回被移出的字段=恢复该字段（R2 §1）
                        if let Ex::Path(n, _, _) = base.as_ref() {
                            if let Some(v) = env.get_mut(n) {
                                v.moved_fields.retain(|f| f != fname);
                                if v.moved_fields.is_empty() { v.partial_moved = false; }
                            }
                        }
                        ft
                    }
                    other => self.check_expr(other, None, env),
                };
                if !compatible(&lt, &rt) { self.mismatch(t_span(lhs), &lt, &rt); }
            }
            St::Expr(Some(e), _, _) => { self.check_expr(e, None, env); }
            St::Expr(None, _, _) => {}
            St::If(cond, then, els, l, c) => {
                let ct = self.check_expr(cond, Some(&Ty::Prim("bool")), env);
                if !compatible(&Ty::Prim("bool"), &ct) { self.mismatch((*l, *c), &Ty::Prim("bool"), &ct); }
                self.check_block(then, None, env);
                match els {
                    Some(ElseTail::Block(b)) => { self.check_block(b, None, env); }
                    Some(ElseTail::If(s)) => { self.check_stmt(s, env); }
                    None => {}
                }
            }
            St::While(cond, body, l, c) => {
                let ct = self.check_expr(cond, Some(&Ty::Prim("bool")), env);
                if !compatible(&Ty::Prim("bool"), &ct) { self.mismatch((*l, *c), &Ty::Prim("bool"), &ct); }
                self.check_block(body, None, env);
            }
        }
    }

    /// 字段可放置/读取的类型（含 moved_fields 判定；不触发 E2102 整体使用）
    fn field_place_ty(&mut self, base: &Ex, fname: &str, l: usize, c: usize, env: &mut HashMap<String, Var>) -> Ty {
        let bt = if let Ex::Path(n, bl, bc) = base {
            match env.get(n).cloned() {
                Some(v) => {
                    if v.unboxed || v.moved {
                        self.read_check(n, *bl, *bc, &v, false);
                        Ty::Bad
                    } else if v.moved_fields.iter().any(|f| f == fname) {
                        self.d("E2101", l, c, format!("字段 '{}' 已移出，不可使用（R2 §1 部分移动）", fname));
                        Ty::Bad
                    } else {
                        v.ty.clone()
                    }
                }
                None => { self.d("E1101", *bl, *bc, format!("unknown identifier '{}'", n)); Ty::Bad }
            }
        } else {
            self.check_expr(base, None, env)
        };
        self.project_field(bt, fname, l, c)
    }

    /// Named/Ref/Box 上取字段类型（b.field 自动借用糖，R2 §5）
    fn project_field(&mut self, mut bt: Ty, fname: &str, l: usize, c: usize) -> Ty {
        if let Ty::Ref(inner, _) = bt { bt = (*inner).clone(); }
        if let Ty::BoxTy(inner) = bt { bt = (*inner).clone(); }
        match &bt {
            Ty::Named(n) => {
                if let Some(fs) = self.structs.get(n) {
                    if let Some((_, t)) = fs.iter().find(|(f, _)| f == fname) { t.clone() }
                    else { self.d("E1201", l, c, format!("struct '{}' 无字段 '{}'", n, fname)); Ty::Bad }
                } else { self.d("E1201", l, c, format!("'{}' 不是 struct", bt.display())); Ty::Bad }
            }
            Ty::Bad => Ty::Bad,
            other => { self.d("E1201", l, c, format!("不可对其取字段 '{}'", other.display())); Ty::Bad }
        }
    }

    fn check_expr(&mut self, e: &Ex, expect: Option<&Ty>, env: &mut HashMap<String, Var>) -> Ty {
        match e {
            Ex::Lit(lit, l, c) => self.check_lit(lit, expect, *l, *c),
            Ex::UnitLit(l, c) => {
                if let Some(x) = expect { if x != &Ty::unit() && x != &Ty::Bad { self.mismatch((*l, *c), x, &Ty::unit()); } }
                Ty::unit()
            }
            Ex::Path(n, l, c) => {
                if let Some(v) = env.get(n).cloned() {
                    self.read_check(n, *l, *c, &v, true);
                    v.ty
                } else { self.d("E1101", *l, *c, format!("unknown identifier '{}'", n)); Ty::Bad }
            }
            Ex::Bin(op, a, b, l, c) => self.check_bin(op, a, b, *l, *c, env),
            Ex::Un(op, a, l, c) => {
                let t = self.check_expr(a, None, env);
                match op.as_str() {
                    "-" => { if !t.is_num() && t != Ty::Bad { self.d("E1201", *l, *c, format!("一元 '-' 要求数值类型，实际 '{}'", t.display())); } t }
                    "!" => { if !t.is_bool() && t != Ty::Bad { self.d("E1201", *l, *c, format!("'!' 要求 bool，实际 '{}'", t.display())); } Ty::Prim("bool") }
                    _ => t,
                }
            }
            Ex::Deref(base, l, c) => {
                let bt = self.check_expr(base, None, env);
                match bt {
                    Ty::Ref(inner, _) => *inner,
                    Ty::BoxTy(inner) => *inner,
                    Ty::Bad => Ty::Bad,
                    other => { self.d("E1201", *l, *c, format!("不可解引用类型 '{}'", other.display())); Ty::Bad }
                }
            }
            Ex::Box(inner, _, _) => {
                let t = self.check_expr(inner, None, env);
                // 值移入堆句柄（R2 §1 移动点：box 构造）
                self.note_move_src(inner, &t, env);
                Ty::BoxTy(Box::new(t))
            }
            Ex::Unbox(n, l, c) => {
                match env.get(n).cloned() {
                    Some(v) => {
                        if v.unboxed { self.d("E2301", *l, *c, format!("'{}' 重复 unbox（double free，R2 §8）", n)); }
                        else if v.moved { self.d("E2101", *l, *c, format!("'{}' 已移出，不可 unbox（R2 §1）", n)); }
                        else if !matches!(v.ty, Ty::BoxTy(_)) { self.d("E1201", *l, *c, format!("unbox 仅适用于 Box 句柄，实际 '{}'", v.ty.display())); }
                        else { if let Some(v) = env.get_mut(n) { v.unboxed = true; } }
                    }
                    None => self.d("E1101", *l, *c, format!("unknown identifier '{}'", n)),
                }
                Ty::unit()
            }
            Ex::Ref(inner, m, l, c) => {
                // R2 §2 借用判定表冲突检查（不记账）：持久借用的记账仅发生在 let 绑定处
                // （§2"借用句柄自创建处到其所在块尾均活跃"——句柄=绑定持有；实参位匿名借用为瞬时）
                if let Ex::Path(n, _, _) = inner.as_ref() {
                    if let Some(vcl) = env.get(n).cloned() {
                        self.read_check(n, *l, *c, &vcl, true);
                        if *m {
                            if vcl.mut_active || vcl.shared > 0 {
                                self.d("E2202", *l, *c, format!("'{}' 已有活跃借用，不可再创建 &mut（R2 §2）", n));
                            }
                        } else if vcl.mut_active {
                            self.d("E2201", *l, *c, format!("'{}' 正被独占借用，不可创建共享借用（R2 §2）", n));
                        }
                    }
                }
                let t = self.check_expr(inner, None, env);
                Ty::Ref(Box::new(t), *m)
            }
            Ex::Call(callee, args, l, c) => {
                if let Ex::Path(n, _, _) = callee.as_ref() {
                    if (n.as_str() == "println" || n.as_str() == "print") && !self.fns.contains_key(n) {
                        for a in args { self.check_expr(a, None, env); }
                        if args.is_empty() && n == "println" { self.d("E1601", *l, *c, "println 至少一个参数"); }
                        return Ty::unit();
                    }
                    if let Some(vs) = self.enums.get(n) {
                        // Enum 是变体构造函数
                        if let Some((vname, _)) = vs.iter().next() { let _ = vname; }
                    }
                    // 变体构造：Name 是 enum 名 → 需要变体名，0.1.x 语法 Variant(...)；先按函数查
                    if let Some((ps, ret)) = self.fns.get(n).cloned() {
                        if ps.len() != args.len() {
                            self.d("E1601", *l, *c, format!("调用 '{}' 实参个数 {}，签名要求 {}", n, args.len(), ps.len()));
                        }
                        for (a, p) in args.iter().zip(ps.iter()) {
                            let at = self.check_expr(a, Some(p), env);
                            if !compatible(p, &at) { self.mismatch(t_span(a), p, &at); }
                            // 所有权形参（非引用）收非 Copy 实参=移动（R2 §1 实参传递）
                            if !matches!(p, Ty::Ref(..)) { self.note_move_src(a, p, env); }
                        }
                        return ret;
                    }
                    // enum 变体构造 Variant(..)：n 为变体名
                    let hit: Option<(String, Vec<Ty>)> = self
                        .enums
                        .iter()
                        .find_map(|(en, vs)| vs.get(n).map(|pl| (en.clone(), pl.clone())));
                    if let Some((en, pl)) = hit {
                        if pl.len() != args.len() {
                            self.d("E1601", *l, *c, format!("变体 '{}' 载荷 {}，实参 {}", n, pl.len(), args.len()));
                        }
                        for (a, p) in args.iter().zip(pl.iter()) {
                            let at = self.check_expr(a, Some(p), env);
                            if !compatible(p, &at) { self.mismatch(t_span(a), p, &at); }
                            if !matches!(p, Ty::Ref(..)) { self.note_move_src(a, p, env); }
                        }
                        return Ty::Named(en);
                    }
                    self.d("E1101", *l, *c, format!("unknown function or variant '{}'", n));
                    for a in args { self.check_expr(a, None, env); }
                    return Ty::Bad;
                }
                self.d("E1601", *l, *c, "仅支持具名函数/变体调用");
                for a in args { self.check_expr(a, None, env); }
                Ty::Bad
            }
            Ex::Method(recv, name, args, l, c) => {
                let rt = self.check_expr(recv, None, env);
                if name.starts_with("to_") {
                    let cand = &name[3..];
                    if let Some(p) = prim_names().into_iter().find(|p| *p == cand) {
                        if args.len() != 1 - 1 { /* arity 0 */ }
                        if !args.is_empty() { self.d("E1601", *l, *c, "to_xxx 转换函数无参数"); }
                        if rt.is_num() || rt == Ty::Bad { return Ty::Prim(p); }
                        self.d("E1201", *l, *c, format!("'{}' 仅数值类型可调用 to_xxx", rt.display()));
                        return Ty::Prim(p);
                    }
                    self.d("E1601", *l, *c, format!("unknown method '{}'（显式转换仅 to_<prim>）", name));
                    return Ty::Bad;
                }
                self.d("E1601", *l, *c, format!("0.1.x 无方法语法，'{}' 未定义", name));
                for a in args { self.check_expr(a, None, env); }
                Ty::Bad
            }
            Ex::Index(base, idx, l, c) => {
                let bt = self.check_expr(base, None, env);
                match bt {
                    Ty::Arr(t, _) => { let it = self.check_expr(idx, None, env); if !it.is_int() && it != Ty::Bad { self.d("E1201", *l, *c, format!("数组下标要求整型，实际 '{}'", it.display())); } *t }
                    Ty::Tuple(ts) => {
                        if let Ex::Lit(Lit::Int(k), _, _) = idx.as_ref() {
                            if (*k as usize) < ts.len() { ts[*k as usize].clone() } else { self.d("E1201", *l, *c, format!("元组下标 {} 越界（arity {}）", k, ts.len())); Ty::Bad }
                        } else { self.d("E1201", *l, *c, "元组下标必须是字面量"); Ty::Bad }
                    }
                    Ty::Bad => { self.check_expr(idx, None, env); Ty::Bad }
                    other => { self.check_expr(idx, None, env); self.d("E1201", *l, *c, format!("不可索引的类型 '{}'", other.display())); Ty::Bad }
                }
            }
            Ex::Field(base, fname, l, c) => {
                self.field_place_ty(base, fname, *l, *c, env)
            }
            Ex::Record(name, fields, l, c) => {
                if let Some(fs) = self.structs.get(name).cloned() {
                    for (fn_, v) in fields {
                        match fs.iter().find(|(f, _)| f == fn_) {
                            Some((_, t)) => {
                                let vt = self.check_expr(v, Some(t), env);
                                if !compatible(t, &vt) { self.mismatch(t_span(v), t, &vt); }
                                // 移入容器槽位（R2 §1：owner=绑定或容器槽位）
                                self.note_move_src(v, t, env);
                            }
                            None => self.d("E1201", *l, *c, format!("struct '{}' 无字段 '{}'", name, fn_)),
                        }
                    }
                    for (f, _) in &fs {
                        if !fields.iter().any(|(x, _)| x == f) { self.d("E1201", *l, *c, format!("record 初始化缺少字段 '{}'", f)); }
                    }
                    Ty::Named(name.clone())
                } else if self.enums.contains_key(name) {
                    self.d("E1201", *l, *c, format!("'{}' 是 enum，构造需走变体", name));
                    Ty::Bad
                } else {
                    self.d("E1101", *l, *c, format!("unknown struct '{}'", name));
                    Ty::Bad
                }
            }
            Ex::Tup(es, _, _) => {
                Ty::Tuple(es.iter().map(|x| self.check_expr(x, None, env)).collect())
            }
            Ex::ArrLit(es, l, c) => {
                if es.is_empty() {
                    let t = expect.filter(|x| matches!(x, Ty::Arr(..))).map(|x| elem_of(x)).unwrap_or(Ty::Bad);
                    return Ty::Arr(Box::new(t), 0);
                }
                let want = expect.and_then(elem_ref_of);
                let first = self.check_expr(&es[0], want.as_ref(), env);
                for x in &es[1..] {
                    let t = self.check_expr(x, Some(&first), env);
                    if !compatible(&first, &t) { self.mismatch(t_span(x), &first, &t); }
                }
                // GAP-01 补可达性：推导形态（无数组类型标注）的字面量元素类型入既有数组元素判定（文案=L175 在册串逐字）
                if matches!(first, Ty::Ref(..)) { self.d("E1501", *l, *c, "数组元素不得为引用类型"); }
                Ty::Arr(Box::new(first), es.len() as u64)
            }
            Ex::Block(blk) => {
                // 裸块：借用块尾失活，移动/unbox 保守外传（与 if/while 块行为一致）
                self.check_block(blk, expect, env);
                expect.cloned().unwrap_or(Ty::unit())
            }
        }
    }

    fn check_lit(&mut self, lit: &Lit, expect: Option<&Ty>, l: usize, c: usize) -> Ty {
        match lit {
            Lit::Int(v) => {
                match expect {
                    Some(Ty::Prim(p)) if int_bits(p).is_some() => {
                        if !fits_int(p, *v) { self.d("E1201", l, c, format!("字面量 {} 超出 '{}' 范围", v, p)); }
                        Ty::Prim(p)
                    }
                    Some(Ty::Prim(p)) if matches!(*p, "f32" | "f64") => {
                        self.d("E1202", l, c, format!("禁止隐式数值转换：整型字面量 {} 标注于浮点型 '{}'", v, p));
                        Ty::Prim(p)
                    }
                    Some(Ty::Bad) => Ty::Bad,
                    Some(x) => { self.d("E1201", l, c, format!("类型不匹配：整型字面量无法赋于 '{}'", x.display())); x.clone() }
                    None => {
                        if *v > i64::MAX as u128 { self.d("E1201", l, c, format!("字面量 {} 超出默认 i64", v)); }
                        Ty::Prim("i64")
                    }
                }
            }
            Lit::Float(v) => {
                match expect {
                    Some(Ty::Prim(p)) if matches!(*p, "f32" | "f64") => Ty::Prim(p),
                    Some(Ty::Prim(p)) if int_bits(p).is_some() => {
                        self.d("E1202", l, c, format!("禁止隐式数值转换：浮点字面量 {} 标注于整型 '{}'", v, p));
                        Ty::Prim(p)
                    }
                    Some(Ty::Bad) => Ty::Bad,
                    Some(x) => { self.d("E1201", l, c, format!("类型不匹配：浮点字面量无法赋于 '{}'", x.display())); x.clone() }
                    None => Ty::Prim("f64"),
                }
            }
            Lit::Bool(_) => {
                match expect {
                    Some(Ty::Prim("bool")) | None => Ty::Prim("bool"),
                    Some(Ty::Bad) => Ty::Bad,
                    Some(x) => { self.d("E1201", l, c, format!("类型不匹配：bool 字面量无法赋于 '{}'", x.display())); x.clone() }
                }
            }
            Lit::Str(s) => {
                let n = s.as_bytes().len() as u64;
                let t = Ty::Arr(Box::new(Ty::Prim("byte")), n);
                match expect {
                    Some(e) if *e == t => t,
                    Some(Ty::Bad) => Ty::Bad,
                    Some(e) => { self.mismatch((l, c), e, &t); t }
                    None => t,
                }
            }
        }
    }

    fn check_bin(&mut self, op: &str, a: &Ex, b: &Ex, l: usize, c: usize, env: &mut HashMap<String, Var>) -> Ty {
        match op {
            "&&" | "||" => {
                let lt = self.check_expr(a, Some(&Ty::Prim("bool")), env);
                let rt = self.check_expr(b, Some(&Ty::Prim("bool")), env);
                if !compatible(&Ty::Prim("bool"), &lt) { self.mismatch(t_span(a), &Ty::Prim("bool"), &lt); }
                if !compatible(&Ty::Prim("bool"), &rt) { self.mismatch(t_span(b), &Ty::Prim("bool"), &rt); }
                Ty::Prim("bool")
            }
            "==" | "!=" => {
                let lit_side = |x: &Ex| matches!(x, Ex::Lit(Lit::Int(_) | Lit::Float(_), _, _));
                let (t, o) = if lit_side(a) != lit_side(b) {
                    let (typed, lit) = if lit_side(a) { (b, a) } else { (a, b) };
                    let tt = self.check_expr(typed, None, env);
                    let lt = self.check_expr(lit, Some(&tt), env);
                    (tt, lt)
                } else {
                    let lt = self.check_expr(a, None, env);
                    let rt = self.check_expr(b, Some(&lt), env);
                    (lt, rt)
                };
                if !compatible(&t, &o) && t != Ty::Bad && o != Ty::Bad {
                    self.d(if t.is_num() && o.is_num() { "E1202" } else { "E1201" }, l, c,
                        format!("'==' 操作数类型不一致：'{}' vs '{}'（禁隐式转换）", t.display(), o.display()));
                }
                Ty::Prim("bool")
            }
            "<" | "<=" | ">" | ">=" => {
                let lit_side = |x: &Ex| matches!(x, Ex::Lit(Lit::Int(_) | Lit::Float(_), _, _));
                let (t, o) = if lit_side(a) != lit_side(b) {
                    let (typed, lit) = if lit_side(a) { (b, a) } else { (a, b) };
                    let tt = self.check_expr(typed, None, env);
                    let lt = self.check_expr(lit, Some(&tt), env);
                    (tt, lt)
                } else {
                    let lt = self.check_expr(a, None, env);
                    let rt = self.check_expr(b, Some(&lt), env);
                    (lt, rt)
                };
                if !t.is_num() && t != Ty::Bad { self.d("E1201", l, c, format!("关系运算要求数值类型，实际 '{}'", t.display())); }
                if !o.is_num() && o != Ty::Bad { self.d("E1201", l, c, format!("关系运算另一操作数非数值 '{}'", o.display())); }
                if t != o && t != Ty::Bad && o != Ty::Bad {
                    self.d("E1201", l, c, format!("关系运算操作数类型须相同：'{}' vs '{}'", t.display(), o.display()));
                }
                Ty::Prim("bool")
            }
            _ => {
                // + - * / %
                let lit_side = |x: &Ex| matches!(x, Ex::Lit(Lit::Int(_) | Lit::Float(_), _, _));
                let (t, o) = if lit_side(a) != lit_side(b) {
                    let (typed, lit) = if lit_side(a) { (b, a) } else { (a, b) };
                    let tt = self.check_expr(typed, None, env);
                    let lt = self.check_expr(lit, Some(&tt), env);
                    (tt, lt)
                } else {
                    let lt = self.check_expr(a, None, env);
                    let rt = self.check_expr(b, Some(&lt), env);
                    (lt, rt)
                };
                if t == Ty::Bad || o == Ty::Bad { return Ty::Bad; }
                if t != o {
                    self.d("E1201", l, c, format!("算术操作数类型必须相同：'{}' vs '{}'（跨类型须显式 to_xxx）", t.display(), o.display()));
                    return t;
                }
                if !t.is_num() {
                    self.d("E1201", l, c, format!("算术运算 '{}' 不支持类型 '{}'", op, t.display()));
                }
                t
            }
        }
    }
}

fn elem_of(t: &Ty) -> Ty {
    if let Ty::Arr(x, _) = t { (**x).clone() } else { Ty::Bad }
}

fn elem_ref_of(x: &Ty) -> Option<Ty> {
    if let Ty::Arr(e, _) = x { Some((**e).clone()) } else { None }
}

fn compatible(expect: &Ty, got: &Ty) -> bool {
    if expect == got || expect == &Ty::Bad || got == &Ty::Bad { return true; }
    match (expect, got) {
        // 字面量默认已解析：其余必须精确匹配（禁隐式转换）
        (Ty::Arr(a, n1), Ty::Arr(b, n2)) => a == b && (n1 == n2 || *n1 == 0),
        _ => false,
    }
}

fn t_span(e: &Ex) -> (usize, usize) {
    match e {
        Ex::Lit(_, l, c) | Ex::Path(_, l, c) | Ex::UnitLit(l, c) | Ex::Bin(_, _, _, l, c)
        | Ex::Un(_, _, l, c) | Ex::Call(_, _, l, c) | Ex::Method(_, _, _, l, c)
        | Ex::Index(_, _, l, c) | Ex::Field(_, _, l, c) | Ex::Record(_, _, l, c)
        | Ex::Tup(_, l, c) | Ex::ArrLit(_, l, c) | Ex::Ref(_, _, l, c)
        | Ex::Box(_, l, c) | Ex::Deref(_, l, c) | Ex::Unbox(_, l, c) => (*l, *c),
        Ex::Block(b) => b.span,
    }
}

fn variant_span(prog: &Prog, en: &str, var: &str) -> Option<(usize, usize)> {
    prog.enums.iter().find(|e| e.name == en).and_then(|e| e.variants.iter().find(|(v, _, _, _)| v == var)).map(|(_, _, l, c)| (*l, *c))
}

fn named_is(t: &Ty, n: &str) -> bool {
    if let Ty::Named(x) = t { x == n } else { false }
}

fn collect_named(t: &Ty, out: &mut Vec<String>) {
    match t {
        Ty::Named(n) => out.push(n.clone()),
        Ty::Arr(i, _) => collect_named(i, out),
        Ty::Tuple(ts) => { for x in ts { collect_named(x, out); } }
        Ty::Ref(i, _) => collect_named(i, out),
        Ty::BoxTy(_) => {} // R2 §5：堆间址断环（Box 递归链可表达）
        _ => {}
    }
}

fn fits_int(p: &str, v: u128) -> bool {
    match p {
        "i8" => v <= 127, "i16" => v <= 32767, "i32" => v <= i32::MAX as u128, "i64" => v <= i64::MAX as u128,
        "u8" | "byte" => v <= u8::MAX as u128, "u16" => v <= u16::MAX as u128, "u32" => v <= u32::MAX as u128, "u64" => v <= u64::MAX as u128,
        _ => true,
    }
}

// DFS：从 start 出发能否回到 start；返回路径
type CkRef<'a, 'b> = &'b mut Ck<'a>;

fn dfs_cycle<'a, 'b>(
    stack: &mut Vec<(String, Vec<String>, usize)>,
    path: &mut Vec<String>,
    visited: &mut HashMap<String, bool>,
    ck: CkRef<'a, 'b>,
    start: String,
) -> Option<Vec<String>> {
    while let Some(top) = stack.last_mut() {
        let (_node, edges, idx) = top;
        if *idx < edges.len() {
            let next = edges[*idx].clone();
            *idx += 1;
            if next == start {
                path.push(next.clone());
                return Some(path.clone());
            }
            if !path.contains(&next) {
                path.push(next.clone());
                stack.push((next.clone(), ck.edges_of(&next), 0));
            }
        } else {
            stack.pop();
            path.pop();
        }
        let _ = visited;
    }
    None
}
