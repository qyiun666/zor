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
fn struct_source_is_rejected_and_leaves_no_artifact() {
    let d = TempDir::new("struct");
    let src = write(
        d.path(),
        "mark.zor",
        "struct Mark {}\n\nfn main() {\n    let m = Mark {};\n}\n",
    );
    let out = d.path().join("mark");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1), "前端通过、子集闸必须 rc=1");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("zorcc: codegen:"), "{err}");
    assert!(err.contains("首片不支持") && err.contains("struct"), "{err}");
    assert!(
        !err.contains("E1") && !err.contains("E2") && !err.contains("E3"),
        "codegen 诊断不占 E 码: {err}"
    );
    assert!(!out.exists(), "失败不得留产物");
    assert_eq!(names(d.path()), vec!["mark.zor".to_string()]);
}

#[test]
fn non_string_println_is_rejected_by_subset() {
    let d = TempDir::new("printint");
    let src = write(d.path(), "p.zor", "fn main() {\n    println(42);\n}\n");
    let out = d.path().join("p");
    let o = run(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("zorcc: codegen:") && err.contains("首片不支持"), "{err}");
    assert!(!out.exists());
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
