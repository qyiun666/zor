//! 最小 typed IR。
//!
//! 边界：输入来自 check 之后的 parser AST，经 subset 闸与 lower 降级；
//! L2a 起在 `[byte; N]`／unit 之上加入 `i64` 与 `Box<i64>`，以及 move／overwrite／
//! Box 载荷的显式事件。运行期代码生成（malloc／free／drop）见 `c::emit`；
//! 本层只产出语义事件与静态 `drop_dst`。

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Bytes(u64),
    I64,
    Box(Box<Ty>),
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
    /// `let x = <int>;`：Copy 值，无 drop。
    BindI64 {
        local: LocalId,
        value: i64,
        line: usize,
        col: usize,
    },
    /// `let x = box <int>;`：新建 Box 载荷（emit：malloc）。
    BindBox {
        local: LocalId,
        value: i64,
        line: usize,
        col: usize,
    },
    /// `x = box <int>;`：重初始化；`drop_dst` 由 lower 静态判定（emit：必要时先 free）。
    BindBoxAssign {
        local: LocalId,
        value: i64,
        drop_dst: bool,
        line: usize,
        col: usize,
    },
    /// `let v = *b;`：读 Box 载荷（Copy 结果）。
    BindI64Deref {
        local: LocalId,
        src: LocalId,
        line: usize,
        col: usize,
    },
    /// `let y = x;`／`x = y;`（目标无活值）：移动；源槽位置为无活值。
    MoveAssign {
        dst: LocalId,
        src: LocalId,
        line: usize,
        col: usize,
    },
    /// `x = y;`（目标有活值）：先 drop 旧值再写入；`drop_dst` 由 lower 静态判定。
    OverwriteAssign {
        dst: LocalId,
        src: LocalId,
        drop_dst: bool,
        line: usize,
        col: usize,
    },
    /// `x = x;`：自赋值，不 drop。
    SelfAssign {
        local: LocalId,
        line: usize,
        col: usize,
    },
    /// `unbox x;`：交出／释放 Box 载荷（emit：free）。
    Unbox {
        local: LocalId,
        line: usize,
        col: usize,
    },
    /// 作用域结束：按 `drops` 顺序（声明逆序）drop 仍活跃的 owner。
    ScopeExit {
        drops: Vec<LocalId>,
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
