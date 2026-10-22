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
    F32,
    F64,
    Box(Box<Ty>),
    Unit,
}

/// L2b codegen 的整型宽度（其余在册宽度由 lower fail-closed）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntTy {
    I64,
    U8,
    U32,
}

impl IntTy {
    pub fn ir_ty(self) -> Ty {
        match self {
            IntTy::I64 => Ty::I64,
            IntTy::U8 => Ty::U8,
            IntTy::U32 => Ty::U32,
        }
    }

    pub fn c_ty(self) -> &'static str {
        match self {
            IntTy::I64 => "int64_t",
            IntTy::U8 => "uint8_t",
            IntTy::U32 => "uint32_t",
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntTy::I64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl ArithOp {
    pub fn symbol(self) -> &'static str {
        match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
            ArithOp::Rem => "%",
        }
    }

    pub fn checked_builtin(self) -> Option<&'static str> {
        match self {
            ArithOp::Add => Some("__builtin_add_overflow"),
            ArithOp::Sub => Some("__builtin_sub_overflow"),
            ArithOp::Mul => Some("__builtin_mul_overflow"),
            ArithOp::Div | ArithOp::Rem => None,
        }
    }
}

/// 整型算术表达式树：操作数同宽（check 已判），节点带源位置。
#[derive(Clone, Debug)]
pub enum IntExpr {
    Const(i128),
    Local(LocalId),
    Neg {
        expr: Box<IntExpr>,
        line: usize,
        col: usize,
    },
    Bin {
        op: ArithOp,
        lhs: Box<IntExpr>,
        rhs: Box<IntExpr>,
        line: usize,
        col: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub ty: Ty,
}

/// `println` 的字符串实参来源：已绑定局部，或直接字面量。
#[derive(Clone, Debug)]
pub enum StrArg {
    Local(LocalId),
    Bytes(Vec<u8>),
}

/// L2c 打印实参：字符串／字节串、三宽度运行期整型、编译期格式化字节。
#[derive(Clone, Debug)]
pub enum PrintArg {
    Str(StrArg),
    Int { ty: IntTy, expr: IntExpr },
    /// 布尔／byte／unit／窄整型／浮点的契约格式化结果（编译期确定）。
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
    /// `println(a1, ..., aN)`：实参间单空格，整次调用恰一枚 LF。
    Print {
        args: Vec<PrintArg>,
        newline: bool,
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
    /// `let x = <int 表达式>;`：i64／u8／u32 的算术结果（checked／wrap 见 emit）。
    BindInt {
        local: LocalId,
        ty: IntTy,
        expr: IntExpr,
        line: usize,
        col: usize,
    },
    /// 整型表达式语句（结果丢弃）：仍保留 checked 副作用。
    EvalInt {
        ty: IntTy,
        expr: IntExpr,
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
