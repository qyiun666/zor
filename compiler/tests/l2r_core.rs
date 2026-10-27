//! L2r 核心集成测试：单层限定枚举变体构造 `E::V(args...)`（含零实参 `E::V()`）。
//!
//! 覆盖：标量／绑定（标注与无标注）／struct 字段／数组／元组／多实参／既有嵌套／
//! enum-in-enum；与裸变体逐字节同构；A/B 同名变体消歧与标注错误；
//! 裸 `E::V`／单位变体作值／多级路径／模块／关联项／非枚举／变体不存在 → E1101；
//! 错元数 E1601、跨宽 E1201、整→浮点 E1202、bool→浮点 E1201；
//! `Box<enum>`／含 enum 字段 Box／嵌套 Box／变量载荷／反方向维持 fail-closed；
//! 六档 `-O*` 稳定。只依赖宿主 `cc`；不访问网络。

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
            "zorcc-l2r-{}-{}-{}-{}",
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

/// 同构断言：源码按「限定形式 → 裸变体形式」成对打印，逐对行相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 限定与裸变体须同字节: {out:?}");
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
fn qualified_construction_positions() {
    assert_runs(
        "enum A { V(i64), N }\nenum B { V(f64), N }\nenum I { V(i64) }\nenum O { P(I) }\nstruct S { e: B }\nfn main() {\n    println(B::V(1.5));\n    println(B::N());\n    let a: B = B::V(1.5);\n    println(a);\n    let b = B::V(2.5);\n    println(b);\n    println(S { e: B::V(3.5) });\n    println([B::V(1.5), B::N()]);\n    println((B::V(1.5), 1));\n    println(B::V(1.5), B::N());\n    println(O::P(I::V(7)));\n    println(A::V(9));\n}\n",
        "positions",
        b"V(1.5)\nN\nV(1.5)\nV(2.5)\nS { e: V(3.5) }\n[V(1.5), N]\n(V(1.5), 1)\nV(1.5) N\nP(V(7))\nV(9)\n",
    );
}

#[test]
fn qualified_matches_bare_variant() {
    assert_pairs_isomorphic(
        "iso",
        "enum E { V(f64), N }\nenum I { W(i64) }\nenum O { P(I) }\nfn main() {\n    println(E::V(1.5));\n    println(V(1.5));\n    println(E::N());\n    println(N());\n    println(O::P(I::W(7)));\n    println(P(W(7)));\n}\n",
    );
}

#[test]
fn let_binding_and_zero_arg_annotation() {
    assert_runs(
        "enum E { V(f64), N }\nfn main() {\n    let a: E = E::V(1.5);\n    println(a);\n    let b = E::V(2.5);\n    println(b);\n    let c: E = E::N();\n    println(c);\n}\n",
        "binding",
        b"V(1.5)\nV(2.5)\nN\n",
    );
}

#[test]
fn bare_qualified_and_noncall_stay_e1101() {
    // 裸 `E::V`（无括号）不作值。
    assert_rc1(
        "bare",
        "enum E { V(i64) }\nfn main() {\n    println(E::V);\n}\n",
        "E1101",
    );
    // 单位变体 `E::W` 作值不纳入（零载荷仅 `E::W()`）。
    assert_rc1(
        "unit-value",
        "enum E { V(i64), W }\nfn main() {\n    println(E::W);\n}\n",
        "E1101",
    );
    // 多级路径。
    assert_rc1(
        "multi-level",
        "enum A { V(i64) }\nfn main() {\n    println(A::V::X(1));\n}\n",
        "E1101",
    );
    // 未知枚举。
    assert_rc1(
        "unknown-enum",
        "enum E { V(i64) }\nfn main() {\n    println(X::V(1));\n}\n",
        "E1101",
    );
    // 非枚举限定（struct）。
    assert_rc1(
        "non-enum",
        "struct S { f: i64 }\nfn main() {\n    println(S::V(1));\n}\n",
        "E1101",
    );
    // 变体不属于该枚举。
    assert_rc1(
        "unknown-variant",
        "enum E { V(i64) }\nfn main() {\n    println(E::X(1));\n}\n",
        "E1101",
    );
    // 关联项／关联函数。
    assert_rc1(
        "associated",
        "enum E { V(i64) }\nfn main() {\n    println(E::new());\n}\n",
        "E1101",
    );
}

#[test]
fn arity_and_type_fail_closed() {
    assert_rc1(
        "arity-less",
        "enum E { V(i64) }\nfn main() {\n    println(E::V());\n}\n",
        "E1601",
    );
    assert_rc1(
        "arity-more",
        "enum E { V(i64) }\nfn main() {\n    println(E::V(1, 2));\n}\n",
        "E1601",
    );
    assert_rc1(
        "cross-width",
        "enum E { V(f32) }\nfn main() {\n    println(E::V(f64::NAN));\n}\n",
        "E1201",
    );
    assert_rc1(
        "int-to-float",
        "enum E { V(f64) }\nfn main() {\n    println(E::V(1));\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool-to-float",
        "enum E { V(f64) }\nfn main() {\n    println(E::V(true));\n}\n",
        "E1201",
    );
}

#[test]
fn annotation_mismatch_is_e1201() {
    assert_rc1(
        "annotation",
        "enum A { V(i64) }\nenum B { V(f64) }\nfn main() {\n    let x: A = B::V(1.5);\n    println(x);\n}\n",
        "E1201",
    );
}

#[test]
fn box_and_variable_payload_stay_fail_closed() {
    // L2t：`Box<E>` 常量子集转正（正例矩阵见 l2t_core）。
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box E::V(1);\n    println(*b);\n}\n",
        "box-enum-l2t",
        b"V(1)\n",
    );
    // L2w：常量复合 enum 叶子转正（Box<S{e:E}>，限定变体）。
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: E::V(1) };\n    println(*b);\n}\n",
        "box-struct-enum-field",
        b"S { e: V(1) }\n",
    );
    assert_rc1(
        "nested-box",
        "fn main() {\n    let b: Box<Box<i64>> = box box 1;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "variable-payload",
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
}

#[test]
fn reverse_direction_stays_fail_closed() {
    assert_rc1(
        "reverse-struct",
        "struct P { f: f64 }\nstruct T { e: E }\nenum E { V(P) }\nfn main() { println(T { e: V(P { f: f64::NAN }) }); }\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "reverse-array",
        "struct T { a: [E; 1] }\nenum E { V([f64; 2]) }\nfn main() { println(T { a: [V([f64::NAN, 1.0])] }); }\n",
        "println 实参不可打印",
    );
}

#[test]
fn qualified_forms_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(f64), N }\nenum I { V(i64) }\nenum O { P(I) }\nstruct S { e: E }\nfn main() {\n    println(E::V(1.5));\n    println(E::N());\n    println(S { e: E::V(2.5) });\n    println(O::P(I::V(7)));\n}\n",
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
