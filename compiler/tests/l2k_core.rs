//! L2k 核心集成测试：Box 常量载荷（struct／定长数组／有限浮点标量）。
//!
//! 覆盖：注解构造、`println(*b)` 与同值普通复合打印逐字节同构、多实参／重复读、
//! 移动／重赋值／unbox／作用域 drop、元素标注下推（全部允许叶子）、空 struct／`[T;0]`、
//! 有限浮点上溢／下溢；以及冻结件 §二 的全部 fail-closed 红线。
//! 只依赖宿主 `cc`；不访问网络；不写死 L2k 期望字节（同构用运行期对照）。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let p = std::env::temp_dir().join(format!(
            "zorcc-l2k-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            n
        ));
        fs::create_dir(&p).expect("create temp dir");
        TempDir(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn zorcc() -> &'static str {
    env!("CARGO_BIN_EXE_zorcc")
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(zorcc())
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn zorcc")
}

fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, src).expect("write source");
    p
}

fn stderr_text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn assert_no_temp(dir: &Path) {
    for e in fs::read_dir(dir).expect("read dir") {
        let n = e.expect("entry").file_name().to_string_lossy().into_owned();
        assert!(!n.starts_with(".zorcc-tmp-"), "临时件残留: {n}");
    }
}

fn build_run(dir: &Path, src: &Path, flags: &[&str], tag: &str) -> (Output, Option<Output>) {
    let out = dir.join(format!("out-{tag}"));
    let _ = fs::remove_file(&out);
    let mut args: Vec<&str> = flags.to_vec();
    args.push(src.to_str().expect("src utf8"));
    args.push("-o");
    args.push(out.to_str().expect("out utf8"));
    let build = run_in(dir, &args);
    if build.status.code() == Some(0) && out.is_file() {
        let run = Command::new(&out).output().expect("run artifact");
        (build, Some(run))
    } else {
        (build, None)
    }
}

/// 编译并运行；`want` 仅用于**普通值对照**（既有契约），不承载 L2k 新期望。
fn assert_runs(src: &str, tag: &str, want: &[u8]) {
    let d = TempDir::new(tag);
    let path = write(d.path(), "m.zor", src);
    let (b, r) = build_run(d.path(), &path, &[], tag);
    assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
    let r = r.expect("artifact");
    assert_eq!(r.status.code(), Some(0), "{tag}");
    assert_eq!(r.stdout, want, "{tag}");
    assert!(r.stderr.is_empty(), "{tag} stderr={:?}", r.stderr);
    assert_no_temp(d.path());
}

/// 同构断言：源码依次打印普通对照与 `println(*b)`，逐行相等。
fn assert_isomorphic(tag: &str, src: &str) {
    let d = TempDir::new(tag);
    let path = write(d.path(), "m.zor", src);
    let (b, r) = build_run(d.path(), &path, &[], tag);
    assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
    let r = r.expect("artifact");
    assert_eq!(r.status.code(), Some(0), "{tag}");
    let out = String::from_utf8_lossy(&r.stdout).into_owned();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "{tag} stdout={out:?}");
    for pair in lines.chunks(2) {
        assert_eq!(pair[0], pair[1], "{tag} 解引用打印须与普通打印同构: {out:?}");
    }
    assert_no_temp(d.path());
}

fn assert_rc1(tag: &str, src: &str, token: &str) {
    let d = TempDir::new(tag);
    let path = write(d.path(), &format!("{tag}.zor"), src);
    let out = d.path().join(format!("out-{tag}"));
    let b = run_in(d.path(), &[path.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    let err = stderr_text(&b);
    assert_eq!(b.status.code(), Some(1), "{tag} stderr={err}");
    assert!(!out.exists(), "{tag} 不得留下产物");
    assert!(err.contains(token), "{tag} 缺少判词 {token} → {err}");
    assert!(!err.contains("panic"), "{tag} 不得 panic: {err}");
    assert!(!err.contains("C 编译器"), "{tag} 不得退化为 cc 失败: {err}");
    assert_no_temp(d.path());
}

// —— 复合载荷：打印同构 ——

#[test]
fn struct_payload_prints_like_plain_struct() {
    assert_isomorphic(
        "struct-basic",
        "struct S { x: i64, y: i8 }\nfn main() {\n    let b: Box<S> = box S { x: 1, y: -2 };\n    println(S { x: 1, y: -2 });\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "struct-float-fields",
        "struct P { x: f32, y: f64 }\nfn main() {\n    let b: Box<P> = box P { x: 1.5, y: -0.0 };\n    println(P { x: 1.5, y: -0.0 });\n    println(*b);\n}\n",
    );
    // S14-31：声明宽度最小值的负字面量叶子（i8::MIN／i64::MIN）。
    assert_isomorphic(
        "struct-width-min",
        "struct S { a: i64, b: i8 }\nfn main() {\n    let b: Box<S> = box S { a: -9223372036854775808, b: -128 };\n    println(S { a: -9223372036854775808, b: -128 });\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "struct-nested",
        "struct P { x: i64 }\nstruct Q { p: P, flag: bool, a: [u8;2] }\nfn main() {\n    let b: Box<Q> = box Q { p: P { x: 7 }, flag: true, a: [3, 4] };\n    println(Q { p: P { x: 7 }, flag: true, a: [3, 4] });\n    println(*b);\n}\n",
    );
}

#[test]
fn array_payloads_cover_all_allowed_element_kinds() {
    for (tag, decl, plain_decl, payload, control) in [
        ("arr-i64", "[i64;3]", "[i64;3]", "[1, 2, 3]", "[1, 2, 3]"),
        ("arr-i8", "[i8;3]", "[i8;3]", "[1, 2, 3]", "[1, 2, 3]"),
        ("arr-u16", "[u16;2]", "[u16;2]", "[7, 8]", "[7, 8]"),
        ("arr-u64", "[u64;2]", "[u64;2]", "[18446744073709551615, 1]", "[18446744073709551615, 1]"),
        ("arr-f64", "[f64;2]", "[f64;2]", "[1.5, -0.0]", "[1.5, -0.0]"),
        ("arr-f32", "[f32;2]", "[f32;2]", "[1.5, 0.0]", "[1.5, 0.0]"),
        ("arr-bool", "[bool;2]", "[bool;2]", "[true, false]", "[true, false]"),
    ] {
        let src = format!(
            "fn main() {{\n    let b: Box<{decl}> = box {payload};\n    let a: {plain_decl} = {control};\n    println(a);\n    println(*b);\n}}\n"
        );
        assert_isomorphic(tag, &src);
    }
    assert_isomorphic(
        "arr-width-min",
        "fn main() {\n    let b: Box<[i8;2]> = box [-1, -128];\n    let a: [i8;2] = [-1, -128];\n    println(a);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "arr-nested",
        "fn main() {\n    let b: Box<[[u8;2];2]> = box [[1, 2], [3, 4]];\n    let a: [[u8;2];2] = [[1, 2], [3, 4]];\n    println(a);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "arr-of-struct",
        "struct P { x: i64, y: f32 }\nfn main() {\n    let b: Box<[P;2]> = box [P { x: 1, y: 1.5 }, P { x: 2, y: -0.0 }];\n    let a: [P;2] = [P { x: 1, y: 1.5 }, P { x: 2, y: -0.0 }];\n    println(a);\n    println(*b);\n}\n",
    );
}

#[test]
fn empty_struct_and_zero_length_array_are_supported() {
    assert_isomorphic(
        "empty-struct",
        "struct E {}\nfn main() {\n    let b: Box<E> = box E {};\n    println(E {});\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "zero-array",
        "fn main() {\n    let b: Box<[i64;0]> = box [];\n    let a: [i64;0] = [];\n    println(a);\n    println(*b);\n}\n",
    );
}

// —— 有限浮点标量 ——

#[test]
fn finite_float_scalar_payloads_print_like_plain_values() {
    assert_isomorphic(
        "f64-finite",
        "fn main() {\n    let b: Box<f64> = box 1.5;\n    println(1.5);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "f64-negzero",
        "fn main() {\n    let b: Box<f64> = box -0.0;\n    println(-0.0);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "f64-negative",
        "fn main() {\n    let b: Box<f64> = box -2.25;\n    println(-2.25);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "f32-finite",
        "fn main() {\n    let b: Box<f32> = box 1.5;\n    let x: f32 = 1.5;\n    println(x);\n    println(*b);\n}\n",
    );
    // 上溢／下溢按既有常量求值。
    assert_isomorphic(
        "f64-overflow",
        "fn main() {\n    let b: Box<f64> = box 1e999;\n    let x: f64 = 1e999;\n    println(x);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "f64-underflow",
        "fn main() {\n    let b: Box<f64> = box 1e-999;\n    let x: f64 = 1e-999;\n    println(x);\n    println(*b);\n}\n",
    );
}

// —— 所有权语义 ——

#[test]
fn composite_payload_move_reassign_unbox_and_scope_drop() {
    assert_runs(
        "fn main() {\n    let b: Box<[i64;3]> = box [1, 2, 3];\n    let c = b;\n    println(*c);\n    b = box [4, 5, 6];\n    println(*b);\n    unbox b;\n    println(*c);\n}\n",
        "composite-own",
        b"[1, 2, 3]\n[4, 5, 6]\n[1, 2, 3]\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let a: Box<S> = box S { x: 1 };\n    let b: Box<S> = box S { x: 2 };\n    b = a;\n    println(*b);\n    unbox b;\n    println(1);\n}\n",
        "composite-assign",
        b"S { x: 1 }\n1\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<S> = box S { x: 1 };\n    b = b;\n    println(*b);\n}\nstruct S { x: i64 }\n",
        "composite-self",
        b"S { x: 1 }\n",
    );
}

#[test]
fn multi_arg_and_repeated_reads_do_not_consume() {
    assert_runs(
        "fn main() {\n    let a: Box<[i64;2]> = box [1, 2];\n    let b: Box<S> = box S { x: 3 };\n    println(*a);\n    println(*b, *a);\n    println(*a);\n    println(*b);\n}\nstruct S { x: i64 }\n",
        "composite-multi",
        b"[1, 2]\nS { x: 3 } [1, 2]\n[1, 2]\nS { x: 3 }\n",
    );
}

#[test]
fn use_after_unbox_and_double_unbox_stay_e2301() {
    assert_rc1(
        "composite-use-after-unbox",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    unbox b;\n    println(*b);\n}\n",
        "E2301",
    );
    assert_rc1(
        "composite-double-unbox",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
}

#[test]
fn composite_payloads_are_stable_across_opt_levels() {
    let d = TempDir::new("l2k-opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct S { x: i64, y: f64 }\nfn main() {\n    let b: Box<S> = box S { x: 1, y: 1.5 };\n    println(*b);\n    let a: Box<[i8;3]> = box [1, 2, 3];\n    println(*a);\n    let f: Box<f32> = box -0.0;\n    println(*f);\n}\n",
    );
    let mut seen: Option<Vec<u8>> = None;
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert!(r.stderr.is_empty(), "{flag} stderr={:?}", r.stderr);
        match &seen {
            None => seen = Some(r.stdout.clone()),
            Some(first) => assert_eq!(first, &r.stdout, "{flag} 与 -O0 不一致"),
        }
    }
    assert_no_temp(d.path());
}

// —— fail-closed 红线 ——

#[test]
fn nested_box_and_box_in_composite_stay_fail_closed() {
    assert_rc1(
        "box-box",
        "fn main() {\n    let b: Box<Box<i64>> = box box 1;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "struct-with-box-field",
        "struct H { b: Box<i8> }\nfn main() {\n    let h: Box<H> = box H { b: box 1 };\n    println(*h);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "array-of-box",
        "fn main() {\n    let b: Box<[Box<i64>;2]> = box [box 1, box 2];\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn enum_unit_tuple_and_byte_payloads_stay_fail_closed() {
    // L2t：`Box<E>` 常量子集转正（正例矩阵见 l2t_core）。
    assert_runs(
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    println(*b);\n}\n",
        "box-enum-l2t",
        b"A(1)\n",
    );
    // L2w：常量复合 enum 叶子转正（Box<S{e:E}>）。
    assert_runs(
        "enum E { A(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(1) };\n    println(*b);\n}\n",
        "struct-enum-field",
        b"S { e: A(1) }\n",
    );
    assert_rc1(
        "struct-unit-field",
        "struct S { u: unit }\nfn main() {\n    let b: Box<S> = box S { u: () };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "struct-tuple-field",
        "struct S { t: (i64, i64) }\nfn main() {\n    let b: Box<S> = box S { t: (1, 2) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-bool",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-byte",
        "fn main() {\n    let b: Box<byte> = box 0;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "box-unit",
        "fn main() {\n    let b: Box<unit> = box ();\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn byte_array_payloads_and_nested_byte_arrays_stay_fail_closed() {
    assert_rc1(
        "box-byte-array",
        "fn main() {\n    let b: Box<[byte;2]> = box \"ab\";\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "struct-byte-array-field",
        "struct S { b: [byte;2] }\nfn main() {\n    let b: Box<S> = box S { b: \"ab\" };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "array-of-byte-array",
        "fn main() {\n    let b: Box<[[byte;2];2]> = box [\"ab\", \"cd\"];\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn variable_and_non_constant_payloads_stay_fail_closed() {
    // L3b：顶层整棵常量复合变量载荷已开放（原负控翻转）。
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    println(*b);\n}\n",
        "variable-struct",
        b"P { x: 1 }\n",
    );
    assert_runs(
        "fn main() {\n    let a: [i64;3] = [1, 2, 3];\n    let b: Box<[i64;3]> = box a;\n    println(*b);\n}\n",
        "variable-array",
        b"[1, 2, 3]\n",
    );
    // L2z：复合树内变量标量叶已开放（原同源负控翻转）。
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<P> = box P { x: x };\n    println(*b);\n}\n",
        "variable-leaf",
        b"P { x: 1 }\n",
    );
    // 负整数值进无符号叶子：由 check 的既有宽度判定拦下（E1201）。
    assert_rc1(
        "negative-int-leaf-in-array",
        "fn main() {\n    let b: Box<[u8;2]> = box [-1, 2];\n    println(*b);\n}\n",
        "E1201",
    );
}

#[test]
fn width_and_length_strictness_reuses_existing_codes() {
    assert_rc1(
        "int-into-f32",
        "fn main() {\n    let b: Box<f32> = box 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "int-into-f64",
        "fn main() {\n    let b: Box<f64> = box 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "cross-width-special",
        "fn main() {\n    let b: Box<f32> = box f64::NAN;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "array-length-mismatch",
        "fn main() {\n    let b: Box<[i64;3]> = box [1, 2];\n    println(*b);\n}\n",
        "E1201",
    );
    // 越界负值仍由既有宽度判定拦截（E1201），不得放宽。
    assert_rc1(
        "i8-negative-out-of-range",
        "fn main() {\n    let b: Box<[i8;2]> = box [-129, 1];\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "nested-array-length-mismatch",
        "fn main() {\n    let b: Box<[[u8;2];2]> = box [[1, 2], [3]];\n    println(*b);\n}\n",
        "E1201",
    );
}

#[test]
fn non_deref_reads_and_projections_stay_fail_closed() {
    assert_rc1(
        "print-box",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "print-box-and-deref",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    println(*b, b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "tuple-with-deref",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    println((*b, 1));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "let-deref",
        "fn main() {\n    let b: Box<[i64;2]> = box [1, 2];\n    let s = *b;\n    println(s);\n}\n",
        "解引用源不是 Box 绑定",
    );
    assert_rc1(
        "field-projection",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    println((*b).x);\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn unannotated_box_payloads_stay_fail_closed() {
    assert_rc1(
        "unannotated-struct",
        "struct P { x: i64 }\nfn main() {\n    let b = box P { x: 1 };\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "unannotated-array",
        "fn main() {\n    let b = box [1, 2];\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "unannotated-float",
        "fn main() {\n    let b = box 1.5;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    // L3d：i64 负号字面量表达式载荷已开放（原负控翻转）。
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box -1;\n    println(*b);\n}\n",
        "negative-int-scalar-payload",
        b"-1\n",
    );
}

#[test]
fn special_value_boxes_zero_regression() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f32> = box f32::NEG_INFINITY;\n    println(*a, *b);\n}\n",
        "special-regress",
        b"nan -inf\n",
    );
}
