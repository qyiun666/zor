//! L2z 核心集成测试：⑤d 复合树内变量标量叶。
//!
//! 覆盖：struct 标量字段／数组标量元素／struct 内数组／嵌套 struct／struct 数组／
//! 嵌套数组；常量／变量与 enum 变量叶混排；运行期 i64／u8／u32 与编译期
//! i8／i16／i32／u16／u64／bool／f32／f64；Copy、标量复合重初始化、move／unbox／
//! drop、打印等价、六档 `-O*`；以及顶层标量 Box、整棵变量载荷、表达式标量叶、
//! deref 派生窄宽度、嵌套 Box、enum 含 Box、`seen` 同类重复、无标注、读边界。

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
            "zorcc-l2z-{}-{}-{}-{}",
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

/// 同构断言：每两行一组（Box 打印／同字面量打印），逐对相等。
fn assert_pairs_isomorphic(tag: &str, src: &str) {
    let d = TempDir::new(tag);
    let path = write(d.path(), "m.zor", src);
    let (b, r) = build_run(d.path(), &path, &[], tag);
    assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
    let r = r.expect("artifact");
    assert_eq!(r.status.code(), Some(0), "{tag}");
    let out = String::from_utf8_lossy(&r.stdout).into_owned();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2 && lines.len() % 2 == 0, "{tag} stdout={out:?}");
    for pair in lines.chunks(2) {
        assert_eq!(pair[0], pair[1], "{tag} Box 与字面量须同字节: {out:?}");
    }
    assert!(r.stderr.is_empty(), "{tag} stderr={:?}", r.stderr);
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

#[test]
fn struct_and_array_scalar_leaves_match_literal() {
    assert_pairs_isomorphic(
        "struct-scalar",
        "struct S { x: i64 }\nfn main() {\n    let v = 1;\n    let b: Box<S> = box S { x: v };\n    println(*b);\n    println(S { x: v });\n}\n",
    );
    assert_pairs_isomorphic(
        "array-scalar",
        "fn main() {\n    let a = 1;\n    let c = 2;\n    let b: Box<[i64; 2]> = box [a, c];\n    println(*b);\n    println([a, c]);\n}\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let e = V(1);\n    let x = 7;\n    let b: Box<S> = box S { e: e, x: x };\n    println(*b);\n    println(S { e: e, x: x });\n    println(e);\n    println(x);\n}\n",
        "enum-scalar-mix",
        b"S { e: V(1), x: 7 }\nS { e: V(1), x: 7 }\nV(1)\n7\n",
    );
}

#[test]
fn nested_struct_array_and_struct_array_leaves() {
    assert_pairs_isomorphic(
        "nested-scalar",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    let b: Box<O> = box O { i: I { x: x } };\n    println(*b);\n    println(O { i: I { x: x } });\n}\n",
    );
    assert_pairs_isomorphic(
        "struct-array-scalar",
        "struct S { a: [i64; 2] }\nfn main() {\n    let x = 1;\n    let y = 2;\n    let b: Box<S> = box S { a: [x, y] };\n    println(*b);\n    println(S { a: [x, y] });\n}\n",
    );
    assert_pairs_isomorphic(
        "array-of-struct-scalar",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<[S; 1]> = box [S { x: x }];\n    println(*b);\n    println([S { x: x }]);\n}\n",
    );
    assert_pairs_isomorphic(
        "nested-array-scalar",
        "fn main() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n    let w = 4;\n    let b: Box<[[i64; 2]; 2]> = box [[x, y], [z, w]];\n    println(*b);\n    println([[x, y], [z, w]]);\n}\n",
    );
    assert_pairs_isomorphic(
        "mixed-const-var",
        "struct S { a: [i64; 3], x: i64 }\nfn main() {\n    let x = 9;\n    let b: Box<S> = box S { a: [1, x, 3], x: x };\n    println(*b);\n    println(S { a: [1, x, 3], x: x });\n}\n",
    );
}

#[test]
fn runtime_widths_and_arithmetic_leaves() {
    assert_pairs_isomorphic(
        "runtime-widths",
        "struct S { a: u8, b: u32, c: i64 }\nfn main() {\n    let a: u8 = 255;\n    let b: u32 = 4000000000;\n    let c = 1 + 2;\n    let x: Box<S> = box S { a: a, b: b, c: c };\n    println(*x);\n    println(S { a: a, b: b, c: c });\n}\n",
    );
    assert_pairs_isomorphic(
        "deref-i64",
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let x = *b0;\n    let b: Box<[i64; 1]> = box [x];\n    println(*b);\n    println([x]);\n}\n",
    );
}

#[test]
fn compile_time_widths_are_constant_leaves() {
    assert_pairs_isomorphic(
        "compile-widths",
        "struct S { a: i8, b: i16, c: i32, d: u16, e: u64, f: bool, g: f32, h: f64 }\nfn main() {\n    let a: i8 = -5;\n    let b: i16 = -300;\n    let c: i32 = -70000;\n    let d: u16 = 65535;\n    let e: u64 = 42;\n    let f: bool = true;\n    let g: f32 = 2.5;\n    let h: f64 = 1.5;\n    let x: Box<S> = box S { a: a, b: b, c: c, d: d, e: e, f: f, g: g, h: h };\n    println(*x);\n    println(S { a: a, b: b, c: c, d: d, e: e, f: f, g: g, h: h });\n}\n",
    );
    assert_runs(
        "struct S { a: [f64; 2] }\nfn main() {\n    let x = f64::NAN;\n    let y = f64::MAX;\n    let b: Box<S> = box S { a: [x, y] };\n    println(*b);\n    println(S { a: [x, y] });\n    println(x);\n    println(y);\n}\n",
        "float-bindings",
        b"S { a: [nan, 1.7976931348623157e+308] }\nS { a: [nan, 1.7976931348623157e+308] }\nnan\n1.7976931348623157e+308\n",
    );
}

#[test]
fn copy_multiple_boxes_and_reinit_keep_originals() {
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b1: Box<S> = box S { x: x };\n    let b2: Box<S> = box S { x: x };\n    println(*b1);\n    println(*b2);\n    println(x);\n    let x2 = 2;\n    b1 = box S { x: x2 };\n    println(*b1);\n    println(x2);\n    let c = b2;\n    println(*c);\n}\n",
        "copy-multi-box",
        b"S { x: 1 }\nS { x: 1 }\n1\nS { x: 2 }\n2\nS { x: 1 }\n",
    );
    assert_runs(
        "struct S { a: [i64; 2] }\nfn main() {\n    let x = 1;\n    let y = 2;\n    let b: Box<S> = box S { a: [x, y] };\n    println(*b);\n    let x2 = 3;\n    let y2 = 4;\n    b = box S { a: [x2, y2] };\n    println(*b);\n    println(x2);\n    println(y2);\n}\n",
        "array-reinit",
        b"S { a: [1, 2] }\nS { a: [3, 4] }\n3\n4\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x };\n    let c = b;\n    println(*c);\n    let d: Box<S> = box S { x: x };\n    unbox d;\n    println(x);\n}\n",
        "move-unbox",
        b"S { x: 1 }\n1\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    // L3a：顶层标量 Box 变量载荷／重初始化已开放（原负控翻转）。
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x;\n    println(*b);\n}\n",
        "top-scalar-init",
        b"1\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    unbox b;\n    let x: i64 = 2;\n    b = box x;\n    println(*b);\n}\n",
        "top-scalar-reinit",
        b"2\n",
    );
    assert_rc1(
        "whole-payload",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "whole-payload-reinit",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let p = P { x: 2 };\n    b = box p;\n    println(*b);\n}\n",
        "赋值语句",
    );
    assert_rc1(
        "expression-field",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "expression-array",
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "deref-narrow",
        "struct S { x: i32 }\nfn main() {\n    let b0: Box<i32> = box 7;\n    let x = *b0;\n    let b: Box<S> = box S { x: x };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-ctor-expression",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x: i64 = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "unannotated",
        "struct S { x: i64 }\nfn main() {\n    let b = box S { x: 1 };\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "nested-box",
        "struct S { x: i64 }\nfn main() {\n    let b: Box<Box<S>> = box box S { x: 1 };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-with-box",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "seen-repeat",
        "enum E { V(i64) }\nstruct S { a: E, b: E }\nfn main() {\n    let b: Box<S> = box S { a: V(1), b: V(2) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "println-box",
        "struct S { x: i64 }\nfn main() {\n    let b: Box<S> = box S { x: 1 };\n    println(b);\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn scalar_leaf_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct S { x: i64 }\nstruct A { a: [i64; 2] }\nfn main() {\n    let x = 1;\n    let y = 2;\n    let b: Box<S> = box S { x: x };\n    println(*b);\n    let a: Box<A> = box A { a: [x, y] };\n    println(*a);\n    let z = 3;\n    b = box S { x: z };\n    println(*b);\n}\n",
    );
    let mut baseline: Option<Vec<u8>> = None;
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert!(r.stderr.is_empty(), "{flag} stderr={:?}", r.stderr);
        match &baseline {
            None => baseline = Some(r.stdout),
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
