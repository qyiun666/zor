// tests/specs/compare/t-b-holder-ref-field-annotated.rs — AC-R2-4 对照源（T-b struct 持引用字段）
// 预期：rustc 编译通过（Rust 侧需结构体生命周期参数；去标注变体见 t-b-holder-ref-field-unannotated.rs）。
// ZOR 侧对应：tests/specs/positive/r2-ref-02-struct-hold-value.zor（R1 §3 禁引用入容器，改持值）。

struct Holder<'a> {
    v: &'a i64,
}

fn main() {
    let x = 42i64;
    let h = Holder { v: &x };
    println!("{}", h.v);
}
