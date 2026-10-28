//! L3e 核心集成测试：⑤f-2a 复合字面量表达式叶。
//!
//! 覆盖：`Box<S>`／`Box<[T;N]>` 负载为 struct／array 字面量时，标量叶直接写表达式
//! （常量表达式／算术／负号／单层字段投影／单层标量 `*b`），任意既有嵌套深度；
//! 表达式叶 box 时刻求值一次并快照；唯一临时局部；重初始化；Copy／move／unbox／drop；
//! 打印等价；六档；以及 enum 构造表达式叶（⑤f-2b）、浮点／非运行期宽度表达式叶、
//! 窄宽度投影／deref、复合 `box *b`、bool 顶层、无标注、嵌套 Box、enum 含 Box／
//! struct、`seen`、`println(b)`、E2301／E2101、字面量算术形 E1201 边界。

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
            "zorcc-l3e-{}-{}-{}-{}",
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

/// 打印等价：每两行一组（`println(同值字面量)`／`println(*b)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 复合字面量与同值打印须同字节: {out:?}");
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
fn struct_literal_expression_leaves() {
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let b: Box<S> = box S { x: 1 + 2 };\n    println(*b);\n}\n",
        "const-expr",
        b"S { x: 3 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "var-expr",
        b"S { x: 2 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: -x };\n    println(*b);\n}\n",
        "neg-expr",
        b"S { x: -1 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: (x + 1) * 2 };\n    println(*b);\n}\n",
        "nested-arith",
        b"S { x: 4 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 7;\n    let b: Box<S> = box S { x: x / 2 };\n    println(*b);\n}\n",
        "div",
        b"S { x: 3 }\n",
    );
    assert_runs(
        "struct S { x: u8 }\nfn main() {\n    let x: u8 = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "u8-expr",
        b"S { x: 2 }\n",
    );
    assert_runs(
        "struct S { x: u32 }\nfn main() {\n    let x: u32 = 2;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "u32-expr",
        b"S { x: 3 }\n",
    );
}

#[test]
fn array_literal_expression_leaves() {
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "arr-expr",
        b"[2]\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 2]> = box [x, 1 + 2];\n    println(*b);\n}\n",
        "arr-mixed",
        b"[1, 3]\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[[i64; 1]; 1]> = box [[x + 1]];\n    println(*b);\n}\n",
        "arr-nested",
        b"[[2]]\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<[S; 1]> = box [S { x: x + 1 }];\n    println(*b);\n}\n",
        "arr-struct",
        b"[S { x: 2 }]\n",
    );
    assert_runs(
        "struct S { a: [i64; 1] }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { a: [x + 1] };\n    println(*b);\n}\n",
        "struct-arr",
        b"S { a: [2] }\n",
    );
    assert_runs(
        "fn main() {\n    let x: u8 = 1;\n    let b: Box<[u8; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "arr-u8",
        b"[2]\n",
    );
    assert_runs(
        "fn main() {\n    let x: u32 = 2;\n    let b: Box<[u32; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "arr-u32",
        b"[3]\n",
    );
}

#[test]
fn nested_and_mixed_leaves() {
    assert_runs(
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    let b: Box<O> = box O { i: I { x: x + 1 } };\n    println(*b);\n}\n",
        "nested-struct",
        b"O { i: I { x: 2 } }\n",
    );
    assert_runs(
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let b: Box<O> = box O { i: I { x: 1 + 2 } };\n    println(*b);\n}\n",
        "nested-const",
        b"O { i: I { x: 3 } }\n",
    );
    assert_runs(
        "struct S { a: i64, b: i64, c: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { a: 1, b: x, c: x + 1 };\n    println(*b);\n}\n",
        "mixed-leaves",
        b"S { a: 1, b: 1, c: 2 }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(1), x: x + 2 };\n    println(*b);\n}\n",
        "enum-mixed",
        b"S { e: V(1), x: 3 }\n",
    );
    assert_runs(
        "struct I { x: i64 }\nstruct O { i: I }\nstruct S { o: O, a: [i64; 2] }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { o: O { i: I { x: x + 1 } }, a: [x, x + 2] };\n    println(*b);\n}\n",
        "deep-mixed",
        b"S { o: O { i: I { x: 2 } }, a: [1, 3] }\n",
    );
}

#[test]
fn field_projection_and_scalar_deref_leaves() {
    assert_runs(
        "struct P { x: i64 }\nstruct S { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<S> = box S { x: p.x };\n    println(*b);\n}\n",
        "field-literal",
        b"S { x: 1 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nstruct S { x: i64 }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let b: Box<S> = box S { x: p.x };\n    println(*b);\n}\n",
        "field-runtime",
        b"S { x: 3 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<S> = box S { x: *b0 };\n    println(*b);\n}\n",
        "deref",
        b"S { x: 7 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<S> = box S { x: *b0 + 1 };\n    println(*b);\n}\n",
        "deref-add",
        b"S { x: 8 }\n",
    );
    assert_runs(
        "struct I { x: i64 }\nstruct O { i: I }\nstruct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<O> = box O { i: I { x: p.x } };\n    println(*b);\n}\n",
        "nested-field",
        b"O { i: I { x: 1 } }\n",
    );
    assert_runs(
        "struct S { x: u8 }\nfn main() {\n    let b0: Box<u8> = box 7;\n    let b: Box<S> = box S { x: *b0 };\n    println(*b);\n}\n",
        "deref-u8",
        b"S { x: 7 }\n",
    );
}

#[test]
fn reinit_snapshot_and_temp_stability() {
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: 1 };\n    b = box S { x: x + 1 };\n    println(*b);\n}\n",
        "reinit-struct",
        b"S { x: 2 }\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 1]> = box [1];\n    b = box [x + 2];\n    println(*b);\n}\n",
        "reinit-array",
        b"[3]\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[[i64; 1]; 1]> = box [[1]];\n    b = box [[x + 2]];\n    println(*b);\n}\n",
        "reinit-nested",
        b"[[3]]\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    let y = 2 + 3;\n    println(*b);\n    println(y);\n    println(*b);\n}\n",
        "temp-stability",
        b"S { x: 2 }\n5\nS { x: 2 }\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b1: Box<S> = box S { x: x + 1 };\n    let y = 10;\n    let b2: Box<S> = box S { x: x + 2 };\n    println(*b1);\n    println(*b2);\n    println(y);\n}\n",
        "independent",
        b"S { x: 2 }\nS { x: 3 }\n10\n",
    );
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    let c = b;\n    println(*c);\n    let d: Box<S> = box S { x: x + 2 };\n    unbox d;\n    println(*c);\n}\n",
        "copy-move-unbox",
        b"S { x: 2 }\nS { x: 2 }\n",
    );
}

#[test]
fn print_equivalence() {
    assert_pairs_isomorphic(
        "eq-struct",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    println(S { x: 2 });\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-array",
        "fn main() {\n    let x = 1;\n    println([2]);\n    let b: Box<[i64; 1]> = box [x + 1];\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-nested",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    println(O { i: I { x: 3 } });\n    let b: Box<O> = box O { i: I { x: x + 2 } };\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-u8",
        "struct S { x: u8 }\nfn main() {\n    let x: u8 = 1;\n    println(S { x: 2 });\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-field",
        "struct P { x: i64 }\nstruct S { x: i64 }\nfn main() {\n    let p = P { x: 3 };\n    println(S { x: 3 });\n    let b: Box<S> = box S { x: p.x };\n    println(*b);\n}\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    // ⑤f-2b：enum 构造表达式叶维持 fail-closed。
    assert_rc1(
        "enum-ctor",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-ctor-field",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-ctor-array",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<[E; 1]> = box [V(1 + 2)];\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-ctor-nested",
        "enum E { V(i64) }\nstruct I { e: E }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    let b: Box<O> = box O { i: I { e: V(x) } };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "float-expr",
        "struct S { x: f64 }\nfn main() {\n    let b: Box<S> = box S { x: 1.5 + 1.5 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    // 非运行期宽度表达式叶：类型化变量形到达 box 闸。
    assert_rc1(
        "i8-expr",
        "struct S { x: i8 }\nfn main() {\n    let x: i8 = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "i16-expr",
        "struct S { x: i16 }\nfn main() {\n    let x: i16 = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "u64-expr",
        "struct S { x: u64 }\nfn main() {\n    let x: u64 = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    // 字面量算术形：check 期 E1201（v1.1 §1 边界，不占 Q）。
    assert_rc1(
        "i8-literal-arith",
        "struct S { x: i8 }\nfn main() {\n    let b: Box<S> = box S { x: 1 + 2 };\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "narrow-field",
        "struct P { x: i8 }\nstruct S { x: i8 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<S> = box S { x: p.x };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "narrow-deref",
        "struct S { x: i8 }\nfn main() {\n    let b0: Box<i8> = box 1;\n    let x = *b0;\n    let b: Box<S> = box S { x: x };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "composite-deref",
        "struct P { x: i64 }\nfn main() {\n    let b0: Box<P> = box P { x: 1 };\n    let b: Box<P> = box *b0;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "add-unparenthesized",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x } + 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "bool-top-level",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "unannotated",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b = box S { x: x + 1 };\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "nested-box",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<Box<S>> = box box S { x: x + 1 };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(P { x: 1 }) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "seen-repeat",
        "struct I { x: i64 }\nstruct S { a: I, b: I }\nfn main() {\n    let b: Box<S> = box S { a: I { x: 1 }, b: I { x: 2 } };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "println-box",
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "struct S { x: i64 }\nfn main() {\n    let b: Box<S> = box S { x: 1 };\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "struct S { x: i64 }\nfn main() {\n    let b: Box<S> = box S { x: 1 };\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn composite_expression_leaves_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct I { x: i64 }\nstruct O { i: I }\nstruct A { a: [i64; 2] }\nfn main() {\n    let x = 1;\n    let b: Box<O> = box O { i: I { x: x + 1 } };\n    println(*b);\n    let b0: Box<i64> = box 7;\n    let c: Box<A> = box A { a: [*b0, 1 + 2] };\n    println(*c);\n    b = box O { i: I { x: -x } };\n    println(*b);\n    let y: u8 = 2;\n    let d: Box<[u8; 1]> = box [y + 1];\n    println(*d);\n    let z: u32 = 2;\n    let e: Box<[u32; 1]> = box [z + 1];\n    println(*e);\n}\n",
    );
    let mut baseline: Option<Vec<u8>> = None;
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert!(r.stderr.is_empty(), "{flag} stderr={:?}", r.stderr);
        match &baseline {
            None => {
                assert_eq!(
                    r.stdout,
                    b"O { i: I { x: 2 } }\nA { a: [7, 3] }\nO { i: I { x: -1 } }\n[3]\n[3]\n",
                    "{flag} 首档输出"
                );
                baseline = Some(r.stdout);
            }
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
