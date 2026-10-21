pub mod backend;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod check;
pub mod type_table;

use diag::Diag;
use type_table::TypeTable;

/// 前端一次编译的完整产物：AST、check 类型表与诊断（按位置排序）。
/// `types` 是类型与所有权判定的唯一来源，codegen 只读不重算。
pub struct Compilation {
    pub prog: parser::Prog,
    pub types: TypeTable,
    pub diags: Vec<Diag>,
}

/// R1 前端流水线：lex → parse → typecheck（同时产出类型表）。
/// codegen 仅在诊断为空时进入；`Prog` 是后端入口（ADR-0001 §1.5）。
/// 诊断非空即视为编译失败（无 `-o` 时 R1 检查通过 = 该子集编译通过）。
pub fn compile_typed(src: &str) -> Compilation {
    let mut diags: Vec<Diag> = Vec::new();
    let toks = lexer::lex(src, &mut diags);
    let mut p = parser::Parser::new(toks, diags);
    let prog = p.parse_program();
    let mut diags = p.diags;
    let (mut ckd, types) = check::Ck::new(&prog).run_typed();
    diags.append(&mut ckd);
    diags.sort_by_key(|d| (d.line, d.col));
    Compilation { prog, types, diags }
}

/// 薄包装：保留 0.1.x 在册 API 与既有单测调用形。
pub fn compile(src: &str) -> (parser::Prog, Vec<Diag>) {
    let c = compile_typed(src);
    (c.prog, c.diags)
}

/// 薄包装：只取诊断（检查模式）。
pub fn compile_to_diags(src: &str) -> Vec<Diag> {
    compile_typed(src).diags
}
