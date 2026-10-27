//! L2x 核心集成测试：⑤b 一层变量复合 enum 叶。
//!
//! 覆盖：`Box<S{e:e}>`／`Box<[E;N]>` 变量元素／常量变量混合／多个不同 enum 字段／
//! 常量标量混排／变量复合重初始化（struct／array）／e 自身运行期字段
//! （`V(x)`／`V(1+2)`／`-x`）／Copy（e 仍可用）／move／unbox／drop／打印等价／
//! 六档 `-O*` 稳定；以及变量标量叶、整棵变量载荷、表达式构造叶、嵌套变量叶、
//! 嵌套 Box、`seen` 同类重复、非标量 enum 字段、无标注、读／所有权边界。

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
            "zorcc-l2x-{}-{}-{}-{}",
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
fn variable_struct_leaf_matches_literal() {
    assert_pairs_isomorphic(
        "struct",
        "enum E { V(i64), W(f64), N, P(i64, bool, f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, x: 9 };\n    println(*b);\n    println(S { e: e, x: 9 });\n    let n = N();\n    let b2: Box<S> = box S { e: n, x: 2 };\n    println(*b2);\n    println(S { e: n, x: 2 });\n    let w = W(1.5);\n    let b3: Box<S> = box S { e: w, x: 3 };\n    println(*b3);\n    println(S { e: w, x: 3 });\n    let q = P(1024, true, 1.5);\n    let b4: Box<S> = box S { e: q, x: 4 };\n    println(*b4);\n    println(S { e: q, x: 4 });\n}\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, x: 9 };\n    println(*b);\n    println(e);\n}\n",
        "copy-usability",
        b"S { e: V(1), x: 9 }\nV(1)\n",
    );
}

#[test]
fn variable_array_leaf_matches_literal() {
    assert_pairs_isomorphic(
        "array",
        "enum E { V(i64), W(f64), N }\nfn main() {\n    let e1 = V(1);\n    let e2 = W(2.5);\n    let n = N();\n    let a: Box<[E; 2]> = box [e1, e2];\n    println(*a);\n    println([e1, e2]);\n    let b: Box<[E; 2]> = box [V(1), e2];\n    println(*b);\n    println([V(1), e2]);\n    let c: Box<[E; 2]> = box [e1, W(2.5)];\n    println(*c);\n    println([e1, W(2.5)]);\n    let d: Box<[E; 1]> = box [n];\n    println(*d);\n    println([n]);\n}\n",
    );
    assert_runs(
        "enum E { V(i64), W(f64) }\nfn main() {\n    let e1 = V(1);\n    let e2 = W(2.5);\n    let a: Box<[E; 2]> = box [e1, e2];\n    println(*a);\n    println(e1);\n    println(e2);\n}\n",
        "array-copy",
        b"[V(1), W(2.5)]\nV(1)\nW(2.5)\n",
    );
}

#[test]
fn mixed_fields_and_two_enum_types() {
    assert_pairs_isomorphic(
        "mixed",
        "enum E { V(i64) }\nenum F { Q(f64) }\nstruct S { e: E, x: i64 }\nstruct T { e: E, f: F }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, x: 9 };\n    println(*b);\n    println(S { e: e, x: 9 });\n    let f = Q(2.5);\n    let t: Box<T> = box T { e: e, f: f };\n    println(*t);\n    println(T { e: e, f: f });\n}\n",
    );
}

#[test]
fn live_leaf_with_constant_subtrees() {
    assert_pairs_isomorphic(
        "const-subtrees",
        "enum E { V(i64) }\nenum F { Q(f64) }\nstruct P { x: i64 }\nstruct S { e: E, p: P }\nstruct T { e: E, a: [F; 2] }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, p: P { x: 1 } };\n    println(*b);\n    println(S { e: e, p: P { x: 1 } });\n    let t: Box<T> = box T { e: e, a: [Q(1.5), Q(2.5)] };\n    println(*t);\n    println(T { e: e, a: [Q(1.5), Q(2.5)] });\n}\n",
    );
}

#[test]
fn variable_reinit_move_unbox_and_scope_drop() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let b: Box<S> = box S { e: V(1), x: 1 };\n    let e2 = W(2.5);\n    b = box S { e: e2, x: 2 };\n    println(*b);\n    println(e2);\n    let c = b;\n    println(*c);\n    let d: Box<S> = box S { e: V(3), x: 3 };\n    unbox d;\n    println(V(3));\n}\n",
        "reinit",
        b"S { e: W(2.5), x: 2 }\nW(2.5)\nS { e: W(2.5), x: 2 }\nV(3)\n",
    );
    assert_runs(
        "enum E { V(i64), W(f64) }\nfn main() {\n    let a: Box<[E; 2]> = box [V(1), V(2)];\n    let e1 = V(4);\n    let e3 = W(5.5);\n    a = box [e1, e3];\n    println(*a);\n    println(e1);\n    println(e3);\n}\n",
        "array-reinit",
        b"[V(4), W(5.5)]\nV(4)\nW(5.5)\n",
    );
}

#[test]
fn runtime_enum_fields_are_materialized() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nstruct S { e: E }\nfn main() {\n    let x: i64 = 7;\n    let e = V(x);\n    let b: Box<S> = box S { e: e };\n    println(*b);\n    println(e);\n    let e2 = V(1 + 2);\n    let b2: Box<S> = box S { e: e2 };\n    println(*b2);\n    println(e2);\n    let e3 = V(-x);\n    let b3: Box<S> = box S { e: e3 };\n    println(*b3);\n    println(e3);\n    let e4 = W(f64::NAN);\n    let b4: Box<S> = box S { e: e4 };\n    println(*b4);\n    println(e4);\n    let e5 = W(f64::MAX);\n    let b5: Box<S> = box S { e: e5 };\n    println(*b5);\n    println(e5);\n}\n",
        "runtime",
        b"S { e: V(7) }\nV(7)\nS { e: V(3) }\nV(3)\nS { e: V(-7) }\nV(-7)\nS { e: W(nan) }\nW(nan)\nS { e: W(1.7976931348623157e+308) }\nW(1.7976931348623157e+308)\n",
    );
}

#[test]
fn array_runtime_and_mixed_leaf() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nfn main() {\n    let x: i64 = 7;\n    let e = V(x);\n    let w = W(2.5);\n    let b: Box<[E; 2]> = box [e, w];\n    println(*b);\n    println([e, w]);\n    let c: Box<[E; 2]> = box [V(1), e];\n    println(*c);\n    println([V(1), e]);\n    println(e);\n}\n",
        "array-runtime",
        b"[V(7), W(2.5)]\n[V(7), W(2.5)]\n[V(1), V(7)]\n[V(1), V(7)]\nV(7)\n",
    );
}

#[test]
fn nested_variable_leaves_stay_fail_closed() {
    // L2y：任意既有复合深度的变量 enum 叶转正（5 条）。
    assert_runs(
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e } };\n    println(*b);\n}\n",
        "nested-struct",
        b"S2 { inner: S1 { e: V(1) } }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { a: [E; 2] }\nfn main() {\n    let e1 = V(1);\n    let e2 = V(2);\n    let b: Box<S> = box S { a: [e1, e2] };\n    println(*b);\n}\n",
        "struct-enum-array",
        b"S { a: [V(1), V(2)] }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<[S; 1]> = box [S { e: e }];\n    println(*b);\n}\n",
        "array-of-struct",
        b"[S { e: V(1) }]\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let b: Box<S2> = box S2 { inner: S1 { e: V(1) } };\n    let e = V(2);\n    b = box S2 { inner: S1 { e: e } };\n    println(*b);\n}\n",
        "nested-reinit",
        b"S2 { inner: S1 { e: V(2) } }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<[S; 1]> = box [S { e: V(1) }];\n    let e = V(2);\n    b = box [S { e: e }];\n    println(*b);\n}\n",
        "array-struct-reinit",
        b"[S { e: V(2) }]\n",
    );
}

#[test]
fn scalar_leaves_run_and_whole_payload_stays_fail_closed() {
    // L2z：复合树内变量标量叶已开放（原同源负控翻转）。
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let v: i64 = 1;\n    let b: Box<S> = box S { x: v };\n    println(*b);\n}\n",
        "struct-scalar-var",
        b"S { x: 1 }\n",
    );
    assert_runs(
        "fn main() {\n    let a: i64 = 1;\n    let c: i64 = 2;\n    let b: Box<[i64; 2]> = box [a, c];\n    println(*b);\n}\n",
        "array-scalar-var",
        b"[1, 2]\n",
    );
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<[i64; 1]> = box [x];\n    println(*b);\n}\n",
        "array-scalar-one",
        b"[1]\n",
    );
    // L3b：顶层整棵常量复合变量载荷已开放（原负控翻转）。
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let s = S { x: 1 };\n    let b: Box<S> = box s;\n    println(*b);\n}\n",
        "whole-struct",
        b"S { x: 1 }\n",
    );
    assert_runs(
        "fn main() {\n    let a: [i64; 3] = [1, 2, 3];\n    let b: Box<[i64; 3]> = box a;\n    println(*b);\n}\n",
        "whole-array",
        b"[1, 2, 3]\n",
    );
    // L2z：标量复合重初始化已开放（原同源负控翻转）。
    assert_runs(
        "fn main() {\n    let b: Box<[i64; 1]> = box [1];\n    let x: i64 = 2;\n    b = box [x];\n    println(*b);\n}\n",
        "scalar-reinit",
        b"[2]\n",
    );
}

#[test]
fn expression_constructor_leaves_stay_fail_closed() {
    assert_rc1(
        "constructor-field",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x: i64 = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "constructor-arith",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1 + 2) };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "constructor-array",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<[E; 1]> = box [V(1 + 2)];\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
}

#[test]
fn seen_non_scalar_and_nested_box_stay_fail_closed() {
    assert_rc1(
        "seen-repeat",
        "enum E { V(i64) }\nstruct S { a: E, b: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { a: e, b: e };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box-field",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nstruct S { e: E }\nfn main() {\n    let e = V(P { x: 1 });\n    let b: Box<S> = box S { e: e };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "nested-box",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<Box<S>> = box box S { e: V(1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn read_and_ownership_boundaries_stay_fail_closed() {
    assert_rc1(
        "let-deref",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e };\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
    assert_rc1(
        "print-box",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e };\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e };\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e };\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn variable_leaf_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i64), W(f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, x: 1 };\n    println(*b);\n    let e2 = W(2.5);\n    b = box S { e: e2, x: 2 };\n    println(*b);\n    let a: Box<[E; 2]> = box [e, e2];\n    println(*a);\n}\n",
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
