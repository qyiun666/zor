//! L3f 核心集成测试：⑤f-2b enum 构造表达式叶。
//!
//! 覆盖：顶层 `box V(args)` 与 struct／array 字面量内任意既有嵌套深度的
//! enum 构造表达式叶（常量表达式／变量／算术／负号／单层字段投影／单层标量
//! `*b` 实参），含重初始化；box 时刻求值一次并快照；唯一临时；u8／u32 载荷；
//! 零字段回归；与 ⑤f-2a 标量表达式叶混排；Copy／move／unbox／drop；打印等价；
//! 六档；以及非标量 enum 载荷、浮点／非运行期宽度实参、窄宽度投影／deref、
//! enum 含 Box、bool 顶层、无标注、嵌套 Box、`seen`、`println(b)`、E2301／
//! E2101、`box x + 1` E1201；并锁定普通枚举构造 `let e=V(expr)` 与 `box e`
//! 既有行为不变。

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
            "zorcc-l3f-{}-{}-{}-{}",
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

/// 打印等价：每两行一组（`println(同值 enum 构造)`／`println(*b)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} enum 构造与 Box 打印须同字节: {out:?}");
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
fn top_level_enum_constructor_expression_payloads() {
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box V(1 + 2);\n    println(*b);\n}\n",
        "const-arg",
        b"V(3)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x);\n    println(*b);\n}\n",
        "var-arg",
        b"V(1)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "arith-arg",
        b"V(2)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(-x);\n    println(*b);\n}\n",
        "neg-arg",
        b"V(-1)\n",
    );
    assert_runs(
        "struct P { x: i64 }\nenum E { V(i64) }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<E> = box V(p.x);\n    println(*b);\n}\n",
        "field-literal-arg",
        b"V(1)\n",
    );
    assert_runs(
        "struct P { x: i64 }\nenum E { V(i64) }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let b: Box<E> = box V(p.x);\n    println(*b);\n}\n",
        "field-runtime-arg",
        b"V(3)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<E> = box V(*b0);\n    println(*b);\n}\n",
        "deref-i64-arg",
        b"V(7)\n",
    );
    assert_runs(
        "enum E { V(u8) }\nfn main() {\n    let b0: Box<u8> = box 7;\n    let b: Box<E> = box V(*b0);\n    println(*b);\n}\n",
        "deref-u8-arg",
        b"V(7)\n",
    );
    assert_runs(
        "enum E { V(u32) }\nfn main() {\n    let b0: Box<u32> = box 7;\n    let b: Box<E> = box V(*b0);\n    println(*b);\n}\n",
        "deref-u32-arg",
        b"V(7)\n",
    );
}

#[test]
fn composite_literal_enum_constructor_leaves() {
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "struct-var-leaf",
        b"S { e: V(1) }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1 + 2) };\n    println(*b);\n}\n",
        "struct-const-leaf",
        b"S { e: V(3) }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<[E; 1]> = box [V(x + 1)];\n    println(*b);\n}\n",
        "array-leaf",
        b"[V(2)]\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let b: Box<[E; 2]> = box [V(1), V(2)];\n    println(*b);\n}\n",
        "array-multi-leaf",
        b"[V(1), V(2)]\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let b: Box<[S; 1]> = box [S { e: V(x) }];\n    println(*b);\n}\n",
        "array-struct-leaf",
        b"[S { e: V(1) }]\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct I { e: E }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    let b: Box<O> = box O { i: I { e: V(x) } };\n    println(*b);\n}\n",
        "nested-struct-leaf",
        b"O { i: I { e: V(1) } }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<[[E; 1]; 1]> = box [[V(x + 1)]];\n    println(*b);\n}\n",
        "nested-array-leaf",
        b"[[V(2)]]\n",
    );
    assert_runs(
        "enum E { V(u8) }\nfn main() {\n    let x: u8 = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "u8-payload",
        b"V(2)\n",
    );
    assert_runs(
        "enum E { V(u32) }\nfn main() {\n    let x: u32 = 2;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "u32-payload",
        b"V(3)\n",
    );
    assert_runs(
        "enum E { A }\nfn main() {\n    let b: Box<E> = box A();\n    println(*b);\n}\n",
        "zero-field",
        b"A\n",
    );
}

#[test]
fn reinit_mixed_and_ownership() {
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(1);\n    b = box V(x + 1);\n    println(*b);\n}\n",
        "reinit-top",
        b"V(2)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(1) };\n    b = box S { e: V(x + 1) };\n    println(*b);\n}\n",
        "reinit-composite",
        b"S { e: V(2) }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E, n: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(x), n: x + 1 };\n    println(*b);\n}\n",
        "mixed-leaf",
        b"S { e: V(1), n: 2 }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b1: Box<E> = box V(x + 1);\n    let y = 10;\n    let b2: Box<E> = box V(x + 2);\n    println(*b1);\n    println(*b2);\n    println(y);\n}\n",
        "independent",
        b"V(2)\nV(3)\n10\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x + 1);\n    let c = b;\n    println(*c);\n    let d: Box<E> = box V(x);\n    unbox d;\n    println(*c);\n}\n",
        "copy-move-unbox",
        b"V(2)\nV(2)\n",
    );
}

#[test]
fn temp_stability_and_print_equivalence() {
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x + 1);\n    let y = 2 + 3;\n    println(*b);\n    println(y);\n    println(*b);\n}\n",
        "temp-stability",
        b"V(2)\n5\nV(2)\n",
    );
    assert_pairs_isomorphic(
        "eq-top",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    println(V(2));\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-composite",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    println(S { e: V(1) });\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-array",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    println([V(2)]);\n    let b: Box<[E; 1]> = box [V(x + 1)];\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-nested",
        "enum E { V(i64) }\nstruct I { e: E }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    println(O { i: I { e: V(3) } });\n    let b: Box<O> = box O { i: I { e: V(x + 2) } };\n    println(*b);\n}\n",
    );
}

#[test]
fn ordinary_construction_unchanged() {
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let e = V(x + 1);\n    println(e);\n    let b: Box<E> = box e;\n    println(*b);\n}\n",
        "ordinary-var-box",
        b"V(2)\nV(2)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let e = V(x + 1);\n    let b: Box<S> = box S { e: e };\n    println(*b);\n}\n",
        "ordinary-nested",
        b"S { e: V(2) }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nenum E { V(i64) }\nfn main() {\n    let p = P { x: 3 };\n    let e = V(p.x);\n    println(e);\n}\n",
        "ordinary-field",
        b"V(3)\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    // 非标量 enum 载荷：更早的 let 类型标注闸。
    assert_rc1(
        "enum-struct-payload",
        "struct P { x: i64 }\nenum E { V(P) }\nfn main() {\n    let b: Box<E> = box V(P { x: 1 });\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-array-payload",
        "enum E { V([i64; 2]) }\nfn main() {\n    let b: Box<E> = box V([1, 2]);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-tuple-payload",
        "enum E { V((i64, i64)) }\nfn main() {\n    let b: Box<E> = box V((1, 2));\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-unit-payload",
        "enum E { V(()) }\nfn main() {\n    let b: Box<E> = box V(());\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-enum-payload",
        "enum I { A(i64) }\nenum E { V(I) }\nfn main() {\n    let b: Box<E> = box V(A(1));\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box-payload",
        "enum E { V(Box<i64>) }\nfn main() {\n    let b: Box<E> = box V(box 1);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "float-arg",
        "enum E { V(f64) }\nfn main() {\n    let b: Box<E> = box V(1.5 + 1.5);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "i8-arg",
        "enum E { V(i8) }\nfn main() {\n    let x: i8 = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "i16-arg",
        "enum E { V(i16) }\nfn main() {\n    let x: i16 = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "u64-arg",
        "enum E { V(u64) }\nfn main() {\n    let x: u64 = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "narrow-field-arg",
        "struct P { x: i8 }\nenum E { V(i8) }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<E> = box V(p.x);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "narrow-deref-arg",
        "enum E { V(i8) }\nfn main() {\n    let b0: Box<i8> = box 1;\n    let b: Box<E> = box V(*b0);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-variant-box-field",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "bool-top-level",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "unannotated",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b = box V(x);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "nested-box",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<Box<E>> = box box V(x);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "seen-repeat",
        "struct I { x: i64 }\nstruct S { a: I, b: I }\nfn main() {\n    let b: Box<S> = box S { a: I { x: 1 }, b: I { x: 2 } };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "println-box",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x);\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "composite-deref",
        "enum E { V(i64) }\nfn main() {\n    let b0: Box<E> = box V(1);\n    let b: Box<E> = box *b0;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "add-unparenthesized",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x) + 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "double-unbox",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box V(1);\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box V(1);\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn enum_constructor_leaves_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i64) }\nenum U { Uv(u8) }\nenum W { Wv(u32) }\nstruct S { e: E }\nstruct I { e: E }\nstruct O { i: I }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x + 1);\n    println(*b);\n    let c: Box<S> = box S { e: V(x) };\n    println(*c);\n    let d: Box<O> = box O { i: I { e: V(x + 2) } };\n    println(*d);\n    b = box V(x + 3);\n    println(*b);\n    let ux: u8 = 2;\n    let e2: Box<U> = box Uv(ux + 1);\n    println(*e2);\n    let wx: u32 = 2;\n    let e3: Box<W> = box Wv(wx + 1);\n    println(*e3);\n}\n",
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
                    b"V(2)\nS { e: V(1) }\nO { i: I { e: V(3) } }\nV(4)\nUv(3)\nWv(3)\n",
                    "{flag} 首档输出"
                );
                baseline = Some(r.stdout);
            }
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
