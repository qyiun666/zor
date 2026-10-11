// r1_diag：E1xxx 诊断码逐码触发 + 正例零诊断（AC-R1-2 可执行前置，T-4 格式锚点）
use zorcc::{compile_to_diags, diag::Diag};

fn codes(src: &str) -> Vec<String> {
    compile_to_diags(src).iter().map(|d| d.code.clone()).collect()
}
fn has(src: &str, code: &str) -> bool {
    codes(src).iter().any(|c| c == code)
}
fn clean(src: &str) {
    let d = compile_to_diags(src);
    assert!(d.is_empty(), "expected clean, got: {:?}", d.iter().map(|x| x.render("<t>")).collect::<Vec<_>>());
}

// ———— 正例：R1 合法子集零诊断 ————

#[test]
fn pos_hello() {
    clean(r#"fn main() { let greeting = "Hello, ZOR"; println(greeting); }"#);
}

#[test]
fn pos_primitives_and_inference() {
    clean("fn main() { let a: i8 = 1; let b: u64 = 2; let c: f32 = 1.5; let d: bool = true; let e = 42; let f = 2.5; println(a); println(b); println(c); println(d); println(e); println(f); }");
}

#[test]
fn pos_struct_record_field() {
    clean("struct Point { x: i64, y: i64 }\nfn main() { let p = Point { x: 1, y: 2 }; println(p.x); }");
}

#[test]
fn pos_enum_variants() {
    clean("enum Shape { Circle(i64), Rect(i64, i64) }\nfn main() { let a = Circle(3); let b = Rect(1, 2); }");
}

#[test]
fn pos_array_tuple_index() {
    clean("fn main() { let a: [i32; 3] = [1, 2, 3]; let t = (1, true); println(a[0]); }");
}

#[test]
fn pos_fn_call_and_explicit_convert() {
    clean("fn area(w: i64, h: i64) -> i64 { w * h }\nfn main() { let x: i64 = 10; let y = x.to_i32(); println(area(2, 3)); }");
}

#[test]
fn pos_ref_params() {
    clean("fn show(v: &i64) { println(v); }\nfn main() { let n = 7; show(&n); }");
}

// ———— 负例：逐码触发 ————

#[test]
fn neg_e1101_unknown_type() {
    assert!(has("fn f(x: Foo) { } fn main() { }", "E1101"));
}

#[test]
fn neg_e1101_illegal_char() {
    assert!(has("fn main() { let x = 1 # 2; }", "E1101"));
}

#[test]
fn neg_e1201_bool_to_int() {
    assert!(has("fn main() { let x: i32 = true; }", "E1201"));
}

#[test]
fn neg_e1201_literal_overflow() {
    assert!(has("fn main() { let x: i8 = 200; }", "E1201"));
}

#[test]
fn neg_e1202_int_lit_on_float() {
    assert!(has("fn main() { let x: f64 = 1; }", "E1202"));
}

#[test]
fn neg_e1202_float_lit_on_int() {
    assert!(has("fn main() { let x: i64 = 1.5; }", "E1202"));
}

#[test]
fn neg_e1301_non_const_len() {
    assert!(has("fn main() { let n = 3; let a: [i32; n] = [1, 2, 3]; }", "E1301"));
}

#[test]
fn neg_e1401_recursive_struct() {
    assert!(has("struct Node { next: Node } fn main() { }", "E1401"));
}

#[test]
fn neg_e1401_recursive_enum() {
    assert!(has("enum T { Inner(T) } fn main() { }", "E1401"));
}

#[test]
fn neg_e1501_ref_in_struct() {
    assert!(has("struct S { r: &i32 } fn main() { }", "E1501"));
}

#[test]
fn neg_e1502_ref_return() {
    // R2 §4：返回引用归位 E1502（R1 期暂用 E1501，r2 批次按规范修正；E2401 别名不实现）
    assert!(has("fn f() -> &i32 { } fn main() { }", "E1502"));
}

#[test]
fn neg_e1501_lifetime_syntax() {
    assert!(has("fn f<'a>(x: &'a i32) { } fn main() { }", "E1501"));
}

#[test]
fn neg_e1601_arity() {
    assert!(has("fn f(x: i32) { } fn main() { f(1, 2); }", "E1601"));
}

// ———— T-4 黄金文件格式锚点：zorcc: {E-code} {file}:{line}:{col}: {message} ————

#[test]
fn diag_render_format() {
    let d = Diag::new("E1201", 3, 7, "test msg");
    assert_eq!(d.render("a/b.zor"), "zorcc: E1201 a/b.zor:3:7: test msg");
}

#[test]
fn diag_carries_position() {
    let ds = compile_to_diags("fn main() {\n  let x: i8 = 200;\n}");
    assert_eq!(ds.len(), 1);
    assert_eq!((ds[0].code.as_str(), ds[0].line, ds[0].col), ("E1201", 2, 15));
}
