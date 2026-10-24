use std::collections::HashMap;
use crate::diag::Diag;
use crate::parser::*;
use crate::type_table::{FnSig, TypeTable};

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
    /// L2h：绑定的值来源是六个特殊值之一（含经 `let` 复制）；一元负号据此 E1201。
    pub float_special: bool,
}

impl Var {
    fn fresh(ty: Ty) -> Self {
        Var {
            ty,
            moved: false,
            unboxed: false,
            partial_moved: false,
            moved_fields: vec![],
            shared: 0,
            mut_active: false,
            float_special: false,
        }
    }
}

/// L2h：表达式是否为特殊值来源（直接形态或绑定别名）。
fn special_origin(e: &Ex, env: &HashMap<String, Var>) -> Option<Ty> {
    match e {
        Ex::FloatSpecial(w, ..) => Some(w.ty()),
        Ex::Path(n, ..) => env
            .get(n)
            .filter(|v| v.float_special)
            .map(|v| v.ty.clone()),
        _ => None,
    }
}

pub struct Ck<'a> {
    prog: &'a Prog,
    diags: Vec<Diag>,
    structs: HashMap<String, Vec<(String, Ty)>>,
    enums: HashMap<String, HashMap<String, Vec<Ty>>>,
    fns: HashMap<String, (Vec<Ty>, Ty)>,
    /// 类型规则唯一来源的落点：check 写，lower 只读（L2a typed IR 使能项）。
    types: TypeTable,
    /// 同语句 E2204 待落：读取位置；若同语句已对该绑定发 E2203，则丢弃（只留变更诊断）。
    pending_e2204: Vec<(String, usize, usize)>,
    /// 当前语句已发 E2203 的绑定（同语句同槽位去重）。
    scope_e2203: Vec<String>,
    /// 作用域栈：语句／块尾开始处的 (scope_e2203, pending_e2204) 长度。
    place_starts: Vec<(usize, usize)>,
    /// 借用创建期间的绑定：创建 `&v`／`&mut v` 不是对 v 的值读取，不落 E2204。
    e2204_exempt: Vec<String>,
    /// 已发 E2201／E2202 的绑定：借用状态已判错，后续不再级联 E2203／E2204（首错优先，保既有 golden）。
    borrow_conflict: Vec<String>,
}

fn int_bits(p: &str) -> Option<u32> {
    Some(match p { "i8" => 8, "i16" => 16, "i32" => 32, "i64" => 64, "u8" => 8, "u16" => 16, "u32" => 32, "u64" => 64, "byte" => 8, _ => return None })
}

impl<'a> Ck<'a> {
    pub fn new(prog: &'a Prog) -> Self {
        Ck {
            prog,
            diags: vec![],
            structs: HashMap::new(),
            enums: HashMap::new(),
            fns: HashMap::new(),
            types: TypeTable::new(),
            pending_e2204: vec![],
            scope_e2203: vec![],
            place_starts: vec![],
            e2204_exempt: vec![],
            borrow_conflict: vec![],
        }
    }

    /// 检查并产出（诊断, 类型表）。类型表是 check 结果的公开面，lower 不得重算。
    pub fn run_typed(mut self) -> (Vec<Diag>, TypeTable) {
        self.check_entry_presence(); // 入口存在性先于定义名域（0.1.22 项6 入口契约句 → E1702）
        // 0. 定义名域检查（R1 §6 定义名重复句 → E1701，0.1.20 项5a）：先于登记跑；重复不改变后续登记形制（后定义覆盖前定义＝BASE 现态零变）
        self.check_def_names();
        self.check_entry_signature(); // 入口签名形后于唯一性、只判首现那枚（0.1.22 项6 入口契约句 → E1703）
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
        let sigs = self
            .fns
            .iter()
            .map(|(n, (ps, ret))| (n.clone(), FnSig { params: ps.clone(), ret: ret.clone() }))
            .collect::<Vec<_>>();
        for (n, sig) in sigs {
            self.types.insert_fn_sig(n, sig);
        }
        (self.diags, self.types)
    }

    /// 兼容壳：既有调用方只取诊断。
    pub fn run(self) -> Vec<Diag> {
        self.run_typed().0
    }

    fn d(&mut self, code: &str, l: usize, c: usize, msg: impl Into<String>) {
        self.diags.push(Diag::new(code, l, c, msg));
    }

    /// 语句／块尾作用域：E2203 同槽位去重；E2204 待落到作用域尾再定，
    /// 若同作用域已对该绑定发 E2203（变更优先），则不落 E2204。
    fn place_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let e3_start = self.scope_e2203.len();
        let e4_start = self.pending_e2204.len();
        self.place_starts.push((e3_start, e4_start));
        let r = f(self);
        let (s3, s4) = self.place_starts.pop().unwrap_or((0, 0));
        let e3_names: Vec<String> = self.scope_e2203.drain(s3..).collect();
        let pending: Vec<(String, usize, usize)> = self.pending_e2204.drain(s4..).collect();
        let mut seen: Vec<String> = Vec::new();
        for (n, l, c) in pending {
            if e3_names.iter().any(|x| x == &n) || seen.iter().any(|x| x == &n) {
                continue;
            }
            seen.push(n.clone());
            self.d(
                "E2204",
                l,
                c,
                format!("'{}' 正被独占借用（&mut），不可直接读取（R2 §2 owner 槽位直接访问）", n),
            );
        }
        r
    }

    /// E2203：借用活跃期间变更 owner 槽位（写／移出／unbox）；同语句同槽位只发一枚。
    fn e2203(&mut self, n: &str, l: usize, c: usize, msg: impl Into<String>) {
        if self.borrow_conflict.iter().any(|x| x == n) {
            return;
        }
        let start = self.place_starts.last().map(|x| x.0).unwrap_or(0);
        if self.scope_e2203[start..].iter().any(|x| x == n) {
            return;
        }
        self.scope_e2203.push(n.to_string());
        self.d("E2203", l, c, msg);
    }

    fn note_e2204(&mut self, n: &str, l: usize, c: usize) {
        if self.borrow_conflict.iter().any(|x| x == n) || self.e2204_exempt.iter().any(|x| x == n) {
            return;
        }
        self.pending_e2204.push((n.to_string(), l, c));
    }

    fn mark_borrow_conflict(&mut self, n: &str) {
        if !self.borrow_conflict.iter().any(|x| x == n) {
            self.borrow_conflict.push(n.to_string());
        }
    }

    /// 借用创建期间抑制 E2204（`&v`／`&mut v` 不是值读取）。
    fn exempt_e2204<R>(&mut self, n: Option<&str>, f: impl FnOnce(&mut Self) -> R) -> R {
        if let Some(name) = n {
            self.e2204_exempt.push(name.to_string());
        }
        let r = f(self);
        if n.is_some() {
            self.e2204_exempt.pop();
        }
        r
    }

    fn known_named(&self, n: &str) -> bool {
        self.structs.contains_key(n) || self.enums.contains_key(n)
    }

    /// L2g typed Box 载荷白名单：8 个整型宽度（check 期望下推用）。
    fn l2f_box_payload(t: &Ty) -> bool {
        matches!(
            t,
            Ty::Prim(p) if matches!(*p, "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64")
        )
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

    /// 移动源登记：e 属主转移入新位置（绑定/实参/返回/赋值/容器槽位）。
    /// 借用活跃期间移出 owner 槽位＝E2203（R2 §2）；状态错误优先，不叠报。
    fn note_move_src(&mut self, e: &Ex, ty: &Ty, env: &mut HashMap<String, Var>) {
        if self.is_copy(ty) { return; }
        match e {
            Ex::Path(n, l, c) => {
                if let Some(v) = env.get(n) {
                    if !v.moved && !v.unboxed && !v.partial_moved && (v.shared > 0 || v.mut_active) {
                        let msg = format!("'{}' 有活跃借用，不可移出（R2 §2 owner 槽位直接访问）", n);
                        self.e2203(n, *l, *c, msg);
                    }
                }
                if let Some(v) = env.get_mut(n) { v.moved = true; }
            }
            Ex::Field(base, fname, l, c) => {
                if let Some(n) = place_root(base) {
                    if let Some(v) = env.get(n) {
                        if !v.moved && !v.unboxed && (v.shared > 0 || v.mut_active) {
                            let msg = format!("'{}.{}' 的 owner 槽位有活跃借用，不可移出字段（R2 §2）", n, fname);
                            self.e2203(n, *l, *c, msg);
                        }
                    }
                }
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
            // 尾表达式＝value 位置；E2203 若成立则同作用域内不落 E2204（移动优先）
            self.place_scope(|ck| {
                let t = ck.check_expr(tail, expect, &mut env2);
                if let Some(e) = expect {
                    if !compatible(e, &t) { ck.mismatch(t_span(tail), e, &t); }
                }
                // 返回=移动点（R2 §1）
                ck.note_move_src(tail.as_ref(), &t, &mut env2);
            });
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
        // 每条语句一个 E2203／E2204 作用域（嵌套语句各自结算，外侧状态恢复）
        self.place_scope(|ck| ck.check_stmt_inner(st, env));
    }

    fn check_stmt_inner(&mut self, st: &St, env: &mut HashMap<String, Var>) {
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
                self.types.insert_let(*l, *c, vt.clone());
                let keeps_special = special_origin(init, env).is_some();
                if matches!(ann, Some(Ty::Ref(..))) { self.d("E1501", *l, *c, "引用类型只能出现在函数参数/局部借用，不得作为 let 标注持久化"); }
                self.note_move_src(init, &vt, env);
                // 持久借用记账：句柄绑定 `let h = &v / &mut v`（活跃至所在块尾，R2 §2）；
                // 字段借用按绑定级记在根绑定上（`&o.a` 记为对 `o` 的借用，L2a 保守口径）
                if let Ex::Ref(inner, m, _, _) = init {
                    if let Some(n) = place_root(inner) {
                        if let Some(v) = env.get_mut(n) {
                            if *m { v.mut_active = true; } else { v.shared += 1; }
                        }
                    }
                }
                let mut bound = Var::fresh(vt);
                bound.float_special = keeps_special;
                env.insert(name.clone(), bound);
            }
            St::LetDecl(name, _, l, c) => {
                self.d("E1601", *l, *c, format!("let '{}' 缺少初始化（0.1.x 不支持无初始值绑定）", name));
            }
            St::Assign(lhs, rhs, l, c) => {
                // L2f C2：仅当 lhs 为三型 Box 且 rhs 为 box 字面量时，把 Box<T>
                // 期望传给右值（重赋值 `a = box 2` 的宽度推断）；其余保持 None。
                let rhs_expect = if matches!(rhs, Ex::Box(..)) {
                    match lhs {
                        Ex::Path(n, _, _) => env.get(n).and_then(|v| match &v.ty {
                            Ty::BoxTy(p) if Self::l2f_box_payload(p) => Some(Ty::BoxTy(p.clone())),
                            _ => None,
                        }),
                        _ => None,
                    }
                } else {
                    None
                };
                let rt = self.check_expr(rhs, rhs_expect.as_ref(), env);
                let rhs_special = special_origin(rhs, env).is_some();
                self.note_move_src(rhs, &rt, env);
                let lt = match lhs {
                    Ex::Path(n, ll, lc) => {
                        // 重赋整值：恢复/覆盖所有权状态（R2 §1 再赋整值恢复），不做读检查；
                        // 借用活跃期间写 owner 槽位＝E2203（R2 §2，状态错误优先不叠报）
                        match env.get_mut(n) {
                            Some(v) => {
                                if !v.moved && !v.unboxed && (v.shared > 0 || v.mut_active) {
                                    let msg = format!("'{}' 有活跃借用，不可写入（R2 §2 owner 槽位直接访问）", n);
                                    self.e2203(n, *ll, *lc, msg);
                                }
                                let t = v.ty.clone();
                                let mut nv = Var::fresh(rt.clone());
                                nv.float_special = rhs_special;
                                *v = nv;
                                t
                            }
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
                        let ft = self.field_place_ty(base, fname, *fl, *fc, env, false);
                        // 字段写＝对根绑定槽位的变更；借用活跃期间→E2203（绑定级保守口径）
                        if let Some(n) = place_root(base) {
                            if let Some(v) = env.get(n) {
                                if !v.moved && !v.unboxed && (v.shared > 0 || v.mut_active) {
                                    let msg = format!("'{}.{}' 的 owner 槽位有活跃借用，不可写入（R2 §2）", n, fname);
                                    self.e2203(n, *fl, *fc, msg);
                                }
                            }
                        }
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
                self.types.insert_assign(*l, *c, lt.clone());
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

    /// 字段可放置/读取的类型（含 moved_fields 判定；不触发 E2102 整体使用）。
    /// `value_read=true` 表示值位置读取：`&mut` 活跃期间直接读→E2204。
    fn field_place_ty(&mut self, base: &Ex, fname: &str, l: usize, c: usize, env: &mut HashMap<String, Var>, value_read: bool) -> Ty {
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
                        if value_read && v.mut_active {
                            self.note_e2204(n, l, c);
                        }
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

    /// 表达式检查入口：统一把结果类型写入 type table（lower 的唯一类型来源）。
    fn check_expr(&mut self, e: &Ex, expect: Option<&Ty>, env: &mut HashMap<String, Var>) -> Ty {
        let ty = self.check_expr_inner(e, expect, env);
        self.types.insert_expr(e, ty.clone());
        ty
    }

    fn check_expr_inner(&mut self, e: &Ex, expect: Option<&Ty>, env: &mut HashMap<String, Var>) -> Ty {
        match e {
            Ex::Lit(lit, l, c) => self.check_lit(lit, expect, *l, *c),
            // L2h：特殊值宽度严格——期望宽度不符 E1201；整型标注 E1202；不新增 E 码。
            Ex::FloatSpecial(w, _, l, c) => {
                let t = w.ty();
                match expect {
                    Some(Ty::Bad) => Ty::Bad,
                    Some(e) if *e == t => t,
                    Some(Ty::Prim(p)) if int_bits(p).is_some() => {
                        self.d(
                            "E1202",
                            *l,
                            *c,
                            format!("禁止隐式数值转换：特殊值 '{}' 标注于整型 '{}'", t.display(), p),
                        );
                        Ty::Bad
                    }
                    Some(x) => {
                        self.d(
                            "E1201",
                            *l,
                            *c,
                            format!("特殊值宽度严格：'{}' 不可赋于 '{}'", t.display(), x.display()),
                        );
                        Ty::Bad
                    }
                    None => t,
                }
            }
            Ex::UnitLit(l, c) => {
                if let Some(x) = expect { if x != &Ty::unit() && x != &Ty::Bad { self.mismatch((*l, *c), x, &Ty::unit()); } }
                Ty::unit()
            }
            Ex::Path(n, l, c) => {
                if let Some(v) = env.get(n).cloned() {
                    self.read_check(n, *l, *c, &v, true);
                    // 值位置直接读：`&mut` 活跃期间→E2204（状态错误优先；移动点由 E2203 收口）
                    if !(v.unboxed || v.moved || v.partial_moved) && v.mut_active {
                        self.note_e2204(n, *l, *c);
                    }
                    v.ty
                } else { self.d("E1101", *l, *c, format!("unknown identifier '{}'", n)); Ty::Bad }
            }
            Ex::Bin(op, a, b, l, c) => self.check_bin(op, a, b, *l, *c, env),
            Ex::Un(op, a, l, c) => {
                match op.as_str() {
                    "-" => {
                        // L2h 裁定 3：一元负号只拒特殊值——直接六形式与经绑定的别名
                        // 同为 E1201；有限字面量／有限非字面量 `-x` 的既有行为不动。
                        if let Some(t) = special_origin(a, env) {
                            self.check_expr(a, Some(&t), env);
                            self.d(
                                "E1201",
                                *l,
                                *c,
                                format!(
                                    "一元 '-' 不接受特殊值 '{}'（含绑定后读取；0.3.7 仅直接打印）",
                                    t.display()
                                ),
                            );
                            return t;
                        }
                        // L2c：负字面量按期望宽度收敛（含补码最小值），不新增 E 码。
                        if let Ex::Lit(Lit::Int(v), ll, lc) = a.as_ref() {
                            let target: Option<Ty> = match expect {
                                Some(Ty::Prim(p)) if int_bits(p).is_some() => Some(Ty::Prim(*p)),
                                Some(Ty::Bad) => Some(Ty::Bad),
                                None => Some(Ty::Prim("i64")),
                                Some(_) => None,
                            };
                            if let Some(t) = target {
                                if let Ty::Prim(p) = &t {
                                    if int_bits(p).is_some() && !neg_int_fits(p, *v) {
                                        self.d(
                                            "E1201",
                                            *ll,
                                            *lc,
                                            format!("字面量 -{} 超出 '{}' 范围", v, p),
                                        );
                                    }
                                }
                                self.types.insert_expr(a, t.clone());
                                return t;
                            }
                        }
                        if let Ex::Lit(Lit::Float(_), _, _) = a.as_ref() {
                            if let Some(Ty::Prim(p)) = expect {
                                if matches!(*p, "f32" | "f64") {
                                    let t = Ty::Prim(*p);
                                    self.check_expr(a, Some(&t), env);
                                    return t;
                                }
                            }
                            return self.check_expr(a, None, env);
                        }
                        let t = self.check_expr(a, expect.filter(|x| x.is_num()), env);
                        if !t.is_num() && t != Ty::Bad {
                            self.d("E1201", *l, *c, format!("一元 '-' 要求数值类型，实际 '{}'", t.display()));
                        }
                        t
                    }
                    "!" => { let t = self.check_expr(a, None, env); if !t.is_bool() && t != Ty::Bad { self.d("E1201", *l, *c, format!("'!' 要求 bool，实际 '{}'", t.display())); } Ty::Prim("bool") }
                    _ => self.check_expr(a, None, env),
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
                // L2f C1：标注 Box<T>（T 三型）时把载荷期望下推给字面量。
                // L2k：有限浮点字面量（含一元负号）按标注收窄；复合载荷把标注下推给
                // 数组字面量元素／record 字段。整型字面量等维持 None，保证
                // `Box<f32> = box 1` 仍为 E1201（N-115／N-116 红线）。
                let payload_expect = match expect {
                    Some(Ty::BoxTy(p)) => match p.as_ref() {
                        Ty::Prim("f32") | Ty::Prim("f64") if float_literal_form(inner) => {
                            Some((**p).clone())
                        }
                        Ty::Named(n) if self.structs.contains_key(n) => Some((**p).clone()),
                        Ty::Arr(elem, _) if !matches!(elem.as_ref(), Ty::Prim("byte")) => {
                            Some((**p).clone())
                        }
                        _ if Self::l2f_box_payload(p) => Some((**p).clone()),
                        _ => None,
                    },
                    _ => None,
                };
                let t = self.check_expr(inner, payload_expect.as_ref(), env);
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
                        else if v.shared > 0 || v.mut_active {
                            // 借用活跃期间 unbox＝变更 owner 槽位（E2203，R2 §2）
                            let msg = format!("'{}' 有活跃借用，不可 unbox（R2 §2 owner 槽位直接访问）", n);
                            self.e2203(n, *l, *c, msg);
                        } else { if let Some(v) = env.get_mut(n) { v.unboxed = true; } }
                    }
                    None => self.d("E1101", *l, *c, format!("unknown identifier '{}'", n)),
                }
                Ty::unit()
            }
            Ex::Ref(inner, m, l, c) => {
                // R2 §2 借用判定表冲突检查（不记账）：持久借用的记账仅发生在 let 绑定处
                // （§2"借用句柄自创建处到其所在块尾均活跃"——句柄=绑定持有；实参位匿名借用为瞬时）
                // 字段借用按绑定级对根绑定判（`&o.a` 视为对 `o` 的借用）。
                let root = place_root(inner);
                if let Some(n) = root {
                    if let Some(vcl) = env.get(n).cloned() {
                        if matches!(inner.as_ref(), Ex::Path(..)) {
                            self.read_check(n, *l, *c, &vcl, true);
                        }
                        if *m {
                            if vcl.mut_active || vcl.shared > 0 {
                                self.d("E2202", *l, *c, format!("'{}' 已有活跃借用，不可再创建 &mut（R2 §2）", n));
                                self.mark_borrow_conflict(n);
                            }
                        } else if vcl.mut_active {
                            self.d("E2201", *l, *c, format!("'{}' 正被独占借用，不可创建共享借用（R2 §2）", n));
                            self.mark_borrow_conflict(n);
                        }
                    }
                }
                // 借用创建本身不是值读取：`&v`／`&mut v` 只走 E2201／E2202，不叠 E2204
                let t = self.exempt_e2204(root, |ck| ck.check_expr(inner, None, env));
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
                self.field_place_ty(base, fname, *l, *c, env, true)
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

    /// R1 §6 定义名重复句（0.1.20 项5a 实发点）：同一模块命名域内，同名且同类的具名定义至多一枚，
    /// 第二次及其后的每一枚报 E1701（首现不报）。四类形制＝重复 `fn`／重复 `struct`／重复 `enum`／多枚 `fn main`，
    /// 其中多枚 `main` 由 fn 域收（名为 main 的函数至多一枚）；`main` 的存在性与签名形态归 0.1.22 项 6 入口契约句，该两判自 0.1.22 起已由 `check_entry_presence()`／`check_entry_signature()` 实发（实态句，非待销缺口），本函数只收枚数超限那一枚。
    /// 三类各自成集、互不查：跨类同名（`struct Foo`＋`enum Foo`）在 0.1.x 未作判定（R1 §6 跨类注记句），本窗不拦。
    /// 四形报点＝该定义的关键字位（0.1.21 项5b 甲案）：`fn`／`struct`／`enum` 各取其 span，多枚 `main` 由 fn 域收；
    /// `StructDecl.span` 由 parse_struct 现取（与 `parse_fn`／`parse_enum` 同形补齐），故零字段 struct 亦无兜底支路。
    fn check_def_names(&mut self) {
        let prog = self.prog;
        let groups: [(&str, Vec<(String, (usize, usize))>); 3] = [
            ("fn", prog.fns.iter().map(|f| (f.name.clone(), f.span)).collect()),
            ("struct", prog.structs.iter().map(|s| (s.name.clone(), s.span)).collect()),
            ("enum", prog.enums.iter().map(|e| (e.name.clone(), e.span)).collect()),
        ];
        let mut seen: HashMap<(&str, String), (usize, usize)> = HashMap::new();
        for (kind, items) in groups {
            for (name, (l, c)) in items {
                let first = seen.get(&(kind, name.clone())).copied();
                match first {
                    Some((fl, fc)) => self.d("E1701", l, c, format!("{} '{}' 定义名重复（同域同名同类的具名定义至多一枚，R1 §6 定义名重复句；首现 {}:{}）", kind, name, fl, fc)),
                    None => { seen.insert((kind, name.clone()), (l, c)); }
                }
            }
        }
        // 0.1.23 项7 乙制（禁止同命名域，R1 §6 跨类同名禁令句 → E1704）：同名而异类的具名定义并存＝后定义者那一枚报。
        // 上行原注「跨类同名…本窗不拦」自本面作废＝前形留档、逐字不回改（本窗 del 上界一枚已整枚归 r1 §6 L65 的整句替换）。
        // 取上支 seen（每类首现枚）入跨域比较＝同名同类枚已归 E1701，两码不叠报；报点＝后定义者关键字位（与 E1701 同位制，0.1.21 甲案随标沿用）。
        let mut cross: Vec<(&str, String, (usize, usize))> = seen.into_iter().map(|((k, n), s)| (k, n, s)).collect();
        cross.sort_by_key(|(_, _, (l, c))| (*l, *c));
        let mut by_name: HashMap<String, (&str, (usize, usize))> = HashMap::new();
        for (kind, name, (l, c)) in cross {
            let prev = by_name.get(&name).copied();
            if let Some((fk, (fl, fc))) = prev {
                if fk != kind { self.d("E1704", l, c, format!("{} '{}' 与 {} '{}' 跨类定义名冲突（同一模块命名域内禁止异类同名，R1 §6 跨类同名禁令句；先现 {}:{}）", kind, name, fk, name, fl, fc)); }
            } else { by_name.insert(name, (kind, (l, c))); }
        }
    }

    /// R1 §6 入口契约句·存在性半句（0.1.22 项6 实发点）：一个编译单元（0.1.x 单档即程序，无多文件链接）内
    /// 名为 `main` 的函数零枚时报 E1702，报点＝该文件 `1:1`（程序级缺失无自然 span）。
    /// 唯一性不在此判（多枚由上行定义名重复域收 E1701）；签名形在唯一性之后单列，零枚时不叠报（无对象）。
    fn check_entry_presence(&mut self) {
        let prog = self.prog;
        if !prog.fns.iter().any(|f| f.name == "main") {
            self.d("E1702", 1, 1, "程序入口缺失（编译单元内须恰一枚 fn main，R1 §6 入口契约句；本单元实见 0 枚）");
        }
    }

    /// R1 §6 入口契约句·签名形半句（0.1.22 项6 实发点）：首现那枚 `fn main` 若显式写返回类型且非 `unit` 报 E1703，
    /// 报点＝该返回类型位（`FnDecl.ret` 由 parse_fn 现取，含 span）。缺省（不写 `-> R`）与显式 `-> unit` 皆合法
    /// （沿 §6 首行「无 `R` 时视为 `-> unit`」，本支不新增语义）；带参数形态 `fn main(x: T)` 0.1.x 未作判定。
    fn check_entry_signature(&mut self) {
        let prog = self.prog;
        if let Some(f) = prog.fns.iter().find(|f| f.name == "main") {
            if let Some((t, l, c)) = &f.ret {
                if !matches!(t, Ty::Prim("unit")) {
                    self.d("E1703", *l, *c, format!("fn 'main' 签名形不合（程序入口返回类型须为 unit，R1 §6 入口契约句；实见 '{}'）", t.display()));
                }
            }
        }
    }
}

/// L2k：有限浮点字面量形态（含一元负号）。特殊值仍走既有宽度严格 E1201 路径。
fn float_literal_form(e: &Ex) -> bool {
    match e {
        Ex::Lit(Lit::Float(_), _, _) => true,
        Ex::Un(op, x, _, _) if op == "-" => matches!(x.as_ref(), Ex::Lit(Lit::Float(_), _, _)),
        _ => false,
    }
}

fn elem_of(t: &Ty) -> Ty {    if let Ty::Arr(x, _) = t { (**x).clone() } else { Ty::Bad }
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
    e.span()
}

/// owner 槽位根绑定：`v`／`v.f`／`v.f.g` 均归到 `v`（绑定级保守口径，R2 §2）。
/// 经句柄 `*m`／`m.f` 的访问不属 owner 槽位，返回 `None`。
fn place_root(e: &Ex) -> Option<&str> {
    match e {
        Ex::Path(n, _, _) => Some(n),
        Ex::Field(base, _, _, _) => place_root(base),
        _ => None,
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

/// 前缀一元 `-` 的字面量范围：有符号类型允许补码最小值；无符号仅允许 `-0`。
fn neg_int_fits(p: &str, v: u128) -> bool {
    match p {
        "i8" => v <= 128,
        "i16" => v <= 32768,
        "i32" => v <= (i32::MAX as u128) + 1,
        "i64" => v <= (i64::MAX as u128) + 1,
        "u8" | "u16" | "u32" | "u64" | "byte" => v == 0,
        _ => false,
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

#[cfg(test)]
mod tests {
    use crate::compile_to_diags;

    fn codes(src: &str) -> Vec<String> {
        compile_to_diags(src).iter().map(|d| d.code.clone()).collect()
    }

    fn assert_one(src: &str, code: &str) {
        let cs = codes(src);
        assert_eq!(cs, vec![code.to_string()], "诊断应恰为 {code}：{cs:?}");
    }

    // —— G-E-1b：借用活跃期间变更 owner 槽位 → E2203（6 枚主形）——

    #[test]
    fn e2203_write_while_shared_borrow() {
        assert_one("fn main() {\n    let x = box 1;\n    let r = &x;\n    x = box 2;\n    println(*r);\n}\n", "E2203");
    }

    #[test]
    fn e2203_move_while_shared_borrow() {
        assert_one("fn main() {\n    let x = box 1;\n    let r = &x;\n    let y = x;\n    println(*r);\n}\n", "E2203");
    }

    #[test]
    fn e2203_unbox_while_shared_borrow() {
        assert_one("fn main() {\n    let b = box 1;\n    let r = &b;\n    unbox b;\n    println(*r);\n}\n", "E2203");
    }

    #[test]
    fn e2203_move_field_while_field_borrowed() {
        assert_one("struct P { x: Box<i64> }\nfn main() {\n    let o = P { x: box 1 };\n    let r = &o.x;\n    let y = o.x;\n    println(*r);\n}\n", "E2203");
    }

    #[test]
    fn e2203_write_field_while_field_borrowed() {
        assert_one("struct P { x: Box<i64> }\nfn main() {\n    let o = P { x: box 1 };\n    let r = &o.x;\n    o.x = box 3;\n    println(*r);\n}\n", "E2203");
    }

    #[test]
    fn e2203_write_while_mut_borrow() {
        assert_one("fn main() {\n    let x = box 1;\n    let m = &mut x;\n    x = box 2;\n    println(*m);\n}\n", "E2203");
    }

    // —— G-E-1c：&mut 活跃期间直接读 → E2204（2 枚主形）——

    #[test]
    fn e2204_read_while_mut_borrow() {
        assert_one("fn main() {\n    let x: i64 = 1;\n    let m = &mut x;\n    println(x);\n    println(*m);\n}\n", "E2204");
    }

    #[test]
    fn e2204_read_field_while_mut_borrow() {
        assert_one("struct P { x: i64, y: i64 }\nfn main() {\n    let o = P { x: 1, y: 2 };\n    let m = &mut o;\n    println(o.x);\n}\n", "E2204");
    }

    // —— 放行口径：p12／p3／p10d 必须 rc=0（零诊断）——

    #[test]
    fn allow_borrow_ended_by_block() {
        assert!(codes("fn main() {\n    let x = box 1;\n    if true {\n        let r = &x;\n        println(*r);\n    }\n    x = box 2;\n    println(x);\n}\n").is_empty());
    }

    #[test]
    fn allow_self_assign_without_borrow() {
        assert!(codes("fn main() {\n    let x = box 1;\n    x = x;\n}\n").is_empty());
    }

    #[test]
    fn allow_access_through_mut_handle() {
        assert!(codes("fn main() {\n    let x: i64 = 1;\n    let m = &mut x;\n    println(*m);\n    *m = 5;\n    println(*m);\n}\n").is_empty());
    }

    // —— 同语句去重与状态错误优先 ——

    #[test]
    fn same_statement_read_write_reports_only_e2203() {
        assert_one("fn main() {\n    let x: i64 = 1;\n    let m = &mut x;\n    x = x + 1;\n    println(*m);\n}\n", "E2203");
    }

    #[test]
    fn state_error_precedes_borrow_diagnostic() {
        let cs = codes("fn main() {\n    let x = box 1;\n    unbox x;\n    let r = &x;\n    println(*r);\n}\n");
        assert!(!cs.is_empty() && cs.iter().all(|c| c == "E2301"), "{cs:?}");
    }

    #[test]
    fn non_copy_field_move_under_mut_borrow_is_e2203_not_e2204() {
        assert_one("struct P { x: Box<i64>, y: i64 }\nfn main() {\n    let o = P { x: box 1, y: 2 };\n    let m = &mut o;\n    let y = o.x;\n    println(y);\n}\n", "E2203");
    }
}
