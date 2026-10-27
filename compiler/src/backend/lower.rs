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
    payload_c_ty, ArithOp, BoxConst, BoxIntTy, ConstValue, EnumFieldInit, EnumInit, Func, IntExpr,
    IntTy, Local, LocalId, PayloadDef, PrintArg, Program, Stmt, StrArg, Ty, Value,
};
use super::subset::{self, SubsetError};
use crate::parser::{Ex, FloatSpecialKind, FloatWidth, Lit, Prog, St, Ty as AstTy};
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
    /// L2h：特殊值保留来源标记——区别于 `1e999` 等有限字面量溢出得到的 inf，
    /// 使绑定后的一元负号 `-x` 可按裁定 3 fail-closed。
    F32Special(FloatSpecialKind),
    F64Special(FloatSpecialKind),
    /// L2k：Box 复合载荷的编译期常量树（仅 Box 槽位使用）。
    Composite(ConstValue),
    /// L2v：运行期枚举变量副本的打印值树（仅 Box<enum> 变量载荷使用）。
    EnumLive(Value),
}

/// L2a 局部槽位状态：`live` 是静态可判的 owner 活跃位。
#[derive(Clone, Debug)]
struct Slot {
    ty: Ty,
    live: bool,
    /// 仅编译期标量（bool／byte／窄整型／浮点等）有值；运行期整型为 `None`。
    konst: Option<ConstVal>,
    /// `[byte; N]` 绑定的已知字节（复合值内按原始字节展开时需要）。
    bytes: Option<Vec<u8>>,
}

/// 复合值降级期间的可变构建面：新增的运行期整型叶子临时局部与对应语句。
struct Build<'a> {
    types: &'a TypeTable,
    prog: &'a Prog,
    slots: &'a mut Vec<Slot>,
    locals: &'a mut Vec<Local>,
    body: &'a mut Vec<Stmt>,
}

pub fn lower(prog: &Prog, types: &TypeTable) -> Result<Program, SubsetError> {
    subset::check(prog, types)?;
    let mut funcs = Vec::new();
    let mut payload_defs: Vec<PayloadDef> = Vec::new();
    for f in &prog.fns {
        if f.name != "main" {
            continue;
        }
        let mut locals: Vec<Local> = Vec::new();
        let mut slots: Vec<Slot> = Vec::new();
        let mut body: Vec<Stmt> = Vec::new();
        let mut scope: HashMap<String, LocalId> = HashMap::new();
        let mut values: HashMap<String, Value> = HashMap::new();
        // L3b：常量复合字面量初始化登记（整棵 box 载荷旁路；遮蔽即失效）。
        let mut const_composites: HashMap<String, ConstValue> = HashMap::new();
        let mut last_pos = f.span;
        for st in &f.body.stmts {
            match st {
                St::Let(name, _ann, init, l, c) => {
                    last_pos = (*l, *c);
                    const_composites.remove(name);
                    let init_ty = types
                        .expr_ty(init)
                        .ok_or_else(|| SubsetError::new(*l, *c, "lower：let 初值缺少 type table 记录"))?
                        .clone();
                    if is_composite_ir_ty(&init_ty) {
                        let recorded = types
                            .let_ty(*l, *c)
                            .ok_or_else(|| SubsetError::new(*l, *c, "lower：let 缺少 type table 记录"))?;
                        if recorded != &init_ty {
                            return Err(SubsetError::new(*l, *c, "lower：let 初值类型与绑定类型不符"));
                        }
                        let value = {
                            let mut b = Build {
                                types,
                                prog,
                                slots: &mut slots,
                                locals: &mut locals,
                                body: &mut body,
                            };
                            lower_value(init, &mut b, &scope, &values)?
                        };
                        values.insert(name.clone(), value);
                        let literal = matches!(init, Ex::Record(..) | Ex::ArrLit(..) | Ex::Call(..));
                        let is_enum = matches!(
                            &init_ty,
                            AstTy::Named(n) if prog.enums.iter().any(|e| &e.name == n)
                        );
                        if literal && !is_enum {
                            if let Ok(cv) = const_value(init, &init_ty, types, &slots, &scope, prog) {
                                const_composites.insert(name.clone(), cv);
                            }
                        }
                        scope.remove(name);
                        continue;
                    }
                    let byte_len = type_table::byte_array_len(&init_ty);
                    if let (Some(n), Ex::ArrLit(..)) = (byte_len, init) {
                        let value = {
                            let mut b = Build {
                                types,
                                prog,
                                slots: &mut slots,
                                locals: &mut locals,
                                body: &mut body,
                            };
                            lower_value(init, &mut b, &scope, &values)?
                        };
                        let Value::Bytes(bytes) = value else {
                            return Err(SubsetError::new(*l, *c, "lower：字节数组初值形态"));
                        };
                        let id = LocalId(locals.len() as u32);
                        body.push(Stmt::BindStr { local: id, bytes: bytes.clone(), line: *l, col: *c });
                        locals.push(Local { id, name: name.clone(), ty: Ty::Bytes(n) });
                        slots.push(Slot {
                            ty: Ty::Bytes(n),
                            live: true,
                            konst: None,
                            bytes: Some(bytes),
                        });
                        scope.insert(name.clone(), id);
                        values.remove(name);
                        continue;
                    }
                    let id = LocalId(locals.len() as u32);
                    let local_ty = let_type(types, prog, init, *l, *c)?;
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
                            body.push(Stmt::BindStr {
                                local: id,
                                bytes: bytes.clone(),
                                line: *l,
                                col: *c,
                            });
                            slots.push(Slot {
                                ty: Ty::Bytes(n),
                                live: true,
                                konst: None,
                                bytes: Some(bytes),
                            });
                        }
                        Ex::Box(inner, _, _) => {
                            let payload = match &local_ty {
                                Ty::Box(p) => (**p).clone(),
                                _ => return Err(SubsetError::new(*l, *c, "lower：box 绑定类型非 Box<T>")),
                            };
                            if is_composite_payload(&payload) {
                                // L2k：复合常量载荷——一棵常量树同时承载存储初始化与打印投影。
                                // L2v：`box e` 变量载荷——EnumLive 配方＋打印值树（Copy）。
                                let payload_ast = match types.let_ty(*l, *c) {
                                    Some(AstTy::BoxTy(p)) => (**p).clone(),
                                    _ => return Err(SubsetError::new(*l, *c, "lower：box 复合载荷缺少标注")),
                                };
                                let (box_const, konst) = box_enum_live_or_const(
                                    inner,
                                    &payload_ast,
                                    types,
                                    &slots,
                                    &scope,
                                    &values,
                                    &const_composites,
                                    prog,
                                )?;
                                register_payload(&payload, prog, &mut payload_defs)?;
                                body.push(Stmt::BindBox {
                                    local: id,
                                    ty: payload.clone(),
                                    value: box_const,
                                    line: *l,
                                    col: *c,
                                });
                                slots.push(Slot {
                                    ty: local_ty,
                                    live: true,
                                    konst: Some(konst),
                                    bytes: None,
                                });
                                let slot = slots.last().expect("slot pushed").clone();
                                locals.push(Local { id, name: name.clone(), ty: slot.ty });
                                scope.insert(name.clone(), id);
                                values.remove(name);
                                continue;
                            }
                            let ty = box_int_ty(&payload).ok_or_else(|| {
                                SubsetError::new(*l, *c, "lower：box 载荷类型非 8 个整型宽度之一")
                            })?;
                            let (value, konst) =
                                scalar_box_value(inner, &payload, ty, types, &slots, &scope)?;
                            body.push(Stmt::BindBox {
                                local: id,
                                ty: payload,
                                value,
                                line: *l,
                                col: *c,
                            });
                            slots.push(Slot { ty: local_ty, live: true, konst, bytes: None });
                        }
                        Ex::Lit(Lit::Int(v), il, ic) => match &local_ty {
                            Ty::I64 => {
                                let value = i64::try_from(*v)
                                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i64"))?;
                                body.push(Stmt::BindI64 { local: id, value, line: *l, col: *c });
                                slots.push(Slot { ty: Ty::I64, live: true, konst: None, bytes: None });
                            }
                            Ty::U8 | Ty::U32 => {
                                let ty = ir_int_ty(&local_ty).expect("u8/u32");
                                let expr = IntExpr::Const(int_const(v, ty, init)?);
                                body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                                slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                            }
                            _ => {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
                            }
                        },
                        // L2q：具名整型常量与整数字面量同口径绑定（含 `MIN_EXP` 负值）。
                        Ex::Lit(Lit::IntConst(v), il, ic) => match &local_ty {
                            Ty::I64 => {
                                let value = i64::try_from(*v)
                                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i64"))?;
                                body.push(Stmt::BindI64 { local: id, value, line: *l, col: *c });
                                slots.push(Slot { ty: Ty::I64, live: true, konst: None, bytes: None });
                            }
                            Ty::U8 | Ty::U32 => {
                                let ty = ir_int_ty(&local_ty).expect("u8/u32");
                                let expr = IntExpr::Const(int_const_i128(*v, ty, init)?);
                                body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                                slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                            }
                            _ => {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
                            }
                        },
                        Ex::Lit(Lit::Bool(_) | Lit::Float(_), _, _)
                        | Ex::UnitLit(..)
                        | Ex::FloatSpecial(..) => {
                            let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                            slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
                        }
                        Ex::Deref(base, dl, dc) => {
                            let known = ir_int_ty(&local_ty).is_some()
                                || box_int_ty(&local_ty).is_some();
                            if !known {
                                return Err(SubsetError::new(*l, *c, "lower：解引用结果类型非整型"));
                            }
                            let src = match base.as_ref() {
                                Ex::Path(n, _, _) => lookup(&scope, n, *dl, *dc)?,
                                other => {
                                    let (el, ec) = subset::expr_span(other);
                                    return Err(SubsetError::new(el, ec, "lower：解引用源形态"));
                                }
                            };
                            let src_ty = slots.get(src.0 as usize).map(|s| &s.ty);
                            if src_ty != Some(&Ty::Box(Box::new(local_ty.clone()))) {
                                return Err(SubsetError::new(*dl, *dc, "lower：解引用源类型与目标不符"));
                            }
                            body.push(Stmt::BindDeref { local: id, src, line: *dl, col: *dc });
                            slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                        }
                        Ex::Path(n, nl, nc) => {
                            let src = lookup(&scope, n, *nl, *nc)?;
                            if matches!(local_ty, Ty::Box(_)) {
                                if slots.get(src.0 as usize).map(|s| &s.ty) != Some(&local_ty) {
                                    return Err(SubsetError::new(*nl, *nc, "lower：移动源类型与目标不符"));
                                }
                                // L2j／L2k：浮点与复合载荷带编译期值；其余载荷槽位恒为 `None`。
                                let konst = slots.get(src.0 as usize).and_then(|s| s.konst.clone());
                                body.push(Stmt::MoveAssign { dst: id, src, line: *nl, col: *nc });
                                set_live(&mut slots, src, false);
                                slots.push(Slot { ty: local_ty, live: true, konst, bytes: None });
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
                                slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                            } else if box_int_ty(&local_ty).is_some() {
                                // L2g：五新宽度的运行期副本（仅来自 `*b`）；常量仍走编译期格式化。
                                let src_slot = slots
                                    .get(src.0 as usize)
                                    .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：拷贝源槽位缺失"))?;
                                if src_slot.ty != local_ty {
                                    return Err(SubsetError::new(*nl, *nc, "lower：整型拷贝源类型与目标不符"));
                                }
                                if src_slot.konst.is_some() {
                                    let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                    slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
                                } else {
                                    body.push(Stmt::BindBoxIntCopy {
                                        dst: id,
                                        src,
                                        line: *nl,
                                        col: *nc,
                                    });
                                    slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                                }
                            } else if is_const_scalar_ty(&local_ty) {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
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
                            slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                        }
                        Ex::Un(..) => {
                            if let Some(ty) = ir_int_ty(&local_ty) {
                                let expr = lower_int_expr(init, ty, types, &slots, &scope)?;
                                body.push(Stmt::BindInt { local: id, ty, expr, line: *l, col: *c });
                                slots.push(Slot { ty: local_ty, live: true, konst: None, bytes: None });
                            } else {
                                let val = const_scalar(init, &local_ty, types, &slots, &scope)?;
                                slots.push(Slot { ty: local_ty, live: true, konst: Some(val), bytes: None });
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
                    values.remove(name);
                }
                St::Expr(Some(Ex::Call(callee, args, l, c)), _, _) => {
                    last_pos = (*l, *c);
                    let is_println =
                        matches!(callee.as_ref(), Ex::Path(n, _, _) if n == "println");
                    if !is_println || args.is_empty() {
                        return Err(SubsetError::new(*l, *c, "lower：println 形态"));
                    }
                    let mut pargs = Vec::with_capacity(args.len());
                    {
                        let mut b = Build {
                            types,
                            prog,
                            slots: &mut slots,
                            locals: &mut locals,
                            body: &mut body,
                        };
                        for arg in args {
                            pargs.push(print_arg(arg, &mut b, &scope, &values)?);
                        }
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
                            // 二次防线：subset 已保证同载荷；漂移时在此 fail-closed。
                            let ta = slots.get(ia.0 as usize).map(|s| s.ty.clone());
                            let tb = slots.get(ib.0 as usize).map(|s| s.ty.clone());
                            if ta != tb {
                                return Err(SubsetError::new(
                                    *al,
                                    *ac,
                                    "lower：Box 赋值两侧载荷类型不符",
                                ));
                            }
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
                                let konst = slots.get(ib.0 as usize).and_then(|s| s.konst.clone());
                                if let Some(slot) = slots.get_mut(ia.0 as usize) {
                                    slot.konst = konst;
                                }
                            }
                        }
                        (Ex::Path(a, al, ac), Ex::Box(inner, _, _)) => {
                            let ia = lookup(&scope, a, *al, *ac)?;
                            let payload = match slots.get(ia.0 as usize).map(|s| &s.ty) {
                                Some(Ty::Box(p)) => (**p).clone(),
                                _ => return Err(SubsetError::new(*al, *ac, "lower：目标非 Box 绑定")),
                            };
                            let drop_dst = is_live(&slots, ia);
                            if is_composite_payload(&payload) {
                                // L2k：复合载荷重初始化（旧值按 drop_dst 释放一次）。
                                // L2v：变量枚举载荷同样走 EnumLive。
                                let payload_ast = match types.expr_ty(rhs) {
                                    Some(AstTy::BoxTy(p)) => (**p).clone(),
                                    _ => return Err(SubsetError::new(*al, *ac, "lower：box 复合载荷缺少标注")),
                                };
                                let (box_const, konst) = box_enum_live_or_const(
                                    inner,
                                    &payload_ast,
                                    types,
                                    &slots,
                                    &scope,
                                    &values,
                                    &const_composites,
                                    prog,
                                )?;
                                register_payload(&payload, prog, &mut payload_defs)?;
                                body.push(Stmt::BindBoxAssign {
                                    local: ia,
                                    ty: payload,
                                    value: box_const,
                                    drop_dst,
                                    line: *al,
                                    col: *ac,
                                });
                                set_live(&mut slots, ia, true);
                                if let Some(slot) = slots.get_mut(ia.0 as usize) {
                                    slot.konst = Some(konst);
                                }
                                continue;
                            }
                            let ty = box_int_ty(&payload).ok_or_else(|| {
                                SubsetError::new(*al, *ac, "lower：box 载荷类型非 8 个整型宽度之一")
                            })?;
                            let (value, konst) =
                                scalar_box_value(inner, &payload, ty, types, &slots, &scope)?;
                            body.push(Stmt::BindBoxAssign {
                                local: ia,
                                ty: payload,
                                value,
                                drop_dst,
                                line: *al,
                                col: *ac,
                            });
                            set_live(&mut slots, ia, true);
                            if let Some(slot) = slots.get_mut(ia.0 as usize) {
                                slot.konst = konst;
                            }
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
    Ok(Program { funcs, payload_defs })
}

/// println 实参降级：字符串／字节串／运行期整型／编译期格式化标量／复合值树。
fn print_arg(
    e: &Ex,
    b: &mut Build,
    scope: &HashMap<String, LocalId>,
    values: &HashMap<String, Value>,
) -> Result<PrintArg, SubsetError> {
    let (el, ec) = subset::expr_span(e);
    let expr_ty = b
        .types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(el, ec, "lower：println 实参缺少 type table 记录"))?
        .clone();
    // 复合值、只读投影：统一走 Value 树；标量叶子仍复用 L2c 分支。
    if is_composite_ir_ty(&expr_ty) || matches!(e, Ex::Field(..) | Ex::Index(..)) {
        let value = lower_value(e, b, scope, values)?;
        return Ok(match value {
            Value::Int { ty, expr } => PrintArg::Int { ty, expr },
            Value::Bytes(bytes) => PrintArg::Bytes(bytes),
            other => PrintArg::Composite(other),
        });
    }
    match e {
        Ex::Lit(Lit::Str(s), sl, sc) => {
            let n = expr_byte_len(b.types, e, *sl, *sc, "println 实参")?;
            if n != s.len() as u64 {
                return Err(SubsetError::new(
                    *sl,
                    *sc,
                    format!("lower：println 实参类型 [byte; {}] 与字面量 {} 字节不符", n, s.len()),
                ));
            }
            Ok(PrintArg::Str(StrArg::Bytes(s.as_bytes().to_vec())))
        }
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _) => {
            let ty = int_ty_of(b.types, e, "println 实参")?;
            let expr = lower_int_expr(e, ty, b.types, b.slots, scope)?;
            Ok(PrintArg::Int { ty, expr })
        }
        Ex::Lit(Lit::Float(_) | Lit::Bool(_), _, _)
        | Ex::UnitLit(..)
        | Ex::FloatSpecial(..) => {
            let irt = ir_ty_of(b.types, e, "println 实参")?;
            let val = const_scalar(e, &irt, b.types, b.slots, scope)?;
            Ok(PrintArg::Bytes(format_const(&val, &irt)?))
        }
        Ex::Path(n, nl, nc) => {
            let id = lookup(scope, n, *nl, *nc)?;
            let slot = b
                .slots
                .get(id.0 as usize)
                .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：println 实参槽位缺失"))?;
            match &slot.ty {
                Ty::Bytes(n_bytes) => {
                    // 与 check 的类型表交叉核对（唯一类型来源）
                    let table = expr_byte_len(b.types, e, *nl, *nc, "println 实参")?;
                    if table != *n_bytes {
                        return Err(SubsetError::new(*nl, *nc, "lower：println 实参类型与局部绑定不符"));
                    }
                    Ok(PrintArg::Str(StrArg::Local(id)))
                }
                ty => {
                    let irt = ir_ty_of(b.types, e, "println 实参")?;
                    if &irt != ty {
                        return Err(SubsetError::new(*nl, *nc, "lower：println 实参类型与局部绑定不符"));
                    }
                    if let Some(ity) = ir_int_ty(ty) {
                        Ok(PrintArg::Int { ty: ity, expr: IntExpr::Local(id) })
                    } else if let Some(bty) = box_int_ty(ty) {
                        // L2g：五新宽度的运行期局部（`let v = *b` 或副本）；常量仍编译期格式化。
                        if slot.konst.is_some() {
                            let val = slot.konst.clone().expect("checked");
                            Ok(PrintArg::Bytes(format_const(&val, ty)?))
                        } else {
                            Ok(PrintArg::BoxInt { ty: bty, local: id })
                        }
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
            let src_ty = b
                .slots
                .get(src.0 as usize)
                .map(|s| s.ty.clone())
                .ok_or_else(|| SubsetError::new(*dl, *dc, "lower：解引用源槽位缺失"))?;
            let payload = match &src_ty {
                Ty::Box(p) => (**p).clone(),
                _ => return Err(SubsetError::new(*dl, *dc, "lower：解引用源类型与实参不符")),
            };
            // L2k：复合载荷的打印投影（常量树，不读堆）。
            if is_composite_payload(&payload) {
                let cv = match b.slots.get(src.0 as usize).and_then(|s| s.konst.clone()) {
                    Some(ConstVal::Composite(cv)) => cv,
                    // L2v：运行期枚举副本的打印值树。
                    Some(ConstVal::EnumLive(v)) => return Ok(PrintArg::Composite(v)),
                    _ => return Err(SubsetError::new(*dl, *dc, "lower：Box 复合载荷缺少编译期值")),
                };
                return Ok(PrintArg::Composite(value_of_const(&cv, &payload)?));
            }
            let irt = ir_ty_of(b.types, e, "println 实参")?;
            if src_ty != Ty::Box(Box::new(irt.clone())) {
                return Err(SubsetError::new(*dl, *dc, "lower：解引用源类型与实参不符"));
            }
            if ir_int_ty(&irt).is_none() && box_int_ty(&irt).is_none() {
                return Err(SubsetError::new(*dl, *dc, "lower：解引用结果类型非整型"));
            }
            // L2j：Box<f32>／Box<f64> 载荷恒为编译期特殊值，直接复用 float_fmt 字节。
            if matches!(irt, Ty::F32 | Ty::F64) {
                let val = b
                    .slots
                    .get(src.0 as usize)
                    .and_then(|s| s.konst.clone())
                    .ok_or_else(|| SubsetError::new(*dl, *dc, "lower：Box 浮点载荷缺少编译期值"))?;
                return Ok(PrintArg::Bytes(format_const(&val, &irt)?));
            }
            // 读入临时量：Box owner 槽位只声明 zor_box_<id>，不能把其 id
            // 当作载荷局部 id 使用。与 lower_value 的复合解引用路径保持一致。
            let id = new_temp(b, irt.clone());
            b.body.push(Stmt::BindDeref { local: id, src, line: *dl, col: *dc });
            if let Some(ty) = ir_int_ty(&irt) {
                Ok(PrintArg::Int { ty, expr: IntExpr::Local(id) })
            } else {
                let bty = box_int_ty(&irt).expect("checked integer width");
                Ok(PrintArg::BoxInt { ty: bty, local: id })
            }
        }
        Ex::Un(..) | Ex::Bin(..) => {
            let irt = ir_ty_of(b.types, e, "println 实参")?;
            if let Some(ty) = ir_int_ty(&irt) {
                let expr = lower_int_expr(e, ty, b.types, b.slots, scope)?;
                Ok(PrintArg::Int { ty, expr })
            } else {
                let val = const_scalar(e, &irt, b.types, b.slots, scope)?;
                Ok(PrintArg::Bytes(format_const(&val, &irt)?))
            }
        }
        other => {
            let (l, c) = subset::expr_span(other);
            Err(SubsetError::new(l, c, "lower：println 实参不可打印"))
        }
    }
}

/// 复合类型（非 `[byte; N]`）：tuple／array／struct／enum。
fn is_composite_ir_ty(t: &AstTy) -> bool {
    match t {
        AstTy::Tuple(_) | AstTy::Named(_) => true,
        AstTy::Arr(elem, _) => !matches!(elem.as_ref(), AstTy::Prim("byte")),
        _ => false,
    }
}

/// 复合值降级：结构在编译期确定；运行期整型叶子先行求值进临时局部，
/// 保证 `println` 的原子性与源序求值。
fn lower_value(
    e: &Ex,
    b: &mut Build,
    scope: &HashMap<String, LocalId>,
    values: &HashMap<String, Value>,
) -> Result<Value, SubsetError> {
    let (l, c) = subset::expr_span(e);
    // Field 的 Path 基座是纯 place root：check 不为其写 type table 项，
    // 这里以绑定槽位（其类型同样来自 check）为准。
    if let Ex::Path(n, nl, nc) = e {
        if let Some(v) = values.get(n) {
            return Ok(v.clone());
        }
        let id = lookup(scope, n, *nl, *nc)?;
        let slot = b
            .slots
            .get(id.0 as usize)
            .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：复合成员槽位缺失"))?;
        if let Some(bytes) = &slot.bytes {
            return Ok(Value::Bytes(bytes.clone()));
        }
        if let Some(ity) = ir_int_ty(&slot.ty) {
            return Ok(Value::Int { ty: ity, expr: IntExpr::Local(id) });
        }
        let val = slot
            .konst
            .clone()
            .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：复合成员缺少编译期值"))?;
        // L2i：复合内特殊值（含经绑定别名）按既有浮点格式输出。
        return Ok(Value::Bytes(format_const(&val, &slot.ty)?));
    }
    let t = b
        .types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(l, c, "lower：复合值缺少 type table 记录"))?
        .clone();
    match e {
        Ex::Lit(Lit::Str(s), _, _) => {
            let n = type_table::byte_array_len(&t)
                .ok_or_else(|| SubsetError::new(l, c, "lower：字符串字面量类型非 [byte; N]"))?;
            if n != s.len() as u64 {
                return Err(SubsetError::new(
                    l,
                    c,
                    format!("lower：类型 [byte; {}] 与字面量 {} 字节不符", n, s.len()),
                ));
            }
            Ok(Value::Bytes(s.as_bytes().to_vec()))
        }
        Ex::Lit(Lit::Int(_) | Lit::IntConst(_), _, _)
        | Ex::Lit(Lit::Float(_) | Lit::Bool(_), _, _)
        | Ex::UnitLit(..)
        | Ex::Un(..)
        | Ex::Bin(..)
        | Ex::FloatSpecial(..) => lower_scalar_value(e, &t, b, scope),
        Ex::Deref(base, dl, dc) => {
            let src = match base.as_ref() {
                Ex::Path(n, _, _) => lookup(scope, n, *dl, *dc)?,
                other => {
                    let (el, ec) = subset::expr_span(other);
                    return Err(SubsetError::new(el, ec, "lower：解引用源形态"));
                }
            };
            let src_ty = b.slots.get(src.0 as usize).map(|s| s.ty.clone());
            // L2k：复合载荷投影（常量树；不读堆，不建复合临时量）。
            if let Some(Ty::Box(p)) = &src_ty {
                if is_composite_payload(p) {
                    match b.slots.get(src.0 as usize).and_then(|s| s.konst.clone()) {
                        Some(ConstVal::Composite(cv)) => return value_of_const(&cv, p),
                        Some(ConstVal::EnumLive(v)) => return Ok(v),
                        _ => {
                            return Err(SubsetError::new(*dl, *dc, "lower：Box 复合载荷缺少编译期值"))
                        }
                    }
                }
            }
            if src_ty != Some(Ty::Box(Box::new(Ty::I64))) {
                return Err(SubsetError::new(*dl, *dc, "lower：解引用源非 Box<i64>"));
            }
            if t != AstTy::Prim("i64") {
                return Err(SubsetError::new(*dl, *dc, "lower：解引用结果类型非 i64"));
            }
            let id = new_temp(b, Ty::I64);
            b.body.push(Stmt::BindDeref { local: id, src, line: *dl, col: *dc });
            Ok(Value::Int { ty: IntTy::I64, expr: IntExpr::Local(id) })
        }
        Ex::Tup(es, _, _) => {
            let want = match &t {
                AstTy::Tuple(ts) => ts,
                _ => return Err(SubsetError::new(l, c, "lower：元组初值类型记录不符")),
            };
            if want.len() != es.len() {
                return Err(SubsetError::new(l, c, "lower：元组元素数量与类型不符"));
            }
            let mut vals = Vec::with_capacity(es.len());
            for x in es {
                vals.push(lower_value(x, b, scope, values)?);
            }
            Ok(Value::Tuple(vals))
        }
        Ex::ArrLit(es, _, _) => {
            let (elem, n) = match &t {
                AstTy::Arr(elem, n) => (elem.as_ref(), *n),
                _ => return Err(SubsetError::new(l, c, "lower：数组初值类型记录不符")),
            };
            if n != es.len() as u64 {
                return Err(SubsetError::new(l, c, "lower：数组元素数量与类型不符"));
            }
            if matches!(elem, AstTy::Prim("byte")) {
                let mut bytes = Vec::with_capacity(es.len());
                for x in es {
                    bytes.push(byte_const(x)?);
                }
                return Ok(Value::Bytes(bytes));
            }
            let mut vals = Vec::with_capacity(es.len());
            for x in es {
                vals.push(lower_value(x, b, scope, values)?);
            }
            Ok(Value::Array(vals))
        }
        Ex::Record(name, fields, _, _) => {
            let decl = b
                .prog
                .structs
                .iter()
                .find(|s| &s.name == name)
                .ok_or_else(|| SubsetError::new(l, c, format!("lower：未知结构体 '{}'", name)))?;
            if t != AstTy::Named(name.clone()) {
                return Err(SubsetError::new(l, c, "lower：record 类型记录不符"));
            }
            // 先按源码顺序求值（运行期副作用顺序），再按声明顺序落树。
            let mut lowered: Vec<(&str, Value)> = Vec::with_capacity(fields.len());
            for (fname, fexpr) in fields {
                if lowered.iter().any(|(n, _)| n == fname) {
                    return Err(SubsetError::new(l, c, format!("lower：字段 '{}' 重复初始化", fname)));
                }
                let v = lower_value(fexpr, b, scope, values)?;
                lowered.push((fname.as_str(), v));
            }
            let mut out_fields = Vec::with_capacity(decl.fields.len());
            for (fname, _, _, _) in &decl.fields {
                let v = lowered
                    .iter()
                    .find(|(n, _)| n == fname)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| SubsetError::new(l, c, format!("lower：结构体字段 '{}' 未初始化", fname)))?;
                out_fields.push((fname.clone(), v));
            }
            if out_fields.len() != lowered.len() {
                return Err(SubsetError::new(l, c, "lower：record 字段集合不符"));
            }
            Ok(Value::Struct { name: name.clone(), fields: out_fields })
        }
        Ex::Call(callee, args, _, _) => {
            let Ex::Path(variant, _, _) = callee.as_ref() else {
                return Err(SubsetError::new(l, c, "lower：枚举构造形态"));
            };
            let AstTy::Named(en) = &t else {
                return Err(SubsetError::new(l, c, "lower：枚举构造类型记录不符"));
            };
            // L2r：限定名只用于解析；输出与裸变体一致（非限定变体名）。
            let var = variant.rsplit_once("::").map(|(_, b)| b).unwrap_or(variant.as_str());
            let decl = b
                .prog
                .enums
                .iter()
                .find(|x| &x.name == en)
                .ok_or_else(|| SubsetError::new(l, c, format!("lower：未知 enum '{}'", en)))?;
            let (_, payload, _, _) = decl
                .variants
                .iter()
                .find(|(n, _, _, _)| n.as_str() == var)
                .ok_or_else(|| SubsetError::new(l, c, format!("lower：enum '{}' 无变体 '{}'", en, var)))?;
            if payload.len() != args.len() {
                return Err(SubsetError::new(l, c, "lower：枚举变体载荷数量不符"));
            }
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(lower_value(a, b, scope, values)?);
            }
            // L2v：为枚举值生成物化配方（仅含标量字段；失败则 init=None，box e fail-closed）。
            let init = build_enum_init(
                callee,
                args,
                &t,
                &vals,
                b.types,
                b.slots.as_slice(),
                scope,
                b.prog,
            );
            Ok(Value::Enum { variant: var.to_string(), payload: vals, init })
        }
        Ex::Field(base, fname, fl, fc) => {
            let base_val = lower_value(base, b, scope, values)?;
            match base_val {
                Value::Struct { fields, .. } => fields
                    .into_iter()
                    .find(|(n, _)| n == fname)
                    .map(|(_, v)| v)
                    .ok_or_else(|| SubsetError::new(*fl, *fc, format!("lower：字段 '{}' 不存在", fname))),
                _ => Err(SubsetError::new(*fl, *fc, "lower：字段投影源非 struct")),
            }
        }
        Ex::Index(base, idx, il, ic) => {
            let base_val = lower_value(base, b, scope, values)?;
            let i = match idx.as_ref() {
                Ex::Lit(Lit::Int(v), _, _) => usize::try_from(*v)
                    .map_err(|_| SubsetError::new(*il, *ic, "lower：下标超出可用范围"))?,
                _ => return Err(SubsetError::new(*il, *ic, "lower：下标须为整数字面量")),
            };
            match base_val {
                Value::Tuple(vs) | Value::Array(vs) => vs
                    .into_iter()
                    .nth(i)
                    .ok_or_else(|| SubsetError::new(*il, *ic, "lower：下标越界")),
                Value::Bytes(bytes) => {
                    let byte = bytes
                        .get(i)
                        .copied()
                        .ok_or_else(|| SubsetError::new(*il, *ic, "lower：下标越界"))?;
                    Ok(Value::Bytes(byte.to_string().into_bytes()))
                }
                _ => Err(SubsetError::new(*il, *ic, "lower：下标投影源非 tuple／array")),
            }
        }
        other => {
            let (ol, oc) = subset::expr_span(other);
            Err(SubsetError::new(ol, oc, "lower：复合值形态不在 L2d codegen 子集"))
        }
    }
}

/// 标量叶子：运行期整型进 Value::Int（非常量先行求值进临时局部），
/// 其余在册原始类型在编译期按契约格式化。
fn lower_scalar_value(
    e: &Ex,
    t: &AstTy,
    b: &mut Build,
    scope: &HashMap<String, LocalId>,
) -> Result<Value, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let irt = ast_to_ir(t)
        .ok_or_else(|| SubsetError::new(l, c, format!("lower：类型 '{}' 不在 codegen 子集", t.display())))?;
    if let Some(ity) = ir_int_ty(&irt) {
        let expr = lower_int_expr(e, ity, b.types, b.slots, scope)?;
        return Ok(int_value(ity, expr, l, c, b));
    }
    let val = const_scalar(e, &irt, b.types, b.slots, scope)?;
    Ok(Value::Bytes(format_const(&val, &irt)?))
}

/// 运行期整型叶子：常量直接留树；表达式先行求值到临时局部，保持原子性与源序。
fn int_value(ity: IntTy, expr: IntExpr, l: usize, c: usize, b: &mut Build) -> Value {
    if matches!(expr, IntExpr::Const(_)) {
        return Value::Int { ty: ity, expr };
    }
    let id = new_temp(b, ity.ir_ty());
    b.body.push(Stmt::BindInt { local: id, ty: ity, expr, line: l, col: c });
    Value::Int { ty: ity, expr: IntExpr::Local(id) }
}

fn new_temp(b: &mut Build, ty: Ty) -> LocalId {
    let id = LocalId(b.locals.len() as u32);
    b.locals.push(Local { id, name: format!("zor_l2d_tmp_{}", id.0), ty: ty.clone() });
    b.slots.push(Slot { ty, live: true, konst: None, bytes: None });
    id
}

/// `[byte; N]` 字面量元素：Q-3 常量元素。
fn byte_const(e: &Ex) -> Result<u8, SubsetError> {
    match e {
        Ex::Lit(Lit::Int(v), l, c) => {
            let n = i128::try_from(*v)
                .map_err(|_| SubsetError::new(*l, *c, "lower：byte 元素超出 i128"))?;
            if !(0..=u8::MAX as i128).contains(&n) {
                return Err(SubsetError::new(*l, *c, "lower：byte 元素超出 0..=255"));
            }
            Ok(n as u8)
        }
        // L2q：具名整型常量作字节元素时同样只接受 0..=255。
        Ex::Lit(Lit::IntConst(v), l, c) => {
            if !(0..=u8::MAX as i128).contains(v) {
                return Err(SubsetError::new(*l, *c, "lower：byte 元素超出 0..=255"));
            }
            Ok(*v as u8)
        }
        other => {
            let (l, c) = subset::expr_span(other);
            Err(SubsetError::new(l, c, "lower：字节数组元素须为整数字面量"))
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
        // L2q：具名整型常量按有符号值直接做范围判定。
        Ex::Lit(Lit::IntConst(v), il, ic) => {
            if !int_in_range(want, *v) {
                return Err(SubsetError::new(*il, *ic, "lower：整型字面量超出声明宽度"));
            }
            Ok(ConstVal::Int(*v))
        }
        Ex::Lit(Lit::Float(v), _, _) => match want {
            Ty::F32 => Ok(ConstVal::F32(*v as f32)),
            Ty::F64 => Ok(ConstVal::F64(*v)),
            _ => Err(SubsetError::new(l, c, "lower：浮点初值类型与绑定不符")),
        },
        // L2h：特殊值在编译期格式化；宽度必须与源形态一致（check 已严格判定）。
        Ex::FloatSpecial(w, kind, il, ic) => match (want, w) {
            (Ty::F32, FloatWidth::F32) => Ok(ConstVal::F32Special(*kind)),
            (Ty::F64, FloatWidth::F64) => Ok(ConstVal::F64Special(*kind)),
            _ => Err(SubsetError::new(*il, *ic, "lower：特殊值宽度与绑定不符")),
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
                // L2h 裁定 3：特殊值经绑定后的一元负号同样 fail-closed；
                // 有限 `-x`（含 `1e999` 溢出得到的 inf）保持 0.3.6 行为。
                ConstVal::F32Special(_) | ConstVal::F64Special(_) => Err(SubsetError::new(
                    *ul,
                    *uc,
                    "lower：一元 '-' 不接受特殊值（按冻结裁定 3）",
                )),
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

fn special_f32(k: FloatSpecialKind) -> f32 {
    match k {
        FloatSpecialKind::Nan => f32::NAN,
        FloatSpecialKind::Infinity => f32::INFINITY,
        FloatSpecialKind::NegInfinity => f32::NEG_INFINITY,
        // L2o：`MIN` 为最负有限值（= -MAX），与 Rust 语义一致。
        FloatSpecialKind::Max => f32::MAX,
        FloatSpecialKind::Min => -f32::MAX,
        FloatSpecialKind::Epsilon => f32::EPSILON,
        // L2p：最小正正规数（不是次正规数、不是 0）。
        FloatSpecialKind::MinPositive => f32::MIN_POSITIVE,
    }
}

fn special_f64(k: FloatSpecialKind) -> f64 {
    match k {
        FloatSpecialKind::Nan => f64::NAN,
        FloatSpecialKind::Infinity => f64::INFINITY,
        FloatSpecialKind::NegInfinity => f64::NEG_INFINITY,
        // L2o：`MIN` 为最负有限值（= -MAX），与 Rust 语义一致。
        FloatSpecialKind::Max => f64::MAX,
        FloatSpecialKind::Min => -f64::MAX,
        FloatSpecialKind::Epsilon => f64::EPSILON,
        // L2p：最小正正规数（不是次正规数、不是 0）。
        FloatSpecialKind::MinPositive => f64::MIN_POSITIVE,
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
        // L2h：特殊值复用既有 float_fmt（nan／inf／-inf），零运行期依赖。
        (ConstVal::F32Special(k), Ty::F32) => Ok(float_fmt::format_f32(special_f32(*k)).into_bytes()),
        (ConstVal::F64Special(k), Ty::F64) => Ok(float_fmt::format_f64(special_f64(*k)).into_bytes()),
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
fn let_type(types: &TypeTable, prog: &Prog, init: &Ex, l: usize, c: usize) -> Result<Ty, SubsetError> {
    let t = types
        .let_ty(l, c)
        .ok_or_else(|| SubsetError::new(l, c, "lower：let 缺少 type table 记录"))?;
    let init_t = types
        .expr_ty(init)
        .ok_or_else(|| SubsetError::new(l, c, "lower：let 初值缺少 type table 记录"))?;
    if let AstTy::BoxTy(p) = t {
        if let Some(payload) = box_payload_ir_ty(p, prog) {
            if is_composite_payload(&payload) {
                // L2k：复合载荷标注无 `ast_to_ir` 映射，按 AST 精确相等核对。
                if init_t != t {
                    return Err(SubsetError::new(l, c, "lower：let 初值类型与绑定类型不符"));
                }
                return Ok(Ty::Box(Box::new(payload)));
            }
        }
    }
    let ir = ast_to_ir(t)
        .ok_or_else(|| SubsetError::new(l, c, format!("lower：let 类型 '{}' 不在 L2c codegen 子集", t.display())))?;
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
        AstTy::BoxTy(inner) => {
            let payload = match inner.as_ref() {
                AstTy::Prim("i8") => Ty::I8,
                AstTy::Prim("i16") => Ty::I16,
                AstTy::Prim("i32") => Ty::I32,
                AstTy::Prim("i64") => Ty::I64,
                AstTy::Prim("u8") => Ty::U8,
                AstTy::Prim("u16") => Ty::U16,
                AstTy::Prim("u32") => Ty::U32,
                AstTy::Prim("u64") => Ty::U64,
                // L2j：仅特殊值载荷可用（有限浮点由 subset 拒绝）。
                AstTy::Prim("f32") => Ty::F32,
                AstTy::Prim("f64") => Ty::F64,
                _ => return None,
            };
            Some(Ty::Box(Box::new(payload)))
        }
        _ => None,
    }
}

/// L2k：仅 struct／定长数组载荷进入复合常量树通道。
fn is_composite_payload(t: &Ty) -> bool {
    matches!(t, Ty::Struct(_) | Ty::Array(..) | Ty::Enum(_))
}

/// L2k：Box 载荷的 IR 类型（标量沿用 `ast_to_ir`，复合在此展开）。
/// L2t：`Named` 需按 `Prog` 区分 enum 与 struct。
fn box_payload_ir_ty(t: &AstTy, prog: &Prog) -> Option<Ty> {
    match t {
        AstTy::Prim(p) => match *p {
            "i8" => Some(Ty::I8),
            "i16" => Some(Ty::I16),
            "i32" => Some(Ty::I32),
            "i64" => Some(Ty::I64),
            "u8" => Some(Ty::U8),
            "u16" => Some(Ty::U16),
            "u32" => Some(Ty::U32),
            "u64" => Some(Ty::U64),
            "f32" => Some(Ty::F32),
            "f64" => Some(Ty::F64),
            "bool" => Some(Ty::Bool),
            _ => None,
        },
        AstTy::Named(n) => Some(if prog.enums.iter().any(|e| &e.name == n) {
            Ty::Enum(n.clone())
        } else {
            Ty::Struct(n.clone())
        }),
        AstTy::Arr(elem, n) if !matches!(elem.as_ref(), AstTy::Prim("byte")) => {
            Some(Ty::Array(Box::new(box_payload_ir_ty(elem, prog)?), *n))
        }
        _ => None,
    }
}

/// L3b：整棵常量复合旁路值的顶层形状核对（struct 名／array 元素与长度）。
fn const_composite_shape_ok(cv: &ConstValue, want: &AstTy, prog: &Prog) -> bool {
    let Some(ir) = box_payload_ir_ty(want, prog) else {
        return false;
    };
    match (cv, &ir) {
        (ConstValue::Struct { name, .. }, Ty::Struct(w)) => name == w,
        (ConstValue::Array { elem, values }, Ty::Array(w, n)) => {
            elem == w.as_ref() && values.len() as u64 == *n
        }
        _ => false,
    }
}

/// L2v：`box e` 分派——变量枚举走 EnumLive 堆副本配方，其余沿用常量树。
/// 变量值缺失配方（init=None）时回落到常量通道，以既有判词 fail-closed。
/// L3b：整棵常量复合变量走 `const_composites` 旁路（仅字面量初始化登记）。
fn box_enum_live_or_const(
    inner: &Ex,
    payload_ast: &AstTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
    values: &HashMap<String, Value>,
    const_composites: &HashMap<String, ConstValue>,
    prog: &Prog,
) -> Result<(BoxConst, ConstVal), SubsetError> {
    if let Ex::Path(n, nl, nc) = inner {
        if let Some(Value::Enum { init: Some(init), .. }) = values.get(n) {
            let AstTy::Named(want) = payload_ast else {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷标注非枚举"));
            };
            if &init.name != want {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷类型与标注不符"));
            }
            // 二次防线：`e` 的节点类型必须与标注载荷一致。
            if types.expr_ty(inner) != Some(payload_ast) {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷类型记录不符"));
            }
            if init.fields.iter().any(|(t, _)| !is_enum_scalar_field_ir(t)) {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷字段不在标量白名单"));
            }
            let value = values
                .get(n)
                .expect("enum value present")
                .clone();
            let box_const = BoxConst::EnumLive { init: init.clone(), value: value.clone() };
            return Ok((box_const, ConstVal::EnumLive(value)));
        }
        // L3b：顶层整棵常量复合变量（仅常量字面量初始化登记）。
        if let Some(cv) = const_composites.get(n) {
            if types.expr_ty(inner) != Some(payload_ast) {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷类型记录不符"));
            }
            if !const_composite_shape_ok(cv, payload_ast, prog) {
                return Err(SubsetError::new(*nl, *nc, "lower：box 变量载荷与标注不符"));
            }
            return Ok((
                BoxConst::Composite(cv.clone()),
                ConstVal::Composite(cv.clone()),
            ));
        }
    }
    let const_result = const_value(inner, payload_ast, types, slots, scope, prog);
    let const_err = match const_result {
        Ok(cv) => return Ok((BoxConst::Composite(cv.clone()), ConstVal::Composite(cv))),
        Err(e) => e,
    };
    // L2x：一层变量 enum 叶——常量通道失败后回落 live 构建；失败仍报原常量判词。
    match composite_value_live(inner, payload_ast, types, slots, scope, values, prog) {
        Ok(cv) => Ok((BoxConst::Composite(cv.clone()), ConstVal::Composite(cv))),
        Err(_) => Err(const_err),
    }
}

/// L2y：任意既有复合深度的 live 树——常量子树优先，enum 变量叶可为任意深度。
fn composite_value_live(
    e: &Ex,
    want: &AstTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
    values: &HashMap<String, Value>,
    prog: &Prog,
) -> Result<ConstValue, SubsetError> {
    live_value(e, want, types, slots, scope, values, prog)?
        .ok_or_else(|| SubsetError::new(0, 0, "lower：box 复合载荷形态不符"))
}

/// 单节点 live 构建：常量子树 → 直接 enum 变量叶 → 递归 struct／array。
/// 变量标量叶／表达式构造叶／整棵变量载荷在非 Record／ArrLit 形态返回 None。
fn live_value(
    e: &Ex,
    want: &AstTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
    values: &HashMap<String, Value>,
    prog: &Prog,
) -> Result<Option<ConstValue>, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let recorded = types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(l, c, "lower：box 复合载荷缺少 type table 记录"))?;
    if recorded != want {
        return Err(SubsetError::new(l, c, "lower：box 复合载荷节点类型与标注不符"));
    }
    if let Ok(cv) = const_value(e, want, types, slots, scope, prog) {
        return Ok(Some(cv));
    }
    if let Some(cv) = live_enum_leaf(e, want, types, values, prog)? {
        return Ok(Some(cv));
    }
    if let Some(cv) = live_scalar_leaf(e, want, types, slots, scope)? {
        return Ok(Some(cv));
    }
    match (e, want) {
        (Ex::Record(name, fields, rl, rc), AstTy::Named(want_name)) if name == want_name => {
            let decl = struct_decl(prog, name)?;
            let mut out = Vec::with_capacity(decl.fields.len());
            for (fname, fty, _, _) in &decl.fields {
                let (_, v) = fields
                    .iter()
                    .find(|(n, _)| n == fname)
                    .ok_or_else(|| {
                        SubsetError::new(*rl, *rc, format!("lower：结构体字段 '{}' 未初始化", fname))
                    })?;
                let ir = box_payload_ir_ty(fty, prog).ok_or_else(|| {
                    SubsetError::new(*rl, *rc, format!("lower：字段 '{}' 类型不在载荷白名单", fname))
                })?;
                let cv = live_value(v, fty, types, slots, scope, values, prog)?
                    .ok_or_else(|| SubsetError::new(*rl, *rc, "lower：box 复合载荷形态不符"))?;
                out.push((fname.clone(), ir, cv));
            }
            if out.len() != fields.len() {
                return Err(SubsetError::new(*rl, *rc, "lower：record 字段集合不符"));
            }
            Ok(Some(ConstValue::Struct { name: name.clone(), fields: out }))
        }
        (Ex::ArrLit(es, al, ac), AstTy::Arr(elem, n)) => {
            if es.len() as u64 != *n {
                return Err(SubsetError::new(*al, *ac, "lower：数组元素数量与标注不符"));
            }
            let ir = box_payload_ir_ty(elem, prog)
                .ok_or_else(|| SubsetError::new(*al, *ac, "lower：数组元素类型不在载荷白名单"))?;
            let mut out = Vec::with_capacity(es.len());
            for x in es {
                let cv = live_value(x, elem, types, slots, scope, values, prog)?
                    .ok_or_else(|| SubsetError::new(*al, *ac, "lower：box 复合载荷形态不符"))?;
                out.push(cv);
            }
            Ok(Some(ConstValue::Array { elem: ir, values: out }))
        }
        _ => Ok(None),
    }
}

/// L2x：直接 enum 变量叶——只接受 `values` 中的 Copy 枚举值（含 L2v 配方）。
fn live_enum_leaf(
    e: &Ex,
    want: &AstTy,
    types: &TypeTable,
    values: &HashMap<String, Value>,
    prog: &Prog,
) -> Result<Option<ConstValue>, SubsetError> {
    let Ex::Path(n, nl, nc) = e else {
        return Ok(None);
    };
    let Some(Value::Enum { init: Some(init), .. }) = values.get(n) else {
        return Ok(None);
    };
    let AstTy::Named(en) = want else {
        return Ok(None);
    };
    if types.expr_ty(e) != Some(want) || &init.name != en {
        return Ok(None);
    }
    if !prog.enums.iter().any(|x| &x.name == en) {
        return Ok(None);
    }
    if init.fields.iter().any(|(t, _)| !is_enum_scalar_field_ir(t)) {
        return Err(SubsetError::new(*nl, *nc, "lower：变量枚举字段不在标量白名单"));
    }
    let value = values.get(n).expect("enum value present").clone();
    let Value::Enum { variant, .. } = &value else {
        return Ok(None);
    };
    if variant != &init.variant {
        return Ok(None);
    }
    Ok(Some(ConstValue::EnumLive { init: init.clone(), value }))
}

/// L2z：复合树内标量变量叶——同型槽位优先取编译期常量；
/// 无常量但有运行期载体（i64／u8／u32）时生成 `ScalarLive`；
/// 无运行期载体的 deref 派生宽度返回 None（维持 fail-closed）。
fn live_scalar_leaf(
    e: &Ex,
    want: &AstTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
) -> Result<Option<ConstValue>, SubsetError> {
    let Ex::Path(n, nl, nc) = e else {
        return Ok(None);
    };
    if types.expr_ty(e) != Some(want) {
        return Ok(None);
    }
    let Some(ir) = ast_to_ir(want) else {
        return Ok(None);
    };
    if !is_enum_scalar_field_ir(&ir) {
        return Ok(None);
    }
    let id = lookup(scope, n, *nl, *nc)?;
    let slot = slots
        .get(id.0 as usize)
        .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：复合成员槽位缺失"))?;
    if slot.ty != ir {
        return Ok(None);
    }
    if let Some(val) = &slot.konst {
        let cv = constval_to_constvalue(val)
            .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：复合成员标量常量形态不支持"))?;
        return Ok(Some(cv));
    }
    if let Some(ty) = ir_int_ty(&ir) {
        return Ok(Some(ConstValue::ScalarLive { ty, expr: IntExpr::Local(id) }));
    }
    Ok(None)
}

/// L2v：枚举值物化配方（tag＋逐字段初始化）。仅标量字段可生成；
/// 无法生成时返回 None（值仍可打印，但 `box e` 将 fail-closed）。
fn build_enum_init(
    callee: &Ex,
    args: &[Ex],
    t: &AstTy,
    vals: &[Value],
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
    prog: &Prog,
) -> Option<EnumInit> {
    let Ex::Path(variant, _, _) = callee else { return None };
    let AstTy::Named(en) = t else { return None };
    let decl = prog.enums.iter().find(|e| &e.name == en)?;
    let var = match variant.rsplit_once("::") {
        Some((q, r)) if q == en.as_str() => r,
        Some(_) => return None,
        None => variant.as_str(),
    };
    let (tag, (_, payload, _, _)) = decl
        .variants
        .iter()
        .enumerate()
        .find(|(_, (n, _, _, _))| n.as_str() == var)?;
    if payload.len() != args.len() || vals.len() != args.len() {
        return None;
    }
    let mut fields = Vec::with_capacity(args.len());
    for ((fty, _, _), (arg, val)) in payload.iter().zip(args.iter().zip(vals.iter())) {
        let ir = box_payload_ir_ty(fty, prog)?;
        if !is_enum_scalar_field_ir(&ir) {
            return None;
        }
        match ir_int_ty(&ir) {
            Some(_) => {
                let Value::Int { expr, .. } = val else { return None };
                fields.push((ir, EnumFieldInit::Int(expr.clone())));
            }
            None => {
                let cv = constval_to_constvalue(&const_scalar(arg, &ir, types, slots, scope).ok()?)?;
                fields.push((ir, EnumFieldInit::Const(cv)));
            }
        }
    }
    Some(EnumInit {
        name: en.clone(),
        tag: tag as u32,
        variant: var.to_string(),
        fields,
    })
}

/// L2v：编译期标量 → 物化常量树叶子（Unit／复合不参与枚举字段）。
fn constval_to_constvalue(v: &ConstVal) -> Option<ConstValue> {
    match v {
        ConstVal::Int(n) => Some(ConstValue::Int(*n)),
        ConstVal::Bool(b) => Some(ConstValue::Bool(*b)),
        ConstVal::F32(f) => Some(ConstValue::F32Bits(f.to_bits())),
        ConstVal::F64(f) => Some(ConstValue::F64Bits(f.to_bits())),
        ConstVal::F32Special(k) => Some(ConstValue::F32Bits(special_f32(*k).to_bits())),
        ConstVal::F64Special(k) => Some(ConstValue::F64Bits(special_f64(*k).to_bits())),
        _ => None,
    }
}

/// L2v：枚举字段物化白名单（与 L2t 标量载荷一致）。
fn is_enum_scalar_field_ir(t: &Ty) -> bool {
    matches!(
        t,
        Ty::I8
            | Ty::I16
            | Ty::I32
            | Ty::I64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Bool
            | Ty::F32
            | Ty::F64
    )
}

fn struct_decl<'a>(prog: &'a Prog, name: &str) -> Result<&'a crate::parser::StructDecl, SubsetError> {
    prog.structs
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| SubsetError::new(0, 0, format!("lower：未知结构体 '{}'", name)))
}

/// L2k：复合载荷的编译期常量树。只接受字面量／常量叶子；变量即 fail-closed。
fn const_value(
    e: &Ex,
    want: &AstTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
    prog: &Prog,
) -> Result<ConstValue, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let recorded = types
        .expr_ty(e)
        .ok_or_else(|| SubsetError::new(l, c, "lower：box 复合载荷缺少 type table 记录"))?;
    if recorded != want {
        return Err(SubsetError::new(l, c, "lower：box 复合载荷节点类型与标注不符"));
    }
    match e {
        Ex::Lit(Lit::Int(v), il, ic) => {
            let n = i128::try_from(*v)
                .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i128"))?;
            let irt = ast_to_ir(want)
                .ok_or_else(|| SubsetError::new(l, c, "lower：box 载荷叶子类型不在 codegen 子集"))?;
            if !int_in_range(&irt, n) {
                return Err(SubsetError::new(*il, *ic, "lower：整型字面量超出声明宽度"));
            }
            Ok(ConstValue::Int(n))
        }
        // L2q：复合载荷叶子按声明宽度直接判定（仅沿用 L2k 既有常量树能力；裁定 §3 归 ⑤，不测试/不宣称）。
        Ex::Lit(Lit::IntConst(v), il, ic) => {
            let irt = ast_to_ir(want)
                .ok_or_else(|| SubsetError::new(l, c, "lower：box 载荷叶子类型不在 codegen 子集"))?;
            if !int_in_range(&irt, *v) {
                return Err(SubsetError::new(*il, *ic, "lower：整型字面量超出声明宽度"));
            }
            Ok(ConstValue::Int(*v))
        }
        Ex::Lit(Lit::Float(v), fl, fc) => match want {
            AstTy::Prim("f32") => Ok(ConstValue::F32Bits((*v as f32).to_bits())),
            AstTy::Prim("f64") => Ok(ConstValue::F64Bits(v.to_bits())),
            _ => Err(SubsetError::new(*fl, *fc, "lower：浮点初值类型与标注不符")),
        },
        Ex::FloatSpecial(w, kind, fl, fc) => match (want, w) {
            (AstTy::Prim("f32"), FloatWidth::F32) => {
                Ok(ConstValue::F32Bits(special_f32(*kind).to_bits()))
            }
            (AstTy::Prim("f64"), FloatWidth::F64) => {
                Ok(ConstValue::F64Bits(special_f64(*kind).to_bits()))
            }
            _ => Err(SubsetError::new(*fl, *fc, "lower：特殊值宽度与标注不符")),
        },
        Ex::Lit(Lit::Bool(v), _, _) => match want {
            AstTy::Prim("bool") => Ok(ConstValue::Bool(*v)),
            _ => Err(SubsetError::new(l, c, "lower：bool 初值类型与标注不符")),
        },
        // L2t：枚举常量变体（字段仅标量；常量子集）。
        Ex::Call(callee, args, cl, cc) => {
            let AstTy::Named(en) = want else {
                return Err(SubsetError::new(*cl, *cc, "lower：枚举构造类型与标注不符"));
            };
            let decl = prog
                .enums
                .iter()
                .find(|e| &e.name == en)
                .ok_or_else(|| SubsetError::new(*cl, *cc, format!("lower：未知 enum '{}'", en)))?;
            let Ex::Path(v, _, _) = callee.as_ref() else {
                return Err(SubsetError::new(*cl, *cc, "lower：枚举构造形态"));
            };
            let var = match v.rsplit_once("::") {
                Some((q, r)) if q == en.as_str() => r,
                Some(_) => {
                    return Err(SubsetError::new(*cl, *cc, "lower：枚举限定名与标注不符"));
                }
                None => v.as_str(),
            };
            let Some((tag, (_, payload, _, _))) = decl
                .variants
                .iter()
                .enumerate()
                .find(|(_, (n, _, _, _))| n.as_str() == var)
            else {
                return Err(SubsetError::new(
                    *cl,
                    *cc,
                    format!("lower：enum '{}' 无变体 '{}'", en, var),
                ));
            };
            if payload.len() != args.len() {
                return Err(SubsetError::new(*cl, *cc, "lower：枚举变体载荷数量不符"));
            }
            let mut fields = Vec::with_capacity(args.len());
            for (a, (fty, _, _)) in args.iter().zip(payload.iter()) {
                let ir = box_payload_ir_ty(fty, prog)
                    .ok_or_else(|| SubsetError::new(*cl, *cc, "lower：枚举字段类型不在载荷白名单"))?;
                if !matches!(
                    ir,
                    Ty::I8
                        | Ty::I16
                        | Ty::I32
                        | Ty::I64
                        | Ty::U8
                        | Ty::U16
                        | Ty::U32
                        | Ty::U64
                        | Ty::Bool
                        | Ty::F32
                        | Ty::F64
                ) {
                    return Err(SubsetError::new(*cl, *cc, "lower：枚举字段类型不在载荷白名单"));
                }
                let (al, ac) = subset::expr_span(a);
                let val = const_scalar(a, &ir, types, slots, scope)
                    .map_err(|_| SubsetError::new(al, ac, "box 载荷须为字面量常量"))?;
                let cv = match val {
                    ConstVal::Int(n) => ConstValue::Int(n),
                    ConstVal::Bool(b) => ConstValue::Bool(b),
                    ConstVal::F32(v) => ConstValue::F32Bits(v.to_bits()),
                    ConstVal::F64(v) => ConstValue::F64Bits(v.to_bits()),
                    ConstVal::F32Special(k) => ConstValue::F32Bits(special_f32(k).to_bits()),
                    ConstVal::F64Special(k) => ConstValue::F64Bits(special_f64(k).to_bits()),
                    _ => return Err(SubsetError::new(al, ac, "box 载荷须为字面量常量")),
                };
                fields.push((ir, cv));
            }
            Ok(ConstValue::Enum {
                name: en.clone(),
                tag: tag as u32,
                variant: var.to_string(),
                fields,
            })
        }
        Ex::Un(op, inner, ul, uc) if op == "-" => {
            let inner_ty = types
                .expr_ty(inner)
                .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：一元 '-' 缺少 type table 记录"))?;
            if inner_ty != want {
                return Err(SubsetError::new(*ul, *uc, "lower：一元 '-' 类型与标注不符"));
            }
            // 负整数字面量按幅值求值：正幅值本身可超出正数范围（i8 的 128／i64 的 2^63），
            // 必须先把幅值取负再按声明宽度判定（与 const_scalar 既有口径一致）。
            if let Ex::Lit(Lit::Int(v), il, ic) = inner.as_ref() {
                let irt = ast_to_ir(want)
                    .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：box 载荷叶子类型不在 codegen 子集"))?;
                if !matches!(
                    irt,
                    Ty::I8 | Ty::I16 | Ty::I32 | Ty::I64 | Ty::U8 | Ty::U16 | Ty::U32 | Ty::U64 | Ty::Byte
                ) {
                    return Err(SubsetError::new(*ul, *uc, "lower：一元 '-' 要求数值类型"));
                }
                let n = i128::try_from(*v)
                    .map_err(|_| SubsetError::new(*il, *ic, "lower：整型字面量超出 i128"))?;
                let n = n
                    .checked_neg()
                    .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：整型常量取负溢出"))?;
                if !int_in_range(&irt, n) {
                    return Err(SubsetError::new(*ul, *uc, "lower：整型字面量超出声明宽度"));
                }
                return Ok(ConstValue::Int(n));
            }
            match const_value(inner, want, types, slots, scope, prog)? {
                ConstValue::Int(n) => {
                    if !matches!(
                        want,
                        AstTy::Prim("i8")
                            | AstTy::Prim("i16")
                            | AstTy::Prim("i32")
                            | AstTy::Prim("i64")
                    ) {
                        return Err(SubsetError::new(*ul, *uc, "lower：无符号一元 '-' 不接受"));
                    }
                    let n = n
                        .checked_neg()
                        .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：整型常量取负溢出"))?;
                    let irt = ast_to_ir(want)
                        .ok_or_else(|| SubsetError::new(*ul, *uc, "lower：box 载荷叶子类型不在 codegen 子集"))?;
                    if !int_in_range(&irt, n) {
                        return Err(SubsetError::new(*ul, *uc, "lower：整型字面量超出声明宽度"));
                    }
                    Ok(ConstValue::Int(n))
                }
                ConstValue::F32Bits(bits) => Ok(ConstValue::F32Bits((-f32::from_bits(bits)).to_bits())),
                ConstValue::F64Bits(bits) => Ok(ConstValue::F64Bits((-f64::from_bits(bits)).to_bits())),
                _ => Err(SubsetError::new(*ul, *uc, "lower：一元 '-' 不接受该载荷叶子")),
            }
        }
        Ex::Record(name, fields, rl, rc) => {
            let AstTy::Named(want_name) = want else {
                return Err(SubsetError::new(*rl, *rc, "lower：record 类型与标注不符"));
            };
            if name != want_name {
                return Err(SubsetError::new(*rl, *rc, "lower：record 名称与标注不符"));
            }
            let decl = struct_decl(prog, name)?;
            let mut out = Vec::with_capacity(decl.fields.len());
            for (fname, fty, _, _) in &decl.fields {
                let v = fields
                    .iter()
                    .find(|(n, _)| n == fname)
                    .ok_or_else(|| SubsetError::new(*rl, *rc, format!("lower：结构体字段 '{}' 未初始化", fname)))?;
                let f_ir = box_payload_ir_ty(fty, prog)
                    .ok_or_else(|| SubsetError::new(*rl, *rc, format!("lower：字段 '{}' 类型不在载荷白名单", fname)))?;
                let cv = const_value(&v.1, fty, types, slots, scope, prog)?;
                out.push((fname.clone(), f_ir, cv));
            }
            if out.len() != fields.len() {
                return Err(SubsetError::new(*rl, *rc, "lower：record 字段集合不符"));
            }
            Ok(ConstValue::Struct { name: name.clone(), fields: out })
        }
        Ex::ArrLit(es, al, ac) => {
            let AstTy::Arr(elem, n) = want else {
                return Err(SubsetError::new(*al, *ac, "lower：数组初值类型与标注不符"));
            };
            if *n != es.len() as u64 {
                return Err(SubsetError::new(*al, *ac, "lower：数组元素数量与标注不符"));
            }
            let elem_ir = box_payload_ir_ty(elem, prog)
                .ok_or_else(|| SubsetError::new(*al, *ac, "lower：数组元素类型不在载荷白名单"))?;
            let mut out = Vec::with_capacity(es.len());
            for x in es {
                out.push(const_value(x, elem, types, slots, scope, prog)?);
            }
            Ok(ConstValue::Array { elem: elem_ir, values: out })
        }
        other => {
            let (ol, oc) = subset::expr_span(other);
            Err(SubsetError::new(ol, oc, "box 载荷须为字面量常量"))
        }
    }
}

/// L2k：按首次使用序登记复合载荷 typedef（子类型先于父类型）。
fn register_payload(
    t: &Ty,
    prog: &Prog,
    defs: &mut Vec<PayloadDef>,
) -> Result<(), SubsetError> {
    match t {
        Ty::Struct(name) => {
            let decl = struct_decl(prog, name)?;
            let mut fields = Vec::with_capacity(decl.fields.len());
            for (fname, fty, _, _) in &decl.fields {
                let ir = box_payload_ir_ty(fty, prog)
                    .ok_or_else(|| SubsetError::new(0, 0, format!("lower：字段 '{}' 类型不在载荷白名单", fname)))?;
                register_payload(&ir, prog, defs)?;
                let cty = payload_c_ty(&ir)
                    .ok_or_else(|| SubsetError::new(0, 0, format!("lower：字段 '{}' 缺少 C 类型", fname)))?;
                fields.push((fname.clone(), cty));
            }
            let def_name = payload_c_ty(t).expect("struct payload c type");
            if !defs.iter().any(|d| match d {
                PayloadDef::Struct { name, .. }
                | PayloadDef::Array { name, .. }
                | PayloadDef::Enum { c_name: name, .. } => name == &def_name,
            }) {
                defs.push(PayloadDef::Struct { name: def_name, fields });
            }
        }
        Ty::Array(elem, n) => {
            register_payload(elem, prog, defs)?;
            let def_name = payload_c_ty(t).expect("array payload c type");
            let elem_cty = payload_c_ty(elem)
                .ok_or_else(|| SubsetError::new(0, 0, "lower：数组元素缺少 C 类型"))?;
            if !defs.iter().any(|d| match d {
                PayloadDef::Struct { name, .. }
                | PayloadDef::Array { name, .. }
                | PayloadDef::Enum { c_name: name, .. } => name == &def_name,
            }) {
                defs.push(PayloadDef::Array {
                    name: def_name,
                    elem: elem_cty,
                    len: *n,
                });
            }
        }
        // L2t：枚举载荷 typedef（tag＋union；变体字段仅标量）。
        Ty::Enum(name) => {
            let decl = prog
                .enums
                .iter()
                .find(|e| &e.name == name)
                .ok_or_else(|| SubsetError::new(0, 0, format!("lower：未知 enum '{}'", name)))?;
            let mut variants = Vec::with_capacity(decl.variants.len());
            for (tag, (_, payload, _, _)) in decl.variants.iter().enumerate() {
                let mut fields = Vec::with_capacity(payload.len());
                for (i, (fty, _, _)) in payload.iter().enumerate() {
                    let ir = box_payload_ir_ty(fty, prog)
                        .ok_or_else(|| SubsetError::new(0, 0, "lower：枚举字段类型不在载荷白名单"))?;
                    let cty = payload_c_ty(&ir)
                        .ok_or_else(|| SubsetError::new(0, 0, "lower：枚举字段缺少 C 类型"))?;
                    fields.push((format!("f{}", i), cty));
                }
                variants.push((tag as u32, fields));
            }
            let c_name = payload_c_ty(t).expect("enum payload c type");
            if !defs.iter().any(|d| match d {
                PayloadDef::Struct { name, .. }
                | PayloadDef::Array { name, .. }
                | PayloadDef::Enum { c_name: name, .. } => name == &c_name,
            }) {
                defs.push(PayloadDef::Enum { c_name, variants });
            }
        }
        _ => {}
    }
    Ok(())
}

/// L2k：常量树 → 既有打印 `Value` 树。叶子投影与普通复合打印同规：
/// i64／u8／u32 保留运行期 printf 通道，其余叶子在编译期格式化为字节。
fn value_of_const(v: &ConstValue, t: &Ty) -> Result<Value, SubsetError> {
    match (v, t) {
        (ConstValue::Int(n), ty) if ir_int_ty(ty).is_some() => Ok(Value::Int {
            ty: ir_int_ty(ty).expect("int ty"),
            expr: IntExpr::Const(*n),
        }),
        (
            ConstValue::Int(n),
            Ty::I8 | Ty::I16 | Ty::I32 | Ty::U16 | Ty::U64 | Ty::Byte,
        ) => Ok(Value::Bytes(n.to_string().into_bytes())),
        (ConstValue::Bool(b), Ty::Bool) => {
            Ok(Value::Bytes(if *b { b"true".to_vec() } else { b"false".to_vec() }))
        }
        (ConstValue::F32Bits(bits), Ty::F32) => Ok(Value::Bytes(
            float_fmt::format_f32(f32::from_bits(*bits)).into_bytes(),
        )),
        (ConstValue::F64Bits(bits), Ty::F64) => Ok(Value::Bytes(
            float_fmt::format_f64(f64::from_bits(*bits)).into_bytes(),
        )),
        (ConstValue::Struct { name, fields }, Ty::Struct(want)) if name == want => {
            let mut out = Vec::with_capacity(fields.len());
            for (fname, fty, fv) in fields {
                out.push((fname.clone(), value_of_const(fv, fty)?));
            }
            Ok(Value::Struct { name: name.clone(), fields: out })
        }
        (ConstValue::Array { elem, values }, Ty::Array(want_elem, n))
            if elem == want_elem.as_ref() && values.len() as u64 == *n =>
        {
            let mut out = Vec::with_capacity(values.len());
            for x in values {
                out.push(value_of_const(x, elem)?);
            }
            Ok(Value::Array(out))
        }
        // L2t：枚举常量 → 既有枚举打印树（变体名非限定；零载荷打印变体名）。
        (ConstValue::Enum { name, variant, fields, .. }, Ty::Enum(want)) if name == want => {
            let mut out = Vec::with_capacity(fields.len());
            for (fty, fv) in fields {
                out.push(value_of_const(fv, fty)?);
            }
            Ok(Value::Enum { variant: variant.clone(), payload: out, init: None })
        }
        // L2x：一层变量枚举叶 → 既有打印值树。
        (ConstValue::EnumLive { value, .. }, Ty::Enum(_)) => Ok(value.clone()),
        // L2z：复合树内运行期标量变量叶 → 既有整型打印树。
        (ConstValue::ScalarLive { ty, expr }, t) if ir_int_ty(t) == Some(*ty) => {
            Ok(Value::Int { ty: *ty, expr: expr.clone() })
        }
        _ => Err(SubsetError::new(0, 0, "lower：复合载荷常量与类型不符")),
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

/// L2g Box 载荷宽度：8 个整型宽度（独立于算术 `IntTy`）。
fn box_int_ty(t: &Ty) -> Option<BoxIntTy> {
    match t {
        Ty::I8 => Some(BoxIntTy::I8),
        Ty::I16 => Some(BoxIntTy::I16),
        Ty::I32 => Some(BoxIntTy::I32),
        Ty::I64 => Some(BoxIntTy::I64),
        Ty::U8 => Some(BoxIntTy::U8),
        Ty::U16 => Some(BoxIntTy::U16),
        Ty::U32 => Some(BoxIntTy::U32),
        Ty::U64 => Some(BoxIntTy::U64),
        Ty::F32 => Some(BoxIntTy::F32),
        Ty::F64 => Some(BoxIntTy::F64),
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

/// L2q：具名整型常量的有符号范围判定（含负值；u8／u32 只接受非负）。
fn int_const_i128(n: i128, ty: IntTy, e: &Ex) -> Result<i128, SubsetError> {
    let (l, c) = subset::expr_span(e);
    let fits = match ty {
        IntTy::I64 => (i64::MIN as i128..=i64::MAX as i128).contains(&n),
        IntTy::U8 => (0..=u8::MAX as i128).contains(&n),
        IntTy::U32 => (0..=u32::MAX as i128).contains(&n),
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
        Ex::Lit(Lit::IntConst(v), _, _) => Ok(IntExpr::Const(int_const_i128(*v, ty, e)?)),
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

/// L2j：特殊值载荷的存储位型（float／double 的 IEEE754 位模式）。
/// L2k：有限字面量同走此通道（`to_bits` 保留单／双精度位型）。
/// 打印仍走编译期 `float_fmt` 字节；此处仅保证载荷按真实位型落堆。
fn box_float_bits(val: &ConstVal, ty: BoxIntTy) -> Result<i128, SubsetError> {
    match (val, ty) {
        (ConstVal::F32Special(k), BoxIntTy::F32) => Ok(special_f32(*k).to_bits() as i128),
        (ConstVal::F64Special(k), BoxIntTy::F64) => Ok(special_f64(*k).to_bits() as i128),
        (ConstVal::F32(v), BoxIntTy::F32) => Ok(v.to_bits() as i128),
        (ConstVal::F64(v), BoxIntTy::F64) => Ok(v.to_bits() as i128),
        _ => Err(SubsetError::new(0, 0, "lower：Box 浮点载荷位型不符")),
    }
}

/// L3a：顶层标量 Box 取值——变量 Path 优先（编译期常量转位型；运行期
/// i64／u8／u32 生成 ScalarLive）；无载体的 deref 派生宽度报既有判词。
fn scalar_box_value(
    e: &Ex,
    payload: &Ty,
    ty: BoxIntTy,
    types: &TypeTable,
    slots: &[Slot],
    scope: &HashMap<String, LocalId>,
) -> Result<(BoxConst, Option<ConstVal>), SubsetError> {
    if let Ex::Path(n, nl, nc) = e {
        let id = lookup(scope, n, *nl, *nc)?;
        let slot = slots
            .get(id.0 as usize)
            .ok_or_else(|| SubsetError::new(*nl, *nc, "lower：box 标量载荷槽位缺失"))?;
        if &slot.ty != payload {
            return Err(SubsetError::new(*nl, *nc, "box 载荷非整型字面量"));
        }
        if let Some(val) = &slot.konst {
            if matches!(payload, Ty::F32 | Ty::F64) {
                return Ok((BoxConst::Bits(box_float_bits(val, ty)?), Some(val.clone())));
            }
            let ConstVal::Int(n) = val else {
                return Err(SubsetError::new(*nl, *nc, "box 载荷非整型字面量"));
            };
            return Ok((BoxConst::Bits(*n), None));
        }
        if let Some(ity) = ir_int_ty(payload) {
            return Ok((
                BoxConst::ScalarLive {
                    ty: ity,
                    expr: IntExpr::Local(id),
                },
                None,
            ));
        }
        return Err(SubsetError::new(*nl, *nc, "box 载荷非整型字面量"));
    }
    if matches!(payload, Ty::F32 | Ty::F64) {
        let val = const_scalar(e, payload, types, slots, scope)?;
        return Ok((BoxConst::Bits(box_float_bits(&val, ty)?), Some(val)));
    }
    Ok((BoxConst::Bits(box_lit(e, ty)?), None))
}

fn box_lit(e: &Ex, ty: BoxIntTy) -> Result<i128, SubsetError> {
    match e {
        Ex::Lit(Lit::Int(v), l, c) => {
            let n = i128::try_from(*v)
                .map_err(|_| SubsetError::new(*l, *c, "lower：整型字面量超出 i128"))?;
            // L2g：载荷仅非负字面量；上限与标注宽度一致（越界由 check E1201 先拦）。
            let max = match ty {
                BoxIntTy::I8 => i8::MAX as i128,
                BoxIntTy::I16 => i16::MAX as i128,
                BoxIntTy::I32 => i32::MAX as i128,
                BoxIntTy::I64 => i64::MAX as i128,
                BoxIntTy::U8 => u8::MAX as i128,
                BoxIntTy::U16 => u16::MAX as i128,
                BoxIntTy::U32 => u32::MAX as i128,
                BoxIntTy::U64 => u64::MAX as i128,
                // L2j：浮点载荷不经过整型字面量路径。
                BoxIntTy::F32 | BoxIntTy::F64 => {
                    return Err(SubsetError::new(*l, *c, "lower：浮点载荷非整型字面量"));
                }
            };
            if (0..=max).contains(&n) {
                Ok(n)
            } else {
                Err(SubsetError::new(*l, *c, "lower：整型字面量超出声明宽度"))
            }
        }
        // L2q：具名整型常量按载荷宽度判定；有符号宽度接受负值。
        Ex::Lit(Lit::IntConst(v), l, c) => {
            let (lo, hi) = match ty {
                BoxIntTy::I8 => (i8::MIN as i128, i8::MAX as i128),
                BoxIntTy::I16 => (i16::MIN as i128, i16::MAX as i128),
                BoxIntTy::I32 => (i32::MIN as i128, i32::MAX as i128),
                BoxIntTy::I64 => (i64::MIN as i128, i64::MAX as i128),
                BoxIntTy::U8 => (0, u8::MAX as i128),
                BoxIntTy::U16 => (0, u16::MAX as i128),
                BoxIntTy::U32 => (0, u32::MAX as i128),
                BoxIntTy::U64 => (0, u64::MAX as i128),
                BoxIntTy::F32 | BoxIntTy::F64 => {
                    return Err(SubsetError::new(*l, *c, "lower：浮点载荷非整型字面量"));
                }
            };
            if (lo..=hi).contains(v) {
                Ok(*v)
            } else {
                Err(SubsetError::new(*l, *c, "lower：整型字面量超出声明宽度"))
            }
        }
        other => {
            let (l, c) = subset::expr_span(other);
            Err(SubsetError::new(l, c, "lower：box 载荷非整型字面量"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lower;
    use crate::backend::ir::{ArithOp, BoxConst, ConstValue, IntExpr, IntTy, LocalId, PrintArg, Stmt, StrArg, Ty};
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
        assert!(
            e.what.contains("type table") || e.what.contains("不可打印"),
            "{}",
            e.what
        );
    }

    // —— G-E-2 状态机：6 条 manifest 样本 + 2 条矩阵空格 ——

    #[test]
    fn box_create_read_unbox_has_no_scope_drop() {
        let b = body("fn main() {\n    let b = box 1;\n    let v = *b;\n    unbox b;\n}\n");
        assert_eq!(b.len(), 3, "{b:?}");
        assert!(matches!(b[0], Stmt::BindBox { value: BoxConst::Bits(1), .. }));
        assert!(matches!(b[1], Stmt::BindDeref { .. }));
        assert!(matches!(b[2], Stmt::Unbox { .. }));
    }

    /// L2e：`println(*b)` 必须先把 Box 载荷读入 i64 临时量；不得把 Box 槽位 id
    /// 当作 i64 局部 id（后者会让 emit 生成未声明的 `zor_i64_<box_id>`）。
    #[test]
    fn deref_print_uses_i64_temp_not_box_slot() {
        let b = body("fn main() {\n    let b = box 42;\n    println(*b);\n}\n");
        assert_eq!(b.len(), 4, "{b:?}");
        assert!(matches!(b[0], Stmt::BindBox { local: LocalId(0), value: BoxConst::Bits(42), .. }));
        let temp = match &b[1] {
            Stmt::BindDeref { local, src, .. } => {
                assert_eq!(src.0, 0, "解引用源必须是 Box 槽位");
                *local
            }
            other => panic!("expected BindDeref, got {other:?}"),
        };
        assert_ne!(temp.0, 0, "临时量不得复用 Box 槽位 id");
        match &b[2] {
            Stmt::Print { args, .. } => match &args[0] {
                PrintArg::Int { ty: IntTy::I64, expr: IntExpr::Local(id) } => {
                    assert_eq!(*id, temp);
                }
                other => panic!("expected Int(Local(temp)), got {other:?}"),
            },
            other => panic!("expected Print, got {other:?}"),
        }
        assert!(matches!(b[3], Stmt::ScopeExit { .. }));
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
                assert_eq!((*drop_dst, value.clone()), (false, BoxConst::Bits(2)));
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

    // —— L2k：复合常量载荷 ——

    #[test]
    fn composite_print_projects_same_tree_as_plain_print() {
        let boxed = printed(
            "struct S { x: i64, y: f32, f: bool }\nfn main() { let b: Box<S> = box S { x: 1, y: -0.0, f: true }; println(*b); }",
        );
        let plain = printed(
            "struct S { x: i64, y: f32, f: bool }\nfn main() { let s = S { x: 1, y: -0.0, f: true }; println(s); }",
        );
        assert_eq!(format!("{boxed:?}"), format!("{plain:?}"));
    }

    #[test]
    fn finite_float_box_payload_bits_are_exact() {
        for (src, want) in [
            ("fn main() { let b: Box<f64> = box -1.5; println(*b); }", (-1.5f64).to_bits() as i128),
            ("fn main() { let b: Box<f64> = box -0.0; println(*b); }", (-0.0f64).to_bits() as i128),
            ("fn main() { let b: Box<f32> = box 1.5; println(*b); }", (1.5f32).to_bits() as i128),
        ] {
            let b = body(src);
            match &b[0] {
                Stmt::BindBox { value: BoxConst::Bits(v), .. } => assert_eq!(*v, want, "{src}"),
                other => panic!("{src}: expected BindBox(Bits), got {other:?}"),
            }
        }
    }

    #[test]
    fn composite_payload_tree_and_element_pushdown() {
        let b = body("fn main() { let b: Box<[u8;3]> = box [1, 2, 3]; println(*b); }");
        match &b[0] {
            Stmt::BindBox {
                ty: Ty::Array(elem, 3),
                value: BoxConst::Composite(ConstValue::Array { elem: ce, values }),
                ..
            } => {
                assert_eq!(**elem, Ty::U8);
                assert_eq!(*ce, Ty::U8);
                assert_eq!(values.len(), 3);
            }
            other => panic!("expected composite array BindBox, got {other:?}"),
        }
        let s = body(
            "struct P { x: i8 }\nstruct S { p: P, a: [u8;2] }\nfn main() { let b: Box<S> = box S { p: P { x: -1 }, a: [7, 8] }; println(*b); }",
        );
        match &s[0] {
            Stmt::BindBox { ty: Ty::Struct(n), value: BoxConst::Composite(ConstValue::Struct { fields, .. }), .. } => {
                assert_eq!(n, "S");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "p");
                assert_eq!(fields[1].0, "a");
            }
            other => panic!("expected composite struct BindBox, got {other:?}"),
        }
    }

    #[test]
    fn width_min_negative_leaves_are_accepted() {
        let b = body(
            "struct S { a: i64, b: i8 }\nfn main() { let b: Box<S> = box S { a: -9223372036854775808, b: -128 }; println(*b); }",
        );
        match &b[0] {
            Stmt::BindBox { value: BoxConst::Composite(ConstValue::Struct { fields, .. }), .. } => {
                assert_eq!(fields[0].2, ConstValue::Int(i64::MIN as i128));
                assert_eq!(fields[1].2, ConstValue::Int(i8::MIN as i128));
            }
            other => panic!("expected composite struct BindBox, got {other:?}"),
        }
        let a = body("fn main() { let b: Box<[i8;2]> = box [-1, -128]; println(*b); }");
        match &a[0] {
            Stmt::BindBox { value: BoxConst::Composite(ConstValue::Array { values, .. }), .. } => {
                assert_eq!(values[0], ConstValue::Int(-1));
                assert_eq!(values[1], ConstValue::Int(i8::MIN as i128));
            }
            other => panic!("expected composite array BindBox, got {other:?}"),
        }
    }

    #[test]
    fn composite_reassign_and_move_carry_payload() {
        let b = body("fn main() { let b: Box<[i64;2]> = box [1, 2]; b = box [3, 4]; println(*b); }");
        match b.iter().find(|s| matches!(s, Stmt::BindBoxAssign { .. })) {
            Some(Stmt::BindBoxAssign { drop_dst, value: BoxConst::Composite(_), .. }) => {
                assert!(*drop_dst, "活跃目标重赋值须释放旧值")
            }
            other => panic!("expected BindBoxAssign(Composite), got {other:?}"),
        }
        let m = body("struct P { x: i64 }\nfn main() { let a: Box<P> = box P { x: 1 }; let c = a; println(*c); }");
        assert!(m.iter().any(|s| matches!(s, Stmt::MoveAssign { .. })));
    }
}
