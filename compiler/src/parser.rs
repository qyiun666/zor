use crate::diag::Diag;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Prim(&'static str),
    Named(String),
    Arr(Box<Ty>, u64),
    Tuple(Vec<Ty>),
    Ref(Box<Ty>, bool), // (inner, is_mut)
    BoxTy(Box<Ty>),     // R2 §5 内建堆句柄 Box<T>（具名内建，非泛型语法开放）
    Bad,
}

/// 类型标注位置（决定引用禁则诊断码：容器=E1501，返回值=E1502，R2 §4）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TyPos { Normal, Container, Return }

impl Ty {
    pub fn unit() -> Ty { Ty::Prim("unit") }
    pub fn is_int(&self) -> bool { matches!(self, Ty::Prim(s) if matches!(*s, "i8"|"i16"|"i32"|"i64"|"u8"|"u16"|"u32"|"u64"|"byte")) }
    pub fn is_float(&self) -> bool { matches!(self, Ty::Prim(s) if matches!(*s, "f32"|"f64")) }
    pub fn is_num(&self) -> bool { self.is_int() || self.is_float() }
    pub fn is_bool(&self) -> bool { matches!(self, Ty::Prim("bool")) }
    pub fn display(&self) -> String {
        match self {
            Ty::Prim(s) => s.to_string(),
            Ty::Named(n) => n.clone(),
            Ty::Arr(t, k) => format!("[{}; {}]", t.display(), k),
            Ty::Tuple(ts) => format!("({})", ts.iter().map(|t| t.display()).collect::<Vec<_>>().join(", ")),
            Ty::Ref(t, true) => format!("&mut {}", t.display()),
            Ty::Ref(t, false) => format!("&{}", t.display()),
            Ty::BoxTy(t) => format!("Box<{}>", t.display()),
            Ty::Bad => "?".to_string(),
        }
    }
}

/// 字面量在推断前以 (Lit kind, span) 表达，check 时按期望收敛。
/// L2q：`IntConst` 为具名整型常量成员（有符号、编译期常量），语义等同整数字面量；
/// 与 `Int(u128)` 分列以承载 `MIN_EXP` 的负值，且不改变既有无符号字面量口径。
#[derive(Clone, Debug, PartialEq)]
pub enum Lit { Int(u128), IntConst(i128), Float(f64), Str(String), Bool(bool) }

/// L2h：特殊浮点值的宽度由源形态自带（宽度严格，禁跨宽推断）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatWidth { F32, F64 }

/// L2h：三个特殊值形态；L2o：追加三个命名有限常量（`MAX`／`MIN`／`EPSILON`）；
/// L2p：追加 `MIN_POSITIVE`（最小正正规数）。NaN 载荷／信号 NaN 不在面内。
/// L2q：十个整型成员（`MAX_EXP`／`MIN_EXP`／`MANTISSA_DIGITS`／`DIGITS`／`RADIX` × f32／f64）
/// 不建本枚举，解析为 `Lit::IntConst`（整数字面量语义）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatSpecialKind { Nan, Infinity, NegInfinity, Max, Min, Epsilon, MinPositive }

impl FloatWidth {
    pub fn ty(self) -> Ty {
        Ty::Prim(match self { FloatWidth::F32 => "f32", FloatWidth::F64 => "f64" })
    }
}

#[derive(Clone, Debug)]
pub enum Ex {
    Lit(Lit, usize, usize),
    Path(String, usize, usize),
    UnitLit(usize, usize),
    /// L2h：`f32|f64::NAN|INFINITY|NEG_INFINITY`（六形式；宽度随形态携带）。
    FloatSpecial(FloatWidth, FloatSpecialKind, usize, usize),
    Bin(String, Box<Ex>, Box<Ex>, usize, usize),
    Un(String, Box<Ex>, usize, usize),
    Call(Box<Ex>, Vec<Ex>, usize, usize),
    Method(Box<Ex>, String, Vec<Ex>, usize, usize),
    Index(Box<Ex>, Box<Ex>, usize, usize),
    Field(Box<Ex>, String, usize, usize),
    Record(String, Vec<(String, Ex)>, usize, usize),
    Tup(Vec<Ex>, usize, usize),
    ArrLit(Vec<Ex>, usize, usize),
    Ref(Box<Ex>, bool, usize, usize), // &x / &mut x（R1 仅接受并记类型，R2 查借用规则）
    Box(Box<Ex>, usize, usize),       // box expr（R2 §5，移动入堆句柄）
    Deref(Box<Ex>, usize, usize),     // *expr（R2 §5 解引用读写）
    Unbox(String, usize, usize),      // unbox b（R2 §5 释放/交出所有权）
    Block(Blk),
}

impl Ex {
    /// 表达式起始位置（1-based 行／列）；type table 以位置＋类别作键。
    pub fn span(&self) -> (usize, usize) {
        match self {
            Ex::Lit(_, l, c)
            | Ex::Path(_, l, c)
            | Ex::UnitLit(l, c)
            | Ex::FloatSpecial(_, _, l, c)
            | Ex::Bin(_, _, _, l, c)
            | Ex::Un(_, _, l, c)
            | Ex::Call(_, _, l, c)
            | Ex::Method(_, _, _, l, c)
            | Ex::Index(_, _, l, c)
            | Ex::Field(_, _, l, c)
            | Ex::Record(_, _, l, c)
            | Ex::Tup(_, l, c)
            | Ex::ArrLit(_, l, c)
            | Ex::Ref(_, _, l, c)
            | Ex::Box(_, l, c)
            | Ex::Deref(_, l, c)
            | Ex::Unbox(_, l, c) => (*l, *c),
            Ex::Block(b) => b.span,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Blk {
    pub stmts: Vec<St>,
    pub tail: Option<Box<Ex>>,
    pub span: (usize, usize),
}

#[derive(Clone, Debug)]
pub enum St {
    Let(String, Option<Ty>, Ex, usize, usize),
    LetDecl(String, Option<Ty>, usize, usize), // 无初始化：0.1.x 不允许 → E1601（由 check 报）
    Assign(Ex, Ex, usize, usize),              // lhs = rhs;（R2 §1 移动点之一）
    Expr(Option<Ex>, usize, usize),
    If(Ex, Blk, Option<ElseTail>, usize, usize),
    While(Ex, Blk, usize, usize),
}

#[derive(Clone, Debug)]
pub enum ElseTail {
    Block(Blk),
    If(Box<St>),
}

#[derive(Debug)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<(String, Ty, usize, usize)>,
    pub ret: Option<(Ty, usize, usize)>,
    pub body: Blk,
    pub span: (usize, usize),
}

#[derive(Debug)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<(String, Ty, usize, usize)>,
    pub span: (usize, usize),
}

#[derive(Debug)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<(String, Vec<(Ty, usize, usize)>, usize, usize)>, // (变体名, 载荷, line, col)
    pub span: (usize, usize),
}

#[derive(Debug)]
pub struct Prog {
    pub fns: Vec<FnDecl>,
    pub structs: Vec<StructDecl>,
    pub enums: Vec<EnumDecl>,
}

pub struct Parser {
    toks: Vec<(crate::lexer::Tok, usize, usize)>,
    pos: usize,
    pub diags: Vec<Diag>,
    /// 条件表达式解析中=true：抑制 Path+LBrace 的 record 糖，避免吞掉 if/while 块头
    no_record: bool,
}

use crate::lexer::Tok;

pub fn prim_names() -> Vec<&'static str> {
    vec!["i8","i16","i32","i64","u8","u16","u32","u64","f32","f64","bool","byte","unit"]
}

impl Parser {
    pub fn new(toks: Vec<(Tok, usize, usize)>, diags: Vec<Diag>) -> Self {
        Parser { toks, pos: 0, diags, no_record: false }
    }
    fn peek(&self) -> &Tok { &self.toks[self.pos].0 }
    fn peek_at(&self, n: usize) -> &Tok { &self.toks[(self.pos + n).min(self.toks.len() - 1)].0 }
    fn at(&self) -> (usize, usize) { (self.toks[self.pos].1, self.toks[self.pos].2) }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].0.clone();
        if self.pos + 1 < self.toks.len() { self.pos += 1; }
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t { self.bump(); true } else { false }
    }
    fn expect(&mut self, t: &Tok, what: &str) -> bool {
        if self.eat(t) { true }
        else {
            let (l, c) = self.at();
            self.diags.push(Diag::new("E1101", l, c, format!("expected {}, found {:?}", what, self.peek())));
            false
        }
    }
    fn diag(&mut self, code: &str, msg: impl Into<String>) {
        let (l, c) = self.at();
        self.diags.push(Diag::new(code, l, c, msg));
    }
    fn diag_at(&mut self, code: &str, l: usize, c: usize, msg: impl Into<String>) {
        self.diags.push(Diag::new(code, l, c, msg));
    }
    /// L2h／L2o／L2p／L2q：`::` 支持六个特殊值、八个命名有限浮点常量与十个整型成员
    /// （共 24 形式）；其余成员形态统一 E1101。单一发点，便于 e1101_catalog 守卫只记 +1。
    fn err_float_member(&mut self, l: usize, c: usize, msg: impl Into<String>) {
        self.diag_at("E1101", l, c, msg);
    }

    pub fn parse_program(&mut self) -> Prog {
        let mut p = Prog { fns: vec![], structs: vec![], enums: vec![] };
        while !matches!(self.peek(), Tok::Eof) {
            if let Tok::Ident(k) = self.peek().clone() {
                match k.as_str() {
                    "fn" => { let f = self.parse_fn(); p.fns.push(f); }
                    "struct" => { let s = self.parse_struct(); p.structs.push(s); }
                    "enum" => { let e = self.parse_enum(); p.enums.push(e); }
                    _ => { self.diag("E1101", "expected item (fn/struct/enum)"); self.bump(); }
                }
            } else {
                self.diag("E1101", "expected item (fn/struct/enum)");
                self.bump();
            }
        }
        p
    }

    fn parse_fn(&mut self) -> FnDecl {
        let span = self.at();
        self.bump(); // fn
        let name = match self.bump() { Tok::Ident(s) => s, t => { self.diag("E1101", format!("fn name expected, found {:?}", t)); "_err".to_string() } };
        // 泛型语法禁则：fn name<...>
        if matches!(self.peek(), Tok::Lt) {
            self.diag("E1501", "0.1.x 禁泛型/生命周期尖括号语法（无静态参数）");
            self.skip_angle();
        }
        self.expect(&Tok::LParen, "'(' for fn params");
        let mut params = Vec::new();
        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
            let (l, c) = self.at();
            let pname = match self.bump() { Tok::Ident(s) => s, t => { self.diag("E1101", format!("param name expected, found {:?}", t)); "_p".to_string() } };
            self.expect(&Tok::Colon, "':' after param name");
            let ty = self.parse_ty(TyPos::Normal);
            params.push((pname, ty, l, c));
            if !self.eat(&Tok::Comma) { break; }
        }
        self.expect(&Tok::RParen, "')' closing params");
        let ret = if self.eat(&Tok::FatArrow) {
            let (l, c) = self.at();
            let t = self.parse_ty(TyPos::Return); // 返回位引用=E1502（R2 §4）
            Some((t, l, c))
        } else { None };
        let body = self.parse_block();
        FnDecl { name, params, ret, body, span }
    }

    fn parse_struct(&mut self) -> StructDecl {
        let span = self.at();
        self.bump(); // struct
        let name = match self.bump() { Tok::Ident(s) => s, _ => { self.diag("E1101", "struct name expected"); "_err".to_string() } };
        if matches!(self.peek(), Tok::Lt) { self.diag("E1501", "0.1.x 禁泛型尖括号"); self.skip_angle(); }
        self.expect(&Tok::LBrace, "'{' for struct body");
        let mut fields = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            let (l, c) = self.at();
            let fname = match self.bump() { Tok::Ident(s) => s, t => { self.diag("E1101", format!("field name expected, found {:?}", t)); "_f".to_string() } };
            self.expect(&Tok::Colon, "':' after field name");
            let ty = self.parse_ty(TyPos::Container); // 容器位：引用→E1501
            fields.push((fname, ty, l, c));
            // 0.1.x 语法未定死字段分隔符：';' 与 ',' 均接受（spec OI 项，黄金文件前定论）
            let sep = self.eat(&Tok::Semi) || self.eat(&Tok::Comma);
            if !sep && !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
                self.expect(&Tok::Semi, "';' or ',' after field");
            }
        }
        self.expect(&Tok::RBrace, "'}' closing struct");
        StructDecl { name, fields, span }
    }

    fn parse_enum(&mut self) -> EnumDecl {
        let span = self.at();
        self.bump(); // enum
        let name = match self.bump() { Tok::Ident(s) => s, _ => { self.diag("E1101", "enum name expected"); "_err".to_string() } };
        if matches!(self.peek(), Tok::Lt) { self.diag("E1501", "0.1.x 禁泛型尖括号"); self.skip_angle(); }
        self.expect(&Tok::LBrace, "'{' for enum body");
        let mut variants = Vec::new();
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            let (l, c) = self.at();
            let vname = match self.bump() { Tok::Ident(s) => s, t => { self.diag("E1101", format!("variant name expected, found {:?}", t)); "_v".to_string() } };
            let mut payload = Vec::new();
            if self.eat(&Tok::LParen) {
                while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
                    let (pl, pc) = self.at();
                    let ty = self.parse_ty(TyPos::Container); // 载荷=容器位
                    payload.push((ty, pl, pc));
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect(&Tok::RParen, "')' closing variant payload");
            }
            variants.push((vname, payload, l, c));
            self.eat(&Tok::Comma);
            if self.eat(&Tok::Semi) { /* 宽松容忍分号分隔 */ }
        }
        self.expect(&Tok::RBrace, "'}' closing enum");
        EnumDecl { name, variants, span }
    }

    fn skip_angle(&mut self) {
        // 跳过匹配的 <...>（已报禁则），容错不深入
        let mut depth = 0;
        loop {
            match self.peek() {
                Tok::Lt => { depth += 1; self.bump(); }
                Tok::Gt => { depth -= 1; self.bump(); if depth <= 0 { break; } }
                Tok::Eof => break,
                _ => { self.bump(); }
            }
        }
    }

    /// pos 决定禁则：Container=禁引用字段/载荷（E1501）；Return=禁返回引用（E1502）
    fn parse_ty(&mut self, pos: TyPos) -> Ty {
        let (l, c) = self.at();
        match self.peek().clone() {
            Tok::Amp => {
                self.bump();
                let mut mutf = false;
                if let Tok::Ident(s) = self.peek() { if s.as_str() == "mut" { self.bump(); mutf = true; } }
                let inner = self.parse_ty(pos);
                match pos {
                    TyPos::Return => {
                        self.diag_at("E1502", l, c, "返回类型不得含引用（R2 §4 禁引用逃逸）");
                        Ty::Bad
                    }
                    TyPos::Container => {
                        self.diag_at("E1501", l, c, "引用类型不得出现在容器位置（0.1.x 禁逃逸）");
                        Ty::Bad
                    }
                    TyPos::Normal => Ty::Ref(Box::new(inner), mutf),
                }
            }
            Tok::Star => {
                self.bump();
                self.diag_at("E1501", l, c, "裸指针类型非法（FFI 仅 Handle<T>，0.1.x）");
                if let Tok::Ident(s) = self.peek() { if s == "const" || s == "mut" { self.bump(); } }
                let _ = self.parse_ty(pos);
                Ty::Bad
            }
            Tok::Apost => {
                self.bump();
                self.diag_at("E1501", l, c, "生命周期标注语法在 0.1.x 不存在");
                if let Tok::Ident(_) = self.peek() { self.bump(); }
                Ty::Bad
            }
            Tok::LBracket => {
                self.bump();
                let elem = self.parse_ty(TyPos::Container); // 数组元素=容器位
                self.expect(&Tok::Semi, "';' in array type");
                let (nl, nc) = self.at();
                let n = match self.peek().clone() {
                    Tok::Int(v) => { self.bump(); v }
                    Tok::Ident(name) => { self.bump(); self.diag_at("E1301", nl, nc, format!("数组长度必须是编译期整数字面量，found '{}'", name)); 0 }
                    t => { self.diag_at("E1301", nl, nc, format!("数组长度必须是编译期整数字面量，found {:?}", t)); self.bump(); 0 }
                };
                self.expect(&Tok::RBracket, "']' closing array type");
                if n > u64::MAX as u128 { self.diag_at("E1301", nl, nc, "数组长度越界"); }
                Ty::Arr(Box::new(elem), n as u64)
            }
            Tok::LParen => {
                self.bump();
                if self.eat(&Tok::RParen) { return Ty::unit(); }
                let mut ts = Vec::new();
                loop {
                    ts.push(self.parse_ty(pos));
                    if !self.eat(&Tok::Comma) { break; }
                    if matches!(self.peek(), Tok::RParen) { break; }
                }
                self.expect(&Tok::RParen, "')' closing tuple type");
                if ts.len() == 1 { ts.pop().unwrap() } else { Ty::Tuple(ts) }
            }
            Tok::Ident(name) => {
                self.bump();
                if matches!(self.peek(), Tok::Lt) {
                    if name == "Box" {
                        // R2 §5 内建堆句柄：唯一允许尖括号的具名类型（泛型语法仍不开放）
                        self.bump(); // <
                        let inner = self.parse_ty(TyPos::Container); // Box 内禁引用
                        self.expect(&Tok::Gt, "'>' closing Box type");
                        return Ty::BoxTy(Box::new(inner));
                    }
                    self.diag_at("E1501", l, c, "0.1.x 无泛型类型参数（具名类型禁尖括号）");
                    self.skip_angle();
                }
                if prim_names().contains(&name.as_str()) {
                    Ty::Prim(prim_names().into_iter().find(|p| *p == name.as_str()).unwrap())
                } else if name == "Self" {
                    self.diag_at("E1101", l, c, "0.1.x 无 Self 类型");
                    Ty::Bad
                } else {
                    Ty::Named(name)
                }
            }
            t => {
                self.diag_at("E1101", l, c, format!("malformed type, found {:?}", t));
                self.bump();
                Ty::Bad
            }
        }
    }

    pub fn parse_block(&mut self) -> Blk {
        let span = self.at();
        self.expect(&Tok::LBrace, "'{' opening block");
        let mut blk = Blk { stmts: vec![], tail: None, span: (span.0, span.1) };
        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            let (l, c) = self.at();
            // 裸块语句：{ ... }（R2 §2 失活边界/结构手段）
            if matches!(self.peek(), Tok::LBrace) {
                let inner = self.parse_block();
                blk.stmts.push(St::Expr(Some(Ex::Block(inner)), l, c));
                continue;
            }
            if let Tok::Ident(k) = self.peek().clone() {
                match k.as_str() {
                    "let" => {
                        self.bump();
                        let n = match self.bump() { Tok::Ident(s) => s, t => { self.diag_at("E1101", l, c, format!("let binding name expected, found {:?}", t)); "_".to_string() } };
                        let ann = if self.eat(&Tok::Colon) { Some(self.parse_ty(TyPos::Normal)) } else { None };
                        if self.eat(&Tok::Eq) {
                            let init = self.parse_expr();
                            if !self.eat(&Tok::Semi) { self.diag_at("E1101", l, c, "expected ';' after let"); }
                            blk.stmts.push(St::Let(n, ann, init, l, c));
                        } else {
                            if self.eat(&Tok::Semi) { /* 无初始化形式 */ }
                            blk.stmts.push(St::LetDecl(n, ann, l, c));
                        }
                        continue;
                    }
                    "if" => {
                        self.bump();
                        let cond = self.parse_cond();
                        let then = self.parse_block();
                        let els = self.parse_else();
                        blk.stmts.push(St::If(cond, then, els, l, c));
                        continue;
                    }
                    "while" => {
                        self.bump();
                        let cond = self.parse_cond();
                        let body = self.parse_block();
                        if self.eat(&Tok::Semi) { /* 宽松 */ }
                        blk.stmts.push(St::While(cond, body, l, c));
                        continue;
                    }
                    _ => {}
                }
            }
            let e = self.parse_expr();
            if matches!(self.peek(), Tok::Eq) {
                // 赋值语句：lhs = rhs;（R2 §1 移动点；lhs 由 check 判定可放置）
                self.bump();
                let v = self.parse_expr();
                // 块尾赋值可省分号（尾表达式形态：`fn bump(x: &mut i64) { *x = 1 }`）
                if !self.eat(&Tok::Semi) && !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
                    self.diag_at("E1101", l, c, "expected ';' after assignment");
                }
                blk.stmts.push(St::Assign(e, v, l, c));
                continue;
            }
            let semied = self.eat(&Tok::Semi);
            if !semied {
                // 表达式无分号且未到块尾 → 作为尾表达式
                blk.tail = Some(Box::new(e));
                break;
            }
            blk.stmts.push(St::Expr(Some(e), l, c));
        }
        self.expect(&Tok::RBrace, "'}' closing block");
        blk
    }

    fn eat_else(&mut self) -> bool {
        if let Tok::Ident(s) = self.peek() { if s.as_str() == "else" { self.bump(); return true; } }
        false
    }

    fn parse_else(&mut self) -> Option<ElseTail> {
        if !self.eat_else() { return None; }
        if let Tok::Ident(s) = self.peek() {
            if s.as_str() == "if" {
                let (l, c) = self.at();
                return Some(ElseTail::If(Box::new(self.parse_if_stmt(l, c))));
            }
        }
        Some(ElseTail::Block(self.parse_block()))
    }

    fn parse_if_stmt(&mut self, l: usize, c: usize) -> St {
        self.bump(); // if
        let cond = self.parse_cond();
        let then = self.parse_block();
        let els = self.parse_else();
        St::If(cond, then, els, l, c)
    }

    /// if/while 条件：表达式语法同 parse_expr，但 Path 后 `{` 不当 record 字面量
    fn parse_cond(&mut self) -> Ex {
        let saved = self.no_record;
        self.no_record = true;
        let e = self.parse_expr();
        self.no_record = saved;
        e
    }

    pub fn parse_expr(&mut self) -> Ex { self.p_or() }

    fn p_or(&mut self) -> Ex {
        let mut l = self.p_and();
        while matches!(self.peek(), Tok::PipePipe) { let (lc, cc) = self.at(); self.bump(); let r = self.p_and(); l = Ex::Bin("||".into(), Box::new(l), Box::new(r), lc, cc); }
        l
    }
    fn p_and(&mut self) -> Ex {
        let mut l = self.p_cmp();
        while matches!(self.peek(), Tok::AmpAmp) { let (lc, cc) = self.at(); self.bump(); let r = self.p_cmp(); l = Ex::Bin("&&".into(), Box::new(l), Box::new(r), lc, cc); }
        l
    }
    fn p_cmp(&mut self) -> Ex {
        let l = self.p_add();
        let op = match self.peek() { Tok::EqEq => "==", Tok::NotEq => "!=", Tok::Lt => "<", Tok::Le => "<=", Tok::Gt => ">", Tok::Ge => ">=", _ => return l };
        let (lc, cc) = self.at(); self.bump(); let r = self.p_add();
        Ex::Bin(op.into(), Box::new(l), Box::new(r), lc, cc)
    }
    fn p_add(&mut self) -> Ex {
        let mut l = self.p_mul();
        loop {
            let op = match self.peek() { Tok::Plus => "+", Tok::Minus => "-", _ => break };
            let (lc, cc) = self.at(); self.bump(); let r = self.p_mul();
            l = Ex::Bin(op.into(), Box::new(l), Box::new(r), lc, cc);
        }
        l
    }
    fn p_mul(&mut self) -> Ex {
        let mut l = self.p_unary();
        loop {
            let op = match self.peek() { Tok::Star => "*", Tok::Slash => "/", Tok::Percent => "%", _ => break };
            let (lc, cc) = self.at(); self.bump(); let r = self.p_unary();
            l = Ex::Bin(op.into(), Box::new(l), Box::new(r), lc, cc);
        }
        l
    }
    fn p_unary(&mut self) -> Ex {
        let (l, c) = self.at();
        match self.peek() {
            Tok::Minus => { self.bump(); Ex::Un("-".into(), Box::new(self.p_unary()), l, c) }
            Tok::Bang => { self.bump(); Ex::Un("!".into(), Box::new(self.p_unary()), l, c) }
            Tok::Star => { self.bump(); Ex::Deref(Box::new(self.p_unary()), l, c) } // R2 §5 解引用
            Tok::Amp => {
                self.bump();
                let mut mutf = false;
                if let Tok::Ident(s) = self.peek() { if s.as_str() == "mut" { self.bump(); mutf = true; } }
                let inner = self.p_postfix();
                Ex::Ref(Box::new(inner), mutf, l, c)
            }
            Tok::Ident(s) if s.as_str() == "box" => {
                self.bump();
                Ex::Box(Box::new(self.p_unary()), l, c) // R2 §5 前缀构造
            }
            Tok::Ident(s) if s.as_str() == "unbox" => {
                self.bump();
                let (nl, nc) = self.at();
                match self.bump() {
                    Tok::Ident(n) => Ex::Unbox(n, l, c), // R2 §5 释放/交出所有权
                    t => { self.diag_at("E1101", nl, nc, format!("unbox 要求句柄名，found {:?}", t)); Ex::Unbox("_err".to_string(), l, c) }
                }
            }
            _ => self.p_postfix(),
        }
    }
    fn p_postfix(&mut self) -> Ex {
        let mut e = self.p_atom();
        loop {
            let (l, c) = self.at();
            match self.peek() {
                Tok::LParen => {
                    self.bump();
                    let mut args = Vec::new();
                    while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
                        args.push(self.parse_expr());
                        if !self.eat(&Tok::Comma) { break; }
                    }
                    self.expect(&Tok::RParen, "')' closing call args");
                    e = Ex::Call(Box::new(e), args, l, c);
                }
                Tok::LBracket => {
                    self.bump();
                    let idx = self.parse_expr();
                    self.expect(&Tok::RBracket, "']' closing index");
                    e = Ex::Index(Box::new(e), Box::new(idx), l, c);
                }
                Tok::Dot => {
                    self.bump();
                    let (dl, dc) = self.at();
                    let name = match self.bump() { Tok::Ident(s) => s, t => { self.diag_at("E1101", dl, dc, format!("after '.', expected method/field, found {:?}", t)); "_".to_string() } };
                    if matches!(self.peek(), Tok::LParen) {
                        self.bump();
                        let mut args = Vec::new();
                        while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
                            args.push(self.parse_expr());
                            if !self.eat(&Tok::Comma) { break; }
                        }
                        self.expect(&Tok::RParen, "')' closing method args");
                        e = Ex::Method(Box::new(e), name, args, l, c);
                    } else {
                        e = Ex::Field(Box::new(e), name, l, c);
                    }
                }
                Tok::LBrace => {
                    if self.no_record { break; }
                    if let Ex::Path(name, _, _) = &e {
                        let name = name.clone();
                        self.bump();
                        let mut fields = Vec::new();
                        while !matches!(self.peek(), Tok::RBrace | Tok::Eof) {
                            let (fl, fc) = self.at();
                            let fname = match self.bump() { Tok::Ident(s) => s, t => { self.diag_at("E1101", fl, fc, format!("record field expected, found {:?}", t)); "_".to_string() } };
                            self.expect(&Tok::Colon, "':' in record literal");
                            let v = self.parse_expr();
                            fields.push((fname, v));
                            if !self.eat(&Tok::Comma) { break; }
                        }
                        self.expect(&Tok::RBrace, "'}' closing record");
                        e = Ex::Record(name, fields, l, c);
                    } else { break; }
                }
                _ => break,
            }
        }
        e
    }
    fn p_atom(&mut self) -> Ex {
        let (l, c) = self.at();
        match self.bump() {
            Tok::Int(v) => Ex::Lit(Lit::Int(v), l, c),
            Tok::Float(v) => Ex::Lit(Lit::Float(v), l, c),
            Tok::Str(s) => Ex::Lit(Lit::Str(s), l, c),
            Tok::Ident(s) if s == "true" => Ex::Lit(Lit::Bool(true), l, c),
            Tok::Ident(s) if s == "false" => Ex::Lit(Lit::Bool(false), l, c),
            Tok::Ident(s) if s == "mut" => { self.diag_at("E1101", l, c, "'mut' 仅用于 &mut 位置"); Ex::Lit(Lit::Bool(true), l, c) }
            // L2h／L2o／L2p／L2q：`f32|f64::MEMBER` 双冒号前瞻（不改 lexer、不新增关键字）。
            // 六个特殊值与八个命名有限浮点常量成为 FloatSpecial；十个整型成员成为
            // `Lit::IntConst`（整数字面量语义）；其余 `::` 成员一律 E1101。
            Tok::Ident(s)
                if (s == "f32" || s == "f64")
                    && matches!(self.peek(), Tok::Colon)
                    && matches!(self.peek_at(1), Tok::Colon) =>
            {
                let width = if s == "f32" { FloatWidth::F32 } else { FloatWidth::F64 };
                self.bump();
                self.bump();
                let (ml, mc) = self.at();
                let member = match self.bump() {
                    Tok::Ident(m) => m,
                    t => {
                        self.err_float_member(ml, mc, format!("'{}::' 后需成员名，found {:?}", s, t));
                        return Ex::Path(s, l, c);
                    }
                };
                // L2q ③b：五个整型成员 × f32／f64（值由产品裁定件 §2 锁定，与 rustc 同名常量一致）。
                let int_value = match (s.as_str(), member.as_str()) {
                    ("f64", "MAX_EXP") => Some(1024),
                    ("f64", "MIN_EXP") => Some(-1021),
                    ("f64", "MANTISSA_DIGITS") => Some(53),
                    ("f64", "DIGITS") => Some(15),
                    ("f64", "RADIX") => Some(2),
                    ("f32", "MAX_EXP") => Some(128),
                    ("f32", "MIN_EXP") => Some(-125),
                    ("f32", "MANTISSA_DIGITS") => Some(24),
                    ("f32", "DIGITS") => Some(6),
                    ("f32", "RADIX") => Some(2),
                    _ => None,
                };
                if let Some(v) = int_value {
                    if matches!(self.peek(), Tok::LParen) {
                        let (cl, cc) = self.at();
                        self.err_float_member(
                            cl,
                            cc,
                            "特殊值／命名常量不是函数，不可调用（NaN 载荷不在支持面内）",
                        );
                    }
                    return Ex::Lit(Lit::IntConst(v), l, c);
                }
                let kind = match member.as_str() {
                    "NAN" => Some(FloatSpecialKind::Nan),
                    "INFINITY" => Some(FloatSpecialKind::Infinity),
                    "NEG_INFINITY" => Some(FloatSpecialKind::NegInfinity),
                    "MAX" => Some(FloatSpecialKind::Max),
                    "MIN" => Some(FloatSpecialKind::Min),
                    "EPSILON" => Some(FloatSpecialKind::Epsilon),
                    "MIN_POSITIVE" => Some(FloatSpecialKind::MinPositive),
                    _ => None,
                };
                match kind {
                    Some(k) => {
                        if matches!(self.peek(), Tok::LParen) {
                            let (cl, cc) = self.at();
                            self.err_float_member(
                                cl,
                                cc,
                                "特殊值／命名常量不是函数，不可调用（NaN 载荷不在支持面内）",
                            );
                        }
                        Ex::FloatSpecial(width, k, l, c)
                    }
                    None => {
                        self.err_float_member(
                            ml,
                            mc,
                            format!(
                                "'{}::{}' 不在支持面（支持六特殊值＋八命名有限浮点常量＋十整型成员，共 24 形式）",
                                s, member
                            ),
                        );
                        Ex::Path(s, l, c)
                    }
                }
            }
            Tok::Ident(s) => Ex::Path(s, l, c),
            Tok::LParen => {
                let mut es = Vec::new();
                let mut first_col = true;
                while !matches!(self.peek(), Tok::RParen | Tok::Eof) {
                    es.push(self.parse_expr());
                    first_col = false;
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect(&Tok::RParen, "')' closing expression");
                let _ = first_col;
                if es.is_empty() { Ex::UnitLit(l, c) } else if es.len() == 1 { es.pop().unwrap() } else { Ex::Tup(es, l, c) }
            }
            Tok::LBracket => {
                let mut es = Vec::new();
                while !matches!(self.peek(), Tok::RBracket | Tok::Eof) {
                    es.push(self.parse_expr());
                    if !self.eat(&Tok::Comma) { break; }
                }
                self.expect(&Tok::RBracket, "']' closing array literal");
                Ex::ArrLit(es, l, c)
            }
            t => {
                self.diag_at("E1101", l, c, format!("unexpected token in expression: {:?}", t));
                Ex::Lit(Lit::Bool(true), l, c)
            }
        }
    }
}

