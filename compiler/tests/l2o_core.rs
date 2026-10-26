//! L2o 核心集成测试：命名有限常量（`MAX`／`MIN`／`EPSILON`，f64／f32）。
//!
//! 覆盖：标量打印、绑定（含无注解）、Box 标量／复合载荷与重赋值、枚举直接与嵌套、
//! 复合字段／数组／元组、多实参、常量↔字面量同构；`MIN = -MAX`（Rust 语义）；
//! 以及宽度严格、其他 `::` 成员 E1101、一元负号、算术／比较、反方向与 `Box<enum>`
//! 的 fail-closed 红线与六档稳定性。
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
            "zorcc-l2o-{}-{}-{}-{}",
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

/// 同构断言：源码按「常量形式 → 等值字面量形式」成对打印，逐对行相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 常量与等值字面量须同字节: {out:?}");
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
fn six_constants_scalar_print() {
    assert_runs(
        "fn main() {\n    println(f64::MAX);\n    println(f64::MIN);\n    println(f64::EPSILON);\n    println(f32::MAX);\n    println(f32::MIN);\n    println(f32::EPSILON);\n}\n",
        "scalar",
        b"1.7976931348623157e+308\n-1.7976931348623157e+308\n2.220446049250313e-16\n3.4028235e+38\n-3.4028235e+38\n1.1920929e-07\n",
    );
}

#[test]
fn bindings_with_and_without_annotation() {
    assert_runs(
        "fn main() {\n    let a = f64::MAX;\n    println(a);\n    let b: f32 = f32::EPSILON;\n    println(b);\n    let c = a;\n    println(c);\n}\n",
        "binding",
        b"1.7976931348623157e+308\n1.1920929e-07\n1.7976931348623157e+308\n",
    );
}

#[test]
fn box_scalar_payload_and_reinit() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::MAX;\n    println(*a);\n    let b: Box<f32> = box f32::MIN;\n    println(*b);\n    a = box f64::EPSILON;\n    println(*a);\n}\n",
        "box",
        b"1.7976931348623157e+308\n-3.4028235e+38\n2.220446049250313e-16\n",
    );
}

#[test]
fn box_composite_payload() {
    assert_runs(
        "struct S { f: f64 }\nfn main() {\n    let b: Box<S> = box S { f: f64::MAX };\n    println(*b);\n    let c: Box<[f64; 2]> = box [f64::EPSILON, 1.0];\n    println(*c);\n}\n",
        "box-composite",
        b"S { f: 1.7976931348623157e+308 }\n[2.220446049250313e-16, 1]\n",
    );
}

#[test]
fn enum_direct_and_nested_forms() {
    assert_runs(
        "enum E { V(f64), W(f64, f64) }\nenum F { A(f32) }\nstruct S { e: E, f: F }\nfn main() {\n    println(V(f64::MAX));\n    println(W(f64::MAX, f64::MIN));\n    println(S { e: V(f64::EPSILON), f: A(f32::MAX) });\n    println([V(f64::MAX)]);\n    println((A(f32::EPSILON), 1));\n}\n",
        "enum",
        b"V(1.7976931348623157e+308)\nW(1.7976931348623157e+308, -1.7976931348623157e+308)\nS { e: V(2.220446049250313e-16), f: A(3.4028235e+38) }\n[V(1.7976931348623157e+308)]\n(A(1.1920929e-07), 1)\n",
    );
}

#[test]
fn composite_fields_and_multi_arg() {
    assert_runs(
        "struct S { f: f64 }\nfn main() {\n    println(S { f: f64::MAX });\n    println([f64::MAX, 1.0]);\n    println((f64::MIN, 1));\n    println(f64::MAX, f32::EPSILON);\n}\n",
        "composite",
        b"S { f: 1.7976931348623157e+308 }\n[1.7976931348623157e+308, 1]\n(-1.7976931348623157e+308, 1)\n1.7976931348623157e+308 1.1920929e-07\n",
    );
}

#[test]
fn constants_match_equivalent_literals() {
    assert_pairs_isomorphic(
        "iso-f64",
        "fn main() {\n    println(f64::MAX);\n    println(1.7976931348623157e308);\n    println(f64::MIN);\n    println(-1.7976931348623157e308);\n    println(f64::EPSILON);\n    println(2.220446049250313e-16);\n}\n",
    );
    assert_pairs_isomorphic(
        "iso-f32",
        "fn main() {\n    let a: Box<f32> = box f32::MAX;\n    println(*a);\n    let b: Box<f32> = box 3.4028235e38;\n    println(*b);\n    let c: Box<f32> = box f32::EPSILON;\n    println(*c);\n    let d: Box<f32> = box 1.1920929e-7;\n    println(*d);\n}\n",
    );
}

#[test]
fn width_is_strict_for_constants() {
    assert_rc1(
        "cross-let",
        "fn main() {\n    let x: f32 = f64::MAX;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "cross-box",
        "fn main() {\n    let b: Box<f32> = box f64::MAX;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "int",
        "fn main() {\n    let x: i64 = f64::MAX;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool",
        "fn main() {\n    let x: bool = f64::MAX;\n    println(x);\n}\n",
        "E1201",
    );
}

#[test]
fn other_float_members_stay_e1101() {
    // L2p：`MIN_POSITIVE` 已转正（见 l2p_core）；L2q：`MAX_EXP` 已转正（见 l2q_core）；
    // 其余未列名成员与裸名仍 E1101。
    for (tag, member) in [
        ("dig", "DIG"),
        ("neg-nan", "NEG_NAN"),
        ("nan-call", "NAN(1)"),
        ("lower-nan", "nan"),
    ] {
        assert_rc1(
            tag,
            &format!("fn main() {{\n    println(f64::{member});\n}}\n"),
            "E1101",
        );
    }
}

#[test]
fn unary_minus_and_arithmetic_stay_fail_closed() {
    assert_rc1(
        "unary",
        "fn main() {\n    println(-f64::MAX);\n}\n",
        "一元 '-' 不接受特殊值",
    );
    assert_rc1(
        "arith",
        "fn main() {\n    println(f64::MAX + 1.0);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "cmp",
        "fn main() {\n    println(f64::MAX > 1.0);\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn reverse_direction_box_enum_and_path_stay_fail_closed() {
    assert_rc1(
        "reverse",
        "struct P { f: f64 }\nenum E { V(P, f64) }\nfn main() {\n    println(V(P { f: f64::MAX }, 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "box-enum",
        "enum E { V(f64) }\nfn main() {\n    let b: Box<E> = box V(f64::MAX);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "path",
        "enum E { V(f64) }\nfn main() {\n    let e = E::V(f64::MAX);\n    println(e);\n}\n",
        "E1101",
    );
}

#[test]
fn constants_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(f64) }\nfn main() {\n    println(f64::MAX);\n    println(V(f64::MIN));\n}\n",
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
