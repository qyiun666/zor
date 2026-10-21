pub mod backend;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod check;

use diag::Diag;

/// R1 前端流水线：lex → parse → typecheck。返回解析结果与全部诊断（按位置排序）。
/// codegen 仅在诊断为空时进入；`Prog` 是后端入口（ADR-0001 §1.5）。
/// 诊断非空即视为编译失败（无 `-o` 时 R1 检查通过 = 该子集编译通过）。
pub fn compile(src: &str) -> (parser::Prog, Vec<Diag>) {
    let mut diags: Vec<Diag> = Vec::new();
    let toks = lexer::lex(src, &mut diags);
    let mut p = parser::Parser::new(toks, diags);
    let prog = p.parse_program();
    let mut diags = p.diags;
    let mut ckd = check::Ck::new(&prog).run();
    diags.append(&mut ckd);
    diags.sort_by_key(|d| (d.line, d.col));
    (prog, diags)
}

/// 薄包装：保留 0.1.x 在册 API 与既有单测调用形。
pub fn compile_to_diags(src: &str) -> Vec<Diag> {
    compile(src).1
}
