//! 0.2.0 首片 codegen 集成测试：`-o` 构建 → 运行 → stdout／rc 逐字节；
//! 子集外 fail-closed；CLI／路径／cc 错误 rc=2 且零残留。
//! 测试只依赖宿主 `cc` 与 `/bin/sh` 之外的普通进程能力，不访问网络。

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
            "zorcc-codegen-{}-{}-{}-{}",
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

const HELLO: &str = "fn main() {\n    let greeting = \"Hello, ZOR\";\n    println(greeting);\n}\n";

fn zorcc() -> &'static str {
    env!("CARGO_BIN_EXE_zorcc")
}

fn run(args: &[&str]) -> Output {
    Command::new(zorcc()).args(args).output().expect("spawn zorcc")
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

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn assert_no_temp(dir: &Path) {
    for n in names(dir) {
        assert!(!n.starts_with(".zorcc-tmp-"), "临时件残留: {n}");
    }
}

#[test]
fn hello_builds_runs_and_prints_exact_bytes() {
    let d = TempDir::new("hello");
    let src = write(d.path(), "hello.zor", HELLO);
    let out = d.path().join("hello");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "build stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(out.is_file(), "产物缺失");
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"Hello, ZOR\n");
    assert!(r.stderr.is_empty(), "stderr={:?}", r.stderr);
    assert_eq!(
        names(d.path()),
        vec!["hello".to_string(), "hello.zor".to_string()]
    );
}

#[test]
fn two_builds_are_reproducible_in_stdout_and_rc() {
    let mut observed = Vec::new();
    for tag in ["det1", "det2"] {
        let d = TempDir::new(tag);
        let src = write(d.path(), "hello.zor", HELLO);
        let out = d.path().join("hello");
        let b = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
        assert_eq!(b.status.code(), Some(0));
        let r = Command::new(&out).output().expect("run artifact");
        observed.push((r.status.code(), r.stdout, r.stderr));
    }
    assert_eq!(observed[0], observed[1]);
    assert_eq!(observed[0].1, b"Hello, ZOR\n");
}

#[test]
fn empty_and_utf8_strings_preserve_bytes() {
    let d = TempDir::new("bytes");
    let src = write(
        d.path(),
        "bytes.zor",
        "fn main() {\n    let empty = \"\";\n    let cn = \"你好\";\n    println(empty);\n    println(cn);\n}\n",
    );
    let out = d.path().join("bytes");
    let b = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(
        b.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&b.stderr)
    );
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"\n\xe4\xbd\xa0\xe5\xa5\xbd\n");
}

#[test]
fn literal_println_argument_is_supported() {
    let d = TempDir::new("literal");
    let src = write(d.path(), "lit.zor", "fn main() { println(\"hi\"); }\n");
    let out = d.path().join("lit");
    let b = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(b.status.code(), Some(0));
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.stdout, b"hi\n");
    assert_eq!(r.status.code(), Some(0));
}

#[test]
fn empty_main_builds_and_exits_zero() {
    let d = TempDir::new("empty");
    let src = write(d.path(), "empty.zor", "fn main() {}\n");
    let out = d.path().join("empty");
    let b = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(b.status.code(), Some(0));
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.status.code(), Some(0));
    assert!(r.stdout.is_empty());
    assert!(r.stderr.is_empty());
}

#[test]
fn unsupported_composite_leaves_no_artifact() {
    // L2d：struct 输出已在册；仍不可打印的复合（含 Box 成员）必须 rc=1、零产物。
    let d = TempDir::new("boxcomposite");
    let src = write(
        d.path(),
        "boxc.zor",
        "fn main() {\n    let b = box 1;\n    println((b, 1));\n}\n",
    );
    let out = d.path().join("boxc");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1), "前端通过、子集闸必须 rc=1");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("zorcc: codegen:"), "{err}");
    assert!(err.contains("首片不支持"), "{err}");
    assert!(
        !err.contains("E1") && !err.contains("E2") && !err.contains("E3"),
        "codegen 诊断不占 E 码: {err}"
    );
    assert!(!out.exists(), "失败不得留产物");
    assert_eq!(names(d.path()), vec!["boxc.zor".to_string()]);
}

#[test]
fn int_println_builds_and_prints_decimal() {
    let d = TempDir::new("printint");
    let src = write(d.path(), "p.zor", "fn main() {\n    println(42, -7, true);\n}\n");
    let out = d.path().join("p");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"42 -7 true\n");
    assert!(r.stderr.is_empty(), "{:?}", r.stderr);
}

#[test]
fn no_output_flag_stays_check_mode_and_writes_nothing() {
    let d = TempDir::new("check");
    write(d.path(), "hello.zor", HELLO);
    let o = run_in(d.path(), &["hello.zor"]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
    assert!(o.stderr.is_empty());
    assert_eq!(names(d.path()), vec!["hello.zor".to_string()]);
}

#[test]
fn output_flag_misuse_is_rc2_and_leaves_no_artifacts() {
    let d = TempDir::new("usage");
    let src = write(d.path(), "hello.zor", HELLO);
    let srcs = src.to_str().unwrap();

    let o = run(&[srcs, "-o"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("缺值"));

    let o = run(&["-o", "a", "-o", "b", srcs]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("重复"));

    let o = run(&[srcs, "-o", ""]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("为空"));

    let o = run(&["--output", "out.bin", srcs]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("--output"));

    let missing = d.path().join("missing").join("hello");
    let o = run(&[srcs, "-o", missing.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("输出目录不存在"));
    assert!(!d.path().join("missing").exists(), "不得创建父目录");

    let o = run(&[srcs, "-o", d.path().to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("目录"));

    let o = run(&[srcs, "-o", srcs]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("相同"));
    assert_eq!(fs::read_to_string(&src).unwrap(), HELLO);

    assert_eq!(names(d.path()), vec!["hello.zor".to_string()]);
    assert_no_temp(d.path());
}

#[test]
fn frontend_diagnostics_win_rc1_and_no_artifact() {
    let d = TempDir::new("diag");
    let src = write(d.path(), "bad.zor", "fn main() { let x: i64 = \"s\"; }\n");
    let out = d.path().join("bad");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("zorcc: E"));
    assert!(!out.exists());
    assert_eq!(names(d.path()), vec!["bad.zor".to_string()]);
}

#[test]
fn cc_failure_is_rc2_cleans_temp_and_keeps_existing_target() {
    let d = TempDir::new("ccfail");
    let src = write(d.path(), "hello.zor", HELLO);
    let out = d.path().join("hello");
    fs::write(&out, b"KEEP").expect("seed target");
    let o = Command::new(zorcc())
        .env("CC", "false")
        .args([src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .expect("spawn zorcc");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("C 编译器"));
    assert_eq!(fs::read(&out).unwrap(), b"KEEP", "既有目标不得被改动");
    assert_no_temp(d.path());
    assert_eq!(
        names(d.path()),
        vec!["hello".to_string(), "hello.zor".to_string()]
    );
}

#[test]
fn missing_cc_is_rc2_and_cleans_temp() {
    let d = TempDir::new("nocc");
    let src = write(d.path(), "hello.zor", HELLO);
    let out = d.path().join("hello");
    let o = Command::new(zorcc())
        .env("CC", "/nonexistent/zorcc-cc-probe")
        .args([src.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .expect("spawn zorcc");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("无法执行 C 编译器"));
    assert!(!out.exists());
    assert_no_temp(d.path());
}

// —— L2b 运行期 E30xx：真实构建 → 运行 → rc／stderr／stdout ——

const L2B_OVERFLOW: &str =
    "fn main() {\n    let max = 9223372036854775807;\n    let c = max + 1;\n}\n";

const L2B_DIV_ZERO: &str = "fn main() {\n    let c = 1 / 0;\n}\n";

const L2B_U8_OVERFLOW: &str =
    "fn main() {\n    let a: u8 = 250;\n    let c = a + 10;\n}\n";

fn code_of(n: u16) -> String {
    format!("E{n}")
}

fn build_and_run(tag: &str, src_text: &str, flags: &[&str]) -> (TempDir, std::process::Output) {
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
    assert_no_temp(d.path());
    let r = Command::new(&out).output().expect("run artifact");
    (d, r)
}

fn assert_terminate(r: &std::process::Output, code: u16, line: usize, col: usize, msg: &str) {
    assert_eq!(r.status.code(), Some(101));
    let s = String::from_utf8_lossy(&r.stderr);
    assert_eq!(s.matches('\n').count(), 1, "stderr 必须恰一行: {s:?}");
    assert!(s.ends_with('\n'), "stderr 必须以 LF 结尾: {s:?}");
    assert!(
        s.contains(&format!("zor: runtime error[{}]", code_of(code))),
        "{s:?}"
    );
    assert!(s.contains(&format!(":{}:{}: {}", line, col, msg)), "{s:?}");
    assert!(!s.contains("zorcc:"), "{s:?}");
    assert!(!s.contains("Sanitizer"), "{s:?}");
}

#[test]
fn l2b_checked_overflow_terminates_101_with_source_position() {
    let (_d, r) = build_and_run("l2b-check", L2B_OVERFLOW, &["-O0"]);
    assert_terminate(&r, 3001, 3, 17, "integer overflow");
    assert!(r.stdout.is_empty());
}

#[test]
fn l2b_wrap_overflow_is_zero_at_o2_and_default() {
    for flags in [&["-O2"][..], &[][..], &["-O0", "-Zoverflow=wrap"][..]] {
        let (_d, r) = build_and_run("l2b-wrap", L2B_OVERFLOW, flags);
        assert_eq!(r.status.code(), Some(0), "flags={flags:?}");
        assert!(r.stdout.is_empty());
        assert!(r.stderr.is_empty(), "{:?}", r.stderr);
    }
}

#[test]
fn l2b_explicit_overflow_mode_overrides_opt_level() {
    let (_d, r) = build_and_run("l2b-o2-check", L2B_OVERFLOW, &["-O2", "-Zoverflow=check"]);
    assert_terminate(&r, 3001, 3, 17, "integer overflow");
    let (_d, r) = build_and_run("l2b-o0-wrap", L2B_OVERFLOW, &["-O0", "-Zoverflow=wrap"]);
    assert_eq!(r.status.code(), Some(0));
}

#[test]
fn l2b_div_zero_terminates_101_in_both_modes() {
    for flags in [&["-O0"][..], &["-O2"][..], &["-O2", "-Zoverflow=check"][..]] {
        let (_d, r) = build_and_run("l2b-div0", L2B_DIV_ZERO, flags);
        assert_terminate(&r, 3002, 2, 15, "division by zero");
        assert!(r.stdout.is_empty());
    }
}

#[test]
fn l2b_flush_preserves_stdout_before_termination() {
    let src = "fn main() {\n    println(\"before\");\n    let max = 9223372036854775807;\n    let c = max + 1;\n}\n";
    let (_d, r) = build_and_run("l2b-flush", src, &["-O0"]);
    assert_terminate(&r, 3001, 4, 17, "integer overflow");
    assert_eq!(r.stdout, b"before\n");
}

#[test]
fn l2b_u8_checked_and_wrap() {
    let (_d, r) = build_and_run("l2b-u8", L2B_U8_OVERFLOW, &["-O0"]);
    assert_terminate(&r, 3001, 3, 15, "integer overflow");
    let (_d, r) = build_and_run("l2b-u8", L2B_U8_OVERFLOW, &["-O2"]);
    assert_eq!(r.status.code(), Some(0));
    assert!(r.stderr.is_empty());
}

#[test]
fn l2b_min_cases() {
    let src = "fn main() {\n    let max = 9223372036854775807;\n    let m = -max - 1;\n    let c = m % -1;\n}\n";
    for flags in [&["-O0"][..], &["-O2"][..]] {
        let (_d, r) = build_and_run("l2b-minmod", src, flags);
        assert_eq!(r.status.code(), Some(0), "MIN%-1 必须为 0: {flags:?}");
        assert!(r.stderr.is_empty());
    }
    let src = "fn main() {\n    let max = 9223372036854775807;\n    let m = -max - 1;\n    let c = m / -1;\n}\n";
    let (_d, r) = build_and_run("l2b-mindiv", src, &["-O0"]);
    assert_terminate(&r, 3001, 4, 15, "integer overflow");
    let (_d, r) = build_and_run("l2b-mindiv", src, &["-O2"]);
    assert_eq!(r.status.code(), Some(0));
}

#[test]
fn l2b_width_failclosed_is_rc1_zero_artifact() {
    for (tag, src_text) in [
        ("i8", "fn main() {\n    let a: i8 = 127;\n    let c = a + 1;\n}\n"),
        ("i16", "fn main() {\n    let a: i16 = 32767;\n    let c = a + 1;\n}\n"),
        ("i32", "fn main() {\n    let a: i32 = 2147483647;\n    let c = a + 1;\n}\n"),
        ("u16", "fn main() {\n    let a: u16 = 65535;\n    let c = a + 1;\n}\n"),
        ("u64", "fn main() {\n    let a: u64 = 18446744073709551615;\n    let c = a + 1;\n}\n"),
        ("byte", "fn main() {\n    let a: byte = 255;\n    let c = a + 1;\n}\n"),
    ] {
        for opt in ["-O0", "-O2"] {
            let d = TempDir::new(tag);
            let src = write(d.path(), "case.zor", src_text);
            let out = d.path().join("case");
            let o = run(&[opt, src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
            assert_eq!(o.status.code(), Some(1), "{tag} {opt}");
            assert!(!out.exists(), "{tag} {opt} 不得有产物");
            let s = String::from_utf8_lossy(&o.stderr);
            assert!(s.contains("zorcc: codegen:"), "{tag} {opt}: {s}");
            assert!(!s.contains("zor: runtime error"), "{tag} {opt}: {s}");
            assert_no_temp(d.path());
        }
    }
}

#[test]
fn l2b_invalid_zoverflow_and_unaccepted_opt_are_rc2_zero_artifact() {
    let d = TempDir::new("l2b-cli");
    let src = write(d.path(), "case.zor", L2B_OVERFLOW);
    for flag in [
        "-Zoverflow=bogus",
        "-Zoverflow=",
        "-Zoverflow",
        "-Og",
        "-O1",
        "-O3",
        "-Os",
    ] {
        let out = d.path().join("case-out");
        let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap(), flag]);
        assert_eq!(o.status.code(), Some(2), "{flag}");
        assert!(!out.exists(), "{flag} 不得有产物");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("未知选项"),
            "{flag}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    assert_no_temp(d.path());
}
