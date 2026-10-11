pub mod diag;
pub mod lexer;
pub mod parser;
pub mod check;

use diag::Diag;

/// R1 前端流水线：lex → parse → typecheck。返回全部诊断（按位置排序）。
/// 诊断非空即视为编译失败（0.1.x 无代码生成，R1 检查通过 = 该子集编译通过）。
pub fn compile_to_diags(src: &str) -> Vec<Diag> {
    let mut diags: Vec<Diag> = Vec::new();
    let toks = lexer::lex(src, &mut diags);
    let mut p = parser::Parser::new(toks, diags);
    let prog = p.parse_program();
    let mut diags = p.diags;
    let mut ckd = check::Ck::new(&prog).run();
    diags.append(&mut ckd);
    diags.sort_by_key(|d| (d.line, d.col));
    diags
}
