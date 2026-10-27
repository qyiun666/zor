//! L2y 核心集成测试：⑤c 任意既有复合深度的变量 enum 叶。
//!
//! 覆盖：2／3 层嵌套 struct、struct 内 enum 数组、struct 数组变量叶、嵌套 enum
//! 数组、struct 内 struct 数组、常量／变量混排、嵌套重初始化（struct／array）、
//! 运行期 `V(x)`／`V(1+2)`、Copy、move／unbox／drop、打印等价、六档 `-O*`；
//! 以及表达式构造叶、变量标量叶、整棵变量载荷、变量标量重初始化、嵌套 Box、
//! enum 含 Box、`seen` 同类重复、非标量 enum、无标注、读／所有权边界。

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
            "zorcc-l2y-{}-{}-{}-{}",
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
fn nested_struct_two_and_three_levels() {
    assert_pairs_isomorphic(
        "nested-struct",
        "enum E { V(i64), W(f64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nstruct S3 { outer: S2 }\nfn main() {\n    let e = V(1);\n    let o: Box<S2> = box S2 { inner: S1 { e: e, x: 1 } };\n    println(*o);\n    println(S2 { inner: S1 { e: e, x: 1 } });\n    let o3: Box<S3> = box S3 { outer: S2 { inner: S1 { e: e, x: 2 } } };\n    println(*o3);\n    println(S3 { outer: S2 { inner: S1 { e: e, x: 2 } } });\n}\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e, x: 1 } };\n    println(*b);\n    println(e);\n}\n",
        "copy",
        b"S2 { inner: S1 { e: V(1), x: 1 } }\nV(1)\n",
    );
}

#[test]
fn struct_enum_array_and_array_of_structs() {
    assert_pairs_isomorphic(
        "nested-array",
        "enum E { V(i64), W(f64) }\nstruct SA { a: [E; 2] }\nstruct SB { e: E }\nfn main() {\n    let e1 = V(1);\n    let e2 = W(2.5);\n    let sa: Box<SA> = box SA { a: [e1, e2] };\n    println(*sa);\n    println(SA { a: [e1, e2] });\n    let arr: Box<[SB; 2]> = box [SB { e: e1 }, SB { e: e2 }];\n    println(*arr);\n    println([SB { e: e1 }, SB { e: e2 }]);\n}\n",
    );
}

#[test]
fn nested_arrays_and_struct_array_of_structs() {
    assert_pairs_isomorphic(
        "deep",
        "enum E { V(i64), W(f64) }\nstruct S1 { e: E, x: i64 }\nstruct SC { a: [S1; 2] }\nfn main() {\n    let e1 = V(1);\n    let e2 = W(2.5);\n    let na: Box<[[E; 2]; 2]> = box [[e1, e2], [e2, e1]];\n    println(*na);\n    println([[e1, e2], [e2, e1]]);\n    let sc: Box<SC> = box SC { a: [S1 { e: e1, x: 1 }, S1 { e: e2, x: 2 }] };\n    println(*sc);\n    println(SC { a: [S1 { e: e1, x: 1 }, S1 { e: e2, x: 2 }] });\n}\n",
    );
}

#[test]
fn mixed_constants_and_variables_at_depth() {
    assert_pairs_isomorphic(
        "mixed-depth",
        "enum E { V(i64) }\nenum F { Q(f64) }\nstruct Inner { q: i64 }\nstruct S { e: E, inner: Inner, a: [F; 2], x: i64 }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e, inner: Inner { q: 2 }, a: [Q(1.5), Q(2.5)], x: 9 };\n    println(*b);\n    println(S { e: e, inner: Inner { q: 2 }, a: [Q(1.5), Q(2.5)], x: 9 });\n}\n",
    );
}

#[test]
fn nested_reinit_move_unbox_and_scope_drop() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nstruct SB { e: E }\nfn main() {\n    let b: Box<S2> = box S2 { inner: S1 { e: V(0), x: 0 } };\n    let e2 = W(2.5);\n    b = box S2 { inner: S1 { e: e2, x: 2 } };\n    println(*b);\n    println(e2);\n    let c = b;\n    println(*c);\n    let d: Box<S2> = box S2 { inner: S1 { e: e2, x: 3 } };\n    unbox d;\n    println(e2);\n    let a: Box<[SB; 1]> = box [SB { e: V(1) }];\n    let e3 = V(3);\n    a = box [SB { e: e3 }];\n    println(*a);\n    println(e3);\n}\n",
        "nested-reinit",
        b"S2 { inner: S1 { e: W(2.5), x: 2 } }\nW(2.5)\nS2 { inner: S1 { e: W(2.5), x: 2 } }\nW(2.5)\n[SB { e: V(3) }]\nV(3)\n",
    );
}

#[test]
fn runtime_enum_fields_at_depth() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nfn main() {\n    let x: i64 = 7;\n    let e = V(x);\n    let b: Box<S2> = box S2 { inner: S1 { e: e, x: 1 } };\n    println(*b);\n    println(S2 { inner: S1 { e: e, x: 1 } });\n    println(e);\n    let e2 = V(1 + 2);\n    let b2: Box<S2> = box S2 { inner: S1 { e: e2, x: 2 } };\n    println(*b2);\n    println(S2 { inner: S1 { e: e2, x: 2 } });\n    println(e2);\n    let e3 = W(f64::NAN);\n    let b3: Box<S2> = box S2 { inner: S1 { e: e3, x: 3 } };\n    println(*b3);\n    println(S2 { inner: S1 { e: e3, x: 3 } });\n    println(e3);\n}\n",
        "runtime-depth",
        b"S2 { inner: S1 { e: V(7), x: 1 } }\nS2 { inner: S1 { e: V(7), x: 1 } }\nV(7)\nS2 { inner: S1 { e: V(3), x: 2 } }\nS2 { inner: S1 { e: V(3), x: 2 } }\nV(3)\nS2 { inner: S1 { e: W(nan), x: 3 } }\nS2 { inner: S1 { e: W(nan), x: 3 } }\nW(nan)\n",
    );
}

#[test]
fn nested_fail_closed_leaves() {
    assert_runs(
        "enum E { V(i64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nfn main() {\n    let v: i64 = 1;\n    let b: Box<S2> = box S2 { inner: S1 { e: V(1), x: v } };\n    println(*b);\n}\n",
        "nested-scalar",
        b"S2 { inner: S1 { e: V(1), x: 1 } }\n",
    );
    assert_rc1(
        "nested-expression",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let x: i64 = 1;\n    let b: Box<S2> = box S2 { inner: S1 { e: V(x) } };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "nested-whole-payload",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let s1 = S1 { e: V(1) };\n    let b: Box<S2> = box S2 { inner: s1 };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_runs(
        "struct S { a: [i64; 1] }\nfn main() {\n    let v: i64 = 1;\n    let b: Box<S> = box S { a: [v] };\n    println(*b);\n}\n",
        "array-scalar-leaf",
        b"S { a: [1] }\n",
    );
    assert_rc1(
        "array-whole-payload",
        "struct S { a: [i64; 1] }\nfn main() {\n    let av: [i64; 1] = [1];\n    let b: Box<S> = box S { a: av };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nfn main() {\n    let b: Box<S2> = box S2 { inner: S1 { e: V(1), x: 1 } };\n    let v: i64 = 2;\n    b = box S2 { inner: S1 { e: V(1), x: v } };\n    println(*b);\n}\n",
        "nested-scalar-reinit",
        b"S2 { inner: S1 { e: V(1), x: 2 } }\n",
    );
    assert_rc1(
        "nested-whole-reinit",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let b: Box<S2> = box S2 { inner: S1 { e: V(1) } };\n    let s1 = S1 { e: V(2) };\n    b = box S2 { inner: s1 };\n    println(*b);\n}\n",
        "赋值语句",
    );
}

#[test]
fn direct_fail_closed_leaves_and_reinit() {
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
    // L3a：顶层标量 Box 变量载荷已开放（原 whole-scalar-box 负控翻转）。
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x;\n    println(*b);\n}\n",
        "whole-scalar-box",
        b"1\n",
    );
    assert_rc1(
        "whole-array",
        "fn main() {\n    let a: [i64; 3] = [1, 2, 3];\n    let b: Box<[i64; 3]> = box a;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    // L3a：顶层标量重初始化已开放（原 scalar-reinit 负控翻转）。
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    unbox b;\n    let x: i64 = 2;\n    b = box x;\n    println(*b);\n}\n",
        "scalar-reinit",
        b"2\n",
    );
    assert_rc1(
        "whole-reinit",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let p = P { x: 2 };\n    b = box p;\n    println(*b);\n}\n",
        "赋值语句",
    );
}

#[test]
fn nested_box_seen_non_scalar_and_unannotated() {
    assert_rc1(
        "nested-box",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<Box<S>> = box box S { e: e };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-array-of-box",
        "fn main() {\n    let b: Box<[Box<i64>; 1]> = box [box 1];\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box-field",
        "enum E { V(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let e = V(box 1);\n    let b: Box<S> = box S { e: e };\n    println(*b);\n}\n",
        "let 初始化复合值不可打印或形态不支持",
    );
    assert_rc1(
        "seen-repeat",
        "enum E { V(i64) }\nstruct S { a: E, b: E }\nfn main() {\n    let e1 = V(1);\n    let e2 = V(2);\n    let b: Box<S> = box S { a: e1, b: e2 };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nstruct S { e: E }\nfn main() {\n    let e = V(P { x: 1 });\n    let b: Box<S> = box S { e: e };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "unannotated-nested",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b = box S2 { inner: S1 { e: e } };\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
}

#[test]
fn read_and_ownership_boundaries_stay_fail_closed() {
    assert_rc1(
        "let-deref",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e } };\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
    assert_rc1(
        "print-box",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e } };\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e } };\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "enum E { V(i64) }\nstruct S1 { e: E }\nstruct S2 { inner: S1 }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e } };\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn nested_variable_leaf_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i64), W(f64) }\nstruct S1 { e: E, x: i64 }\nstruct S2 { inner: S1 }\nstruct SB { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S2> = box S2 { inner: S1 { e: e, x: 1 } };\n    println(*b);\n    let e2 = W(2.5);\n    b = box S2 { inner: S1 { e: e2, x: 2 } };\n    println(*b);\n    let a: Box<[SB; 2]> = box [SB { e: e }, SB { e: e2 }];\n    println(*a);\n}\n",
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
