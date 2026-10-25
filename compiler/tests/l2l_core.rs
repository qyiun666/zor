//! L2l 核心集成测试：特殊值字面量直接写入**单载荷变体**实参（表面形式对齐）。
//!
//! 覆盖：f64／f32 六形态直接形式、别名绑定（含标注）、多变体／无载荷变体／多实参、
//! 直接↔绑定↔别名逐行同构、绑定路径零回退；以及嵌套直接／跨宽／一元负号／`E::V`／
//! `Box<enum>`／扩展常量／算术比较的 fail-closed 红线（多字段直接形式自 L2m 起放行，
//! 边界见 l2m_core）。
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
            "zorcc-l2l-{}-{}-{}-{}",
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
fn six_direct_forms_print_in_contract_format() {
    assert_runs(
        "enum E { A(f64), B(f64), C(f64), D(f32), F(f32), G(f32) }\nfn main() {\n    println(A(f64::NAN));\n    println(B(f64::INFINITY));\n    println(C(f64::NEG_INFINITY));\n    println(D(f32::NAN));\n    println(F(f32::INFINITY));\n    println(G(f32::NEG_INFINITY));\n}\n",
        "six",
        b"A(nan)\nB(inf)\nC(-inf)\nD(nan)\nF(inf)\nG(-inf)\n",
    );
}

#[test]
fn direct_forms_are_isomorphic_with_binding_forms() {
    assert_pairs_isomorphic(
        "iso",
        "enum E { V(f64), W(f32) }\nfn main() {\n    println(V(f64::NAN));\n    let a = f64::NAN;\n    println(V(a));\n    println(V(f64::INFINITY));\n    let b = f64::INFINITY;\n    println(V(b));\n    println(V(f64::NEG_INFINITY));\n    let c = f64::NEG_INFINITY;\n    println(V(c));\n    println(W(f32::NAN));\n    let d = f32::NAN;\n    println(W(d));\n    println(W(f32::INFINITY));\n    let e = f32::INFINITY;\n    println(W(e));\n    println(W(f32::NEG_INFINITY));\n    let f = f32::NEG_INFINITY;\n    println(W(f));\n}\n",
    );
}

#[test]
fn alias_binding_prints_special_payload() {
    assert_runs(
        "enum E { V(f64), W(f32) }\nfn main() {\n    let e = V(f64::NAN);\n    println(e, e);\n    let f: E = W(f32::NEG_INFINITY);\n    println(f);\n}\n",
        "alias",
        b"V(nan) V(nan)\nW(-inf)\n",
    );
}

#[test]
fn multi_variant_multi_arg_and_unit_variant_zero_regression() {
    assert_runs(
        "enum E { A(f64), B(f32), N() }\nfn main() {\n    println(N());\n    println(A(f64::NAN), B(f32::INFINITY));\n    println(A(f64::NEG_INFINITY));\n}\n",
        "multi",
        b"N\nA(nan) B(inf)\nA(-inf)\n",
    );
}

#[test]
fn finite_payloads_zero_regression() {
    assert_runs(
        "enum E { V(f64) }\nfn main() {\n    println(V(1.5));\n    println(V(-0.0));\n    println(V(1e999));\n    println(V(1e-999));\n}\n",
        "finite",
        b"V(1.5)\nV(-0)\nV(inf)\nV(0)\n",
    );
}

#[test]
fn binding_path_zero_regression() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    let x = f64::NAN;\n    println(V(x));\n    let e = V(x);\n    println(e);\n    println(S { e: V(x) });\n}\n",
        "binding",
        b"V(nan)\nV(nan)\nS { e: V(nan) }\n",
    );
}

#[test]
fn multi_field_direct_form_prints_after_l2m() {
    // L2m（PMO 批准）：多字段直接特殊值形式转正；边界见 l2m_core。
    assert_runs(
        "enum E { V(f64, f64) }\nfn main() {\n    println(V(f64::NAN, 1.5));\n    println(V(1.5, f64::INFINITY));\n}\n",
        "pair",
        b"V(nan, 1.5)\nV(1.5, inf)\n",
    );
}

#[test]
fn nested_direct_forms_print_after_l2n() {
    // L2n（PMO 批准）：嵌套 struct／tuple／array 与外层别名的直接形式转正。
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n    println((V(f64::NAN), 1));\n    println([V(f64::NAN)]);\n    let s = S { e: V(f64::NAN) };\n    println(s);\n}\n",
        "nested",
        b"S { e: V(nan) }\n(V(nan), 1)\n[V(nan)]\nS { e: V(nan) }\n",
    );
}

#[test]
fn width_and_payload_kind_are_strict() {
    assert_rc1(
        "cross-f64",
        "enum E { V(f32) }\nfn main() {\n    println(V(f64::NAN));\n}\n",
        "E1201",
    );
    assert_rc1(
        "cross-f32",
        "enum E { V(f64) }\nfn main() {\n    println(V(f32::NAN));\n}\n",
        "E1201",
    );
    assert_rc1(
        "int",
        "enum E { V(i64) }\nfn main() {\n    println(V(f64::NAN));\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool",
        "enum E { V(bool) }\nfn main() {\n    println(V(f64::NAN));\n}\n",
        "E1201",
    );
}

#[test]
fn unary_minus_and_path_syntax_stay_fail_closed() {
    assert_rc1(
        "unary",
        "enum E { V(f64) }\nfn main() {\n    println(V(-f64::NAN));\n}\n",
        "一元 '-' 不接受特殊值",
    );
    assert_rc1(
        "path",
        "enum E { V(f64) }\nfn main() {\n    let e = E::V(f64::NAN);\n    println(1);\n}\n",
        "E1101",
    );
}

#[test]
fn box_enum_and_unprintable_variants_stay_fail_closed() {
    assert_rc1(
        "box-enum",
        "enum E { A(f64) }\nfn main() {\n    let b: Box<E> = box A(1.5);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-variant",
        "enum E { A(f64), B(Box<i64>) }\nfn main() {\n    println(A(f64::NAN));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "box-variant-binding",
        "enum E { A(f64), B(Box<i64>) }\nfn main() {\n    let x = f64::NAN;\n    println(A(x));\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn float_arith_and_comparison_stay_fail_closed() {
    assert_rc1(
        "arith",
        "fn main() {\n    println(f64::NAN + 1.5);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "cmp",
        "fn main() {\n    println(f64::NAN == 1.5);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "payload-arith",
        "enum E { V(f64) }\nfn main() {\n    println(V(1.5 + 1.5));\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn direct_forms_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { A(f64), B(f32) }\nfn main() {\n    println(A(f64::NAN));\n    println(B(f32::NEG_INFINITY));\n}\n",
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
