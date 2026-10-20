// tests/specs/compare/t-a-pick-unannotated.rs — AC-R2-4 对照源（T-a 去标注变体）
// 预期：rustc 编译失败，error[E0106]: missing lifetime specifier。

fn pick(a: &i64, b: &i64) -> &i64 {
    if a > b { a } else { b }
}

fn main() {
    let x = 3i64;
    let y = 7i64;
    println!("{}", pick(&x, &y));
}
