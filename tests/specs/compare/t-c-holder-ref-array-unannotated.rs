// tests/specs/compare/t-c-holder-ref-array-unannotated.rs — AC-R2-4 对照源（T-c 去标注变体）
// 预期：rustc 编译失败，error[E0106]: missing lifetime specifier。

struct Holder {
    v: [&i64; 2],
}

fn main() {
    let x = 10i64;
    let y = 20i64;
    let h = Holder { v: [&x, &y] };
    println!("{}", h.v[0] + h.v[1]);
}
