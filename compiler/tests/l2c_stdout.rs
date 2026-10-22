//! L2c stdout 契约／结构化 CLI 集成测试。
//!
//! 覆盖：多实参分隔符与终饰、全部原始类型格式化、`[byte;N]`、终止前原子 stdout、
//! `print`／复合 fail-closed、`--diag=json` NDJSON 六键固定序、`--version`／`-V` 冻结。

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
            "zorcc-l2c-{}-{}-{}-{}",
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

fn run(args: &[&str]) -> Output {
    Command::new(zorcc()).args(args).output().expect("spawn zorcc")
}

fn run_env(args: &[&str], key: &str, value: &str) -> Output {
    Command::new(zorcc())
        .env(key, value)
        .args(args)
        .output()
        .expect("spawn zorcc")
}

fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, src).expect("write source");
    p
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn build_run(tag: &str, src_text: &str, flags: &[&str]) -> (TempDir, Output) {
    let d = TempDir::new(tag);
    let src = write(d.path(), "case.zor", src_text);
    let out = d.path().join("case");
    let mut args: Vec<&str> = flags.to_vec();
    args.push(src.to_str().unwrap());
    args.push("-o");
    args.push(out.to_str().unwrap());
    let b = run(&args);
    assert_eq!(
        b.status.code(),
        Some(0),
        "build stderr={}",
        String::from_utf8_lossy(&b.stderr)
    );
    assert!(out.is_file(), "产物缺失");
    let r = Command::new(&out).output().expect("run artifact");
    (d, r)
}

fn build_fail(tag: &str, src_text: &str) -> (TempDir, Output, PathBuf) {
    let d = TempDir::new(tag);
    let src = write(d.path(), "case.zor", src_text);
    let out = d.path().join("case");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    (d, o, out)
}

fn assert_codegen_fail(o: &Output, out: &Path, case: &str) {
    assert_eq!(o.status.code(), Some(1), "{case}: rc");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.starts_with("zorcc: codegen:"), "{case}: {err}");
    assert!(!err.contains("E1") && !err.contains("E2") && !err.contains("E3"), "{case}: {err}");
    assert!(!out.exists(), "{case}: 不得留产物");
}

// —— S1／S2：多实参、终饰、空串 ——

#[test]
fn s1_multi_arg_separator_is_single_space() {
    for (src, want) in [
        ("fn main() { println(\"a\", \"b\"); }", "a b\n"),
        ("fn main() { println(\"a\", 1, true); }", "a 1 true\n"),
        ("fn main() { println(1, 2); }", "1 2\n"),
        ("fn main() { println(\"\", \"\"); }", " \n"),
        ("fn main() { println(\"solo\"); }", "solo\n"),
        ("fn main() { println(\"a\", \"b\", \"c\", \"d\"); }", "a b c d\n"),
    ] {
        let (_d, r) = build_run("s1", src, &[]);
        assert_eq!(r.status.code(), Some(0), "{src}");
        assert_eq!(r.stdout, want.as_bytes(), "{src}: {:?}", r.stdout);
        assert!(r.stderr.is_empty(), "{src}: {:?}", r.stderr);
    }
}

#[test]
fn s2_terminator_is_exactly_one_lf_per_call() {
    let (_d, r) = build_run("s2", "fn main() { println(1); println(\"\"); println(\"a\"); }", &[]);
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"1\n\na\n");
    assert!(!r.stdout.contains(&b'\r'));
}

// —— S3：12 原始类型格式化 ——

#[test]
fn s3_integer_min_max_all_widths() {
    let src = "fn main() {\n\
        let a: i8 = -128;\n\
        let b: i16 = -32768;\n\
        let c: i32 = -2147483648;\n\
        let d: i64 = -9223372036854775808;\n\
        let e: u8 = 255;\n\
        let f: u16 = 65535;\n\
        let g: u32 = 4294967295;\n\
        let h: u64 = 18446744073709551615;\n\
        let i: byte = 255;\n\
        println(a, b, c, d, e, f, g, h, i);\n\
    }\n";
    let (_d, r) = build_run("s3int", src, &[]);
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(
        r.stdout,
        b"-128 -32768 -2147483648 -9223372036854775808 255 65535 4294967295 18446744073709551615 255\n"
    );
}

#[test]
fn s3_bool_byte_unit() {
    let (_d, r) = build_run(
        "s3misc",
        "fn main() { let t = true; let f = false; let b: byte = 0; let u = (); println(t, f, b, u); }",
        &[],
    );
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"true false 0 ()\n");
}

const F64_PRINT: &str = "fn main() {\n\
    println(1.0, 1.5, -2.25, 0.1, 1e-4, 1e-5, 1e15, 1e16, -0.0, 1e999, -1e999);\n\
}\n";

const F64_WANT: &[u8] = b"1 1.5 -2.25 0.1 0.0001 1e-05 1000000000000000 1e+16 -0 inf -inf\n";

#[test]
fn s3_f64_contract_bytes_and_two_runs_same() {
    let (_d1, r1) = build_run("s3f64", F64_PRINT, &[]);
    let (_d2, r2) = build_run("s3f64", F64_PRINT, &[]);
    assert_eq!(r1.status.code(), Some(0));
    assert_eq!(r1.stdout, F64_WANT, "{:?}", r1.stdout);
    assert_eq!(r1.stdout, r2.stdout, "两轮字节必须一致");
}

#[test]
fn s3_f32_contract_bytes() {
    let src = "fn main() { let a: f32 = 1.1; let b: f32 = 1e6; let c: f32 = 1e7; let d: f32 = 1e-5; let e: f32 = -0.0; println(a, b, c, d, e); }";
    let (_d, r) = build_run("s3f32", src, &[]);
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"1.1 1000000 1e+07 1e-05 -0\n");
}

#[test]
fn f1_float_bytes_are_locale_independent() {
    let d = TempDir::new("f1locale");
    let src = write(d.path(), "float.zor", F64_PRINT);
    let out_c = d.path().join("float_c");
    let out_u = d.path().join("float_u");
    let args_c = [src.to_str().unwrap(), "-o", out_c.to_str().unwrap()];
    let args_u = [src.to_str().unwrap(), "-o", out_u.to_str().unwrap()];
    let b_c = run_env(&args_c, "LC_ALL", "C");
    let b_u = run_env(&args_u, "LC_ALL", "en_US.UTF-8");
    assert_eq!(b_c.status.code(), Some(0), "{}", String::from_utf8_lossy(&b_c.stderr));
    assert_eq!(b_u.status.code(), Some(0), "{}", String::from_utf8_lossy(&b_u.stderr));
    for locale in ["C", "en_US.UTF-8"] {
        let out = if locale == "C" { &out_c } else { &out_u };
        let r = Command::new(out)
            .env("LC_ALL", locale)
            .output()
            .expect("run artifact");
        assert_eq!(r.status.code(), Some(0));
        assert_eq!(r.stdout, F64_WANT, "locale={locale}: {:?}", r.stdout);
    }
}

// —— S4：[byte;N] ——

#[test]
fn s4_byte_arrays_keep_raw_bytes() {
    let src = "fn main() {\n\
        let empty = \"\";\n\
        let one = \"Z\";\n\
        let ascii = \"Hi\";\n\
        let cn = \"你好\";\n\
        let ctl = \"a\\nb\";\n\
        println(empty);\n\
        println(one);\n\
        println(ascii);\n\
        println(cn);\n\
        println(ctl);\n\
    }\n";
    let (_d, r) = build_run("s4", src, &[]);
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"\nZ\nHi\n\xe4\xbd\xa0\xe5\xa5\xbd\na\nb\n");
}

#[test]
fn s4_byte_array_with_nul_byte_is_verbatim() {
    let src = "fn main() {\n    let s = \"a\u{0}b\";\n    println(s);\n}\n";
    let (_d, r) = build_run("s4nul", src, &[]);
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"a\x00b\n");
}

#[test]
fn s4_annotated_byte_array_is_supported() {
    let (_d, r) = build_run(
        "s4ann",
        "fn main() { let s: [byte; 2] = \"hi\"; let z: [byte; 0] = \"\"; println(s); println(z); }",
        &[],
    );
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"hi\n\n");
}

// —— S6：终止前 stdout 原子性（只判 stdout） ——

#[test]
fn s6_previous_output_survives_and_failing_call_writes_nothing() {
    let src = "fn main() {\n    println(\"before\");\n    println(1 / 0);\n}\n";
    let (_d, r) = build_run("s6div", src, &[]);
    assert_eq!(r.status.code(), Some(101));
    assert_eq!(r.stdout, b"before\n");

    let src = "fn main() {\n    println(\"a\", 1 / 0);\n}\n";
    let (_d, r) = build_run("s6atomic", src, &[]);
    assert_eq!(r.status.code(), Some(101));
    assert!(r.stdout.is_empty(), "半行输出: {:?}", r.stdout);

    let src = "fn main() {\n    println(\"before\");\n    let max = 9223372036854775807;\n    println(\"a\", max + 1);\n}\n";
    let (_d, r) = build_run("s6ovf", src, &["-O0"]);
    assert_eq!(r.status.code(), Some(101));
    assert_eq!(r.stdout, b"before\n");
}

// —— N：print／复合／不可打印 fail-closed ——

#[test]
fn n_print_and_composites_are_rc1_zero_artifact_with_codegen_prefix() {
    for (tag, src) in [
        ("print", "fn main() { print(\"x\"); }\n"),
        ("print0", "fn main() { print(); }\n"),
        ("tuple", "fn main() { println((1, 2)); }\n"),
        ("struct", "struct P { x: i64 }\nfn main() { let p = P { x: 1 }; println(p); }\n"),
        ("enum", "enum E { A }\nfn main() { let e = A(); println(e); }\n"),
        ("box", "fn main() { let b = box 1; println(b); }\n"),
        ("boxstruct", "struct P { x: Box<i64> }\nfn main() { let p = P { x: box 1 }; println(p); }\n"),
        ("ref", "fn main() { let x = 1; let r = &x; println(r); }\n"),
        ("refmut", "fn main() { let x = 1; let r = &mut x; println(r); }\n"),
    ] {
        let (d, o, out) = build_fail(tag, src);
        assert_codegen_fail(&o, &out, tag);
        assert_eq!(names(d.path()), vec!["case.zor".to_string()], "{tag}");
    }
}

// —— C1：--diag=json NDJSON ——

fn two_diag_src() -> &'static str {
    "fn main() {\n    let a = missing1;\n    let b = missing2;\n}\n"
}

fn assert_six_key_order(line: &str, file: &str) {
    let keys = ["\"code\":", "\"severity\":", "\"message\":", "\"file\":", "\"line\":", "\"col\":"];
    let mut last = 0;
    for k in keys {
        let idx = line.find(k).unwrap_or_else(|| panic!("missing {k}: {line}"));
        assert!(idx >= last, "key order broken at {k}: {line}");
        last = idx;
    }
    assert!(line.starts_with('{') && line.ends_with('}'), "{line}");
    assert!(line.contains("\"severity\":\"error\""), "{line}");
    assert!(line.contains(&format!("\"file\":\"{}\"", file)), "{line}");
    assert!(!line.contains("\r"), "{line}");
    assert!(!line.contains("zorcc:"), "{line}");
}

#[test]
fn c1_ndjson_two_diagnostics_order_and_schema() {
    let d = TempDir::new("c1json");
    let src = write(d.path(), "bad.zor", two_diag_src());
    let text = run(&[src.to_str().unwrap()]);
    assert_eq!(text.status.code(), Some(1));
    let txt = String::from_utf8_lossy(&text.stderr).into_owned();
    assert_eq!(txt.lines().count(), 2, "{txt}");
    assert!(txt.lines().all(|l| l.starts_with(&format!("zorcc: E1101 {}", src.display()))), "{txt}");
    assert!(txt.find("missing1") < txt.find("missing2"), "{txt}");

    let json = run(&["--diag=json", src.to_str().unwrap()]);
    assert_eq!(json.status.code(), Some(1));
    assert!(json.stdout.is_empty(), "{:?}", json.stdout);
    let out = String::from_utf8(json.stderr).expect("utf8 stderr");
    assert!(out.ends_with('\n'));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert_six_key_order(lines[0], &src.display().to_string());
    assert_six_key_order(lines[1], &src.display().to_string());
    assert!(lines[0].contains("\"message\":\"unknown identifier 'missing1'\""), "{out}");
    assert!(lines[1].contains("\"message\":\"unknown identifier 'missing2'\""), "{out}");
    assert!(!out.contains('\u{feff}'), "不得有 BOM");
}

#[test]
fn c1_ndjson_no_diagnostics_writes_zero_bytes() {
    let d = TempDir::new("c1clean");
    let src = write(d.path(), "ok.zor", "fn main() { println(\"ok\"); }\n");
    let o = run(&["--diag=json", src.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stdout.is_empty());
    assert!(o.stderr.is_empty(), "{:?}", o.stderr);
}

#[test]
fn c1_ndjson_with_output_flag_is_rc1_zero_artifact() {
    let d = TempDir::new("c1out");
    let src = write(d.path(), "bad.zor", two_diag_src());
    let out = d.path().join("bad");
    let o = run(&["--diag=json", src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(!out.exists());
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.lines().count() == 2, "{err}");
    assert!(err.lines().all(|l| l.starts_with('{')), "{err}");
}

#[test]
fn c1_ndjson_path_escapes_and_utf8_are_valid_json() {
    let d = TempDir::new("c1path");
    let sub = d.path().join("例子");
    fs::create_dir(&sub).expect("subdir");
    let src = write(&sub, "a b.zor", two_diag_src());
    let o = run(&["--diag=json", src.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8(o.stderr).expect("utf8");
    for line in err.lines() {
        assert_six_key_order(line, &src.display().to_string());
    }
    assert!(err.contains("例子"), "{err}");
}

#[test]
fn c1_invalid_diag_value_is_rc2_text_not_json() {
    for bad in ["--diag", "--diag=text", "--diag=xml", "--diag="] {
        let o = run(&[bad, "missing.zor"]);
        assert_eq!(o.status.code(), Some(2), "{bad}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.starts_with("zorcc: CLI:"), "{bad}: {err}");
        assert!(!err.starts_with('{'), "{bad}: {err}");
    }
}

// —— C2：--version／-V 冻结 ——

#[test]
fn c2_version_is_exact_and_short_circuits() {
    let want = format!("zorcc {}\n", env!("CARGO_PKG_VERSION"));
    let o = run(&["--version"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, want.as_bytes());
    assert!(o.stderr.is_empty());

    let o = run(&["-V", "does-not-exist.zor", "--diag=json", "--bogus"]);
    assert_eq!(o.status.code(), Some(0), "版本必须短路未知 flag 与不存在源");
    assert_eq!(o.stdout, want.as_bytes());
    assert!(o.stderr.is_empty(), "版本不得输出 JSON／诊断: {:?}", o.stderr);

    let o = run(&["--version", "--diag=json"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, want.as_bytes());
    assert!(o.stderr.is_empty());
}

// —— C4：usage 不泄露内部位 ——

#[test]
fn c4_usage_is_text_and_does_not_leak_internal_flags() {
    let o = run(&[]);
    assert_eq!(o.status.code(), Some(2));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("usage: zorcc"), "{err}");
    for leak in ["-Zoverflow", "-Og", "-O1", "-O3", "-Os", "--diag=json"] {
        assert!(!err.contains(leak), "usage 泄露 {leak}: {err}");
    }
}
