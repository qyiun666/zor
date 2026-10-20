// tests/specs/compare/t-c-holder-ref-array-annotated.rs — AC-R2-4 对照源（T-c 容器/数组元素持引用）
// 预期：rustc 编译通过（Rust 侧需类型级生命周期参数；去标注变体见 t-c-holder-ref-array-unannotated.rs）。
// ZOR 侧对应：tests/specs/positive/r2-ref-03-array-hold-value.zor（R2 §7 In2 / R1 §3 拒引用入容器，改持值）。

struct Holder<'a> {
    v: [&'a i64; 2],
}

fn main() {
    let x = 10i64;
    let y = 20i64;
    let h = Holder { v: [&x, &y] };
    println!("{}", h.v[0] + h.v[1]);
}
