//! L2m 核心集成测试：特殊值字面量直接写入**多字段变体**任意字段位置（表面形式对齐）。
//!
//! 覆盖：2／3／4 字段、首／中／末位置、特殊／有限混排、同变体多特殊值、f32／f64
//! 混排、多实参、别名（含标注）、直接↔绑定同构、O-1 有限复合字段放行；
//! 以及嵌套直接／含特殊值复合字段／跨宽／整型／bool／`E::V`／`Box<enum>`／扩展常量／
//! 算术比较的 fail-closed 红线与六档稳定性。
//! 只依赖宿主 `cc`；不访问网络。

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
            "zorcc-l2m-{}-{}-{}-{}",
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

/// 同构断言：源码按「直接形式 → 绑定形式」成对打印，逐对行相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 直接形式须与绑定形式同构: {out:?}");
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
fn two_field_any_position_prints() {
    assert_runs(
        "enum E { A(f64, f64), B(f64, f64) }\nfn main() {\n    println(A(f64::NAN, 1.5));\n    println(B(1.5, f64::INFINITY));\n}\n",
        "two",
        b"A(nan, 1.5)\nB(1.5, inf)\n",
    );
}

#[test]
fn three_field_multiple_specials_and_mixed_widths() {
    assert_runs(
        "enum E { V(f64, f32, f64), W(f32, f32) }\nfn main() {\n    println(V(f64::NAN, f32::INFINITY, f64::NEG_INFINITY));\n    println(W(f32::NAN, f32::NEG_INFINITY));\n}\n",
        "three",
        b"V(nan, inf, -inf)\nW(nan, -inf)\n",
    );
}

#[test]
fn four_fields_with_finite_scalar_mix() {
    assert_runs(
        "enum E { V(f64, f32, i64, bool) }\nfn main() {\n    println(V(f64::NAN, f32::INFINITY, 7, true));\n}\n",
        "four",
        b"V(nan, inf, 7, true)\n",
    );
}

#[test]
fn direct_forms_are_isomorphic_with_binding_forms() {
    assert_pairs_isomorphic(
        "iso",
        "enum E { V(f64, f64), W(f64, f32, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n    let a = f64::NAN;\n    println(V(a, 1.5));\n    println(V(1.5, f64::INFINITY));\n    let b = f64::INFINITY;\n    println(V(1.5, b));\n    println(W(f64::NAN, f32::NEG_INFINITY, 2.5));\n    let c = f64::NAN;\n    let d = f32::NEG_INFINITY;\n    println(W(c, d, 2.5));\n}\n",
    );
}

#[test]
fn alias_binding_prints_multi_field_payload() {
    assert_runs(
        "enum E { V(f64, f64), W(f64, f32) }\nfn main() {\n    let e = V(f64::NAN, 1.5);\n    println(e, e);\n    let f: E = W(1.5, f32::NEG_INFINITY);\n    println(f);\n}\n",
        "alias",
        b"V(nan, 1.5) V(nan, 1.5)\nW(1.5, -inf)\n",
    );
}

#[test]
fn multi_arg_and_unit_variant_zero_regression() {
    assert_runs(
        "enum E { V(f64, f64), W(f64, f64), N() }\nfn main() {\n    println(N());\n    println(V(f64::NAN, 1.5), W(1.5, 2.5));\n}\n",
        "multi",
        b"N\nV(nan, 1.5) W(1.5, 2.5)\n",
    );
}

#[test]
fn o1_finite_composite_fields_with_special_print() {
    assert_runs(
        "enum E { V((f64, f64), f64) }\nfn main() {\n    println(V((1.5, 2.5), f64::NAN));\n}\n",
        "o1-tuple",
        b"V((1.5, 2.5), nan)\n",
    );
    assert_runs(
        "struct P { f: f64 }\nenum E { V(P, f64) }\nfn main() {\n    println(V(P { f: 1.5 }, f64::NAN));\n}\n",
        "o1-struct",
        b"V(P { f: 1.5 }, nan)\n",
    );
    assert_runs(
        "enum E { V([f64; 2], f64) }\nfn main() {\n    println(V([1.5, 2.5], f64::NAN));\n}\n",
        "o1-array",
        b"V([1.5, 2.5], nan)\n",
    );
    assert_runs(
        "enum W { A(f64) }\nenum E { V(W, f64) }\nfn main() {\n    println(V(A(1.5), f64::NAN));\n}\n",
        "o1-enum",
        b"V(A(1.5), nan)\n",
    );
}

#[test]
fn finite_multi_field_and_binding_zero_regression() {
    assert_runs(
        "enum E { V(f64, f64), W(f64, i64) }\nfn main() {\n    println(V(1.5, -0.0));\n    println(W(1.5, 7));\n}\n",
        "finite",
        b"V(1.5, -0)\nW(1.5, 7)\n",
    );
    assert_runs(
        "enum E { V(f64, f64) }\nfn main() {\n    let x = f64::NAN;\n    let y = f64::NEG_INFINITY;\n    println(V(x, 1.5));\n    println(V(1.5, y));\n    println(V(x, y));\n}\n",
        "binding",
        b"V(nan, 1.5)\nV(1.5, -inf)\nV(nan, -inf)\n",
    );
}

#[test]
fn composite_field_with_special_stays_fail_closed() {
    assert_rc1(
        "struct",
        "struct P { f: f64 }\nenum E { V(P, f64) }\nfn main() {\n    println(V(P { f: f64::NAN }, 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "array",
        "enum E { V([f64; 2], f64) }\nfn main() {\n    println(V([f64::NAN, 2.5], 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "enum",
        "enum W { A(f64) }\nenum E { V(W, f64) }\nfn main() {\n    println(V(A(f64::NAN), 1.5));\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn nested_direct_forms_print_after_l2n() {
    // L2n（PMO 批准）：多字段枚举的嵌套直接形式转正。
    assert_runs(
        "enum E { V(f64, f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN, 1.5) });\n    println((V(f64::NAN, 1.5), 1));\n    println([V(f64::NAN, 1.5)]);\n}\n",
        "nested",
        b"S { e: V(nan, 1.5) }\n(V(nan, 1.5), 1)\n[V(nan, 1.5)]\n",
    );
}

#[test]
fn width_and_kind_are_strict_per_field() {
    assert_rc1(
        "cross-first",
        "enum E { V(f32, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n}\n",
        "E1201",
    );
    assert_rc1(
        "cross-second",
        "enum E { V(f64, f32) }\nfn main() {\n    println(V(1.5, f64::NAN));\n}\n",
        "E1201",
    );
    assert_rc1(
        "int",
        "enum E { V(i64, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool",
        "enum E { V(bool, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n}\n",
        "E1201",
    );
}

#[test]
fn arity_path_box_and_arith_stay_fail_closed() {
    assert_rc1(
        "arity",
        "enum E { V(f64, f64, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n}\n",
        "E1601",
    );
    assert_rc1(
        "path",
        "enum E { V(f64, f64) }\nfn main() {\n    let e = E::V(f64::NAN, 1.5);\n    println(1);\n}\n",
        "E1101",
    );
    assert_rc1(
        "box",
        "enum E { V(f64, f64) }\nfn main() {\n    let b: Box<E> = box V(1.5, 2.5);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "arith",
        "enum E { V(f64, f64) }\nfn main() {\n    println(V(f64::NAN + 1.5, 1.5));\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn direct_forms_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(f64, f64), W(f32, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n    println(W(f32::NEG_INFINITY, f64::INFINITY));\n}\n",
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
