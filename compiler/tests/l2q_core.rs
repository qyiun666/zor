//! L2q 核心集成测试：十个整型成员常量
//! `MAX_EXP`／`MIN_EXP`／`MANTISSA_DIGITS`／`DIGITS`／`RADIX` × f64／f32。
//!
//! 覆盖：标量、绑定（含整型标注收敛与拷贝）、Box 标量载荷与重初始化、
//! 枚举、struct／数组／元组、多实参、整型表达式（负号／算术）、
//! 常量↔同值字面量同构；未列名成员与成员调用 E1101、整型常量→浮点标注 E1202、
//! 负值／越界 E1201、标量 Box 浮点载荷 E1201、浮点成员→整型标注 E1202 回归，
//! 以及六档 `-O*` 稳定性。Box 复合载荷含成员按裁定 §3 归 ⑤：不测试、不宣称。
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
            "zorcc-l2q-{}-{}-{}-{}",
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
fn scalar_all_ten_members() {
    assert_runs(
        "fn main() {\n    println(f64::MAX_EXP);\n    println(f64::MIN_EXP);\n    println(f64::MANTISSA_DIGITS);\n    println(f64::DIGITS);\n    println(f64::RADIX);\n    println(f32::MAX_EXP);\n    println(f32::MIN_EXP);\n    println(f32::MANTISSA_DIGITS);\n    println(f32::DIGITS);\n    println(f32::RADIX);\n}\n",
        "scalar",
        b"1024\n-1021\n53\n15\n2\n128\n-125\n24\n6\n2\n",
    );
}

#[test]
fn constant_matches_equivalent_literal() {
    assert_pairs_isomorphic(
        "iso",
        "fn main() {\n    println(f64::MAX_EXP);\n    println(1024);\n    println(f64::MIN_EXP);\n    println(-1021);\n    println(f64::MANTISSA_DIGITS);\n    println(53);\n    println(f64::DIGITS);\n    println(15);\n    println(f64::RADIX);\n    println(2);\n    println(f32::MAX_EXP);\n    println(128);\n    println(f32::MIN_EXP);\n    println(-125);\n    println(f32::MANTISSA_DIGITS);\n    println(24);\n    println(f32::DIGITS);\n    println(6);\n    println(f32::RADIX);\n    println(2);\n}\n",
    );
}

#[test]
fn bindings_and_annotations() {
    assert_runs(
        "fn main() {\n    let a = f64::MAX_EXP;\n    println(a);\n    let b: i64 = f64::MAX_EXP;\n    println(b);\n    let c: i32 = f64::MIN_EXP;\n    println(c);\n    let d: u32 = f64::MAX_EXP;\n    println(d);\n    let e: i16 = f32::MAX_EXP;\n    println(e);\n    let f: u8 = f64::RADIX;\n    println(f);\n    let g: i8 = f32::MIN_EXP;\n    println(g);\n    let h = f64::MIN_EXP;\n    let i = h;\n    println(i);\n}\n",
        "binding",
        b"1024\n1024\n-1021\n1024\n128\n2\n-125\n-1021\n",
    );
}

#[test]
fn box_scalar_payloads_and_reinit() {
    assert_runs(
        "fn main() {\n    let a: Box<i32> = box f64::MIN_EXP;\n    println(*a);\n    let b: Box<i64> = box f64::MAX_EXP;\n    println(*b);\n    let c: Box<u8> = box f64::RADIX;\n    println(*c);\n    let d: Box<i8> = box f32::MIN_EXP;\n    println(*d);\n    let e: Box<u16> = box f32::MANTISSA_DIGITS;\n    println(*e);\n    let f: Box<i64> = box 0;\n    f = box f64::MIN_EXP;\n    println(*f);\n    let g: Box<i32> = box 0;\n    g = box f64::MIN_EXP;\n    println(*g);\n}\n",
        "box",
        b"-1021\n1024\n2\n-125\n24\n-1021\n-1021\n",
    );
}

#[test]
fn enum_struct_array_tuple_multi_arg() {
    assert_runs(
        "struct S { a: i16, b: u8 }\nenum E { V(i32), W(i16, i64) }\nfn main() {\n    println(V(f64::MIN_EXP));\n    println(W(f32::MIN_EXP, f32::MAX_EXP));\n    println(S { a: f64::MAX_EXP, b: f64::RADIX });\n    println([f64::MAX_EXP, f64::MIN_EXP]);\n    println((f64::MAX_EXP, f64::DIGITS));\n    println(f64::MAX_EXP, f32::MIN_EXP);\n    println([S { a: f32::RADIX, b: f32::MANTISSA_DIGITS }]);\n    println((V(f64::MAX_EXP), f64::DIGITS));\n}\n",
        "composite",
        b"V(-1021)\nW(-125, 128)\nS { a: 1024, b: 2 }\n[1024, -1021]\n(1024, 15)\n1024 -125\n[S { a: 2, b: 24 }]\n(V(1024), 15)\n",
    );
}

#[test]
fn integer_expressions() {
    assert_runs(
        "fn main() {\n    println(f64::MAX_EXP + f64::DIGITS);\n    println(f64::MAX_EXP - f64::DIGITS);\n    println(f64::MAX_EXP / f64::RADIX);\n    println(f64::MAX_EXP % f64::DIGITS);\n    println(-f64::RADIX);\n    println(-f64::MIN_EXP);\n    let a: i64 = 1;\n    println(a + f64::MAX_EXP);\n    let b: i16 = -f64::RADIX;\n    println(b);\n    let c: i16 = -f64::MIN_EXP;\n    println(c);\n}\n",
        "expr",
        b"1039\n1009\n512\n4\n-2\n1021\n1025\n-2\n1021\n",
    );
}

#[test]
fn unknown_members_and_member_calls_stay_e1101() {
    for (tag, member) in [
        ("min-10-exp", "MIN_10_EXP"),
        ("max-10-exp", "MAX_10_EXP"),
        ("dig", "DIG"),
        ("neg-nan", "NEG_NAN"),
        ("call-max-exp", "MAX_EXP()"),
        ("call-min-exp", "MIN_EXP()"),
    ] {
        assert_rc1(
            tag,
            &format!("fn main() {{\n    println(f64::{member});\n}}\n"),
            "E1101",
        );
    }
    assert_rc1(
        "call-f32-max-exp",
        "fn main() {\n    println(f32::MAX_EXP());\n}\n",
        "E1101",
    );
}

#[test]
fn float_annotation_is_e1202() {
    assert_rc1(
        "let-f64",
        "fn main() {\n    let x: f64 = f64::MAX_EXP;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "let-f32",
        "fn main() {\n    let x: f32 = f32::RADIX;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "struct-field",
        "struct S { f: f32 }\nfn main() {\n    println(S { f: f64::MAX_EXP });\n}\n",
        "E1202",
    );
    assert_rc1(
        "enum-field",
        "enum E { V(f32) }\nfn main() {\n    println(V(f64::MAX_EXP));\n}\n",
        "E1202",
    );
}

/// 裁定 v1.1 R2：标量 Box 浮点载荷不下推浮点期望——成员与整数字面量同码 E1201
/// （N-115／N-116 红线；N-268 改判 E1201）。
#[test]
fn box_float_payload_is_e1201() {
    assert_rc1(
        "box-f32",
        "fn main() {\n    let b: Box<f32> = box f64::MAX_EXP;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "box-f64",
        "fn main() {\n    let b: Box<f64> = box f32::MIN_EXP;\n    println(*b);\n}\n",
        "E1201",
    );
}

#[test]
fn range_violations_are_e1201() {
    assert_rc1(
        "u8-max-exp",
        "fn main() {\n    let x: u8 = f64::MAX_EXP;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "i8-max-exp",
        "fn main() {\n    let x: i8 = f64::MAX_EXP;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "u32-min-exp",
        "fn main() {\n    let x: u32 = f64::MIN_EXP;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "u8-f32-min-exp",
        "fn main() {\n    let x: u8 = f32::MIN_EXP;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "bool-radix",
        "fn main() {\n    let x: bool = f64::RADIX;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "box-u8-min-exp",
        "fn main() {\n    let b: Box<u8> = box f64::MIN_EXP;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "box-i8-max-exp",
        "fn main() {\n    let b: Box<i8> = box f64::MAX_EXP;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "unary-u8-radix",
        "fn main() {\n    let x: u8 = -f64::RADIX;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "unary-i8-min-exp",
        "fn main() {\n    let x: i8 = -f64::MIN_EXP;\n    println(x);\n}\n",
        "E1201",
    );
    // 源形态区分：成员可作标量 Box 载荷，一元负号形态仍 fail-closed（N-19／N-41 口径）。
    assert_rc1(
        "box-neg-min-exp",
        "fn main() {\n    let b: Box<i16> = box -f64::MIN_EXP;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
}

#[test]
fn float_members_to_int_annotation_stay_e1202() {
    assert_rc1(
        "let-int",
        "fn main() {\n    let x: i64 = f64::MAX;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "box-int",
        "fn main() {\n    let b: Box<i64> = box f32::MAX;\n    println(*b);\n}\n",
        "E1202",
    );
}

#[test]
fn members_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i32) }\nfn main() {\n    println(f64::MAX_EXP);\n    println(f64::MIN_EXP);\n    println(f32::MANTISSA_DIGITS);\n    println(V(f32::MIN_EXP));\n    println(f64::MAX_EXP + f64::DIGITS);\n}\n",
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
