//! 受检 AST → 首片 typed IR。
//!
//! subset 闸已先行；这里仍以 `Result` 返回，避免闸与降级判定漂移时 panic。
//! 类型不作为本模块的重算对象：绑定与表达式的类型一律取自 `check` 产出的
//! `TypeTable`；表项缺失或与形态不符即 fail-closed。
//!
//! L2a：本模块实现 G-E-2（move／overwrite／Box 载荷）的状态机，产出显式 IR 事件；
//! malloc／free／drop 的运行期生成归 CORE-04，不在此处发生。

use super::ir::{Func, Local, LocalId, Program, Stmt, StrArg, Ty};
use super::subset::{self, SubsetError};
use crate::parser::{Ex, Lit, Prog, St, Ty as AstTy};
use crate::type_table::{self, TypeTable};
use std::collections::HashMap;

/// L2a 局部槽位状态：`live` 是静态可判的 owner 活跃位。
#[derive(Clone, Debug)]
struct Slot {
    ty: Ty,
    live: bool,
}

pub fn lower(prog: &Prog, types: &TypeTable) -> Result<Program, SubsetError> {
    subset::check(prog)?;
    let mut funcs = Vec::new();
    for f in &prog.fns {
        if f.name != "main" {
            continue;
        }
        let mut locals: Vec<Local> = Vec::new();
        let mut slots: Vec<Slot> = Vec::new();
        let mut body: Vec<Stmt> = Vec::new();
        let mut scope: HashMap<String, LocalId> = HashMap::new();
        let mut last_pos = f.span;
        for st in &f.body.stmts {
            match st {
                St::Let(name, None, init, l, c) => {
                    last_pos = (*l, *c);
                    let id = LocalId(locals.len() as u32);
                    let local_ty = let_type(types, init, *l, *c)?;
                    match init {
                        Ex::Lit(Lit::Str(s), _, _) => {
                            let n = match local_ty {
                                Ty::Bytes(n) => n,
                                _ => return Err(SubsetError::new(*l, *c, "lower：字符串绑定类型非 [byte; N]")),
                            };
                            let bytes = s.as_bytes().to_vec();
                            if bytes.len() as u64 != n {
                                return Err(SubsetError::new(
                                    *l,
                                    *c,
                                    format!("lower：let 类型 [byte; {}] 与初始值 {} 字节不符", n, bytes.len()),
                                ));
                            }
                            body.push(Stmt::BindStr { local: id, bytes, line: *l, col: *c });
                            slots.push(Slot { ty: Ty::Bytes(n), live: true });
                        }
                        Ex::Box(inner, _, _) => {
                            if local_ty != Ty::Box(Box::new(Ty::I64)) {
                                return Err(SubsetError::new(*l, *c, "lower：box 绑定类型非 Box<i64>"));
                            }
                            let value = int_lit(inner)?;
                            body.push(Stmt::BindBox { local: id, value, line: *l, col: *c });
                            slots.push(Slot { ty: local_ty, live: true });
                        }
                        Ex::Lit(Lit::Int(v), _, _) => {
                            if local_ty != Ty::I64 {
                                return Err(SubsetError::new(*l, *c, "lower：整型绑定类型非 i64"));
                            }
                            let value = i64::try_from(*v)
                                .map_err(|_| SubsetError::new(*l, *c, "lower：整型字面量超出 i64"))?;
                            body.push(Stmt::BindI64 { local: id, value, line: *l, col: *c });
                            slots.push(Slot { ty: Ty::I64, live: true });
                        }
                        Ex::Deref(base, dl, dc) => {
                            if local_ty != Ty::I64 {
                                return Err(SubsetError::new(*l, *c, "lower：解引用结果类型非 i64"));
                            }
                            let src = match base.as_ref() {
                                Ex::Path(n, _, _) => lookup(&scope, n, *dl, *dc)?,
                                other => {
                                    let (el, ec) = subset::expr_span(other);
                                    return Err(SubsetError::new(el, ec, "lower：解引用源形态"));
                                }
                            };
                            if slots.get(src.0 as usize).map(|s| &s.ty) != Some(&Ty::Box(Box::new(Ty::I64))) {
                                return Err(SubsetError::new(*dl, *dc, "lower：解引用源非 Box<i64>"));
                            }
                            body.push(Stmt::BindI64Deref { local: id, src, line: *dl, col: *dc });
                            slots.push(Slot { ty: Ty::I64, live: true });
                        }
                        Ex::Path(n, nl, nc) => {
                            let src = lookup(&scope, n, *nl, *nc)?;
                            if !matches!(local_ty, Ty::Box(_)) {
                                return Err(SubsetError::new(*nl, *nc, "lower：移动目标类型非 Box"));
                            }
                            if slots.get(src.0 as usize).map(|s| &s.ty) != Some(&local_ty) {
                                return Err(SubsetError::new(*nl, *nc, "lower：移动源类型与目标不符"));
                            }
                            body.push(Stmt::MoveAssign { dst: id, src, line: *nl, col: *nc });
                            set_live(&mut slots, src, false);
                            slots.push(Slot { ty: local_ty, live: true });
                        }
                        other => {
                            let (il, ic) = subset::expr_span(other);
                            return Err(SubsetError::new(il, ic, "lower：let 初始化形态"));
                        }
                    }
                    let slot = slots.last().expect("slot pushed").clone();
                    locals.push(Local { id, name: name.clone(), ty: slot.ty });
                    scope.insert(name.clone(), id);
                }
                St::Expr(Some(Ex::Call(callee, args, l, c)), _, _) => {
                    last_pos = (*l, *c);
                    let is_println =
                        matches!(callee.as_ref(), Ex::Path(n, _, _) if n == "println");
                    if !is_println || args.len() != 1 {
                        return Err(SubsetError::new(*l, *c, "lower：println 形态"));
                    }
                    let arg = match &args[0] {
                        Ex::Path(n, nl, nc) => {
                            let id = lookup(&scope, n, *nl, *nc)?;
                            let n_bytes = match slots.get(id.0 as usize).map(|s| &s.ty) {
                                Some(Ty::Bytes(n)) => *n,
                                _ => return Err(SubsetError::new(*nl, *nc, "lower：println 实参非字符串绑定")),
                            };
                            // 与 check 的类型表交叉核对（唯一类型来源）
                            let table = expr_byte_len(types, &args[0], *nl, *nc, "println 实参")?;
                            if table != n_bytes {
                                return Err(SubsetError::new(*nl, *nc, "lower：println 实参类型与局部绑定不符"));
                            }
                            StrArg::Local(id)
                        }
                        Ex::Lit(Lit::Str(s), sl, sc) => {
                            let n = expr_byte_len(types, &args[0], *sl, *sc, "println 实参")?;
                            if n != s.len() as u64 {
                                return Err(SubsetError::new(
                                    *sl,
                                    *sc,
                                    format!("lower：println 实参类型 [byte; {}] 与字面量 {} 字节不符", n, s.len()),
                                ));
                            }
                            StrArg::Bytes(s.as_bytes().to_vec())
                        }
                        other => {
                            let (al, ac) = subset::expr_span(other);
                            return Err(SubsetError::new(al, ac, "lower：println 实参"));
                        }
                    };
                    body.push(Stmt::Println { arg, line: *l, col: *c });
                }
                St::Expr(Some(Ex::Unbox(n, ul, uc)), _, _) => {
                    last_pos = (*ul, *uc);
                    let id = lookup(&scope, n, *ul, *uc)?;
                    if !matches!(slots.get(id.0 as usize).map(|s| &s.ty), Some(Ty::Box(_))) {
                        return Err(SubsetError::new(*ul, *uc, "lower：unbox 操作数非 Box"));
                    }
                    body.push(Stmt::Unbox { local: id, line: *ul, col: *uc });
                    set_live(&mut slots, id, false);
                }
                St::Assign(lhs, rhs, l, c) => {
                    last_pos = (*l, *c);
                    match (lhs, rhs) {
                        (Ex::Path(a, al, ac), Ex::Path(b, bl, bc)) => {
                            let ia = lookup(&scope, a, *al, *ac)?;
                            let ib = lookup(&scope, b, *bl, *bc)?;
                            require_box(&slots, ia, *al, *ac)?;
                            require_box(&slots, ib, *bl, *bc)?;
                            if ia == ib {
                                body.push(Stmt::SelfAssign { local: ia, line: *al, col: *ac });
                            } else {
                                let drop_dst = is_live(&slots, ia);
                                if drop_dst {
                                    body.push(Stmt::OverwriteAssign {
                                        dst: ia,
                                        src: ib,
                                        drop_dst: true,
                                        line: *al,
                                        col: *ac,
                                    });
                                } else {
                                    body.push(Stmt::MoveAssign { dst: ia, src: ib, line: *al, col: *ac });
                                }
                                set_live(&mut slots, ib, false);
                                set_live(&mut slots, ia, true);
                            }
                        }
                        (Ex::Path(a, al, ac), Ex::Box(inner, _, _)) => {
                            let ia = lookup(&scope, a, *al, *ac)?;
                            require_box(&slots, ia, *al, *ac)?;
                            let value = int_lit(inner)?;
                            let drop_dst = is_live(&slots, ia);
                            body.push(Stmt::BindBoxAssign {
                                local: ia,
                                value,
                                drop_dst,
                                line: *al,
                                col: *ac,
                            });
                            set_live(&mut slots, ia, true);
                        }
                        _ => return Err(SubsetError::new(*l, *c, "lower：赋值形态")),
                    }
                }
                _ => return Err(SubsetError::new(0, 0, "lower：未预期语句")),
            }
        }
        // 作用域清算：声明逆序、仅仍活跃的 Box owner
        let drops: Vec<LocalId> = locals
            .iter()
            .rev()
            .filter(|loc| matches!(loc.ty, Ty::Box(_)) && is_live(&slots, loc.id))
            .map(|loc| loc.id)
            .collect();
        if !drops.is_empty() {
            body.push(Stmt::ScopeExit {
                drops,
                line: last_pos.0.max(1),
                col: last_pos.1.max(1),
            });
        }
        funcs.push(Func {
            name: "main".to_string(),
            line: f.span.0.max(1),
            locals,
            body,
        });
    }
    Ok(Program { funcs })
}

fn lookup(
    scope: &HashMap<String, LocalId>,
    name: &str,
    line: usize,
    col: usize,
) -> Result<LocalId, SubsetError> {
    scope
        .get(name)
        .copied()
        .ok_or_else(|| SubsetError::new(line, col, format!("lower：'{}' 未绑定", name)))
}

fn is_live(slots: &[Slot], id: LocalId) -> bool {
    slots.get(id.0 as usize).map(|s| s.live).unwrap_or(false)
}

fn set_live(slots: &mut [Slot], id: LocalId, live: bool) {
    if let Some(s) = slots.get_mut(id.0 as usize) {
        s.live = live;
    }
}

fn require_box(slots: &[Slot], id: LocalId, line: usize, col: usize) -> Result<(), SubsetError> {
    if matches!(slots.get(id.0 as usize).map(|s| &s.ty), Some(Ty::Box(_))) {
        Ok(())
    } else {
        Err(SubsetError::new(line, col, "lower：目标非 Box 绑定"))
    }
}

/// 绑定类型：check 的 `let` 类型表 → IR 类型。
fn let_type(types: &TypeTable, init: &Ex, l: usize, c: usize) -> Result<Ty, SubsetError> {
    let t = types
        .let_ty(l, c)
        .ok_or_else(|| SubsetError::new(l, c, "lower：let 缺少 type table 记录"))?;
    let ir = ast_to_ir(t)
        .ok_or_else(|| SubsetError::new(l, c, format!("lower：let 类型 '{}' 不在 L2a 子集", t.display())))?;
    let init_t = types
        .expr_ty(init)
        .ok_or_else(|| SubsetError::new(l, c, "lower：let 初值缺少 type table 记录"))?;
    if ast_to_ir(init_t) != Some(ir.clone()) {
        return Err(SubsetError::new(l, c, "lower：let 初值类型与绑定类型不符"));
    }
    Ok(ir)
}

fn expr_byte_len(
    types: &TypeTable,
    e: &Ex,
    line: usize,
    col: usize,
    what: &str,
) -> Result<u64, SubsetError> {
    let t = types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(line, col, format!("lower：{} 缺少 type table 记录", what)))?;
    type_table::byte_array_len(t).ok_or_else(|| {
        SubsetError::new(line, col, format!("lower：{} 类型 '{}' 非 [byte; N]", what, t.display()))
    })
}

fn ast_to_ir(t: &AstTy) -> Option<Ty> {
    match t {
        AstTy::Arr(elem, n) if matches!(elem.as_ref(), AstTy::Prim("byte")) => Some(Ty::Bytes(*n)),
        AstTy::Prim("i64") => Some(Ty::I64),
        AstTy::BoxTy(inner) if matches!(inner.as_ref(), AstTy::Prim("i64")) => {
            Some(Ty::Box(Box::new(Ty::I64)))
        }
        _ => None,
    }
}

fn int_lit(e: &Ex) -> Result<i64, SubsetError> {
    match e {
        Ex::Lit(Lit::Int(v), l, c) => i64::try_from(*v)
            .map_err(|_| SubsetError::new(*l, *c, "lower：整型字面量超出 i64")),
        other => {
            let (l, c) = subset::expr_span(other);
            Err(SubsetError::new(l, c, "lower：box 载荷非整型字面量"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lower;
    use crate::backend::ir::{LocalId, Stmt, StrArg, Ty};
    use crate::type_table::TypeTable;
    use crate::compile_typed;

    fn ir(src: &str) -> crate::backend::ir::Program {
        let c = compile_typed(src);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        lower(&c.prog, &c.types).expect("lower")
    }

    fn body(src: &str) -> Vec<Stmt> {
        ir(src).funcs[0].body.clone()
    }

    #[test]
    fn hello_lowers_to_one_local_and_two_stmts() {
        let p = ir("fn main() {\n    let greeting = \"Hello, ZOR\";\n    println(greeting);\n}\n");
        assert_eq!(p.funcs.len(), 1);
        let f = &p.funcs[0];
        assert_eq!(f.name, "main");
        assert_eq!(f.line, 1);
        assert_eq!(f.locals.len(), 1);
        assert_eq!(f.locals[0].ty, Ty::Bytes(10));
        assert_eq!(f.body.len(), 2);
        match &f.body[0] {
            Stmt::BindStr { local, bytes, line, .. } => {
                assert_eq!(local.0, 0);
                assert_eq!(bytes.as_slice(), b"Hello, ZOR");
                assert_eq!(*line, 2);
            }
            other => panic!("expected BindStr, got {other:?}"),
        }
        match &f.body[1] {
            Stmt::Println { arg: StrArg::Local(id), line, .. } => {
                assert_eq!(id.0, 0);
                assert_eq!(*line, 3);
            }
            other => panic!("expected Println(Local), got {other:?}"),
        }
    }

    #[test]
    fn literal_println_lowers_to_bytes() {
        let p = ir("fn main() { println(\"hi\"); }");
        match &p.funcs[0].body[0] {
            Stmt::Println { arg: StrArg::Bytes(b), .. } => assert_eq!(b.as_slice(), b"hi"),
            other => panic!("expected Println(Bytes), got {other:?}"),
        }
    }

    #[test]
    fn shadowing_allocates_new_local() {
        let p = ir("fn main() { let s = \"a\"; let s = \"bb\"; println(s); }");
        let f = &p.funcs[0];
        assert_eq!(f.locals.len(), 2);
        match &f.body[2] {
            Stmt::Println { arg: StrArg::Local(id), .. } => assert_eq!(id.0, 1),
            other => panic!("expected Println(Local), got {other:?}"),
        }
    }

    #[test]
    fn missing_type_table_entry_fails_closed() {
        let c = compile_typed("fn main() { let s = \"hi\"; println(s); }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let empty = TypeTable::new();
        let e = lower(&c.prog, &empty).expect_err("lower must fail closed without types");
        assert!(e.what.contains("type table"), "{}", e.what);
    }

    // —— G-E-2 状态机：6 条 manifest 样本 + 2 条矩阵空格 ——

    #[test]
    fn box_create_read_unbox_has_no_scope_drop() {
        let b = body("fn main() {\n    let b = box 1;\n    let v = *b;\n    unbox b;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(matches!(b[0], Stmt::BindBox { value: 1, .. }));
        assert!(matches!(b[1], Stmt::BindI64Deref { .. }));
        assert!(matches!(b[2], Stmt::Unbox { .. }));
    }

    #[test]
    fn box_scope_drop_is_reverse_declaration_order() {
        let b = body("fn main() {\n    let a = box 1;\n    let b = box 2;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        match &b[2] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(1), LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn box_overwrite_drops_live_dst() {
        let b = body("fn main() {\n    let a = box 1;\n    let b = box 2;\n    a = b;\n}\n");
        assert_eq!(b.len(), 4, "{b:?}");
        match &b[2] {
            Stmt::OverwriteAssign { dst, src, drop_dst, .. } => {
                assert_eq!((dst.0, src.0, *drop_dst), (0, 1, true));
            }
            other => panic!("expected OverwriteAssign, got {other:?}"),
        }
        match &b[3] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn box_move_only_new_owner_is_dropped() {
        let b = body("fn main() {\n    let a = box 1;\n    let b = a;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(matches!(b[1], Stmt::MoveAssign { .. }));
        match &b[2] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(1)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn box_self_assign_does_not_drop() {
        let b = body("fn main() {\n    let a = box 1;\n    a = a;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(matches!(b[1], Stmt::SelfAssign { .. }));
        match &b[2] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn box_unbox_then_reinit_does_not_drop_old() {
        let b = body("fn main() {\n    let a = box 1;\n    unbox a;\n    a = box 2;\n}\n");
        assert_eq!(b.len(), 4, "{b:?}");
        assert!(matches!(b[1], Stmt::Unbox { .. }));
        match &b[2] {
            Stmt::BindBoxAssign { drop_dst, value, .. } => {
                assert_eq!((*drop_dst, *value), (false, 2));
            }
            other => panic!("expected BindBoxAssign, got {other:?}"),
        }
        match &b[3] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn reinit_after_move_does_not_drop_old() {
        let b = body("fn main() {\n    let a = box 1;\n    let b = a;\n    a = box 2;\n}\n");
        assert_eq!(b.len(), 4, "{b:?}");
        match &b[2] {
            Stmt::BindBoxAssign { drop_dst, .. } => assert!(!*drop_dst),
            other => panic!("expected BindBoxAssign, got {other:?}"),
        }
        match &b[3] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(1), LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn shadowed_box_bindings_keep_distinct_slots() {
        let b = body("fn main() {\n    let a = box 1;\n    let a = box 2;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        match &b[2] {
            Stmt::ScopeExit { drops, .. } => assert_eq!(drops, &vec![LocalId(1), LocalId(0)]),
            other => panic!("expected ScopeExit, got {other:?}"),
        }
    }

    #[test]
    fn box_program_reports_l2a_runtime_need() {
        let p = ir("fn main() { let b = box 1; }");
        assert!(p.needs_l2a_runtime());
        assert_eq!(p.first_l2a_pos(), Some((1, 13)));
        let hello = ir("fn main() { println(\"hi\"); }");
        assert!(!hello.needs_l2a_runtime());
    }
}
