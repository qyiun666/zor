//! L2f 核心集成测试：typed Box 三型（i64／u8／u32）的注解构造、
//! `println(*b)`、重赋值、move／unbox、`let v = *b` 与 fail-closed 边界；
//! 含六档 `-O*` 稳定性。只依赖宿主 `cc` 与 `/bin/sh`；不访问网络。

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
            "zorcc-l2f-{}-{}-{}-{}",
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

fn stderr_text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
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

#[test]
fn typed_box_three_widths_print() {
    assert_runs(
        "fn main() {\n    let a: Box<i64> = box 0;\n    let b: Box<i64> = box 9223372036854775807;\n    println(*a);\n    println(*b);\n}\n",
        "i64",
        b"0\n9223372036854775807\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u8> = box 0;\n    let b: Box<u8> = box 255;\n    println(*a);\n    println(*b);\n}\n",
        "u8",
        b"0\n255\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u32> = box 0;\n    let b: Box<u32> = box 4294967295;\n    println(*a);\n    println(*b);\n}\n",
        "u32",
        b"0\n4294967295\n",
    );
}

#[test]
fn mixed_typed_boxes_print_in_one_call() {
    assert_runs(
        "fn main() {\n    let a: Box<i64> = box 7;\n    let b: Box<u8> = box 8;\n    let c: Box<u32> = box 9;\n    println(*a, *b, *c);\n    println(*b, 1);\n}\n",
        "mixed",
        b"7 8 9\n8 1\n",
    );
}

#[test]
fn typed_box_reassignment_uses_declared_width() {
    assert_runs(
        "fn main() {\n    let b: Box<u8> = box 1;\n    b = box 255;\n    println(*b);\n}\n",
        "re-u8",
        b"255\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u32> = box 1;\n    b = box 4294967295;\n    println(*b);\n}\n",
        "re-u32",
        b"4294967295\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    b = box 9223372036854775807;\n    println(*b);\n}\n",
        "re-i64",
        b"9223372036854775807\n",
    );
}

#[test]
fn typed_deref_copy_and_move_unbox() {
    assert_runs(
        "fn main() {\n    let b: Box<u8> = box 200;\n    let v = *b;\n    println(v);\n    unbox b;\n}\n",
        "copy-u8",
        b"200\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u32> = box 4294967295;\n    let v = *b;\n    println(v);\n    unbox b;\n}\n",
        "copy-u32",
        b"4294967295\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 42;\n    let v = *b;\n    println(v);\n}\n",
        "copy-i64",
        b"42\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u8> = box 9;\n    let b = a;\n    println(*b);\n    unbox b;\n}\n",
        "move-u8",
        b"9\n",
    );
}

#[test]
fn typed_boxes_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "fn main() {\n    let a: Box<i64> = box 7;\n    let b: Box<u8> = box 255;\n    let c: Box<u32> = box 4294967295;\n    println(*a, *b, *c);\n    unbox a;\n    unbox b;\n    unbox c;\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(r.stdout, b"7 255 4294967295\n", "{flag}");
        assert!(r.stderr.is_empty(), "{flag}");
    }
    assert_no_temp(d.path());
}

#[test]
fn invalid_typed_box_forms_are_rc1_and_fail_closed() {
    // N-21～N-25（`Box<i8>/<i16>/<i32>/<u16>/<u64> = box 1`）由 L2g 支持面承接，
    // 已按冻结件从活跃执行退休；stdout 侧资产保持 byte-identical，不在本表重复。
    // L3a：Box<u8> 变量载荷已开放（原 n20 负控翻转）。
    assert_runs(
        "fn main() {\n    let x: u8 = 1;\n    let b: Box<u8> = box x;\n    println(*b);\n}\n",
        "n20",
        b"1\n",
    );
    let cases: [(&str, &str, &str); 12] = [
        (
            "n16",
            "fn main() {\n    let b: Box<u8> = box 256;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n17",
            "fn main() {\n    let b: Box<u32> = box 4294967296;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n18",
            "fn main() {\n    let b: Box<i64> = box 9223372036854775808;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n19",
            "fn main() {\n    let b: Box<i64> = box -1;\n    println(*b);\n}\n",
            "box 载荷非整型字面量",
        ),
        (
            "n28",
            "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
            "let 类型标注",
        ),
        (
            "n29",
            "fn main() {\n    let b: Box<byte> = box 0;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n30",
            "fn main() {\n    let b: Box<unit> = box ();\n    println(*b);\n}\n",
            "let 类型标注",
        ),
        (
            "n31",
            "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    println(*b);\n}\n",
            "box 载荷须为字面量常量",
        ),
        (
            "n32",
            "fn main() {\n    let a: [i64; 3] = [1, 2, 3];\n    let b: Box<[i64; 3]> = box a;\n    println(*b);\n}\n",
            "box 载荷须为字面量常量",
        ),
        (
            "n33",
            "fn main() {\n    let b: Box<u8> = box 1;\n    println((*b, 1));\n}\n",
            "println 实参不可打印",
        ),
        (
            "n34",
            "fn main() {\n    let b: Box<u8> = box 1;\n    unbox b;\n    println(*b);\n}\n",
            "E2301",
        ),
        (
            "n35",
            "fn main() {\n    let b: Box<u8> = box 1;\n    b = box 256;\n    println(*b);\n}\n",
            "E1201",
        ),
    ];
    let d = TempDir::new("neg");
    for (tag, src, token) in cases {
        let path = write(d.path(), &format!("{tag}.zor"), src);
        let out = d.path().join(format!("out-{tag}"));
        let b = run_in(
            d.path(),
            &[path.to_str().unwrap(), "-o", out.to_str().unwrap()],
        );
        assert_eq!(b.status.code(), Some(1), "{tag} stderr={}", stderr_text(&b));
        assert!(!out.exists(), "{tag} 不得留下产物");
        let err = stderr_text(&b);
        assert!(err.contains(token), "{tag} 缺少判词 {token} → {err}");
        assert!(
            !err.contains("undeclared identifier"),
            "{tag} 不得退化为 cc 失败: {err}"
        );
    }
    assert_no_temp(d.path());
}
