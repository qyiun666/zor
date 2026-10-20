// tests/specs/compare/t-b-holder-ref-field-unannotated.rs — AC-R2-4 对照源（T-b 去标注变体）
// 预期：rustc 编译失败，error[E0106]: missing lifetime specifier。

struct Holder {
    v: &i64,
}

fn main() {
    let x = 42i64;
    let h = Holder { v: &x };
    println!("{}", h.v);
}
