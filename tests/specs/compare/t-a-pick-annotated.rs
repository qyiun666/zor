// tests/specs/compare/t-a-pick-annotated.rs — AC-R2-4 对照源（T-a 二输入择一返回）
// 预期：rustc 编译通过（Rust 侧非写生命周期标注不可；去标注变体见 t-a-pick-unannotated.rs）。
// ZOR 侧对应：tests/specs/positive/r2-ref-01-two-input-select-owned.zor（R2 §4 禁返回引用，改返回拥有值）。

fn pick<'a>(a: &'a i64, b: &'a i64) -> &'a i64 {
    if a > b { a } else { b }
}

fn main() {
    let x = 3i64;
    let y = 7i64;
    println!("{}", pick(&x, &y));
}
