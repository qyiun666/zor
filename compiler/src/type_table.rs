//! 0.2.0 L2a 使能项：check 类型结果的唯一暴露面（typed IR 前置）。
//!
//! 契约：类型与所有权规则只写在 `check`；`check` 把每个表达式的类型、`let` 绑定类型、
//! 赋值目标类型与函数签名写入 `TypeTable`；`lower` 只读本表，不得自行复制类型规则。
//! 表键＝源码位置＋节点类别；同一位置的不同节点类别互不覆盖。

use std::collections::HashMap;

use crate::parser::{Ex, Ty};

/// 表达式节点类别（与 `parser::Ex` 变体一一对应，不含载荷）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExTag {
    Lit,
    Path,
    UnitLit,
    Bin,
    Un,
    Call,
    Method,
    Index,
    Field,
    Record,
    Tup,
    ArrLit,
    Ref,
    Box,
    Deref,
    Unbox,
    Block,
}

impl ExTag {
    pub fn of(e: &Ex) -> ExTag {
        match e {
            Ex::Lit(..) => ExTag::Lit,
            Ex::Path(..) => ExTag::Path,
            Ex::UnitLit(..) => ExTag::UnitLit,
            Ex::Bin(..) => ExTag::Bin,
            Ex::Un(..) => ExTag::Un,
            Ex::Call(..) => ExTag::Call,
            Ex::Method(..) => ExTag::Method,
            Ex::Index(..) => ExTag::Index,
            Ex::Field(..) => ExTag::Field,
            Ex::Record(..) => ExTag::Record,
            Ex::Tup(..) => ExTag::Tup,
            Ex::ArrLit(..) => ExTag::ArrLit,
            Ex::Ref(..) => ExTag::Ref,
            Ex::Box(..) => ExTag::Box,
            Ex::Deref(..) => ExTag::Deref,
            Ex::Unbox(..) => ExTag::Unbox,
            Ex::Block(..) => ExTag::Block,
        }
    }
}

/// 节点类别：表达式，或语句级位置（`let`／赋值）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Expr(ExTag),
    Let,
    Assign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey {
    pub line: usize,
    pub col: usize,
    pub kind: NodeKind,
}

impl NodeKey {
    fn expr(e: &Ex) -> NodeKey {
        let (line, col) = e.span();
        NodeKey { line, col, kind: NodeKind::Expr(ExTag::of(e)) }
    }

    fn let_at(line: usize, col: usize) -> NodeKey {
        NodeKey { line, col, kind: NodeKind::Let }
    }

    fn assign_at(line: usize, col: usize) -> NodeKey {
        NodeKey { line, col, kind: NodeKind::Assign }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnSig {
    pub params: Vec<Ty>,
    pub ret: Ty,
}

/// check 写、lower 读的类型表。缺项＝调用方越过了 check，lower 必须 fail-closed。
#[derive(Clone, Debug, Default)]
pub struct TypeTable {
    exprs: HashMap<NodeKey, Ty>,
    lets: HashMap<NodeKey, Ty>,
    assigns: HashMap<NodeKey, Ty>,
    fn_sigs: HashMap<String, FnSig>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_expr(&mut self, e: &Ex, ty: Ty) {
        self.exprs.insert(NodeKey::expr(e), ty);
    }

    pub fn expr_ty(&self, e: &Ex) -> Option<&Ty> {
        self.exprs.get(&NodeKey::expr(e))
    }

    pub fn insert_let(&mut self, line: usize, col: usize, ty: Ty) {
        self.lets.insert(NodeKey::let_at(line, col), ty);
    }

    pub fn let_ty(&self, line: usize, col: usize) -> Option<&Ty> {
        self.lets.get(&NodeKey::let_at(line, col))
    }

    pub fn insert_assign(&mut self, line: usize, col: usize, ty: Ty) {
        self.assigns.insert(NodeKey::assign_at(line, col), ty);
    }

    pub fn assign_ty(&self, line: usize, col: usize) -> Option<&Ty> {
        self.assigns.get(&NodeKey::assign_at(line, col))
    }

    pub fn insert_fn_sig(&mut self, name: String, sig: FnSig) {
        self.fn_sigs.insert(name, sig);
    }

    pub fn fn_sig(&self, name: &str) -> Option<&FnSig> {
        self.fn_sigs.get(name)
    }

    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }
}

/// `[byte; N]` 的字节数；其余形态返回 `None`（lower 据此 fail-closed）。
pub fn byte_array_len(t: &Ty) -> Option<u64> {
    match t {
        Ty::Arr(elem, n) if matches!(elem.as_ref(), Ty::Prim("byte")) => Some(*n),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{byte_array_len, ExTag};
    use crate::{compile_typed, parser::Ty};

    #[test]
    fn check_records_expr_let_and_fn_sig_types() {
        let c = compile_typed("fn main() {\n    let greeting = \"Hello, ZOR\";\n    println(greeting);\n}\n");
        assert!(c.diags.is_empty(), "{:?}", c.diags);

        let main_sig = c.types.fn_sig("main").expect("main signature recorded");
        assert!(main_sig.params.is_empty());
        assert_eq!(main_sig.ret, Ty::unit());

        // `let` 关键字位于第 2 行第 5 列；字面量与绑定同型。
        let let_ty = c.types.let_ty(2, 5).expect("let type recorded");
        assert_eq!(byte_array_len(let_ty), Some(10));

        let let_init = match &c.prog.fns[0].body.stmts[0] {
            crate::parser::St::Let(_, _, init, _, _) => init,
            other => panic!("expected let, got {other:?}"),
        };
        assert_eq!(ExTag::of(let_init), ExTag::Lit);
        assert_eq!(c.types.expr_ty(let_init), Some(let_ty));
        assert!(c.types.expr_count() >= 3, "let 初值／println 调用／实参均入表");
    }

    #[test]
    fn annotated_let_type_comes_from_check_not_from_literal() {
        let c = compile_typed("fn main() { let s: [byte; 2] = \"hi\"; println(s); }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let t = c.types.let_ty(1, 13).expect("let type recorded");
        assert_eq!(byte_array_len(t), Some(2));
    }

    #[test]
    fn byte_array_len_only_accepts_byte_arrays() {
        assert_eq!(byte_array_len(&Ty::Arr(Box::new(Ty::Prim("byte")), 3)), Some(3));
        assert_eq!(byte_array_len(&Ty::Arr(Box::new(Ty::Prim("i64")), 3)), None);
        assert_eq!(byte_array_len(&Ty::Prim("byte")), None);
        assert_eq!(byte_array_len(&Ty::Bad), None);
    }
}
