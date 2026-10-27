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
    /// L2k：仅作 Box 载荷槽位的复合类型，不产生复合临时量。
    Struct(String),
    /// L2k：仅作 Box 载荷槽位的定长数组（元素为载荷允许叶子／复合）。
    Array(Box<Ty>, u64),
    /// L2t：仅作 Box 载荷槽位的枚举（常量子集；tag＋union）。
    Enum(String),
}

fn sanitize_ident(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

/// 载荷 C 类型拼写：标量用 `<stdint.h>` 类型；复合用首次使用序发射的 typedef 名。
/// `bool` 叶子落 `uint8_t`（载荷不可观测，避免新增 `<stdbool.h>` 依赖）。
pub fn payload_c_ty(t: &Ty) -> Option<String> {
    Some(match t {
        Ty::I8 => "int8_t".to_string(),
        Ty::I16 => "int16_t".to_string(),
        Ty::I32 => "int32_t".to_string(),
        Ty::I64 => "int64_t".to_string(),
        Ty::U8 => "uint8_t".to_string(),
        Ty::U16 => "uint16_t".to_string(),
        Ty::U32 => "uint32_t".to_string(),
        Ty::U64 => "uint64_t".to_string(),
        Ty::Bool => "uint8_t".to_string(),
        Ty::F32 => "float".to_string(),
        Ty::F64 => "double".to_string(),
        Ty::Struct(name) => format!("zor_struct_{}", sanitize_ident(name)),
        Ty::Array(elem, n) => format!("zor_arr_{}_{}", type_key(elem)?, n),
        Ty::Enum(name) => format!("zor_enum_{}", sanitize_ident(name)),
        _ => return None,
    })
}

/// 载荷类型键：用于数组 typedef 名与去重。
pub fn type_key(t: &Ty) -> Option<String> {
    Some(match t {
        Ty::I8 => "i8".to_string(),
        Ty::I16 => "i16".to_string(),
        Ty::I32 => "i32".to_string(),
        Ty::I64 => "i64".to_string(),
        Ty::U8 => "u8".to_string(),
        Ty::U16 => "u16".to_string(),
        Ty::U32 => "u32".to_string(),
        Ty::U64 => "u64".to_string(),
        Ty::Bool => "bool".to_string(),
        Ty::F32 => "f32".to_string(),
        Ty::F64 => "f64".to_string(),
        Ty::Struct(name) => sanitize_ident(name),
        Ty::Array(elem, n) => format!("{}_a{}", type_key(elem)?, n),
        Ty::Enum(name) => format!("e_{}", sanitize_ident(name)),
        _ => return None,
    })
}

/// L2k 复合载荷的树：叶子是标量位型，节点自带成员类型
/// （C 初始化与打印投影都无需回查声明表）；L2x 起可含一层 live enum 叶。
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i128),
    Bool(bool),
    F32Bits(u32),
    F64Bits(u64),
    Struct {
        name: String,
        fields: Vec<(String, Ty, ConstValue)>,
    },
    Array {
        elem: Ty,
        values: Vec<ConstValue>,
    },
    /// L2t：编译期枚举常量（字段仅标量；tag 取变体声明序）。
    Enum {
        name: String,
        tag: u32,
        variant: String,
        fields: Vec<(Ty, ConstValue)>,
    },
    /// L2x：一层 live enum 叶（仅 Struct 字段／Array 元素的直接位置；
    /// 打印投影为 value，初始化投影为 init）。
    EnumLive { init: EnumInit, value: Value },
}

/// Box 载荷常量：标量保留既有 i128 位型通道（C 文本零漂移），复合走常量树。
#[derive(Clone, Debug, PartialEq)]
pub enum BoxConst {
    Bits(i128),
    Composite(ConstValue),
    /// L2v：运行期枚举变量副本（tag＋union 初始化配方＋打印值树）。
    EnumLive { init: EnumInit, value: Value },
}

/// 载荷 typedef 定义（lower 按首次使用序登记，emit 在 main 之前发射）。
#[derive(Clone, Debug, PartialEq)]
pub enum PayloadDef {
    Struct {
        name: String,
        fields: Vec<(String, String)>,
    },
    Array {
        name: String,
        elem: String,
        len: u64,
    },
    /// L2t：枚举载荷 typedef（tag＋union；变体字段仅标量）。
    Enum {
        c_name: String,
        variants: Vec<(u32, Vec<(String, String)>)>,
    },
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

/// L2g Box 载荷宽度：8 个整型宽度。与算术 `IntTy` 分离，保证
/// 非 Box 算术／比较仍只有 i64／u8／u32 可 codegen（零扩容红线）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxIntTy {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    /// L2j：特殊值载荷宽度（载荷恒为编译期特殊值；有限浮点不在子集）。
    F32,
    F64,
}

impl BoxIntTy {
    pub fn ir_ty(self) -> Ty {
        match self {
            BoxIntTy::I8 => Ty::I8,
            BoxIntTy::I16 => Ty::I16,
            BoxIntTy::I32 => Ty::I32,
            BoxIntTy::I64 => Ty::I64,
            BoxIntTy::U8 => Ty::U8,
            BoxIntTy::U16 => Ty::U16,
            BoxIntTy::U32 => Ty::U32,
            BoxIntTy::U64 => Ty::U64,
            BoxIntTy::F32 => Ty::F32,
            BoxIntTy::F64 => Ty::F64,
        }
    }

    pub fn c_ty(self) -> &'static str {
        match self {
            BoxIntTy::I8 => "int8_t",
            BoxIntTy::I16 => "int16_t",
            BoxIntTy::I32 => "int32_t",
            BoxIntTy::I64 => "int64_t",
            BoxIntTy::U8 => "uint8_t",
            BoxIntTy::U16 => "uint16_t",
            BoxIntTy::U32 => "uint32_t",
            BoxIntTy::U64 => "uint64_t",
            BoxIntTy::F32 => "float",
            BoxIntTy::F64 => "double",
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, BoxIntTy::I8 | BoxIntTy::I16 | BoxIntTy::I32 | BoxIntTy::I64)
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
#[derive(Clone, Debug, PartialEq)]
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

/// L2v：枚举副本的字段物化配方（常量或运行期整型表达式）。
#[derive(Clone, Debug, PartialEq)]
pub enum EnumFieldInit {
    Const(ConstValue),
    Int(IntExpr),
}

/// L2v：枚举副本的 tag＋union 初始化配方。
#[derive(Clone, Debug, PartialEq)]
pub struct EnumInit {
    pub name: String,
    pub tag: u32,
    pub variant: String,
    pub fields: Vec<(Ty, EnumFieldInit)>,
}

/// L2d 打印值树：结构在编译期确定；运行期整型叶子引用已求值的局部。
/// 可打印复合全部是 Copy，树不承载所有权／drop 语义。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int { ty: IntTy, expr: IntExpr },
    Bytes(Vec<u8>),
    Tuple(Vec<Value>),
    Array(Vec<Value>),
    Struct { name: String, fields: Vec<(String, Value)> },
    Enum {
        variant: String,
        payload: Vec<Value>,
        /// L2v：可物化配方（构造时生成；常量树来源为 None）。
        init: Option<EnumInit>,
    },
}

/// L2c 打印实参：字符串／字节串、三宽度运行期整型、编译期格式化字节。
/// L2d 增加复合值树；标量分支保持零回退。
#[derive(Clone, Debug)]
pub enum PrintArg {
    Str(StrArg),
    Int { ty: IntTy, expr: IntExpr },
    /// L2g：五新宽度的运行期值（直接 `println(*b)` 或其副本）。
    /// 三型继续走 `Int`，保证既有 C 文本逐字节零漂移。
    BoxInt { ty: BoxIntTy, local: LocalId },
    /// 布尔／byte／unit／窄整型／浮点的契约格式化结果（编译期确定）。
    Bytes(Vec<u8>),
    Composite(Value),
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
    /// `let x = box <int>;`：新建 Box 载荷（emit：malloc；`ty` 为载荷类型）。
    /// L2k：`ty` 可为复合载荷类型，`value` 为结构化常量树。
    BindBox {
        local: LocalId,
        ty: Ty,
        value: BoxConst,
        line: usize,
        col: usize,
    },
    /// `x = box <int>;`：重初始化；`drop_dst` 由 lower 静态判定（emit：必要时先 free）。
    BindBoxAssign {
        local: LocalId,
        ty: Ty,
        value: BoxConst,
        drop_dst: bool,
        line: usize,
        col: usize,
    },
    /// `let v = *b;`：读 Box 载荷（Copy 结果）；宽度由 `local` 的 `Ty` 决定。
    BindDeref {
        local: LocalId,
        src: LocalId,
        line: usize,
        col: usize,
    },
    /// L2g：五新宽度的运行期副本（`let w = v`；v 来自 `*b`）。
    BindBoxIntCopy {
        dst: LocalId,
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
    /// L2k：复合载荷 typedef（首次使用序，确定性输出）。
    pub payload_defs: Vec<PayloadDef>,
}
