//! L3c 核心集成测试：⑤e-3 整棵变量载荷通用化。
//!
//! 覆盖：任意位置（顶层／嵌套字段／数组元素）、任意来源（字面量／Path 复制／
//! 遮蔽后绑定）、任意既有可打印叶（运行期 i64／u8／u32、编译期 bool／f64 等、
//! enum 变量叶、常量叶、嵌套 struct／array）、绑定内表达式叶＋临时局部稳定性、
//! 重初始化、Copy-in 快照、move／unbox／drop、六档；以及直接表达式载荷、
//! `box *b`／`box p.x`／`box V(x)`、deref 窄宽度、bool 顶层、无标注、嵌套 Box、
//! enum 含 Box／非标量、`seen`、读／所有权边界。

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
            "zorcc-l3c-{}-{}-{}-{}",
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

/// 同构断言：每两行一组（`println(*b)`／`println(p)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} Box 与整棵变量须同字节: {out:?}");
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
fn runtime_expression_leaf_and_temp_local_stability() {
    // 绑定内表达式叶 + 中间表达式 + box：临时局部必须稳定，不被后续表达式覆写。
    assert_runs(
        "struct P { x: i64, n: i64 }\nfn main() {\n    let p = P { x: 1 + 2, n: 7 };\n    let y = 3 + 4;\n    let b: Box<P> = box p;\n    println(*b);\n    println(p);\n    println(y);\n}\n",
        "temp-stability",
        b"P { x: 3, n: 7 }\nP { x: 3, n: 7 }\n7\n",
    );
    assert_pairs_isomorphic(
        "runtime-leaf",
        "struct P { x: i64 }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let b: Box<P> = box p;\n    println(*b);\n    println(p);\n}\n",
    );
    assert_pairs_isomorphic(
        "deref-runtime",
        "struct P { a: i64, b: u8 }\nfn main() {\n    let b0: Box<i64> = box 7;\n    let b1: Box<u8> = box 9;\n    let a = *b0;\n    let c = *b1;\n    let p = P { a: a, b: c };\n    let bp: Box<P> = box p;\n    println(*bp);\n    println(p);\n}\n",
    );
}

#[test]
fn path_copies_and_nested_whole_fields() {
    assert_pairs_isomorphic(
        "path-copy",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let q = p;\n    let b: Box<P> = box q;\n    println(*b);\n    println(q);\n}\n",
    );
    assert_pairs_isomorphic(
        "array-copy",
        "fn main() {\n    let a: [i64; 3] = [1, 2, 3];\n    let q = a;\n    let b: Box<[i64; 3]> = box q;\n    println(*b);\n    println(q);\n}\n",
    );
    assert_pairs_isomorphic(
        "nested-whole-field",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let i = I { x: 1 + 2 };\n    let b: Box<O> = box O { i: i };\n    println(*b);\n    println(O { i: i });\n}\n",
    );
    assert_pairs_isomorphic(
        "array-whole-field",
        "struct S { a: [i64; 2] }\nfn main() {\n    let a: [i64; 2] = [1, 2];\n    let b: Box<S> = box S { a: a };\n    println(*b);\n    println(S { a: a });\n}\n",
    );
    assert_pairs_isomorphic(
        "array-of-composites",
        "struct P { x: i64 }\nfn main() {\n    let p1 = P { x: 1 };\n    let p2 = P { x: 2 };\n    let b: Box<[P; 2]> = box [p1, p2];\n    println(*b);\n    println([p1, p2]);\n}\n",
    );
    assert_runs(
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let b: Box<O> = box O { i: I { x: 1 } };\n    let i2 = I { x: 2 };\n    b = box O { i: i2 };\n    println(*b);\n    println(O { i: i2 });\n}\n",
        "nested-reinit",
        b"O { i: I { x: 2 } }\nO { i: I { x: 2 } }\n",
    );
}

#[test]
fn enum_and_compile_time_leaves() {
    assert_pairs_isomorphic(
        "enum-leaf",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let s = S { e: e };\n    let b: Box<S> = box s;\n    println(*b);\n    println(s);\n}\n",
    );
    assert_pairs_isomorphic(
        "compile-time-leaves",
        "struct P { b: bool, f: f64 }\nfn main() {\n    let bb = true;\n    let ff = 1.5;\n    let p = P { b: bb, f: ff };\n    let bp: Box<P> = box p;\n    println(*bp);\n    println(p);\n}\n",
    );
}

#[test]
fn copy_in_snapshot_and_ownership() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b1: Box<P> = box p;\n    let p = P { x: 2 };\n    let b2: Box<P> = box p;\n    println(*b1);\n    println(*b2);\n}\n",
        "snapshot",
        b"P { x: 1 }\nP { x: 2 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b1: Box<P> = box p;\n    let b2: Box<P> = box p;\n    println(*b1);\n    println(*b2);\n    let c = b1;\n    println(*c);\n    let d: Box<P> = box p;\n    unbox d;\n    println(p);\n}\n",
        "copy-move",
        b"P { x: 1 }\nP { x: 1 }\nP { x: 1 }\nP { x: 1 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let p = P { x: 2 };\n    b = box p;\n    println(*b);\n    println(p);\n}\n",
        "reinit",
        b"P { x: 2 }\nP { x: 2 }\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    assert_rc1(
        "nested-expression",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let b: Box<O> = box O { i: I { x: 1 + 2 } };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "array-expression",
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "add-expression",
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box x + 1;\n    println(*b);\n}\n",
        "E1201",
    );
    // L3d：i64 标量表达式载荷已开放（原负控翻转）。
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box -x;\n    println(*b);\n}\n",
        "neg-expression",
        b"-1\n",
    );
    assert_rc1(
        "box-deref",
        "struct P { x: i64 }\nfn main() {\n    let b0: Box<P> = box P { x: 1 };\n    let b: Box<P> = box *b0;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    // L3d：i64 字段投影表达式载荷已开放（原负控翻转）。
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
        "box-field",
        b"1\n",
    );
    assert_rc1(
        "box-ctor",
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "deref-narrow-direct",
        "struct S { x: i8 }\nfn main() {\n    let b0: Box<i8> = box 1;\n    let x = *b0;\n    let b: Box<S> = box S { x: x };\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "deref-narrow-binding",
        "struct S { x: i8 }\nfn main() {\n    let b0: Box<i8> = box 1;\n    let x = *b0;\n    let p = S { x: x };\n    let b: Box<S> = box p;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "bool-box",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "unannotated",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b = box p;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "nested-box",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<Box<P>> = box box P { x: 1 };\n    println(*b);\n}\n",
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
        "type-mismatch",
        "struct P { x: i64 }\nstruct Q { y: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<Q> = box p;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "println-box",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn whole_payload_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct P { x: i64 }\nstruct O { i: P }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let b: Box<O> = box O { i: p };\n    println(*b);\n    let q = P { x: 4 };\n    b = box O { i: q };\n    println(*b);\n}\n",
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
