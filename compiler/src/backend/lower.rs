//! 受检 AST → 首片 typed IR。
//!
//! subset 闸已先行；这里仍以 `Result` 返回，避免闸与降级判定漂移时 panic。
//! 类型不作为本模块的重算对象：绑定与表达式的类型一律取自 `check` 产出的
//! `TypeTable`；表项缺失或与形态不符即 fail-closed。
//!
//! L2a：本模块实现 G-E-2（move／overwrite／Box 载荷）的状态机，产出显式 IR 事件；
//! malloc／free／drop 的运行期生成归 `c::emit`，不在此处发生。
//!
//! L2c：`println` 多实参按 stdout 契约降级——i64／u8／u32 保留运行期值（printf），
//! 其余原始类型由 check 类型表 + 字面量/常量绑定在编译期格式化（不依赖 libc 浮点）。

use super::float_fmt;
use super::ir::{
    ArithOp, Func, IntExpr, IntTy, Local, LocalId, PrintArg, Program, Stmt, StrArg, Ty,
};
use super::subset::{self, SubsetError};
use crate::parser::{Ex, Lit, Prog, St, Ty as AstTy};
use crate::type_table::{self, TypeTable};
use std::collections::HashMap;

/// 编译期可确定的标量值（非运行期整型）。
#[derive(Clone, Debug, PartialEq)]
enum ConstVal {
    Int(i128),
    Bool(bool),
    Unit,
    F32(f32),
    F64(f64),
}

/// L2a 局部槽位状态：`live` 是静态可判的 owner 活跃位。
#[derive(Clone, Debug)]
struct Slot {
    ty: Ty,
    live: bool,
    /// 仅编译期标量（bool／byte／窄整型／浮点等）有值；运行期整型为 `None`。
    konst: Option<ConstVal>,
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
                St::Let(name, _ann, init, l, c) => {
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
                            slots.push(Slot { ty: Ty::Bytes(n), live: true, konst: None });
                        }
                        Ex::Box(inner, _, _) => {
                            if local_ty != Ty::Box(Box::new(Ty::I64)) {
                                return Err(SubsetError::new(*l, *c, "lower：box 绑定类型非 Box<i64>"));
                            }
                            let value = int_lit(inner)?;
                            body.push(Stmt::BindBox { local: id, value, line: *l, col: *c });
                            slots.push(Slot { ty: local_ty, live: true, konst: None });
                        }
                        Ex::Lit(Lit::Int(v), il, ic) => match &local_ty {
                            Ty::I64 => {
                                let value = i64::try_from(*v)
                                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i64"))?;
                                body.push(Stmt::BindI64 { local: id, value, line: *l, col: *c });
                                slots.push(Slot { ty: Ty::I64, live: true, konst: None });
                            }
                            Ty::U8 | Ty::U32 => {
                                let ty = ir_int_ty(&local_ty).expect("u8/u32");
                                let expr = IntExpr::Const(int_const(v, ty, init)?);
                                body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                                slots.push(Slot { ty: local_ty, live: true, konst: None });
                            }
                            _ => {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val) });
                            }
                        },
                        Ex::Lit(Lit::Bool(_) | Lit::Float(_), _, _) | Ex::UnitLit(..) => {
                            let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                            slots.push(Slot { ty: local_ty, live: true, konst: Some(val) });
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
                            slots.push(Slot { ty: Ty::I64, live: true, konst: None });
                        }
                        Ex::Path(n, nl, nc) => {
                            let src = lookup(&scope, n, *nl, *nc)?;
                            if matches!(local_ty, Ty::Box(_)) {
                                if slots.get(src.0 as usize).map(|s| &s.ty) != Some(&local_ty) {
                                    return Err(SubsetError::new(*nl, *nc, "lower：移动源类型与目标不符"));
                                }
                                body.push(Stmt::MoveAssign { dst: id, src, line: *nl, col: *nc });
                                set_live(&mut slots, src, false);
                                slots.push(Slot { ty: local_ty, live: true, konst: None });
                            } else if let Some(ty) = ir_int_ty(&local_ty) {
                                if slots.get(src.0 as usize).map(|s| &s.ty) != Some(&local_ty) {
                                    return Err(SubsetError::new(*nl, *nc, "lower：整型拷贝源类型与目标不符"));
                                }
                                body.push(Stmt::BindInt {
                                    local: id,
                                    ty,
                                    expr: IntExpr::Local(src),
                                    line: *nl,
                                    col: *nc,
                                });
                                slots.push(Slot { ty: local_ty, live: true, konst: None });
                            } else if is_const_scalar_ty(&local_ty) {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val) });
                            } else {
                                return Err(SubsetError::new(*nl, *nc, "lower：移动目标类型非 Box"));
                            }
                        }
                        Ex::Bin(..) => {
                            let ty = ir_int_ty(&local_ty).ok_or_else(|| {
                                SubsetError::new(
                                    *l,
                                    *c,
                                    "lower：算术绑定类型不在 L2b codegen 子集（仅 i64／u8／u32）",
                                )
                            })?;
                            let expr = lower_int_expr(init, ty, types, &slots, &scope)?;
                            body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                            slots.push(Slot { ty: local_ty, live: true, konst: None });
                        }
                        Ex::Un(..) => {
                            if let Some(ty) = ir_int_ty(&local_ty) {
                                let expr = lower_int_expr(init, ty, types, &slots, &scope)?;
                                body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                                slots.push(Slot { ty: local_ty, live: true, konst: None });
                            } else {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val) });
                            }
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
                    if !is_println || args.is_empty() {
                        return Err(SubsetError::new(*l, *c, "lower：println 形态"));
                    }
                    let mut pargs = Vec::with_capacity(args.len());
                    for arg in args {
                        pargs.push(print_arg(arg, types, &slots, &scope)?);
                    }
                    body.push(Stmt::Print { args: pargs, newline: true, line: *l, col: *c });
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
                St::Expr(Some(e @ (Ex::Bin(..) | Ex::Un(..))), l, c) => {
                    last_pos = (*l, *c);
                    let t = types.expr_ty(e).ok_or_else(|| {
                        SubsetError::new(*l, *c, "lower：算术语句缺少 type table 记录")
                    })?;
                    let irt = ast_to_ir(t).ok_or_else(|| {
                        SubsetError::new(
                            *l,
                            *c,
                            "lower：算术语句类型不在 L2b codegen 子集（仅 i64／u8／u32）",
                        )
                    })?;
                    let ty = ir_int_ty(&irt).ok_or_else(|| {
                        SubsetError::new(
                            *l,
                            *c,
                            "lower：算术语句类型不在 L2b codegen 子集（仅 i64／u8／u32）",
                        )
                    })?;
                    if matches!(e, Ex::Un(..)) && ty != IntTy::I64 {
                        return Err(SubsetError::new(*l, *c, "lower：无符号一元 '-' 不在 L2b codegen 子集"));
                    }
                    let expr = lower_int_expr(e, ty, types, &slots, &scope)?;
                    body.push(Stmt::EvalInt { ty, expr, line: *l, col: *c });
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

/// println 实参降级：字符串／字节串／运行期整型／编译期格式化标量。
fn print_arg(
    e: &Ex,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
) -> Result<PrintArg, SubsetError> {
    match e {
        Ex::Lit(Lit::Str(s), sl, sc) => {
            let n = expr_byte_len(types, e, *sl, *sc, "println 实参")?;
            if n != s.len() as u64 {
                return Err(SubsetError::new(
                    *sl,
                    *sc,
                    format!("lower：println 实参类型 [byte; {}] 与字面量 {} 字节不符", n, s.len()),
                ));
            }
            Ok(PrintArg::Str(StrArg::Bytes(s.as_bytes().to_vec())))
        }
        Ex::Lit(Lit::Int(_), _, _) => {
            let ty = int_ty_of(types, e, "println 实参")?;
            let expr = lower_int_expr(e, ty, types, slots, scope)?;
            Ok(PrintArg::Int { ty, expr })
        }
        Ex::Lit(Lit::Float(_) | Lit::Bool(_), _, _) | Ex::UnitLit(..) => {
            let irt = ir_ty_of(types, e, "println 实参")?;
            let val = const_scalar(e, &irt, types, slots, scope)?;
            Ok(PrintArg::Bytes(format_const(&val, &irt)?))
        }
        Ex::Path(n, nl, nc) => {
            let id = lookup(scope, n, *nl, *nc)?;
            let slot = slots
                .get(id.0 as usize)
                .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：println 实参槽位缺失"))?;
            match &slot.ty {
                Ty::Bytes(n_bytes) => {
                    // 与 check 的类型表交叉核对（唯一类型来源）
                    let table = expr_byte_len(types, e, *nl, *nc, "println 实参")?;
                    if table != *n_bytes {
                        return Err(SubsetError::new(*nl, *nc, "lower：println 实参类型与局部绑定不符"));
                    }
                    Ok(PrintArg::Str(StrArg::Local(id)))
                }
                ty => {
                    let irt = ir_ty_of(types, e, "println 实参")?;
                    if &irt != ty {
                        return Err(SubsetError::new(*nl, *nc, "lower：println 实参类型与局部绑定不符"));
                    }
                    if let Some(ity) = ir_int_ty(ty) {
                        Ok(PrintArg::Int { ty: ity, expr: IntExpr::Local(id) })
                    } else {
                        let val = slot
                            .konst
                            .clone()
                            .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：println 常量绑定缺少值"))?;
                        Ok(PrintArg::Bytes(format_const(&val, ty)?))
                    }
                }
            }
        }
        Ex::Deref(base, dl, dc) => {
            let src = match base.as_ref() {
                Ex::Path(n, _, _) => lookup(scope, n, *dl, *dc)?,
                _ => return Err(SubsetError::new(*dl, *dc, "lower：解引用源形态")),
            };
            let irt = ir_ty_of(types, e, "println 实参")?;
            if irt != Ty::I64 {
                return Err(SubsetError::new(*dl, *dc, "lower：解引用结果类型非 i64"));
            }
            Ok(PrintArg::Int { ty: IntTy::I64, expr: IntExpr::Local(src) })
        }
        Ex::Un(..) | Ex::Bin(..) => {
            let irt = ir_ty_of(types, e, "println 实参")?;
            if let Some(ty) = ir_int_ty(&irt) {
                let expr = lower_int_expr(e, ty, types, slots, scope)?;
                Ok(PrintArg::Int { ty, expr })
            } else {
                let val = const_scalar(e, &irt, types, slots, scope)?;
                Ok(PrintArg::Bytes(format_const(&val, &irt)?))
            }
        }
        other => {
            let (l, c) = subset::expr_span(other);
            Err(SubsetError::new(l, c, "lower：println 实参不可打印"))
        }
    }
}

fn int_ty_of(types: &TypeTable, e: &Ex, what: &str) -> Result<IntTy, SubsetError> {
    let irt = ir_ty_of(types, e, what)?;
    ir_int_ty(&irt).ok_or_else(|| {
        let (l, c) = subset::expr_span(e);
        SubsetError::new(l, c, format!("lower：{} 不是运行期整型", what))
    })
}

fn ir_ty_of(types: &TypeTable, e: &Ex, what: &str) -> Result<Ty, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let t = types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(l, c, format!("lower：{} 缺少 type table 记录", what)))?;
    ast_to_ir(t).ok_or_else(|| {
        SubsetError::new(l, c, format!("lower：{} 类型 '{}' 不在 L2c codegen 子集", what, t.display()))
    })
}

fn is_const_scalar_ty(t: &Ty) -> bool {
    matches!(
        t,
        Ty::I8
            | Ty::I16
            | Ty::I32
            | Ty::U16
            | Ty::U64
            | Ty::Byte
            | Ty::Bool
            | Ty::Unit
            | Ty::F32
            | Ty::F64
    )
}

/// 编译期标量求值；类型唯一来源仍为 TypeTable，节点类型与期望宽度逐点核对。
fn const_scalar(
    e: &Ex,
    want: &Ty,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
) -> Result<ConstVal, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let recorded = types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(l, c, "lower：常量初值缺少 type table 记录"))?;
    if ast_to_ir(recorded).as_ref() != Some(want) {
        return Err(SubsetError::new(
            l,
            c,
            format!("lower：常量初值类型 '{}' 与绑定不符", recorded.display()),
        ));
    }
    match e {
        Ex::Lit(Lit::Int(v), il, ic) => {
            let n = i128::try_from(*v)
                .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i128"))?;
            if !int_in_range(want, n) {
                return Err(SubsetError::new(*il, *ic, "lower：整型字面量超出声明宽度"));
            }
            Ok(ConstVal::Int(n))
        }
        Ex::Lit(Lit::Float(v), _, _) => match want {
            Ty::F32 => Ok(ConstVal::F32(*v as f32)),
            Ty::F64 => Ok(ConstVal::F64(*v)),
            _ => Err(SubsetError::new(l, c, "lower：浮点初值类型与绑定不符")),
        },
        Ex::Lit(Lit::Bool(v), _, _) => match want {
            Ty::Bool => Ok(ConstVal::Bool(*v)),
            _ => Err(SubsetError::new(l, c, "lower：bool 初值类型与绑定不符")),
        },
        Ex::UnitLit(_, _) => match want {
            Ty::Unit => Ok(ConstVal::Unit),
            _ => Err(SubsetError::new(l, c, "lower：unit 初值类型与绑定不符")),
        },
        Ex::Path(n, nl, nc) => {
            let id = lookup(scope, n, *nl, *nc)?;
            let slot = slots
                .get(id.0 as usize)
                .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：常量拷贝源槽位缺失"))?;
            if &slot.ty != want {
                return Err(SubsetError::new(*nl, *nc, "lower：常量拷贝源类型与目标不符"));
            }
            slot.konst
                .clone()
                .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：常量拷贝源不是编译期常量"))
        }
        Ex::Un(op, inner, ul, uc) if op == "-" => {
            // 负整数字面量按幅值求值：正幅值本身可超出正数范围（如 i8 的 128）。
            if let Ex::Lit(Lit::Int(v), il, ic) = inner.as_ref() {
                if !matches!(
                    want,
                    Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::Byte
                ) {
                    return Err(SubsetError::new(*ul, *uc, "lower：一元 '-' 要求数值类型"));
                }
                let n = i128::try_from(*v)
                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i128"))?;
                let n = n
                    .checked_neg()
                    .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：整型常量取负溢出"))?;
                if !int_in_range(want, n) {
                    return Err(SubsetError::new(*ul, *uc, "lower：整型字面量超出声明宽度"));
                }
                return Ok(ConstVal::Int(n));
            }
            let v = const_scalar(inner, want, types, slots, scope)?;
            match v {
                ConstVal::Int(n) => {
                    let n = n
                        .checked_neg()
                        .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：整型常量取负溢出"))?;
                    if !int_in_range(want, n) {
                        return Err(SubsetError::new(*ul, *uc, "lower：整型字面量超出声明宽度"));
                    }
                    Ok(ConstVal::Int(n))
                }
                ConstVal::F32(v) => Ok(ConstVal::F32(-v)),
                ConstVal::F64(v) => Ok(ConstVal::F64(-v)),
                _ => Err(SubsetError::new(*ul, *uc, "lower：一元 '-' 非常量数值")),
            }
        }
        Ex::Un(op, inner, ul, uc) if op == "!" => match const_scalar(inner, want, types, slots, scope)? {
            ConstVal::Bool(v) => Ok(ConstVal::Bool(!v)),
            _ => Err(SubsetError::new(*ul, *uc, "lower：'!' 非常量 bool")),
        },
        other => {
            let (el, ec) = subset::expr_span(other);
            Err(SubsetError::new(el, ec, "lower：常量形态不在 L2c codegen 子集"))
        }
    }
}

fn int_in_range(ty: &Ty, n: i128) -> bool {
    match ty {
        Ty::I8 => (-128..=127).contains(&n),
        Ty::I16 => (-32768..=32767).contains(&n),
        Ty::I32 => (i32::MIN as i128..=i32::MAX as i128).contains(&n),
        Ty::I64 => (i64::MIN as i128..=i64::MAX as i128).contains(&n),
        Ty::U8 | Ty::Byte => (0..=u8::MAX as i128).contains(&n),
        Ty::U16 => (0..=u16::MAX as i128).contains(&n),
        Ty::U32 => (0..=u32::MAX as i128).contains(&n),
        Ty::U64 => (0..=u64::MAX as i128).contains(&n),
        _ => false,
    }
}

fn format_const(v: &ConstVal, ty: &Ty) -> Result<Vec<u8>, SubsetError> {
    match (v, ty) {
        (
            ConstVal::Int(n),
            Ty::I8
            | Ty::I16
            | Ty::I32
            | Ty::I64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Byte,
        ) => Ok(n.to_string().into_bytes()),
        (ConstVal::Bool(b), Ty::Bool) => Ok(if *b { b"true".to_vec() } else { b"false".to_vec() }),
        (ConstVal::Unit, Ty::Unit) => Ok(b"()".to_vec()),
        (ConstVal::F32(v), Ty::F32) => Ok(float_fmt::format_f32(*v).into_bytes()),
        (ConstVal::F64(v), Ty::F64) => Ok(float_fmt::format_f64(*v).into_bytes()),
        _ => Err(SubsetError::new(0, 0, "lower：常量格式化类型不符")),
    }
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
        .ok_or_else(|| SubsetError::new(l, c, format!("lower：let 类型 '{}' 不在 L2c codegen 子集", t.display())))?;
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
        AstTy::Prim("i8") => Some(Ty::I8),
        AstTy::Prim("i16") => Some(Ty::I16),
        AstTy::Prim("i32") => Some(Ty::I32),
        AstTy::Prim("i64") => Some(Ty::I64),
        AstTy::Prim("u8") => Some(Ty::U8),
        AstTy::Prim("u16") => Some(Ty::U16),
        AstTy::Prim("u32") => Some(Ty::U32),
        AstTy::Prim("u64") => Some(Ty::U64),
        AstTy::Prim("byte") => Some(Ty::Byte),
        AstTy::Prim("bool") => Some(Ty::Bool),
        AstTy::Prim("unit") => Some(Ty::Unit),
        AstTy::Prim("f32") => Some(Ty::F32),
        AstTy::Prim("f64") => Some(Ty::F64),
        AstTy::BoxTy(inner) if matches!(inner.as_ref(), AstTy::Prim("i64")) => {
            Some(Ty::Box(Box::new(Ty::I64)))
        }
        _ => None,
    }
}

/// 运行期整型宽度：L2b 起仅 i64／u8／u32 走 C 变量与 printf。
fn ir_int_ty(t: &Ty) -> Option<IntTy> {
    match t {
        Ty::I64 => Some(IntTy::I64),
        Ty::U8 => Some(IntTy::U8),
        Ty::U32 => Some(IntTy::U32),
        _ => None,
    }
}

fn int_const(v: &u128, ty: IntTy, e: &Ex) -> Result<i128, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let n = i128::try_from(*v)
        .map_err(|_| SubsetError::new(l, c, "lower：整型字面量超出 i128"))?;
    let fits = match ty {
        IntTy::I64 => n <= i64::MAX as i128,
        IntTy::U8 => n <= u8::MAX as i128,
        IntTy::U32 => n <= u32::MAX as i128,
    };
    if fits {
        Ok(n)
    } else {
        Err(SubsetError::new(l, c, "lower：整型字面量超出声明宽度"))
    }
}

fn arith_op(op: &str) -> Option<ArithOp> {
    match op {
        "+" => Some(ArithOp::Add),
        "-" => Some(ArithOp::Sub),
        "*" => Some(ArithOp::Mul),
        "/" => Some(ArithOp::Div),
        "%" => Some(ArithOp::Rem),
        _ => None,
    }
}

/// 算术表达式降级：类型唯一来源仍是 TypeTable；每个节点与期望宽度交叉核对。
fn lower_int_expr(
    e: &Ex,
    ty: IntTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
) -> Result<IntExpr, SubsetError> {
    let (el, ec) = subset::expr_span(e);
    let recorded = types.expr_ty(e).ok_or_else(|| {
        SubsetError::new(el, ec, "lower：算术节点缺少 type table 记录")
    })?;
    if ast_to_ir(recorded) != Some(ty.ir_ty()) {
        return Err(SubsetError::new(
            el,
            ec,
            format!("lower：算术节点类型 '{}' 与期望 '{}' 不符", recorded.display(), ty.c_ty()),
        ));
    }
    match e {
        Ex::Lit(Lit::Int(v), _, _) => Ok(IntExpr::Const(int_const(v, ty, e)?)),
        Ex::Path(n, nl, nc) => {
            let id = lookup(scope, n, *nl, *nc)?;
            if slots.get(id.0 as usize).map(|s| &s.ty) != Some(&ty.ir_ty()) {
                return Err(SubsetError::new(*nl, *nc, "lower：算术操作数与绑定类型不符"));
            }
            Ok(IntExpr::Local(id))
        }
        Ex::Un(op, inner, l, c) if op == "-" => {
            if ty != IntTy::I64 {
                return Err(SubsetError::new(*l, *c, "lower：无符号一元 '-' 不在 L2b codegen 子集"));
            }
            // 负字面量折叠为常量，覆盖 i64::MIN（正字面量本身不可表示）。
            if let Ex::Lit(Lit::Int(v), il, ic) = inner.as_ref() {
                let n = i128::try_from(*v)
                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i128"))?;
                let n = n
                    .checked_neg()
                    .ok_or_else(|| SubsetError::new(*il, *ic, "lower：整型常量取负溢出"))?;
                if !int_in_range(&Ty::I64, n) {
                    return Err(SubsetError::new(*il, *ic, "lower：整型字面量超出声明宽度"));
                }
                return Ok(IntExpr::Const(n));
            }
            let inner = lower_int_expr(inner, ty, types, slots, scope)?;
            Ok(IntExpr::Neg { expr: Box::new(inner), line: *l, col: *c })
        }
        Ex::Bin(op, a, b, l, c) => {
            let op = arith_op(op).ok_or_else(|| {
                SubsetError::new(*l, *c, format!("lower：算术运算符 '{}' 不在 L2b 子集", op))
            })?;
            let lhs = lower_int_expr(a, ty, types, slots, scope)?;
            let rhs = lower_int_expr(b, ty, types, slots, scope)?;
            Ok(IntExpr::Bin {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                line: *l,
                col: *c,
            })
        }
        _ => Err(SubsetError::new(el, ec, "lower：算术操作数形态不在 L2b 子集")),
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
    use crate::backend::ir::{ArithOp, IntExpr, IntTy, LocalId, PrintArg, Stmt, StrArg, Ty};
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

    fn printed(src: &str) -> Vec<PrintArg> {
        body(src)
            .into_iter()
            .find_map(|s| match s {
                Stmt::Print { args, .. } => Some(args),
                _ => None,
            })
            .expect("print stmt")
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
            Stmt::Print { args, newline, line, .. } => {
                assert!(*newline);
                assert_eq!(*line, 3);
                match &args[0] {
                    PrintArg::Str(StrArg::Local(id)) => assert_eq!(id.0, 0),
                    other => panic!("expected Str(Local), got {other:?}"),
                }
            }
            other => panic!("expected Print, got {other:?}"),
        }
    }

    #[test]
    fn literal_println_lowers_to_bytes() {
        let p = ir("fn main() { println(\"hi\"); }");
        match &p.funcs[0].body[0] {
            Stmt::Print { args, .. } => match &args[0] {
                PrintArg::Str(StrArg::Bytes(b)) => assert_eq!(b.as_slice(), b"hi"),
                other => panic!("expected Str(Bytes), got {other:?}"),
            },
            other => panic!("expected Print, got {other:?}"),
        }
    }

    #[test]
    fn multi_arg_print_preserves_source_order() {
        let args = printed("fn main() { println(\"a\", 1, true, \"b\"); }");
        assert_eq!(args.len(), 4, "{args:?}");
        assert!(matches!(&args[0], PrintArg::Str(StrArg::Bytes(b)) if b == b"a"));
        assert!(matches!(&args[1], PrintArg::Int { ty: IntTy::I64, .. }));
        assert!(matches!(&args[2], PrintArg::Bytes(b) if b == b"true"));
        assert!(matches!(&args[3], PrintArg::Str(StrArg::Bytes(b)) if b == b"b"));
    }

    #[test]
    fn all_narrow_primitives_are_formatted_at_compile_time() {
        for (src, want) in [
            ("fn main() { let x: i8 = -128; println(x); }", "-128"),
            ("fn main() { let x: i16 = -32768; println(x); }", "-32768"),
            ("fn main() { let x: i32 = -2147483648; println(x); }", "-2147483648"),
            ("fn main() { let x: u16 = 65535; println(x); }", "65535"),
            ("fn main() { let x: u64 = 18446744073709551615; println(x); }", "18446744073709551615"),
            ("fn main() { let x: byte = 255; println(x); }", "255"),
            ("fn main() { let x = true; println(x); }", "true"),
            ("fn main() { let x = (); println(x); }", "()"),
        ] {
            match &printed(src)[0] {
                PrintArg::Bytes(b) => assert_eq!(b, want.as_bytes(), "{src}"),
                other => panic!("{src}: expected Bytes, got {other:?}"),
            }
        }
    }

    #[test]
    fn float_literals_follow_contract_at_compile_time() {
        for (src, want) in [
            ("fn main() { println(1.0); }", "1"),
            ("fn main() { println(1e16); }", "1e+16"),
            ("fn main() { println(1e-5); }", "1e-05"),
            ("fn main() { println(-0.0); }", "-0"),
            ("fn main() { let x: f32 = 1e7; println(x); }", "1e+07"),
            ("fn main() { let x: f32 = 1e6; println(x); }", "1000000"),
        ] {
            match &printed(src)[0] {
                PrintArg::Bytes(b) => assert_eq!(b, want.as_bytes(), "{src}"),
                other => panic!("{src}: expected Bytes, got {other:?}"),
            }
        }
    }

    #[test]
    fn runtime_int_print_keeps_runtime_value() {
        let args = printed("fn main() { let a: u8 = 250; println(a); }");
        match &args[0] {
            PrintArg::Int { ty: IntTy::U8, expr: IntExpr::Local(id) } => assert_eq!(id.0, 0),
            other => panic!("expected Int(U8), got {other:?}"),
        }
    }

    #[test]
    fn shadowing_allocates_new_local() {
        let p = ir("fn main() { let s = \"a\"; let s = \"bb\"; println(s); }");
        let f = &p.funcs[0];
        assert_eq!(f.locals.len(), 2);
        match &f.body[2] {
            Stmt::Print { args, .. } => match &args[0] {
                PrintArg::Str(StrArg::Local(id)) => assert_eq!(id.0, 1),
                other => panic!("expected Str(Local), got {other:?}"),
            },
            other => panic!("expected Print, got {other:?}"),
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
    fn box_program_lowers_to_runtime_events() {
        let b = body("fn main() { let b = box 1; }");
        assert_eq!(b.len(), 2, "{b:?}");
        assert!(matches!(b[0], Stmt::BindBox { .. }));
        assert!(matches!(b[1], Stmt::ScopeExit { .. }));
    }

    #[test]
    fn u8_annotation_and_arithmetic_lower_to_bind_int() {
        let p = ir("fn main() {\n    let a: u8 = 250;\n    let c = a + 10;\n}\n");
        assert_eq!(p.funcs[0].locals[0].ty, Ty::U8);
        match &p.funcs[0].body[1] {
            Stmt::BindInt { ty: IntTy::U8, .. } => {}
            other => panic!("expected BindInt(U8), got {other:?}"),
        }
    }

    #[test]
    fn unsupported_width_fails_closed_before_codegen() {
        let c = compile_typed("fn main() {\n    let a: i8 = 127;\n    let c = a + 1;\n}\n");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let e = lower(&c.prog, &c.types).expect_err("i8 arithmetic must fail closed");
        assert!(e.what.contains("L2b"), "{}", e.what);
    }

    #[test]
    fn bare_arithmetic_statement_lowers_to_eval_int() {
        let b = body("fn main() {\n    let a = 1;\n    a + 1;\n}\n");
        assert!(matches!(b[1], Stmt::EvalInt { ty: IntTy::I64, .. }), "{b:?}");
    }

    #[test]
    fn unary_minus_lowers_to_neg_node() {
        let b = body("fn main() {\n    let max = 9223372036854775807;\n    let c = -max - 2;\n}\n");
        match &b[1] {
            Stmt::BindInt { expr: IntExpr::Bin { op: ArithOp::Sub, lhs, .. }, .. } => {
                assert!(matches!(lhs.as_ref(), IntExpr::Neg { .. }), "{lhs:?}");
            }
            other => panic!("expected BindInt(Bin), got {other:?}"),
        }
    }

    #[test]
    fn negative_literal_min_folds_to_constant() {
        let b = body("fn main() {\n    let m = -9223372036854775808;\n}\n");
        match &b[0] {
            Stmt::BindInt { expr: IntExpr::Const(v), .. } => assert_eq!(*v, i64::MIN as i128),
            other => panic!("expected BindInt(Const), got {other:?}"),
        }
    }
}
