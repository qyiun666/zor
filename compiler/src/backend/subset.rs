//! 0.2.0 首片支持面判定（子集闸）。
//!
//! 闸先于 lower：不支持即返回 `SubsetError`，由调用方映射为 rc=1 且不落产物。
//! 本模块只判形态，不复制 check 的类型规则；类型与所有权判定唯一来源仍是 check。

use crate::parser::{Blk, Ex, FnDecl, Lit, Prog, St, Ty};
use std::collections::HashMap;

/// 首片绑定种类：字符串／Box<i64>／i64。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingKind {
    Str,
    BoxI64,
    I64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubsetError {
    pub line: usize,
    pub col: usize,
    pub what: String,
}

impl SubsetError {
    pub fn new(line: usize, col: usize, what: impl Into<String>) -> Self {
        SubsetError { line, col, what: what.into() }
    }
}

/// 支持面：单个 `main`（无参、unit 返回）、字符串字面量 `let`、`println` 单实参。
pub fn check(prog: &Prog) -> Result<(), SubsetError> {
    if let Some(s) = prog.structs.first() {
        return Err(SubsetError::new(
            s.span.0,
            s.span.1,
            format!("struct 声明 '{}'", s.name),
        ));
    }
    if let Some(e) = prog.enums.first() {
        return Err(SubsetError::new(
            e.span.0,
            e.span.1,
            format!("enum 声明 '{}'", e.name),
        ));
    }
    for f in &prog.fns {
        if f.name != "main" {
            return Err(SubsetError::new(
                f.span.0,
                f.span.1,
                format!("额外函数 '{}'（首片只支持 main）", f.name),
            ));
        }
    }
    let main = match prog.fns.iter().find(|f| f.name == "main") {
        Some(f) => f,
        None => return Err(SubsetError::new(1, 1, "缺少 main 函数")),
    };
    check_main(main)?;
    let mut bindings = HashMap::new();
    check_block(&main.body, &mut bindings)
}

fn check_main(f: &FnDecl) -> Result<(), SubsetError> {
    if let Some((_, _, l, c)) = f.params.first() {
        return Err(SubsetError::new(*l, *c, "main 带参数"));
    }
    if let Some((ret, l, c)) = &f.ret {
        if ret != &Ty::unit() {
            return Err(SubsetError::new(
                *l,
                *c,
                format!("main 返回 '{}'（首片只支持 unit）", ret.display()),
            ));
        }
    }
    Ok(())
}

fn check_block(blk: &Blk, bindings: &mut HashMap<String, BindingKind>) -> Result<(), SubsetError> {
    if let Some(tail) = &blk.tail {
        if !matches!(tail.as_ref(), Ex::UnitLit(_, _)) {
            let (l, c) = expr_span(tail);
            return Err(SubsetError::new(l, c, "main 尾表达式"));
        }
    }
    for st in &blk.stmts {
        match st {
            St::Let(name, ann, init, l, c) => {
                if ann.is_some() {
                    return Err(SubsetError::new(*l, *c, "let 类型标注"));
                }
                let kind = match init {
                    Ex::Lit(Lit::Str(_), _, _) => BindingKind::Str,
                    Ex::Box(inner, il, ic) => {
                        if !matches!(inner.as_ref(), Ex::Lit(Lit::Int(_), _, _)) {
                            return Err(SubsetError::new(*il, *ic, "box 载荷非整型字面量"));
                        }
                        BindingKind::BoxI64
                    }
                    Ex::Lit(Lit::Int(_), _, _) => BindingKind::I64,
                    Ex::Deref(base, il, ic) => match base.as_ref() {
                        Ex::Path(n, _, _) if bindings.get(n) == Some(&BindingKind::BoxI64) => {
                            BindingKind::I64
                        }
                        _ => {
                            return Err(SubsetError::new(*il, *ic, "解引用源不是 Box 绑定"))
                        }
                    },
                    Ex::Path(n, il, ic) => match bindings.get(n) {
                        Some(BindingKind::BoxI64) => BindingKind::BoxI64,
                        _ => return Err(SubsetError::new(*il, *ic, "let 初始化非字符串字面量")),
                    },
                    _ => {
                        let (il, ic) = expr_span(init);
                        return Err(SubsetError::new(il, ic, "let 初始化非字符串字面量"));
                    }
                };
                bindings.insert(name.clone(), kind);
            }
            St::Expr(Some(Ex::Call(callee, args, l, c)), _, _) => {
                let is_println =
                    matches!(callee.as_ref(), Ex::Path(n, _, _) if n == "println");
                if !is_println {
                    let (cl, cc) = expr_span(callee);
                    return Err(SubsetError::new(cl, cc, "非 println 调用"));
                }
                if args.len() != 1 {
                    return Err(SubsetError::new(
                        *l,
                        *c,
                        format!("println 实参数量 {}（首片只支持单实参）", args.len()),
                    ));
                }
                match &args[0] {
                    Ex::Lit(Lit::Str(_), _, _) => {}
                    Ex::Path(n, l2, c2) => {
                        if bindings.get(n) != Some(&BindingKind::Str) {
                            return Err(SubsetError::new(
                                *l2,
                                *c2,
                                format!("println 实参 '{}' 不是首片字符串绑定", n),
                            ));
                        }
                    }
                    other => {
                        let (al, ac) = expr_span(other);
                        return Err(SubsetError::new(al, ac, "println 实参形态"));
                    }
                }
            }
            St::Expr(Some(Ex::Unbox(n, ul, uc)), _, _) => {
                if bindings.get(n) != Some(&BindingKind::BoxI64) {
                    return Err(SubsetError::new(
                        *ul,
                        *uc,
                        format!("unbox 操作数 '{}' 不是 Box 绑定", n),
                    ));
                }
            }
            St::Expr(Some(_), l, c) => {
                return Err(SubsetError::new(*l, *c, "表达式语句（首片只支持 println 调用／unbox）"))
            }
            St::Expr(None, l, c) => return Err(SubsetError::new(*l, *c, "空语句")),
            St::LetDecl(_, _, l, c) => return Err(SubsetError::new(*l, *c, "let 无初始化")),
            St::Assign(lhs, rhs, l, c) => {
                let ok = match (lhs, rhs) {
                    (Ex::Path(a, _, _), Ex::Path(b, _, _)) => {
                        bindings.get(a) == Some(&BindingKind::BoxI64)
                            && bindings.get(b) == Some(&BindingKind::BoxI64)
                    }
                    (Ex::Path(a, _, _), Ex::Box(inner, _, _)) => {
                        bindings.get(a) == Some(&BindingKind::BoxI64)
                            && matches!(inner.as_ref(), Ex::Lit(Lit::Int(_), _, _))
                    }
                    _ => false,
                };
                if !ok {
                    return Err(SubsetError::new(
                        *l,
                        *c,
                        "赋值语句（首片只支持 Box 绑定之间／Box 重初始化）",
                    ));
                }
            }
            St::If(_, _, _, l, c) => return Err(SubsetError::new(*l, *c, "if 语句")),
            St::While(_, _, l, c) => return Err(SubsetError::new(*l, *c, "while 语句")),
        }
    }
    Ok(())
}

pub(crate) fn expr_span(e: &Ex) -> (usize, usize) {
    e.span()
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::compile;

    fn err_of(src: &str) -> String {
        let (prog, diags) = compile(src);
        assert!(diags.is_empty(), "front-end must accept: {diags:?}");
        check(&prog).expect_err("subset must reject").what
    }

    fn ok(src: &str) {
        let (prog, diags) = compile(src);
        assert!(diags.is_empty(), "front-end must accept: {diags:?}");
        check(&prog).expect("subset must accept");
    }

    #[test]
    fn hello_is_supported() {
        ok("fn main() {\n    let greeting = \"Hello, ZOR\";\n    println(greeting);\n}\n");
    }

    #[test]
    fn literal_println_is_supported() {
        ok("fn main() { println(\"hi\"); }");
    }

    #[test]
    fn empty_main_is_supported() {
        ok("fn main() {}");
    }

    #[test]
    fn struct_is_rejected() {
        assert!(err_of("struct Mark {}\nfn main() { let m = Mark {}; }").contains("struct"));
    }

    #[test]
    fn enum_is_rejected() {
        assert!(err_of("enum E { A }\nfn main() {}").contains("enum"));
    }

    #[test]
    fn extra_function_is_rejected() {
        assert!(err_of("fn helper() {}\nfn main() {}").contains("额外函数"));
    }

    #[test]
    fn missing_main_is_frontend_rejected() {
        // 入口缺失由 check 的 E1702 拦截，子集闸看不到该形态。
        let (_, diags) = compile("fn helper() {}");
        assert!(!diags.is_empty(), "front-end must reject missing main");
    }

    #[test]
    fn main_params_are_rejected() {
        assert!(err_of("fn main(x: i64) {}").contains("带参数"));
    }

    #[test]
    fn main_non_unit_return_is_frontend_rejected() {
        // 入口返回类型由 check 的 E1703 拦截，子集闸看不到该形态。
        let (_, diags) = compile("fn main() -> i64 { 1 }");
        assert!(!diags.is_empty(), "front-end must reject non-unit main");
    }

    #[test]
    fn let_annotation_is_rejected() {
        assert!(err_of("fn main() { let s: [byte; 2] = \"hi\"; println(s); }").contains("标注"));
    }

    #[test]
    fn non_string_let_is_rejected() {
        assert!(err_of("fn main() { let x = 1 + 2; }").contains("非字符串"));
    }

    #[test]
    fn non_string_println_is_rejected() {
        assert!(err_of("fn main() { println(42); }").contains("println"));
    }

    #[test]
    fn multi_arg_println_is_rejected() {
        assert!(err_of("fn main() { let s = \"x\"; println(s, s); }").contains("单实参"));
    }

    #[test]
    fn zero_arg_println_is_frontend_rejected() {
        // 零参 println 由 check 的 E1601 拦截，子集闸看不到该形态。
        let (_, diags) = compile("fn main() { println(); }");
        assert!(!diags.is_empty(), "front-end must reject println()");
    }

    #[test]
    fn assign_is_rejected() {
        assert!(err_of("fn main() { let s = \"a\"; s = \"b\"; }").contains("赋值"));
    }

    #[test]
    fn if_is_rejected() {
        assert!(err_of("fn main() { if true { } }").contains("if"));
    }

    #[test]
    fn while_is_rejected() {
        assert!(err_of("fn main() { while true { } }").contains("while"));
    }

    // —— L2a G-E-2 形态：6 条 manifest 样本必须通过 subset ——

    #[test]
    fn box_create_read_unbox_is_supported() {
        ok("fn main() { let b = box 1; let v = *b; unbox b; }");
    }

    #[test]
    fn box_move_is_supported() {
        ok("fn main() { let a = box 1; let b = a; }");
    }

    #[test]
    fn box_overwrite_is_supported() {
        ok("fn main() { let a = box 1; let b = box 2; a = b; }");
    }

    #[test]
    fn box_self_assign_is_supported() {
        ok("fn main() { let a = box 1; a = a; }");
    }

    #[test]
    fn box_unbox_then_reinit_is_supported() {
        ok("fn main() { let a = box 1; unbox a; a = box 2; }");
    }

    #[test]
    fn string_assign_is_rejected() {
        assert!(err_of("fn main() { let s = \"a\"; let t = \"b\"; s = t; }").contains("赋值"));
    }

    #[test]
    fn box_non_int_payload_is_rejected() {
        assert!(err_of("fn main() { let b = box \"x\"; }").contains("box"));
    }
}
