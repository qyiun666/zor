//! L2e 核心集成测试：六档 `-O*`、overflow 矩阵、`--help` 固定文本、
//! `println(*b)` 的 `Box<i64>` 解引用修复与 fail-closed 边界。
//! 只依赖宿主 `cc` 与 `/bin/sh`；不访问网络。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
            "zorcc-l2e-{}-{}-{}-{}",
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

const EXPECTED_HELP: &str = "\
usage: zorcc [flags] <file.zor>
       zorcc [flags] <file.zor> -o <output>

flags:
  -o <output>       build a native executable (atomic publish)
  -O0               no optimization; checked overflow
  -O1               basic optimization; checked overflow
  -O2               standard optimization; wrapping overflow (default)
  -O3               aggressive optimization; wrapping overflow
  -Os               optimize for size; wrapping overflow
  -Og               debug-friendly optimization; checked overflow
  -g                emit debug information
  --diag=json       compile diagnostics as NDJSON on stderr
  --version, -V     print version and exit
  --help            print this help and exit
";

const OVERFLOW_SRC: &str = "fn main() {\n    let x: i64 = 9223372036854775807;\n    let y = x + 1;\n    println(y);\n}\n";
const WRAPPED_MAX: &[u8] = b"-9223372036854775808\n";

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

fn run_env(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(zorcc());
    cmd.current_dir(dir).args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn zorcc")
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

/// 构建并运行；构建非 0 时返回 (build, None)。
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

fn stdout_of(r: &Output) -> Vec<u8> {
    r.stdout.clone()
}

fn stderr_text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn opt_levels_build_run_and_are_passed_to_cc() {
    #[cfg(not(unix))]
    return;

    #[cfg(unix)]
    {
        let d = TempDir::new("opt-map");
        let src = write(
            d.path(),
            "opt.zor",
            "fn main() {\n    let x = 41 + 1;\n    println(x);\n}\n",
        );
        let wrapper = d.path().join("cc-wrapper.sh");
        fs::write(
            &wrapper,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ZOR_CC_ARGS_LOG\"\nexec cc \"$@\"\n",
        )
        .expect("write wrapper");
        let mut perm = fs::metadata(&wrapper).expect("wrapper meta").permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&wrapper, perm).expect("chmod wrapper");

        for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
            let out = d.path().join(format!("opt-{}", &flag[1..]));
            let log = d.path().join(format!("cc-{}.log", &flag[1..]));
            let _ = fs::remove_file(&out);
            let log_s = log.to_str().expect("log utf8").to_string();
            let wrapper_s = wrapper.to_str().expect("wrapper utf8").to_string();
            let b = run_env(
                d.path(),
                &[flag, src.to_str().unwrap(), "-o", out.to_str().unwrap()],
                &[("CC", wrapper_s.as_str()), ("ZOR_CC_ARGS_LOG", log_s.as_str())],
            );
            assert_eq!(
                b.status.code(),
                Some(0),
                "{flag} build stderr={}",
                stderr_text(&b)
            );
            let args = fs::read_to_string(&log).expect("cc argv log");
            assert!(
                args.lines().any(|l| l == flag),
                "{flag} 未传给 cc: {args:?}"
            );
            let r = Command::new(&out).output().expect("run artifact");
            assert_eq!(r.status.code(), Some(0), "{flag}");
            assert_eq!(stdout_of(&r), b"42\n", "{flag}");
            assert!(r.stderr.is_empty(), "{flag} stderr={:?}", r.stderr);
        }
        assert_no_temp(d.path());
    }
}

#[test]
fn invalid_opt_levels_are_rc2_and_leave_no_artifact() {
    let d = TempDir::new("opt-invalid");
    let src = write(d.path(), "ok.zor", "fn main() { println(1); }\n");
    for flag in ["-O4", "-O9", "-Ofast", "-O2x", "-O", "-Oz", "-O10"] {
        let out = d.path().join("bad.out");
        let _ = fs::remove_file(&out);
        let b = run_in(
            d.path(),
            &[flag, src.to_str().unwrap(), "-o", out.to_str().unwrap()],
        );
        assert_eq!(b.status.code(), Some(2), "{flag} stderr={}", stderr_text(&b));
        assert!(!out.exists(), "{flag} 不得留下产物");
        let err = stderr_text(&b);
        assert!(err.contains("未知选项") && err.contains(flag), "{flag} → {err}");
    }
    assert_no_temp(d.path());
}

#[test]
fn overflow_matrix_and_precedence() {
    let d = TempDir::new("overflow");
    let src = write(d.path(), "ov.zor", OVERFLOW_SRC);

    for (tag, flags) in [
        ("o0", vec!["-O0"]),
        ("o1", vec!["-O1"]),
        ("og", vec!["-Og"]),
    ] {
        let (b, r) = build_run(d.path(), &src, &flags, tag);
        assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(101), "{tag} 应 checked");
        assert!(r.stdout.is_empty());
        let err = stderr_text(&r);
        assert!(err.contains("E3001") && err.contains("integer overflow"), "{tag}: {err}");
    }

    for (tag, flags) in [
        ("o2", vec!["-O2"]),
        ("o3", vec!["-O3"]),
        ("os", vec!["-Os"]),
    ] {
        let (b, r) = build_run(d.path(), &src, &flags, tag);
        assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{tag} 应 wrap");
        assert_eq!(stdout_of(&r), WRAPPED_MAX, "{tag}");
        assert!(r.stderr.is_empty(), "{tag} stderr={:?}", r.stderr);
    }

    // last-wins 与显式 overflow 覆盖（与顺序无关）。
    let cases: [(&str, Vec<&str>, bool); 8] = [
        ("lw13", vec!["-O1", "-O3"], false),
        ("lw31", vec!["-O3", "-O1"], true),
        ("lwg2", vec!["-Og", "-O2"], false),
        ("lw2g", vec!["-O2", "-Og"], true),
        ("ovc", vec!["-O2", "-Zoverflow=check"], true),
        ("ovw", vec!["-O0", "-Zoverflow=wrap"], false),
        ("ov1w", vec!["-O1", "-Zoverflow=wrap"], false),
        ("ov3c", vec!["-O3", "-Zoverflow=check"], true),
    ];
    for (tag, flags, checked) in cases {
        let (b, r) = build_run(d.path(), &src, &flags, tag);
        assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        if checked {
            assert_eq!(r.status.code(), Some(101), "{tag} 应 checked");
            assert!(stderr_text(&r).contains("E3001"), "{tag}");
        } else {
            assert_eq!(r.status.code(), Some(0), "{tag} 应 wrap");
            assert_eq!(stdout_of(&r), WRAPPED_MAX, "{tag}");
        }
    }
}

#[test]
fn help_is_fixed_and_short_circuits() {
    let d = TempDir::new("help");
    let h = run_in(d.path(), &["--help"]);
    assert_eq!(h.status.code(), Some(0));
    assert_eq!(h.stdout, EXPECTED_HELP.as_bytes());
    assert!(h.stderr.is_empty(), "stderr={:?}", h.stderr);
    assert!(EXPECTED_HELP.len() == 706, "help bytes={}", EXPECTED_HELP.len());
    assert!(!EXPECTED_HELP.contains("-Zoverflow"));

    // 任意位置短路：非法 flag／-o 缺值／不存在源／JSON 同现。
    for args in [
        vec!["--help", "--bogus"],
        vec!["--help", "-o"],
        vec!["--help", "missing.zor"],
        vec!["--diag=json", "--help"],
        vec!["-o", "--help", "missing.zor"],
    ] {
        let o = run_in(d.path(), &args);
        assert_eq!(o.status.code(), Some(0), "{args:?}");
        assert_eq!(o.stdout, EXPECTED_HELP.as_bytes(), "{args:?}");
        assert!(o.stderr.is_empty(), "{args:?} stderr={:?}", o.stderr);
    }

    // version 优先于 help。
    for args in [
        vec!["--version", "--help"],
        vec!["--help", "--version"],
        vec!["--help", "-V"],
    ] {
        let o = run_in(d.path(), &args);
        assert_eq!(o.status.code(), Some(0), "{args:?}");
        assert_eq!(
            o.stdout,
            format!("zorcc {}\n", env!("CARGO_PKG_VERSION")).as_bytes(),
            "{args:?}"
        );
    }

    // -h 与拼写变体仍 rc=2。
    for flag in ["-h", "-help", "--help=1", "--Help"] {
        let o = run_in(d.path(), &[flag]);
        assert_eq!(o.status.code(), Some(2), "{flag}");
        assert!(o.stdout.is_empty(), "{flag}");
        assert!(stderr_text(&o).contains("未知选项"), "{flag}");
    }
    assert_no_temp(d.path());
}

#[test]
fn box_i64_deref_print_supports_read_move_rebind_unbox() {
    let d = TempDir::new("box-deref");
    let cases: [(&str, &str, &[u8]); 8] = [
        (
            "p1",
            "fn main() {\n    let b = box 42;\n    println(*b);\n}\n",
            b"42\n",
        ),
        (
            "p2",
            "fn main() {\n    let b = box 42;\n    println(*b, 1);\n}\n",
            b"42 1\n",
        ),
        (
            "p3",
            "fn main() {\n    let b = box 42;\n    println(*b);\n    println(*b);\n}\n",
            b"42\n42\n",
        ),
        (
            "p4",
            "fn main() {\n    let b = box 42;\n    let c = b;\n    println(*c);\n}\n",
            b"42\n",
        ),
        (
            "p5",
            "fn main() {\n    let b = box 42;\n    b = box 7;\n    println(*b);\n}\n",
            b"7\n",
        ),
        (
            "p6",
            "fn main() {\n    let b = box 42;\n    println(*b);\n    unbox b;\n}\n",
            b"42\n",
        ),
        (
            "p7",
            "fn main() {\n    let b = box 42;\n    let x = *b;\n    println(x);\n}\n",
            b"42\n",
        ),
        (
            "p8",
            "fn main() {\n    let b = box 42;\n    println(1, *b);\n}\n",
            b"1 42\n",
        ),
    ];
    for (tag, src, want) in cases {
        let src = write(d.path(), &format!("{tag}.zor"), src);
        let (b, r) = build_run(d.path(), &src, &[], tag);
        assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{tag}");
        assert_eq!(stdout_of(&r), want, "{tag}");
        assert!(r.stderr.is_empty(), "{tag} stderr={:?}", r.stderr);
    }
    assert_no_temp(d.path());
}

#[test]
fn box_i64_deref_print_is_stable_across_opt_levels() {
    let d = TempDir::new("box-opt");
    let src = write(
        d.path(),
        "box.zor",
        "fn main() {\n    let b = box 42;\n    println(*b, 7);\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(stdout_of(&r), b"42 7\n", "{flag}");
        assert!(r.stderr.is_empty(), "{flag}");
    }
}

#[test]
fn box_deref_negative_forms_are_rc1_and_fail_closed() {
    let d = TempDir::new("box-neg");
    let cases: [(&str, &str); 5] = [
        (
            "n1",
            "fn main() {\n    let b = box 42;\n    println(b);\n}\n",
        ),
        (
            "n2",
            "fn main() {\n    let b = box 42;\n    println(*b + 1);\n}\n",
        ),
        (
            "n3",
            "fn main() {\n    let x: u8 = 1;\n    let b = box x;\n    println(*b);\n}\n",
        ),
        (
            "n4",
            "fn main() {\n    let b = box 1;\n    let c = box b;\n    println(*c);\n}\n",
        ),
        (
            "n5",
            "fn main() {\n    let b = box 1.5;\n    println(*b);\n}\n",
        ),
    ];
    for (tag, src) in cases {
        let src = write(d.path(), &format!("{tag}.zor"), src);
        let out = d.path().join(format!("out-{tag}"));
        let b = run_in(
            d.path(),
            &[src.to_str().unwrap(), "-o", out.to_str().unwrap()],
        );
        assert_eq!(b.status.code(), Some(1), "{tag} stderr={}", stderr_text(&b));
        assert!(!out.exists(), "{tag} 不得留下产物");
        let err = stderr_text(&b);
        assert!(err.contains("zorcc: codegen:"), "{tag} → {err}");
        assert!(
            !err.contains("undeclared identifier"),
            "{tag} 不得退化为 cc 未声明变量: {err}"
        );
    }
    assert_no_temp(d.path());
}
