//! L2c 支持面判定（子集闸）。
//!
//! 闸先于 lower：不支持即返回 `SubsetError`，由调用方映射为 rc=1 且不落产物。
//! 本模块只判形态，不复制 check 的类型规则；类型与所有权判定唯一来源仍是 check。
//!
//! L2c 面：stdout 契约的五实现面（多实参／终饰／12 原始类型／`[byte;N]`／终止前
//! 原子输出）。`print` 与复合类型（元组／struct／enum／数组）按冻结口径 fail-closed。

use crate::parser::{Blk, Ex, FnDecl, Lit, Prog, St, Ty};
use std::collections::HashMap;

/// 首片绑定种类：字节串／Box<i64>／全部在册原始类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingKind {
    Str,
    BoxI64,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Byte,
    Bool,
    Unit,
    F32,
    F64,
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

/// 支持面：单个 `main`（无参、unit 返回）、原始类型／字节串 `let`、`println` 多实参。
/// `print` 与复合类型按 L2c 契约 fail-closed（rc=1、零产物）。
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

/// 标注与初值种类合一；常量负字面量可按标注收窄（`-128: i8`）。
fn unify_ann(
    ann: Option<BindingKind>,
    kind: BindingKind,
    init: &Ex,
    l: usize,
    c: usize,
) -> Result<BindingKind, SubsetError> {
    match ann {
        None => Ok(kind),
        Some(a) if a == kind => Ok(a),
        Some(a) if narrowable_const(init) && width_compatible(a, kind) => Ok(a),
        Some(_) => Err(SubsetError::new(l, c, "let 类型标注与初值类型不符")),
    }
}

fn narrowable_const(e: &Ex) -> bool {
    matches!(
        e,
        Ex::Un(op, inner, _, _)
            if op == "-" && matches!(inner.as_ref(), Ex::Lit(Lit::Int(_) | Lit::Float(_), _, _))
    )
}

fn width_compatible(ann: BindingKind, expr: BindingKind) -> bool {
    (is_int_kind(ann) && is_int_kind(expr))
        || (matches!(ann, BindingKind::F32 | BindingKind::F64) && expr == BindingKind::F64)
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
                // L2c：全部在册原始类型与 [byte; N] 标注进入 codegen；ref／Box 等仍拒判。
                let ann_kind = match ann {
                    None => None,
                    Some(Ty::Prim(p)) => match prim_kind(p) {
                        Some(k) => Some(k),
                        None => {
                            return Err(SubsetError::new(
                                *l,
                                *c,
                                "let 类型标注（仅原始类型与 [byte; N] 可 codegen）",
                            ))
                        }
                    },
                    Some(Ty::Arr(elem, _)) if matches!(elem.as_ref(), Ty::Prim("byte")) => {
                        Some(BindingKind::Str)
                    }
                    Some(_) => {
                        return Err(SubsetError::new(
                            *l,
                            *c,
                            "let 类型标注（仅原始类型与 [byte; N] 可 codegen）",
                        ))
                    }
                };
                let kind = match init {
                    Ex::Lit(Lit::Str(_), _, _) => {
                        if ann_kind.is_some() && ann_kind != Some(BindingKind::Str) {
                            return Err(SubsetError::new(*l, *c, "let 类型标注与字符串初值不符"));
                        }
                        BindingKind::Str
                    }
                    Ex::Box(inner, il, ic) => {
                        if !matches!(inner.as_ref(), Ex::Lit(Lit::Int(_), _, _)) {
                            return Err(SubsetError::new(*il, *ic, "box 载荷非整型字面量"));
                        }
                        if ann_kind.is_some() {
                            return Err(SubsetError::new(*l, *c, "let 类型标注与 Box 初值不符"));
                        }
                        BindingKind::BoxI64
                    }
                    Ex::Lit(Lit::Int(_), _, _) => match ann_kind {
                        None => BindingKind::I64,
                        Some(k) if is_int_kind(k) || k == BindingKind::Byte => k,
                        Some(_) => {
                            return Err(SubsetError::new(*l, *c, "let 类型标注与整型初值不符"))
                        }
                    },
                    Ex::Lit(Lit::Bool(_), _, _) => match ann_kind {
                        None | Some(BindingKind::Bool) => BindingKind::Bool,
                        _ => return Err(SubsetError::new(*l, *c, "let 类型标注与 bool 初值不符")),
                    },
                    Ex::UnitLit(_, _) => match ann_kind {
                        None | Some(BindingKind::Unit) => BindingKind::Unit,
                        _ => return Err(SubsetError::new(*l, *c, "let 类型标注与 unit 初值不符")),
                    },
                    Ex::Lit(Lit::Float(_), _, _) => match ann_kind {
                        None => BindingKind::F64,
                        Some(BindingKind::F32) => BindingKind::F32,
                        Some(BindingKind::F64) => BindingKind::F64,
                        _ => {
                            return Err(SubsetError::new(*l, *c, "let 类型标注与浮点初值不符"))
                        }
                    },
                    Ex::Deref(base, il, ic) => match base.as_ref() {
                        Ex::Path(n, _, _) if bindings.get(n) == Some(&BindingKind::BoxI64) => {
                            match ann_kind {
                                None | Some(BindingKind::I64) => BindingKind::I64,
                                Some(_) => {
                                    return Err(SubsetError::new(
                                        *l,
                                        *c,
                                        "let 类型标注与解引用初值不符",
                                    ))
                                }
                            }
                        }
                        _ => {
                            return Err(SubsetError::new(*il, *ic, "解引用源不是 Box 绑定"))
                        }
                    },
                    Ex::Path(n, il, ic) => match bindings.get(n) {
                        Some(BindingKind::BoxI64) => {
                            if ann_kind.is_some() {
                                return Err(SubsetError::new(*l, *c, "let 类型标注与 Box 初值不符"));
                            }
                            BindingKind::BoxI64
                        }
                        Some(k) if is_copy_kind(*k) => match ann_kind {
                            None => *k,
                            Some(a) if a == *k => a,
                            Some(_) => {
                                return Err(SubsetError::new(
                                    *il,
                                    *ic,
                                    "let 类型标注与复制源类型不符",
                                ))
                            }
                        },
                        _ => return Err(SubsetError::new(*il, *ic, "let 初始化非可复制绑定")),
                    },
                    Ex::Bin(..) => match int_expr_kind(init, bindings) {
                        Some(k) => unify_ann(ann_kind, k, init, *l, *c)?,
                        None => {
                            let (il, ic) = expr_span(init);
                            return Err(SubsetError::new(il, ic, "let 初始化非整型算术表达式"));
                        }
                    },
                    Ex::Un(..) => {
                        if let Some(k) = int_expr_kind(init, bindings) {
                            unify_ann(ann_kind, k, init, *l, *c)?
                        } else if let Some(k) = const_unary_kind(init, bindings) {
                            unify_ann(ann_kind, k, init, *l, *c)?
                        } else {
                            let (il, ic) = expr_span(init);
                            return Err(SubsetError::new(il, ic, "let 初始化非可 codegen 形态"));
                        }
                    }
                    _ => {
                        let (il, ic) = expr_span(init);
                        return Err(SubsetError::new(il, ic, "let 初始化非可 codegen 形态"));
                    }
                };
                bindings.insert(name.clone(), kind);
            }
            St::Expr(Some(Ex::Call(callee, args, l, c)), _, _) => {
                let name = match callee.as_ref() {
                    Ex::Path(n, _, _) => n.as_str(),
                    _ => {
                        let (cl, cc) = expr_span(callee);
                        return Err(SubsetError::new(cl, cc, "调用形态"));
                    }
                };
                if name == "print" {
                    return Err(SubsetError::new(
                        *l,
                        *c,
                        "print 内建（L2c 不实现，fail-closed；stdout 契约只覆盖 println）",
                    ));
                }
                if name != "println" {
                    let (cl, cc) = expr_span(callee);
                    return Err(SubsetError::new(cl, cc, "非 println 调用"));
                }
                if args.is_empty() {
                    return Err(SubsetError::new(*l, *c, "println 实参数量 0（至少一参）"));
                }
                for a in args {
                    if !is_printable_arg(a, bindings) {
                        let (al, ac) = expr_span(a);
                        return Err(SubsetError::new(
                            al,
                            ac,
                            "println 实参不可打印（引用／Box／复合类型或未实现形态）",
                        ));
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
            St::Expr(Some(e), l, c) if matches!(e, Ex::Bin(..) | Ex::Un(..)) => {
                if int_expr_kind(e, bindings).is_none() {
                    let (el, ec) = expr_span(e);
                    return Err(SubsetError::new(el, ec, "整型算术表达式形态"));
                }
            }
            St::Expr(Some(_), l, c) => {
                return Err(SubsetError::new(
                    *l,
                    *c,
                    "表达式语句（首片只支持 println 调用／unbox）",
                ))
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

fn prim_kind(p: &str) -> Option<BindingKind> {
    Some(match p {
        "i8" => BindingKind::I8,
        "i16" => BindingKind::I16,
        "i32" => BindingKind::I32,
        "i64" => BindingKind::I64,
        "u8" => BindingKind::U8,
        "u16" => BindingKind::U16,
        "u32" => BindingKind::U32,
        "u64" => BindingKind::U64,
        "byte" => BindingKind::Byte,
        "bool" => BindingKind::Bool,
        "unit" => BindingKind::Unit,
        "f32" => BindingKind::F32,
        "f64" => BindingKind::F64,
        _ => return None,
    })
}

fn is_int_kind(k: BindingKind) -> bool {
    matches!(
        k,
        BindingKind::I8
            | BindingKind::I16
            | BindingKind::I32
            | BindingKind::I64
            | BindingKind::U8
            | BindingKind::U16
            | BindingKind::U32
            | BindingKind::U64
    )
}

fn is_copy_kind(k: BindingKind) -> bool {
    is_int_kind(k)
        || matches!(
            k,
            BindingKind::Byte
                | BindingKind::Bool
                | BindingKind::Unit
                | BindingKind::F32
                | BindingKind::F64
        )
}

fn is_printable_kind(k: BindingKind) -> bool {
    k == BindingKind::Str || is_copy_kind(k)
}

fn is_int_lit(e: &Ex) -> bool {
    matches!(e, Ex::Lit(Lit::Int(_), _, _))
}

/// 整型算术的形态判定：字面量向同侧整型收敛；其他在册整型放行给 lower。
fn int_expr_kind(e: &Ex, bindings: &HashMap<String, BindingKind>) -> Option<BindingKind> {
    match e {
        Ex::Lit(Lit::Int(_), _, _) => Some(BindingKind::I64),
        Ex::Path(n, _, _) => bindings.get(n).copied().filter(|k| is_int_kind(*k)),
        Ex::Deref(base, _, _) => match base.as_ref() {
            Ex::Path(n, _, _) if bindings.get(n) == Some(&BindingKind::BoxI64) => {
                Some(BindingKind::I64)
            }
            _ => None,
        },
        Ex::Un(op, inner, _, _) if op == "-" => {
            match int_expr_kind(inner, bindings)? {
                // 无符号一元 `-` 不在 codegen 子集。
                BindingKind::U8 | BindingKind::U16 | BindingKind::U32 | BindingKind::U64 => None,
                k => Some(k),
            }
        }
        Ex::Bin(op, a, b, _, _) if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%") => {
            let ka = int_expr_kind(a, bindings)?;
            let kb = int_expr_kind(b, bindings)?;
            if ka == kb {
                return Some(ka);
            }
            // 上下文整型字面量向另一侧收敛（与 check 的 expect 语义一致）
            if kb == BindingKind::I64 && is_int_lit(b) {
                return Some(ka);
            }
            if ka == BindingKind::I64 && is_int_lit(a) {
                return Some(kb);
            }
            None
        }
        _ => None,
    }
}

/// 常量一元形态（浮点／bool）：`-<float>`、`!<bool>`；供 let 与 println 共用。
fn const_unary_kind(e: &Ex, bindings: &HashMap<String, BindingKind>) -> Option<BindingKind> {
    match e {
        Ex::Un(op, inner, _, _) if op == "-" => match scalar_expr_kind(inner, bindings)? {
            BindingKind::F32 | BindingKind::F64 => scalar_expr_kind(inner, bindings),
            _ => None,
        },
        Ex::Un(op, inner, _, _) if op == "!" => {
            match scalar_expr_kind(inner, bindings)? {
                BindingKind::Bool => Some(BindingKind::Bool),
                _ => None,
            }
        }
        _ => None,
    }
}

fn scalar_expr_kind(e: &Ex, bindings: &HashMap<String, BindingKind>) -> Option<BindingKind> {
    match e {
        Ex::Lit(Lit::Int(_), _, _) => Some(BindingKind::I64),
        Ex::Lit(Lit::Float(_), _, _) => Some(BindingKind::F64),
        Ex::Lit(Lit::Bool(_), _, _) => Some(BindingKind::Bool),
        Ex::UnitLit(_, _) => Some(BindingKind::Unit),
        Ex::Path(n, _, _) => bindings.get(n).copied().filter(|k| is_copy_kind(*k)),
        _ => None,
    }
}

/// println 实参形态：字面量／可复制绑定／Box 解引用／整型算术／常量一元。
fn is_printable_arg(e: &Ex, bindings: &HashMap<String, BindingKind>) -> bool {
    match e {
        Ex::Lit(Lit::Str(_) | Lit::Int(_) | Lit::Float(_) | Lit::Bool(_), _, _) => true,
        Ex::UnitLit(_, _) => true,
        Ex::Path(n, _, _) => bindings.get(n).copied().is_some_and(is_printable_kind),
        Ex::Deref(base, _, _) => matches!(
            base.as_ref(),
            Ex::Path(n, _, _) if bindings.get(n) == Some(&BindingKind::BoxI64)
        ),
        Ex::Un(op, inner, _, _) if op == "-" => {
            if let Some(k) = int_expr_kind(e, bindings) {
                return is_runtime_int_kind(k) || is_const_neg_kind(e, bindings);
            }
            matches!(
                scalar_expr_kind(inner, bindings),
                Some(BindingKind::F32 | BindingKind::F64)
            )
        }
        Ex::Un(op, inner, _, _) if op == "!" => {
            matches!(scalar_expr_kind(inner, bindings), Some(BindingKind::Bool))
        }
        Ex::Bin(..) => int_expr_kind(e, bindings).is_some_and(is_runtime_int_kind),
        _ => false,
    }
}

fn is_const_neg_kind(e: &Ex, bindings: &HashMap<String, BindingKind>) -> bool {
    matches!(
        e,
        Ex::Un(op, inner, _, _)
            if op == "-"
                && matches!(
                    int_expr_kind(inner, bindings),
                    Some(BindingKind::I8 | BindingKind::I16 | BindingKind::I32)
                )
    )
}

fn is_runtime_int_kind(k: BindingKind) -> bool {
    matches!(k, BindingKind::I64 | BindingKind::U8 | BindingKind::U32)
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
    fn multi_arg_println_is_supported() {
        ok("fn main() { let s = \"x\"; println(s, 1, true); }");
    }

    #[test]
    fn all_primitives_are_printable() {
        for (decl, arg) in [
            ("let x: i8 = -128;", "x"),
            ("let x: i16 = -32768;", "x"),
            ("let x: i32 = -2147483648;", "x"),
            ("let x: i64 = -9223372036854775808;", "x"),
            ("let x: u8 = 255;", "x"),
            ("let x: u16 = 65535;", "x"),
            ("let x: u32 = 4294967295;", "x"),
            ("let x: u64 = 18446744073709551615;", "x"),
            ("let x: byte = 255;", "x"),
            ("let x = true;", "x"),
            ("let x = ();", "x"),
            ("let x: f32 = 1.1;", "x"),
            ("let x: f64 = -0.0;", "x"),
        ] {
            ok(&format!("fn main() {{ {decl} println({arg}); }}"));
        }
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
    fn byte_array_annotation_is_supported() {
        ok("fn main() { let s: [byte; 2] = \"hi\"; println(s); }");
    }

    #[test]
    fn non_string_let_is_rejected() {
        // 浮点算术不在 L2c stdout 契约内；本测试保留“非 L2c 整型算术”的拒判面。
        assert!(err_of("fn main() { let x = 1.0 + 2.0; }").contains("整型"));
    }

    #[test]
    fn int_println_is_supported() {
        ok("fn main() { println(42); }");
        ok("fn main() { let a = 1; let b = 2; println(a + b); }");
    }

    #[test]
    fn zero_arg_println_is_frontend_rejected() {
        // 零参 println 由 check 的 E1601 拦截，子集闸看不到该形态。
        let (_, diags) = compile("fn main() { println(); }");
        assert!(!diags.is_empty(), "front-end must reject println()");
    }

    #[test]
    fn print_is_fail_closed() {
        assert!(err_of("fn main() { print(\"x\"); }").contains("print"));
        assert!(err_of("fn main() { print(); }").contains("print"));
    }

    #[test]
    fn composite_tuple_print_is_fail_closed() {
        assert!(err_of("fn main() { println((1, 2)); }").contains("不可打印"));
    }

    #[test]
    fn box_print_is_fail_closed() {
        assert!(err_of("fn main() { let b = box 1; println(b); }").contains("不可打印"));
    }

    #[test]
    fn ref_print_is_fail_closed() {
        assert!(err_of("fn main() { let x = 1; let r = &x; println(r); }").contains("let"));
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
