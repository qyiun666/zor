//! 0.2.0 首片最小 typed IR。
//!
//! 边界：输入来自 check 之后的 parser AST，经 subset 闸与 lower 降级；
//! 首片只有 `[byte; N]` 与 unit 两种类型，后续切片在同一 IR 上扩展。

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Bytes(u64),
    Unit,
}

#[derive(Clone, Debug)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub ty: Ty,
}

/// `println` 的单实参来源：已绑定局部，或直接字面量。
#[derive(Clone, Debug)]
pub enum StrArg {
    Local(LocalId),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    /// `let name = "..."`：首片只允许字符串字面量初始化。
    BindStr {
        local: LocalId,
        bytes: Vec<u8>,
        line: usize,
        col: usize,
    },
    /// `println` 单实参：输出原字节 + 一枚 LF。
    Println {
        arg: StrArg,
        line: usize,
        col: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Func {
    pub name: String,
    /// `fn` 关键字所在行，用于函数入口的 `#line`。
    pub line: usize,
    pub locals: Vec<Local>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub funcs: Vec<Func>,
}
