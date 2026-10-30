//! L2d 支持面判定（子集闸）。
//!
//! 闸先于 lower：不支持即返回 `SubsetError`，由调用方映射为 rc=1 且不落产物。
//! 本模块只判形态，不复制 check 的类型规则；类型与所有权判定唯一来源仍是 check。
//!
//! L2d 面：stdout 契约 v1.2 的复合输出（tuple／array／struct／enum、嵌套、
//! `[u8;N]` 数组、只读投影）与 L2c 五面零回退；类型级可打印判定，不可打印整体
//! fail-closed。`print` 仍不在契约。

use crate::parser::{Blk, Ex, FnDecl, Lit, Prog, St, Ty};
use crate::type_table::TypeTable;
use std::collections::{HashMap, HashSet};

/// 绑定种类：字节串／三型 Box（i64／u8／u32）／全部在册原始类型／可打印复合类型。
#[derive(Clone, Debug, PartialEq)]
enum BindingKind {
    Str,
    Box(Ty),
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
    Composite(Ty),
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

/// 支持面：单个 `main`（无参、unit 返回）、原始类型／字节串／可打印复合 `let`、
/// `println` 多实参。`print` 与非可打印复合按契约 fail-closed（rc=1、零产物）。
pub fn check(prog: &Prog, types: &TypeTable) -> Result<(), SubsetError> {
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
    check_block(&main.body, &mut bindings, types, prog)
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
        Some(a) if narrowable_const(init) && width_compatible(&a, &kind) => Ok(a),
        Some(_) => Err(SubsetError::new(l, c, "let 类型标注与初值类型不符")),
    }
}

fn narrowable_const(e: &Ex) -> bool {
    matches!(
        e,
        Ex::Un(op, inner, _, _)
            if op == "-"
                && matches!(
                    inner.as_ref(),
                    Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_), _, _)
                )
    )
}

fn width_compatible(ann: &BindingKind, expr: &BindingKind) -> bool {
    (is_int_kind(ann) && is_int_kind(expr))
        || (matches!(ann, BindingKind::F32 | BindingKind::F64) && expr == &BindingKind::F64)
}

fn check_block(
    blk: &Blk,
    bindings: &mut HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> Result<(), SubsetError> {
    // L3c：可物化复合绑定名（整棵 box 载荷旁路；遮蔽即失效）与编译期标量绑定名。
    let mut live_composites: HashSet<String> = HashSet::new();
    let mut scalar_consts: HashSet<String> = HashSet::new();
    if let Some(tail) = &blk.tail {
        if !matches!(tail.as_ref(), Ex::UnitLit(_, _)) {
            let (l, c) = expr_span(tail);
            return Err(SubsetError::new(l, c, "main 尾表达式"));
        }
    }
    for st in &blk.stmts {
        match st {
            St::Let(name, ann, init, l, c) => {
                let ann_kind = match ann {
                    None => None,
                    Some(Ty::Prim(p)) => match prim_kind(p) {
                        Some(k) => Some(k),
                        None => {
                            return Err(SubsetError::new(
                                *l,
                                *c,
                                "let 类型标注（仅原始类型、[byte; N] 与可打印复合可 codegen）",
                            ))
                        }
                    },
                    Some(Ty::Arr(elem, _)) if matches!(elem.as_ref(), Ty::Prim("byte")) => {
                        Some(BindingKind::Str)
                    }
                    Some(Ty::BoxTy(inner)) => match l2f_payload_ty(inner) {
                        // L2k：浮点载荷标注仅在特殊值／有限浮点字面量成立；
                        // L3a：浮点标量变量（Path）同样成立；其余沿用原判词。
                        Some(p)
                            if matches!(p, Ty::Prim("f32") | Ty::Prim("f64"))
                                && !float_payload_init(init)
                                && !matches!(
                                    init,
                                    Ex::Box(b, _, _)
                                        if matches!(
                                            b.as_ref(),
                                            Ex::Path(n, _, _)
                                                if matches!(
                                                    bindings.get(n),
                                                    Some(BindingKind::F32 | BindingKind::F64)
                                                )
                                        )
                                ) =>
                        {
                            return Err(SubsetError::new(
                                *l,
                                *c,
                                "let 类型标注（Box 仅 8 个整型宽度可 codegen）",
                            ))
                        }
                        Some(p) => Some(BindingKind::Box(p)),
                        None if is_composite_payload_ty(inner) && l2k_payload_ty(inner, prog) => {
                            Some(BindingKind::Box((**inner).clone()))
                        }
                        None => {
                            return Err(SubsetError::new(
                                *l,
                                *c,
                                "let 类型标注（Box 仅 8 个整型宽度可 codegen）",
                            ))
                        }
                    },
                    Some(t @ (Ty::Arr(_, _) | Ty::Tuple(_) | Ty::Named(_)))
                        if printable_ty(t, prog) =>
                    {
                        Some(BindingKind::Composite(t.clone()))
                    }
                    Some(_) => {
                        return Err(SubsetError::new(
                            *l,
                            *c,
                            "let 类型标注（仅原始类型、[byte; N] 与可打印复合可 codegen）",
                        ))
                    }
                };
                let kind =
                    let_init_kind(init, &ann_kind, bindings, &live_composites, types, prog)?;
                let kind = unify_ann(ann_kind, kind, init, *l, *c)?;
                if let BindingKind::Composite(t) = &kind {
                    let live = is_composite_payload_ty(t)
                        && !is_enum_payload_ty(t, prog)
                        && l2k_payload_ty(t, prog)
                        && materializable_node(
                            init,
                            t,
                            bindings,
                            &live_composites,
                            &scalar_consts,
                            types,
                            prog,
                        );
                    if live {
                        live_composites.insert(name.clone());
                    } else {
                        live_composites.remove(name);
                    }
                    scalar_consts.remove(name);
                } else {
                    live_composites.remove(name);
                    if scalar_const_binding(init, &kind, &scalar_consts) {
                        scalar_consts.insert(name.clone());
                    } else {
                        scalar_consts.remove(name);
                    }
                }
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
                    if !is_printable_arg(a, bindings, types, prog) {
                        let (al, ac) = expr_span(a);
                        return Err(SubsetError::new(
                            al,
                            ac,
                            "println 实参不可打印（引用／Box／含不可打印成员的复合或未实现形态）",
                        ));
                    }
                }
            }
            St::Expr(Some(Ex::Unbox(n, ul, uc)), _, _) => {
                if !matches!(bindings.get(n), Some(BindingKind::Box(_))) {
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
                        matches!(
                            (bindings.get(a), bindings.get(b)),
                            (Some(BindingKind::Box(ta)), Some(BindingKind::Box(tb))) if ta == tb
                        )
                    }
                    (Ex::Path(a, _, _), Ex::Box(inner, _, _)) => {
                        let same_payload = match (bindings.get(a), types.expr_ty(rhs)) {
                            (Some(BindingKind::Box(t)), Some(Ty::BoxTy(p))) => t == p.as_ref(),
                            _ => false,
                        };
                        same_payload
                            && box_reinit_ok(
                                inner,
                                types.expr_ty(rhs),
                                bindings,
                                &live_composites,
                                types,
                                prog,
                            )
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

/// `let` 初值形态判定；类型仍以 `TypeTable` 为唯一来源。
fn let_init_kind(
    init: &Ex,
    ann: &Option<BindingKind>,
    bindings: &HashMap<String, BindingKind>,
    live_composites: &HashSet<String>,
    types: &TypeTable,
    prog: &Prog,
) -> Result<BindingKind, SubsetError> {
    let (l, c) = expr_span(init);
    match init {
        Ex::Lit(Lit::Str(_), _, _) => {
            if ann.as_ref().is_some_and(|a| a != &BindingKind::Str) {
                return Err(SubsetError::new(l, c, "let 类型标注与字符串初值不符"));
            }
            Ok(BindingKind::Str)
        }
        Ex::Box(inner, il, ic) => {
            let ann_payload = match ann {
                Some(BindingKind::Box(p)) => Some(p.clone()),
                _ => None,
            };
            // 整型字面量载荷（既有通道，判词不变）。
            if matches!(inner.as_ref(), Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _)) {
                let payload = match ann {
                    None => Ty::Prim("i64"),
                    Some(BindingKind::Box(p)) => p.clone(),
                    Some(_) => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                };
                // 与 check 的 C1 期望下推结果对齐：字面量载荷宽度由标注决定。
                match types.expr_ty(init) {
                    Some(Ty::BoxTy(t)) if **t == payload => {}
                    _ => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                }
                return Ok(BindingKind::Box(payload));
            }
            // L2k：有限浮点字面量载荷（宽度由标注收窄；无注解仍 fail-closed）。
            if let Some(p) = &ann_payload {
                if matches!(p, Ty::Prim("f32") | Ty::Prim("f64")) && float_payload_init(init) {
                    match types.expr_ty(init) {
                        Some(Ty::BoxTy(t)) if **t == *p => {}
                        _ => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                    }
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            // L2j：特殊值载荷（宽度随源形态；跨宽已由 check 报 E1201）。
            if let Ex::FloatSpecial(..) = inner.as_ref() {
                let payload = match ann {
                    // 无注解特殊值维持 fail-closed（N-121／N-122 既有负控）。
                    None => {
                        return Err(SubsetError::new(*il, *ic, "box 载荷非整型字面量"));
                    }
                    Some(BindingKind::Box(p)) => p.clone(),
                    Some(_) => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                };
                match types.expr_ty(init) {
                    Some(Ty::BoxTy(t)) if **t == payload => {}
                    _ => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                }
                return Ok(BindingKind::Box(payload));
            }
            // L2v：变量枚举载荷（`box e`；Copy 语义）——仅标注 Box<enum> 且字段全标量。
            if let (Some(p), Ex::Path(..)) = (&ann_payload, inner.as_ref()) {
                if is_enum_payload_ty(p, prog)
                    && matches!(types.expr_ty(init), Some(Ty::BoxTy(t)) if **t == *p)
                {
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            // L3a：顶层标量变量载荷（Copy；bool／整棵载荷／表达式仍 fail-closed）。
            if let (Some(p), Ex::Path(n, _, _)) = (&ann_payload, inner.as_ref()) {
                if l2f_payload_ty(p).is_some()
                    && scalar_binding_matches(p, n, bindings)
                    && matches!(types.expr_ty(init), Some(Ty::BoxTy(t)) if **t == *p)
                {
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            // L3c：顶层整棵可物化复合变量（任意叶来源；struct／array）。
            if let (Some(p), Ex::Path(n, _, _)) = (&ann_payload, inner.as_ref()) {
                if whole_live_composite_path(n, p, bindings, live_composites, prog)
                    && matches!(types.expr_ty(init), Some(Ty::BoxTy(t)) if **t == *p)
                {
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            // L3d：顶层标量表达式载荷（仅运行期 i64／u8／u32）。
            if let Some(p) = &ann_payload {
                if matches!(p, Ty::Prim("i64" | "u8" | "u32"))
                    && scalar_expr_ok(inner, p, bindings, types, prog)
                    && matches!(types.expr_ty(init), Some(Ty::BoxTy(t)) if **t == *p)
                {
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            // L2k：复合字面量载荷（常量树；变量即 fail-closed）。
            // L2t：枚举常量载荷（`box V(..)`／`box E::V(..)`）同走复合通道。
            // L2x：一层变量 enum 叶（struct 直接字段／array 直接元素）走 live 闸。
            if let Some(p) = &ann_payload {
                if is_composite_payload_ty(p) && l2k_payload_ty(p, prog) {
                    let literal = matches!(
                        inner.as_ref(),
                        Ex::Record(..) | Ex::ArrLit(..) | Ex::Call(..)
                    );
                    let ok = literal
                        && (payload_init_ok(inner, types, prog)
                            || live_composite_ok(inner, p, bindings, live_composites, types, prog));
                    if !ok {
                        return Err(SubsetError::new(*il, *ic, "box 载荷须为字面量常量"));
                    }
                    match types.expr_ty(init) {
                        Some(Ty::BoxTy(t)) if **t == *p => {}
                        _ => return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符")),
                    }
                    return Ok(BindingKind::Box(p.clone()));
                }
            }
            Err(SubsetError::new(*il, *ic, "box 载荷非整型字面量"))
        }
        Ex::Lit(Lit::Int(_), _, _) => match ann {
            None => Ok(BindingKind::I64),
            Some(k) if is_int_kind(k) || k == &BindingKind::Byte => Ok(k.clone()),
            Some(_) => Err(SubsetError::new(l, c, "let 类型标注与整型初值不符")),
        },
        // L2q：具名整型常量与整数字面量同口径绑定。
        Ex::Lit(Lit::IntConst(_), _, _) => match ann {
            None => Ok(BindingKind::I64),
            Some(k) if is_int_kind(k) || k == &BindingKind::Byte => Ok(k.clone()),
            Some(_) => Err(SubsetError::new(l, c, "let 类型标注与整型初值不符")),
        },
        Ex::Lit(Lit::Bool(_), _, _) => match ann {
            None | Some(BindingKind::Bool) => Ok(BindingKind::Bool),
            _ => Err(SubsetError::new(l, c, "let 类型标注与 bool 初值不符")),
        },
        Ex::UnitLit(_, _) => match ann {
            None | Some(BindingKind::Unit) => Ok(BindingKind::Unit),
            _ => Err(SubsetError::new(l, c, "let 类型标注与 unit 初值不符")),
        },
        Ex::Lit(Lit::Float(_), _, _) => match ann {
            None => Ok(BindingKind::F64),
            Some(BindingKind::F32) => Ok(BindingKind::F32),
            Some(BindingKind::F64) => Ok(BindingKind::F64),
            _ => Err(SubsetError::new(l, c, "let 类型标注与浮点初值不符")),
        },
        // L2h：特殊值绑定宽度随源形态；跨宽／整型标注已由 check 报 E1201／E1202。
        Ex::FloatSpecial(w, _, _, _) => {
            let k = match w {
                crate::parser::FloatWidth::F32 => BindingKind::F32,
                crate::parser::FloatWidth::F64 => BindingKind::F64,
            };
            match ann {
                None => Ok(k),
                Some(a) if a == &k => Ok(k),
                Some(_) => Err(SubsetError::new(l, c, "let 类型标注与特殊值初值不符")),
            }
        }
        Ex::Deref(base, il, ic) => match base.as_ref() {
            Ex::Path(n, _, _) => match bindings.get(n) {
                Some(BindingKind::Box(p)) => match l2f_payload_binding(p) {
                    Some(k) => match ann {
                        None => Ok(k),
                        Some(a) if a == &k => Ok(k),
                        Some(_) => Err(SubsetError::new(l, c, "let 类型标注与解引用初值不符")),
                    },
                    None => Err(SubsetError::new(*il, *ic, "解引用源不是 Box 绑定")),
                },
                _ => Err(SubsetError::new(*il, *ic, "解引用源不是 Box 绑定")),
            },
            _ => Err(SubsetError::new(*il, *ic, "解引用源不是 Box 绑定")),
        },
        Ex::Path(n, il, ic) => match bindings.get(n) {
            Some(BindingKind::Box(p)) => {
                if ann.is_some() {
                    return Err(SubsetError::new(l, c, "let 类型标注与 Box 初值不符"));
                }
                Ok(BindingKind::Box(p.clone()))
            }
            Some(BindingKind::Composite(t)) => {
                let got = types
                    .expr_ty(init)
                    .ok_or_else(|| SubsetError::new(*il, *ic, "let 复制源缺少 type table 记录"))?;
                if got != t {
                    return Err(SubsetError::new(*il, *ic, "let 复制源类型与绑定不符"));
                }
                let kind = BindingKind::Composite(t.clone());
                if ann.as_ref().is_some_and(|a| a != &kind) {
                    return Err(SubsetError::new(l, c, "let 类型标注与复制源类型不符"));
                }
                Ok(kind)
            }
            Some(k) if is_copy_kind(k) => match ann {
                None => Ok(k.clone()),
                Some(a) if a == k => Ok(a.clone()),
                Some(_) => Err(SubsetError::new(*il, *ic, "let 类型标注与复制源类型不符")),
            },
            Some(BindingKind::Str) => {
                // 字节串复制不在 L2d codegen 面；保持 fail-closed。
                Err(SubsetError::new(*il, *ic, "let 初始化非可复制绑定"))
            }
            _ => Err(SubsetError::new(*il, *ic, "let 初始化非可复制绑定")),
        },
        Ex::Bin(..) => match let_int_expr_kind(init, bindings, types) {
            Some(k) => unify_ann(ann.clone(), k, init, l, c),
            None => Err(SubsetError::new(l, c, "let 初始化非整型算术表达式")),
        },
        Ex::Un(..) => {
            if let Some(k) = let_int_expr_kind(init, bindings, types) {
                unify_ann(ann.clone(), k, init, l, c)
            } else if let Some(k) = const_unary_kind(init, bindings) {
                unify_ann(ann.clone(), k, init, l, c)
            } else {
                Err(SubsetError::new(l, c, "let 初始化非可 codegen 形态"))
            }
        }
        // L3h：裸字段读取绑定——基座限本地 struct Path 绑定、单层字段；
        // 字段类型仅运行期宽度 i64／u8／u32；其余形态维持既有判词。
        Ex::Field(base, _, _, _) => {
            let Ex::Path(n, _, _) = base.as_ref() else {
                return Err(SubsetError::new(l, c, "let 初始化非可 codegen 形态"));
            };
            if !matches!(bindings.get(n), Some(BindingKind::Composite(Ty::Named(_)))) {
                return Err(SubsetError::new(l, c, "let 初始化非可 codegen 形态"));
            }
            let kind = match types.expr_ty(init) {
                Some(Ty::Prim("i64")) => BindingKind::I64,
                Some(Ty::Prim("u8")) => BindingKind::U8,
                Some(Ty::Prim("u32")) => BindingKind::U32,
                _ => return Err(SubsetError::new(l, c, "let 初始化非可 codegen 形态")),
            };
            unify_ann(ann.clone(), kind, init, l, c)
        }
        Ex::Record(..) | Ex::Tup(..) | Ex::ArrLit(..) | Ex::Call(..) => {
            let t = types
                .expr_ty(init)
                .ok_or_else(|| SubsetError::new(l, c, "let 复合初值缺少 type table 记录"))?;
            // L2n：嵌套复合（及根位置）的特殊值枚举直接形式与绑定形式同源放行。
            if !printable_ty(t, prog) || !supported_value_expr(init, bindings, types, prog) {
                return Err(SubsetError::new(l, c, "let 初始化复合值不可打印或形态不支持"));
            }
            let kind = if is_byte_array_ty(t) {
                BindingKind::Str
            } else {
                BindingKind::Composite(t.clone())
            };
            match ann {
                None => Ok(kind),
                Some(a) if a == &kind => Ok(a.clone()),
                Some(_) => Err(SubsetError::new(l, c, "let 类型标注与复合初值不符")),
            }
        }
        _ => Err(SubsetError::new(l, c, "let 初始化非可 codegen 形态")),
    }
}

fn is_byte_array_ty(t: &Ty) -> bool {
    matches!(t, Ty::Arr(elem, _) if matches!(elem.as_ref(), Ty::Prim("byte")))
}

/// L2f／L2g Box 载荷类型：8 个整型宽度（L2g 追加 i8／i16／i32／u16／u64）。
fn l2f_payload_ty(t: &Ty) -> Option<Ty> {
    match t {
        Ty::Prim(p)
            if matches!(
                *p,
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64"
            ) =>
        {
            Some(t.clone())
        }
        _ => None,
    }
}

/// 载荷类型到绑定种类（八种均为在册原始类型）。
fn l2f_payload_binding(t: &Ty) -> Option<BindingKind> {
    match t {
        Ty::Prim("i8") => Some(BindingKind::I8),
        Ty::Prim("i16") => Some(BindingKind::I16),
        Ty::Prim("i32") => Some(BindingKind::I32),
        Ty::Prim("i64") => Some(BindingKind::I64),
        Ty::Prim("u8") => Some(BindingKind::U8),
        Ty::Prim("u16") => Some(BindingKind::U16),
        Ty::Prim("u32") => Some(BindingKind::U32),
        Ty::Prim("u64") => Some(BindingKind::U64),
        _ => None,
    }
}

/// L3a：顶层标量 Box 变量载荷——标注宽度与绑定种类一致（bool／byte 不在白名单）。
fn scalar_binding_matches(p: &Ty, n: &str, bindings: &HashMap<String, BindingKind>) -> bool {
    let Ty::Prim(pname) = p else {
        return false;
    };
    l2f_payload_ty(p).is_some()
        && prim_kind(pname).is_some_and(|k| bindings.get(n) == Some(&k))
}

/// L3c：可物化叶/节点——常量叶、运行期标量叶、enum 值叶、物化复合 Path 叶、
/// 嵌套字面量；绑定内表达式叶仅限 i64／u8／u32 与白名单 enum 构造叶。
fn materializable_node(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    live: &HashSet<String>,
    scalar_consts: &HashSet<String>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    if payload_init_ok(e, types, prog) {
        return true;
    }
    match (e, want) {
        (Ex::Record(_, fields, _, _), Ty::Named(name)) => {
            let Some(decl) = prog.structs.iter().find(|s| &s.name == name) else {
                return false;
            };
            decl.fields.iter().all(|(fname, fty, _, _)| {
                fields
                    .iter()
                    .find(|(n, _)| n == fname)
                    .is_some_and(|(_, v)| {
                        materializable_node(v, fty, bindings, live, scalar_consts, types, prog)
                    })
            })
        }
        (Ex::ArrLit(es, _, _), Ty::Arr(elem, n)) => {
            es.len() as u64 == *n
                && es.iter().all(|x| {
                    materializable_node(x, elem, bindings, live, scalar_consts, types, prog)
                })
        }
        (Ex::Path(n, _, _), _) => {
            let Some(k) = bindings.get(n) else {
                return false;
            };
            if is_scalar_payload_ty(want) {
                // 运行期标量任意来源；编译期标量必须是编译期绑定。
                if matches!(want, Ty::Prim("i64") | Ty::Prim("u8") | Ty::Prim("u32")) {
                    return scalar_kind_matches(want, n, bindings);
                }
                return scalar_kind_matches(want, n, bindings) && scalar_consts.contains(n);
            }
            if is_enum_payload_ty(want, prog) {
                return matches!(k, BindingKind::Composite(t) if t == want);
            }
            if is_composite_payload_ty(want) {
                return matches!(k, BindingKind::Composite(t) if t == want) && live.contains(n);
            }
            false
        }
        // 绑定内 enum 构造叶（实参已求值；字段白名单由 l2k 保证）。
        (Ex::Call(..), _) if is_enum_payload_ty(want, prog) => l2k_payload_ty(want, prog),
        // 绑定内运行期标量表达式（算术／字段读取／*b）。
        (_, _) if matches!(want, Ty::Prim("i64") | Ty::Prim("u8") | Ty::Prim("u32")) => true,
        _ => false,
    }
}

/// L3c：物化标量叶的绑定种类口径（含 bool 等 L2t 白名单；区别于顶层 Box 标量白名单）。
fn scalar_kind_matches(want: &Ty, n: &str, bindings: &HashMap<String, BindingKind>) -> bool {
    let Ty::Prim(pname) = want else {
        return false;
    };
    is_scalar_payload_ty(want)
        && prim_kind(pname).is_some_and(|k| bindings.get(n) == Some(&k))
}

/// L3d：顶层标量表达式载荷——仅 i64／u8／u32；单层 Field／Deref；
/// Lit／同型 Path／Un("-")（仅 i64）／Bin（+ - * / %）递归。
fn scalar_expr_ok(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    if types.expr_ty(e) != Some(want) {
        return false;
    }
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) => true,
        Ex::Path(n, _, _) => scalar_binding_matches(want, n, bindings),
        Ex::Un(op, x, _, _) if op == "-" => {
            matches!(want, Ty::Prim("i64")) && scalar_expr_ok(x, want, bindings, types, prog)
        }
        Ex::Bin(op, a, b, _, _) => {
            matches!(op.as_str(), "+" | "-" | "*" | "/" | "%")
                && scalar_expr_ok(a, want, bindings, types, prog)
                && scalar_expr_ok(b, want, bindings, types, prog)
        }
        Ex::Field(base, fname, _, _) => {
            let Ex::Path(n, _, _) = base.as_ref() else {
                return false;
            };
            let Some(BindingKind::Composite(Ty::Named(sname))) = bindings.get(n) else {
                return false;
            };
            let Some(decl) = prog.structs.iter().find(|s| &s.name == sname) else {
                return false;
            };
            decl.fields
                .iter()
                .any(|(f, fty, _, _)| f == fname && fty == want)
        }
        Ex::Deref(base, _, _) => {
            let Ex::Path(n, _, _) = base.as_ref() else {
                return false;
            };
            matches!(bindings.get(n), Some(BindingKind::Box(t)) if t == want)
        }
        _ => false,
    }
}

/// L3c：编译期标量绑定判定（与 lower 的 slot.konst 同口径）。
fn scalar_const_binding(init: &Ex, kind: &BindingKind, scalar_consts: &HashSet<String>) -> bool {
    match kind {
        BindingKind::I8
        | BindingKind::I16
        | BindingKind::I32
        | BindingKind::U16
        | BindingKind::U64
        | BindingKind::Bool
        | BindingKind::F32
        | BindingKind::F64 => match init {
            Ex::Deref(..) => false,
            Ex::Path(n, _, _) => scalar_consts.contains(n),
            _ => true,
        },
        _ => false,
    }
}

/// L3c：顶层整棵可物化复合 Path 闸（struct／array；非 enum）。
fn whole_live_composite_path(
    n: &str,
    p: &Ty,
    bindings: &HashMap<String, BindingKind>,
    live_composites: &HashSet<String>,
    prog: &Prog,
) -> bool {
    is_composite_payload_ty(p)
        && !is_enum_payload_ty(p, prog)
        && l2k_payload_ty(p, prog)
        && live_composites.contains(n)
        && matches!(bindings.get(n), Some(BindingKind::Composite(t)) if t == p)
}

fn is_box_i64(k: &BindingKind) -> bool {
    matches!(k, BindingKind::Box(t) if t == &Ty::Prim("i64"))
}

/// L2k：`box <float literal>`／`box -<float literal>`／`box <特殊值>` 形态。
fn float_payload_init(init: &Ex) -> bool {
    match init {
        Ex::Box(b, _, _) => match b.as_ref() {
            Ex::FloatSpecial(..) | Ex::Lit(Lit::Float(_), _, _) => true,
            Ex::Un(op, x, _, _) if op == "-" => matches!(
                x.as_ref(),
                Ex::Lit(Lit::Float(_), _, _) | Ex::FloatSpecial(..)
            ),
            _ => false,
        },
        _ => false,
    }
}

/// L2k：Box 重初始化形态（`b = box <payload>`）——整型字面量／特殊值／
/// 有限浮点字面量／复合常量字面量；变量与负数整型仍 fail-closed。
fn box_reinit_ok(
    inner: &Ex,
    payload: Option<&Ty>,
    bindings: &HashMap<String, BindingKind>,
    live_composites: &HashSet<String>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    let Some(Ty::BoxTy(p)) = payload else {
        return false;
    };
    match inner {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) | Ex::FloatSpecial(..) => true,
        Ex::Lit(Lit::Float(_), _, _) => matches!(p.as_ref(), Ty::Prim("f32") | Ty::Prim("f64")),
        Ex::Un(op, x, _, _) if op == "-" => {
            (matches!(x.as_ref(), Ex::Lit(Lit::Float(_), _, _))
                && matches!(p.as_ref(), Ty::Prim("f32") | Ty::Prim("f64")))
                || (matches!(p.as_ref(), Ty::Prim("i64" | "u8" | "u32"))
                    && scalar_expr_ok(inner, p, bindings, types, prog))
        }
        // L2v：变量枚举重初始化（Copy；类型表二次核对）。
        // L3a：顶层标量变量重初始化（Copy；bool／整棵载荷仍 false）。
        // L3c：顶层整棵可物化复合变量重初始化（任意叶来源）。
        Ex::Path(n, _, _) => {
            (is_enum_payload_ty(p, prog) && types.expr_ty(inner) == Some(p.as_ref()))
                || (l2f_payload_ty(p).is_some()
                    && scalar_binding_matches(p, n, bindings)
                    && types.expr_ty(inner) == Some(p.as_ref()))
                || (whole_live_composite_path(n, p, bindings, live_composites, prog)
                    && types.expr_ty(inner) == Some(p.as_ref()))
        }
        Ex::Record(..) | Ex::ArrLit(..) | Ex::Call(..) => {
            is_composite_payload_ty(p)
                && l2k_payload_ty(p, prog)
                && (payload_init_ok(inner, types, prog)
                    || live_composite_ok(inner, p, bindings, live_composites, types, prog))
        }
        // L3d：标量表达式重初始化（仅运行期 i64／u8／u32）。
        _ => {
            matches!(p.as_ref(), Ty::Prim("i64" | "u8" | "u32"))
                && scalar_expr_ok(inner, p, bindings, types, prog)
        }
    }
}

/// L2k：复合载荷类型（struct／定长数组）——标量顶层面不扩。
fn is_composite_payload_ty(t: &Ty) -> bool {
    matches!(t, Ty::Named(_) | Ty::Arr(..))
}

/// L2v：枚举载荷类型（Box<enum> 变量通道）——命名枚举且字段全标量。
fn is_enum_payload_ty(t: &Ty, prog: &Prog) -> bool {
    matches!(t, Ty::Named(n) if prog.enums.iter().any(|e| &e.name == n))
        && l2k_payload_ty(t, prog)
}

/// L3f：enum 构造表达式叶——变体载荷仅标量；运行期 i64／u8／u32 实参
/// 复用 L3d `scalar_expr_ok`，其余宽度仍仅常量实参（既有 `is_const_scalar_arg`）。
fn enum_ctor_expr_ok(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    if types.expr_ty(e) != Some(want) {
        return false;
    }
    let (Ex::Call(callee, args, _, _), Ty::Named(en)) = (e, want) else {
        return false;
    };
    let Some(decl) = prog.enums.iter().find(|x| &x.name == en) else {
        return false;
    };
    let Ex::Path(v, _, _) = callee.as_ref() else {
        return false;
    };
    let var = match v.rsplit_once("::") {
        Some((q, r)) if q == en.as_str() => r,
        Some(_) => return false,
        None => v.as_str(),
    };
    let Some((_, payload, _, _)) = decl.variants.iter().find(|(n, _, _, _)| n.as_str() == var) else {
        return false;
    };
    payload.len() == args.len()
        && payload.iter().zip(args.iter()).all(|((fty, _, _), arg)| {
            if !is_scalar_payload_ty(fty) {
                return false;
            }
            match fty {
                Ty::Prim("i64" | "u8" | "u32") => {
                    scalar_expr_ok(arg, fty, bindings, types, prog)
                }
                _ => is_const_scalar_arg(arg),
            }
        })
}

/// L2y：任意既有复合深度的 live 形态——常量子树、同型 enum 变量叶、
/// L2z 标量变量叶、L3c 可物化复合 Path 叶、L3e 运行期标量表达式叶，
/// L3f 起 enum 构造表达式叶（仅标量 payload；非标量／窄宽度表达式仍 false）。
fn live_composite_ok(
    e: &Ex,
    payload: &Ty,
    bindings: &HashMap<String, BindingKind>,
    live: &HashSet<String>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    if payload_init_ok(e, types, prog) {
        return true;
    }
    // L3e：复合字面量标量叶直接表达式（仅运行期 i64／u8／u32）。
    if matches!(payload, Ty::Prim("i64" | "u8" | "u32"))
        && scalar_expr_ok(e, payload, bindings, types, prog)
    {
        return true;
    }
    // L3f：enum 构造表达式叶（⑤f-2b）。
    if let Ex::Call(..) = e {
        if enum_ctor_expr_ok(e, payload, bindings, types, prog) {
            return true;
        }
    }
    if live_enum_leaf_ok(e, payload, bindings, types, prog) {
        return true;
    }
    if live_scalar_ok(e, payload, bindings, types) {
        return true;
    }
    if live_composite_leaf_ok(e, payload, bindings, live, types, prog) {
        return true;
    }
    match (e, payload) {
        (Ex::Record(_, fields, _, _), Ty::Named(name)) => {
            let Some(decl) = prog.structs.iter().find(|s| &s.name == name) else {
                return false;
            };
            decl.fields.iter().all(|(fname, fty, _, _)| {
                let Some((_, v)) = fields.iter().find(|(n, _)| n == fname) else {
                    return false;
                };
                live_composite_ok(v, fty, bindings, live, types, prog)
            })
        }
        (Ex::ArrLit(es, _, _), Ty::Arr(elem, n)) => {
            es.len() as u64 == *n
                && es.iter().all(|x| {
                    live_composite_ok(x, elem, bindings, live, types, prog)
                })
        }
        _ => false,
    }
}

/// L3c：直接字面量中的整棵复合 Path 叶（可物化绑定；非 enum）。
fn live_composite_leaf_ok(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    live: &HashSet<String>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    let Ex::Path(n, _, _) = e else {
        return false;
    };
    types.expr_ty(e) == Some(want)
        && is_composite_payload_ty(want)
        && !is_enum_payload_ty(want, prog)
        && l2k_payload_ty(want, prog)
        && live.contains(n)
        && matches!(bindings.get(n), Some(BindingKind::Composite(t)) if t == want)
}

/// L2x：直接 enum 变量叶——Path 绑定为同型枚举值（Copy），enum 字段全标量。
fn live_enum_leaf_ok(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    if !is_enum_payload_ty(want, prog) {
        return false;
    }
    let Ex::Path(n, _, _) = e else {
        return false;
    };
    if types.expr_ty(e) != Some(want) {
        return false;
    }
    matches!(bindings.get(n), Some(BindingKind::Composite(t)) if t == want)
}

/// L2z：复合树内标量变量叶——Path 绑定与字段类型同为 L2t 标量白名单类型。
/// 顶层标量 Box／整棵变量载荷不会到达此处（调用方仅递归 struct／array 子节点）。
fn live_scalar_ok(
    e: &Ex,
    want: &Ty,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
) -> bool {
    if !is_scalar_payload_ty(want) {
        return false;
    }
    let Ex::Path(n, _, _) = e else {
        return false;
    };
    if types.expr_ty(e) != Some(want) {
        return false;
    }
    let Ty::Prim(p) = want else {
        return false;
    };
    match (bindings.get(n), prim_kind(p)) {
        (Some(k), Some(want_kind)) => *k == want_kind,
        _ => false,
    }
}

/// L2k Box 复合载荷白名单（深递归）：8 整型宽度／bool／f32／f64／struct／定长数组；
/// L2t/L2w：enum 允许出现在任意深度（变体字段仍须全标量）。
/// 排除 `[byte; N]`（含嵌套）、enum 变体字段的复合／嵌套 enum／unit、tuple、Box、引用。
fn l2k_payload_ty(t: &Ty, prog: &Prog) -> bool {
    let mut seen = HashSet::new();
    l2k_payload_ty_in(t, prog, &mut seen)
}

fn l2k_payload_ty_in(t: &Ty, prog: &Prog, seen: &mut HashSet<String>) -> bool {
    match t {
        Ty::Prim(p) => matches!(
            *p,
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool"
        ),
        Ty::Arr(elem, _) => {
            !matches!(elem.as_ref(), Ty::Prim("byte"))
                && l2k_payload_ty_in(elem, prog, seen)
        }
        Ty::Named(n) => {
            if !seen.insert(n.clone()) {
                return false;
            }
            // L2w：enum 在任意深度允许（顶层与嵌套同规）；变体字段仍须全标量。
            if let Some(e) = prog.enums.iter().find(|e| &e.name == n) {
                if e.variants.is_empty() {
                    return false;
                }
                return e.variants.iter().all(|(_, payload, _, _)| {
                    payload.iter().all(|(ft, _, _)| is_scalar_payload_ty(ft))
                });
            }
            match prog.structs.iter().find(|s| &s.name == n) {
                Some(s) => s
                    .fields
                    .iter()
                    .all(|(_, ft, _, _)| l2k_payload_ty_in(ft, prog, seen)),
                None => false,
            }
        }
        _ => false,
    }
}

/// L2t：枚举变体字段的标量白名单（8 整型宽度／bool／f32／f64）。
fn is_scalar_payload_ty(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Prim(p)
            if matches!(
                *p,
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "f32" | "f64" | "bool"
            )
    )
}

/// L2t：枚举常量实参形态（字面量／具名常量／负字面量）；变量与表达式拒绝。
fn is_const_scalar_arg(e: &Ex) -> bool {
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_) | Lit::Bool(_), _, _) => true,
        Ex::FloatSpecial(..) => true,
        Ex::Un(op, x, _, _) if op == "-" => matches!(
            x.as_ref(),
            Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_), _, _)
        ),
        _ => false,
    }
}

/// L2k：载荷初值必须是常量字面量形态（变量／投影／算术一律 fail-closed）。
fn payload_init_ok(e: &Ex, types: &TypeTable, prog: &Prog) -> bool {
    let Some(t) = types.expr_ty(e) else {
        return false;
    };
    if !l2k_payload_ty(t, prog) {
        return false;
    }
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_) | Lit::Bool(_), _, _) => true,
        Ex::FloatSpecial(..) => true,
        Ex::Un(op, x, _, _) if op == "-" || op == "!" => payload_init_ok(x, types, prog),
        Ex::Record(_, fields, _, _) => fields.iter().all(|(_, v)| payload_init_ok(v, types, prog)),
        Ex::ArrLit(es, _, _) => es.iter().all(|x| payload_init_ok(x, types, prog)),
        // L2t：枚举常量载荷（变体实参须为常量标量形态；变量／表达式拒绝）。
        Ex::Call(callee, args, _, _) => {
            let Ex::Path(v, _, _) = callee.as_ref() else { return false };
            let Ty::Named(en) = t else { return false };
            let Some(e) = prog.enums.iter().find(|e| &e.name == en) else {
                return false;
            };
            let var = match v.split_once("::") {
                Some((q, r)) if q == en.as_str() => r,
                Some(_) => return false,
                None => v.as_str(),
            };
            let Some((_, payload, _, _)) = e.variants.iter().find(|(n, _, _, _)| n.as_str() == var) else {
                return false;
            };
            payload.len() == args.len() && args.iter().all(is_const_scalar_arg)
        }
        _ => false,
    }
}

/// 可打印类型：类型级递归；enum 任一变体载荷不可打印则整类型不可打印（Q-2）。
fn printable_ty(t: &Ty, prog: &Prog) -> bool {
    let mut seen = HashSet::new();
    printable_ty_inner(t, prog, &mut seen)
}

fn printable_ty_inner(t: &Ty, prog: &Prog, seen: &mut HashSet<String>) -> bool {
    match t {
        Ty::Prim(p) => prim_kind(p).is_some(),
        Ty::Arr(elem, _) => {
            matches!(elem.as_ref(), Ty::Prim("byte")) || printable_ty_inner(elem, prog, seen)
        }
        Ty::Tuple(ts) => ts.iter().all(|x| printable_ty_inner(x, prog, seen)),
        Ty::Named(n) => {
            if !seen.insert(n.clone()) {
                // 按值递归已由 check E1401 拦截；Box 断环时不再展开。
                return true;
            }
            if let Some(s) = prog.structs.iter().find(|s| &s.name == n) {
                return s
                    .fields
                    .iter()
                    .all(|(_, ft, _, _)| printable_ty_inner(ft, prog, seen));
            }
            if let Some(e) = prog.enums.iter().find(|e| &e.name == n) {
                return e.variants.iter().all(|(_, payload, _, _)| {
                    payload.iter().all(|(ft, _, _)| printable_ty_inner(ft, prog, seen))
                });
            }
            false
        }
        Ty::Ref(..) | Ty::BoxTy(_) | Ty::Bad => false,
    }
}

/// 复合／数组初值形态：递归确认叶子可由 lower 处理。
fn supported_value_expr(
    e: &Ex,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    supported_value_expr_in(e, bindings, types, prog, true)
}

/// `allow_special`：L2i 允许浮点特殊值作复合常量成员；Record／Tup／ArrLit 原样
/// 下传，枚举变体载荷实参固定传 `false`（L2n：直接特殊值只在「可打印容器脊」上放行）。
fn supported_value_expr_in(
    e: &Ex,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
    allow_special: bool,
) -> bool {
    let Some(t) = types.expr_ty(e) else {
        return false;
    };
    if !printable_ty(t, prog) {
        return false;
    }
    match e {
        Ex::FloatSpecial(..) => allow_special,
        Ex::Lit(Lit::Str(_), _, _) => is_byte_array_ty(t),
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_) | Lit::Bool(_), _, _) | Ex::UnitLit(_, _) => true,
        Ex::Path(n, _, _) => match bindings.get(n) {
            Some(BindingKind::Str) => is_byte_array_ty(t),
            Some(BindingKind::Composite(bt)) => bt == t,
            Some(k) if is_copy_kind(k) => true,
            _ => false,
        },
        Ex::Deref(base, _, _) => {
            matches!(
                base.as_ref(),
                Ex::Path(n, _, _) if bindings.get(n).is_some_and(is_box_i64)
            ) && t == &Ty::Prim("i64")
        }
        Ex::Un(..) => {
            int_expr_kind(e, bindings).is_some() || const_unary_kind(e, bindings).is_some()
        }
        Ex::Bin(..) => int_expr_kind(e, bindings).is_some(),
        Ex::Record(_, fields, _, _) => {
            let mut names = HashSet::new();
            fields.iter().all(|(n, v)| {
                names.insert(n.clone())
                    && supported_value_expr_in(v, bindings, types, prog, allow_special)
            })
        }
        Ex::Tup(es, _, _) => es
            .iter()
            .all(|x| supported_value_expr_in(x, bindings, types, prog, allow_special)),
        Ex::ArrLit(es, _, _) => {
            let elem = if let Ty::Arr(elem, _) = t { elem.as_ref() } else { return false };
            if matches!(elem, Ty::Prim("byte") | Ty::Prim("u8")) {
                es.iter().all(|x| is_const_value_expr(x))
            } else {
                es.iter()
                    .all(|x| supported_value_expr_in(x, bindings, types, prog, allow_special))
            }
        }
        Ex::Call(callee, args, _, _) => {
            let Ex::Path(v, _, _) = callee.as_ref() else { return false };
            let Ty::Named(en) = t else { return false };
            let Some(e) = prog.enums.iter().find(|e| &e.name == en) else {
                return false;
            };
            // L2r：单层限定名去限定后按变体名查；限定符须与 type table 的 enum 一致。
            let var = match v.split_once("::") {
                Some((q, r)) if q == en.as_str() => r,
                Some(_) => return false,
                None => v.as_str(),
            };
            let Some((_, payload, _, _)) = e.variants.iter().find(|(n, _, _, _)| n.as_str() == var) else {
                return false;
            };
            if payload.len() != args.len() {
                return false;
            }
            // L2n：枚举值处于「可打印容器脊」（根／struct 字段／数组元素／元组元素，
            // 任意深度）时，变体实参允许直接写特殊值；`allow_special` 经
            // Record／Tup／ArrLit 原样下传，遇变体载荷实参固定传 `false`（见下），
            // 因此「特殊值进入枚举自身复合载荷」结构上无法继承许可。
            args.iter().zip(payload.iter()).all(|(a, (field_ty, _, _))| {
                if matches!(a, Ex::FloatSpecial(..)) {
                    allow_special
                        && matches!(
                            (types.expr_ty(a), field_ty),
                            (Some(Ty::Prim("f32")), Ty::Prim("f32"))
                                | (Some(Ty::Prim("f64")), Ty::Prim("f64"))
                        )
                } else {
                    supported_value_expr_in(a, bindings, types, prog, false)
                }
            })
        }
        // check 不为纯 place root 写表项（Field 的 Path 基座）：按绑定种类判定。
        Ex::Field(base, _, _, _) => match base.as_ref() {
            Ex::Path(n, _, _) => matches!(
                bindings.get(n),
                Some(BindingKind::Composite(_) | BindingKind::Str)
            ),
            _ => supported_value_expr_in(base, bindings, types, prog, allow_special),
        },
        // Q-4：下标须为整数字面量；非常量下标 fail-closed。
        Ex::Index(base, idx, _, _) => {
            matches!(idx.as_ref(), Ex::Lit(Lit::Int(_), _, _))
                && supported_value_expr_in(base, bindings, types, prog, allow_special)
        }
        _ => false,
    }
}

/// Q-3：`[byte;N]`／`[u8;N]` 字面量元素须为编译期常量形态。
fn is_const_value_expr(e: &Ex) -> bool {
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_) | Lit::Float(_) | Lit::Bool(_), _, _) | Ex::UnitLit(_, _) => true,
        Ex::Un(op, inner, _, _) if op == "-" || op == "!" => is_const_value_expr(inner),
        _ => false,
    }
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

fn is_int_kind(k: &BindingKind) -> bool {
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

fn is_copy_kind(k: &BindingKind) -> bool {
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

fn is_int_lit(e: &Ex) -> bool {
    matches!(e, Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _))
}

/// 整型算术的形态判定：字面量向同侧整型收敛；其他在册整型放行给 lower。
fn int_expr_kind(e: &Ex, bindings: &HashMap<String, BindingKind>) -> Option<BindingKind> {
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) => Some(BindingKind::I64),
        Ex::Path(n, _, _) => bindings
            .get(n)
            .cloned()
            .filter(|k| is_int_kind(k)),
        Ex::Deref(base, _, _) => match base.as_ref() {
            Ex::Path(n, _, _) if bindings.get(n).is_some_and(is_box_i64) => {
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

/// L3g：普通 let 初始化器算术——在既有 `int_expr_kind` 之上增加
/// 单层 Field／Deref 运行期标量操作数（仅 i64／u8／u32）。
/// 仅供 `let_init_kind` 的 Bin／Un 使用；全局判定与 lower 入口不变。
fn let_int_expr_kind(
    e: &Ex,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
) -> Option<BindingKind> {
    match e {
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) | Ex::Path(..) => {
            int_expr_kind(e, bindings)
        }
        Ex::Field(base, _, _, _) => {
            let Ex::Path(n, _, _) = base.as_ref() else {
                return None;
            };
            if !matches!(bindings.get(n), Some(BindingKind::Composite(Ty::Named(_)))) {
                return None;
            }
            match types.expr_ty(e)? {
                Ty::Prim("i64") => Some(BindingKind::I64),
                Ty::Prim("u8") => Some(BindingKind::U8),
                Ty::Prim("u32") => Some(BindingKind::U32),
                _ => None,
            }
        }
        Ex::Deref(base, _, _) => {
            let Ex::Path(n, _, _) = base.as_ref() else {
                return None;
            };
            match bindings.get(n) {
                Some(BindingKind::Box(p)) => match p {
                    Ty::Prim("i64") => Some(BindingKind::I64),
                    Ty::Prim("u8") => Some(BindingKind::U8),
                    Ty::Prim("u32") => Some(BindingKind::U32),
                    _ => None,
                },
                _ => None,
            }
        }
        Ex::Un(op, inner, _, _) if op == "-" => {
            match let_int_expr_kind(inner, bindings, types)? {
                BindingKind::I64 => Some(BindingKind::I64),
                _ => None,
            }
        }
        Ex::Bin(op, a, b, _, _) if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%") => {
            let ka = let_int_expr_kind(a, bindings, types)?;
            let kb = let_int_expr_kind(b, bindings, types)?;
            if ka == kb {
                return Some(ka);
            }
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
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) => Some(BindingKind::I64),
        Ex::Lit(Lit::Float(_), _, _) => Some(BindingKind::F64),
        Ex::Lit(Lit::Bool(_), _, _) => Some(BindingKind::Bool),
        Ex::UnitLit(_, _) => Some(BindingKind::Unit),
        Ex::Path(n, _, _) => bindings
            .get(n)
            .cloned()
            .filter(|k| is_copy_kind(k)),
        _ => None,
    }
}

/// println 实参：类型级可打印＋lower 支持的形态（标量／复合／只读投影）。
fn is_printable_arg(
    e: &Ex,
    bindings: &HashMap<String, BindingKind>,
    types: &TypeTable,
    prog: &Prog,
) -> bool {
    let Some(t) = types.expr_ty(e) else {
        return false;
    };
    if !printable_ty(t, prog) {
        return false;
    }
    // L2h／L2i：特殊值可直接作 `println` 实参、`let` 初值或复合常量成员。
    // L2n：枚举值处于可打印容器脊（根／struct 字段／数组元素／元组元素）时，
    // 变体实参可直接写特殊值；载荷实参仍由 `supported_value_expr_in` 以
    // allow_special=false 截断。
    if matches!(e, Ex::FloatSpecial(..)) {
        return matches!(t, Ty::Prim("f32") | Ty::Prim("f64"));
    }
    // L2f：直接 `println(*b)` 放开三型 Box 载荷；复合／算术内嵌仍走
    // supported_value_expr 的既有 i64-only 边界（冻结件 §二）。
    if let Ex::Deref(base, _, _) = e {
        if let Ex::Path(n, _, _) = base.as_ref() {
            if let Some(BindingKind::Box(p)) = bindings.get(n) {
                return t == p;
            }
        }
    }
    supported_value_expr(e, bindings, types, prog)
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::compile_typed;

    fn err_of(src: &str) -> String {
        let c = compile_typed(src);
        assert!(c.diags.is_empty(), "front-end must accept: {:?}", c.diags);
        check(&c.prog, &c.types).expect_err("subset must reject").what
    }

    fn ok(src: &str) {
        let c = compile_typed(src);
        assert!(c.diags.is_empty(), "front-end must accept: {:?}", c.diags);
        check(&c.prog, &c.types).expect("subset must accept");
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
    fn empty_struct_let_is_supported() {
        ok("struct Mark {}\nfn main() { let m = Mark {}; println(m); }");
    }

    #[test]
    fn enum_declaration_alone_is_supported() {
        ok("enum E { A }\nfn main() {}");
    }

    #[test]
    fn extra_function_is_rejected() {
        assert!(err_of("fn helper() {}\nfn main() {}").contains("额外函数"));
    }

    #[test]
    fn missing_main_is_frontend_rejected() {
        // 入口缺失由 check 的 E1702 拦截，子集闸看不到该形态。
        let c = compile_typed("fn helper() {}");
        assert!(!c.diags.is_empty(), "front-end must reject missing main");
    }

    #[test]
    fn main_params_are_rejected() {
        assert!(err_of("fn main(x: i64) {}").contains("带参数"));
    }

    #[test]
    fn main_non_unit_return_is_frontend_rejected() {
        // 入口返回类型由 check 的 E1703 拦截，子集闸看不到该形态。
        let c = compile_typed("fn main() -> i64 { 1 }");
        assert!(!c.diags.is_empty(), "front-end must reject non-unit main");
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
        let c = compile_typed("fn main() { println(); }");
        assert!(!c.diags.is_empty(), "front-end must reject println()");
    }

    #[test]
    fn print_is_fail_closed() {
        assert!(err_of("fn main() { print(\"x\"); }").contains("print"));
        assert!(err_of("fn main() { print(); }").contains("print"));
    }

    #[test]
    fn composite_tuple_print_is_supported() {
        ok("fn main() { println((1, 2)); println((1, true, 2)); println(()); }");
    }

    #[test]
    fn composite_with_box_member_is_fail_closed() {
        assert!(
            err_of("fn main() { let b = box 1; println((b, 1)); }").contains("不可打印")
        );
    }

    #[test]
    fn composite_with_ref_member_is_fail_closed() {
        assert!(
            err_of("fn main() { let x = 1; let r = &x; println((r, 1)); }").contains("let")
        );
    }

    #[test]
    fn enum_with_box_variant_is_type_level_fail_closed() {
        assert!(err_of(
            "enum E { A(i64), B(Box<i64>) }\nfn main() { println(A(1)); }"
        )
        .contains("不可打印"));
    }

    #[test]
    fn non_constant_byte_array_element_is_fail_closed() {
        assert!(err_of(
            "fn main() { let x: u8 = 1; let a: [u8; 2] = [x, 2]; println(a); }"
        )
        .contains("不可打印"));
    }

    #[test]
    fn non_literal_index_is_fail_closed() {
        assert!(err_of(
            "fn main() { let a: [u8; 3] = [1, 2, 3]; let i = 1; println(a[i]); }"
        )
        .contains("不可打印"));
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

    // —— L2l：单载荷变体的特殊值字面量直接形式 ——

    #[test]
    fn enum_single_payload_special_literal_is_supported() {
        ok("enum E { V(f64) }\nfn main() { println(V(f64::NAN)); }");
        ok("enum E { V(f32) }\nfn main() { let e = V(f32::INFINITY); println(e); }");
    }

    // —— L2m：多字段变体的特殊值字面量直接形式 ——

    #[test]
    fn enum_multi_field_direct_special_is_supported() {
        ok("enum E { V(f64, f64) }\nfn main() { println(V(f64::NAN, 1.5)); }");
        ok("enum E { V(f64, f32) }\nfn main() { println(V(1.5, f32::INFINITY)); }");
        ok("enum E { V(f64, f64) }\nfn main() { let e = V(f64::NAN, 1.5); println(e); }");
        ok("enum E { V((f64, f64), f64) }\nfn main() { println(V((1.5, 2.5), f64::NAN)); }");
    }

    #[test]
    fn enum_nested_direct_opens_but_composite_special_stays_fail_closed() {
        ok("enum E { V(f64) }\nstruct S { e: E }\nfn main() { println(S { e: V(f64::NAN) }); }");
        ok("enum E { V(f64, f64) }\nfn main() { println((V(f64::NAN, 1.5), 1)); }");
        ok("enum E { V(f64) }\nstruct S { e: E }\nfn main() { let s = S { e: V(f64::NAN) }; println(s); }");
        assert!(err_of(
            "struct P { f: f64 }\nenum E { V(P, f64) }\nfn main() { println(V(P { f: f64::NAN }, 1.5)); }"
        )
        .contains("不可打印"));
        assert!(err_of(
            "enum E { V([f64; 2], f64) }\nfn main() { println(V([f64::NAN, 1.0], 1.5)); }"
        )
        .contains("不可打印"));
    }

    #[test]
    fn enum_payload_binding_path_is_supported() {
        ok("enum E { V(f64) }\nfn main() { let x = f64::NAN; println(V(x)); }");
        ok("enum E { V(f64) }\nfn main() { let e = V(f64::NAN); println(e); }");
    }
}
