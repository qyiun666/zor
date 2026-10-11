#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(u128),
    Float(f64),
    Str(String),
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    Comma, Semi, Colon, Dot,
    FatArrow, // ->
    Eq,       // =
    Plus, Minus, Star, Slash, Percent,
    EqEq, NotEq, Lt, Le, Gt, Ge,
    AmpAmp, PipePipe, Amp, Bang,
    Apost, // ' (lifetime syntax — forbidden in 0.1.x)
    Eof,
}

pub type Spanned = (Tok, usize, usize); // tok, line, col(start of token)

pub fn lex(src: &str, diags: &mut Vec<crate::diag::Diag>) -> Vec<Spanned> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut col = 1usize;
    let mut out: Vec<Spanned> = Vec::new();

    while i < b.len() {
        let c = b[i];
        if c == '\n' { i += 1; line += 1; col = 1; continue; }
        if c.is_whitespace() { i += 1; col += 1; continue; }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' { i += 1; col += 1; }
            continue;
        }
        let sc = col;
        if c.is_alphabetic() || c == '_' {
            let mut s = String::new();
            while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                s.push(b[i]); i += 1; col += 1;
            }
            out.push((Tok::Ident(s), line, sc));
            continue;
        }
        if c.is_ascii_digit() {
            let mut s = String::new();
            let mut isf = false;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '_') {
                if b[i] != '_' { s.push(b[i]); }
                i += 1; col += 1;
            }
            if i < b.len() && b[i] == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
                isf = true; s.push('.'); i += 1; col += 1;
                while i < b.len() && b[i].is_ascii_digit() { s.push(b[i]); i += 1; col += 1; }
            }
            if i < b.len() && (b[i] == 'e' || b[i] == 'E') {
                isf = true; s.push('e'); i += 1; col += 1;
                if i < b.len() && (b[i] == '+' || b[i] == '-') { s.push(b[i]); i += 1; col += 1; }
                while i < b.len() && b[i].is_ascii_digit() { s.push(b[i]); i += 1; col += 1; }
            }
            if isf {
                out.push((Tok::Float(s.parse::<f64>().unwrap_or(0.0)), line, sc));
            } else {
                out.push((Tok::Int(s.parse::<u128>().unwrap_or(u128::MAX)), line, sc));
            }
            continue;
        }
        if c == '"' {
            i += 1; col += 1;
            let mut s = String::new();
            let mut closed = false;
            while i < b.len() {
                if b[i] == '"' { closed = true; i += 1; col += 1; break; }
                if b[i] == '\\' && i + 1 < b.len() {
                    i += 1; col += 1;
                    match b[i] { 'n' => s.push('\n'), 't' => s.push('\t'), o => s.push(o) }
                } else { s.push(b[i]); }
                i += 1; col += 1;
            }
            if !closed {
                diags.push(crate::diag::Diag::new("E1101", line, sc, "unterminated string literal"));
            }
            out.push((Tok::Str(s), line, sc));
            continue;
        }
        if c == '&' && i + 1 < b.len() && b[i + 1] == '&' {
            out.push((Tok::AmpAmp, line, sc));
            i += 2; col += 2;
            continue;
        }
        if c == '|' {
            if i + 1 < b.len() && b[i + 1] == '|' {
                out.push((Tok::PipePipe, line, sc));
                i += 2; col += 2;
            } else {
                diags.push(crate::diag::Diag::new("E1101", line, sc, "illegal character '|'"));
                i += 1; col += 1;
            }
            continue;
        }
        let (t, adv): (Tok, usize) = match c {
            '(' => (Tok::LParen, 1), ')' => (Tok::RParen, 1),
            '{' => (Tok::LBrace, 1), '}' => (Tok::RBrace, 1),
            '[' => (Tok::LBracket, 1), ']' => (Tok::RBracket, 1),
            ',' => (Tok::Comma, 1), ';' => (Tok::Semi, 1), ':' => (Tok::Colon, 1),
            '.' => (Tok::Dot, 1), '\'' => (Tok::Apost, 1),
            '+' => (Tok::Plus, 1), '*' => (Tok::Star, 1), '/' => (Tok::Slash, 1),
            '%' => (Tok::Percent, 1), '&' => (Tok::Amp, 1),
            '=' => if i + 1 < b.len() && b[i + 1] == '=' { (Tok::EqEq, 2) } else { (Tok::Eq, 1) },
            '!' => if i + 1 < b.len() && b[i + 1] == '=' { (Tok::NotEq, 2) } else { (Tok::Bang, 1) },
            '<' => if i + 1 < b.len() && b[i + 1] == '=' { (Tok::Le, 2) } else { (Tok::Lt, 1) },
            '>' => if i + 1 < b.len() && b[i + 1] == '=' { (Tok::Ge, 2) } else { (Tok::Gt, 1) },
            '-' => if i + 1 < b.len() && b[i + 1] == '>' { (Tok::FatArrow, 2) } else { (Tok::Minus, 1) },
            _ => {
                diags.push(crate::diag::Diag::new("E1101", line, sc, format!("illegal character '{}'", c)));
                i += 1; col += 1;
                continue;
            }
        };
        out.push((t, line, sc));
        i += adv; col += adv;
    }
    out.push((Tok::Eof, line, col));
    out
}
