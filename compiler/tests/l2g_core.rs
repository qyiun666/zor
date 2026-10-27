//! L2g 核心集成测试：typed Box 五宽扩容（i8／i16／i32／u16／u64）的注解构造、
//! `println(*b)`、重赋值、move／unbox、`let v = *b` 与零扩容负控（X-01～X-15）；
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
            "zorcc-l2g-{}-{}-{}-{}",
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
fn five_widths_print_zero_and_max() {
    assert_runs(
        "fn main() {\n    let a: Box<i8> = box 0;\n    let b: Box<i8> = box 127;\n    println(*a);\n    println(*b);\n}\n",
        "i8",
        b"0\n127\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<i16> = box 0;\n    let b: Box<i16> = box 32767;\n    println(*a);\n    println(*b);\n}\n",
        "i16",
        b"0\n32767\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<i32> = box 0;\n    let b: Box<i32> = box 2147483647;\n    println(*a);\n    println(*b);\n}\n",
        "i32",
        b"0\n2147483647\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u16> = box 0;\n    let b: Box<u16> = box 65535;\n    println(*a);\n    println(*b);\n}\n",
        "u16",
        b"0\n65535\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u64> = box 0;\n    let b: Box<u64> = box 18446744073709551615;\n    println(*a);\n    println(*b);\n}\n",
        "u64",
        b"0\n18446744073709551615\n",
    );
}

#[test]
fn five_widths_mixed_and_repeated_in_one_call() {
    assert_runs(
        "fn main() {\n    let a: Box<i8> = box 1;\n    let b: Box<i16> = box 2;\n    let c: Box<i32> = box 3;\n    let d: Box<u16> = box 4;\n    let e: Box<u64> = box 18446744073709551615;\n    println(*a, *b, *c, *d, *e);\n    println(*e, *e, *a);\n    println(*d, 7);\n}\n",
        "mixed",
        b"1 2 3 4 18446744073709551615\n18446744073709551615 18446744073709551615 1\n4 7\n",
    );
}

#[test]
fn five_widths_reassignment_uses_declared_width() {
    assert_runs(
        "fn main() {\n    let b: Box<i8> = box 1;\n    b = box 127;\n    println(*b);\n}\n",
        "re-i8",
        b"127\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i16> = box 1;\n    b = box 32767;\n    println(*b);\n}\n",
        "re-i16",
        b"32767\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i32> = box 1;\n    b = box 2147483647;\n    println(*b);\n}\n",
        "re-i32",
        b"2147483647\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u16> = box 1;\n    b = box 65535;\n    println(*b);\n}\n",
        "re-u16",
        b"65535\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u64> = box 1;\n    b = box 18446744073709551615;\n    println(*b);\n}\n",
        "re-u64",
        b"18446744073709551615\n",
    );
}

#[test]
fn five_widths_move_unbox_and_scope_drop() {
    assert_runs(
        "fn main() {\n    let a: Box<i16> = box 9;\n    let b = a;\n    println(*b);\n    unbox b;\n}\n",
        "move-unbox",
        b"9\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<u64> = box 18446744073709551615;\n    let b: Box<i8> = box 127;\n    println(*a, *b);\n}\n",
        "scope-drop",
        b"18446744073709551615 127\n",
    );
    assert_runs(
        "fn main() {\n    let a: Box<i8> = box 1;\n    let b: Box<i8> = box 2;\n    a = b;\n    println(*a);\n    unbox a;\n}\n",
        "overwrite-move",
        b"2\n",
    );
}

#[test]
fn let_deref_copy_for_five_widths() {
    // `let v = *b` 仅核心单测；不进入 stdout 契约计数。
    assert_runs(
        "fn main() {\n    let b: Box<i8> = box 127;\n    let v = *b;\n    let w = v;\n    println(v, w, *b);\n    unbox b;\n}\n",
        "copy-i8",
        b"127 127 127\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i16> = box 32767;\n    let v = *b;\n    println(v);\n}\n",
        "copy-i16",
        b"32767\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i32> = box 2147483647;\n    let v = *b;\n    println(v, 1);\n}\n",
        "copy-i32",
        b"2147483647 1\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u16> = box 65535;\n    let v: u16 = *b;\n    println(v);\n}\n",
        "copy-u16",
        b"65535\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u64> = box 18446744073709551615;\n    let v = *b;\n    println(v, *b);\n}\n",
        "copy-u64",
        b"18446744073709551615 18446744073709551615\n",
    );
}

#[test]
fn default_box_literal_stays_i64() {
    assert_runs(
        "fn main() {\n    let b = box 9223372036854775807;\n    println(*b);\n}\n",
        "default-i64",
        b"9223372036854775807\n",
    );
}

#[test]
fn five_widths_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "fn main() {\n    let a: Box<i8> = box 127;\n    let b: Box<i16> = box 32767;\n    let c: Box<i32> = box 2147483647;\n    let d: Box<u16> = box 65535;\n    let e: Box<u64> = box 18446744073709551615;\n    println(*a, *b, *c, *d, *e);\n    unbox a;\n    unbox b;\n    unbox c;\n    unbox d;\n    unbox e;\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(
            r.stdout,
            b"127 32767 2147483647 65535 18446744073709551615\n",
            "{flag}"
        );
        assert!(r.stderr.is_empty(), "{flag}");
    }
    assert_no_temp(d.path());
}

fn assert_rc1(tag: &str, src: &str, token: &str) {
    let d = TempDir::new(tag);
    let path = write(d.path(), &format!("{tag}.zor"), src);
    let out = d.path().join(format!("out-{tag}"));
    let b = run_in(
        d.path(),
        &[path.to_str().unwrap(), "-o", out.to_str().unwrap()],
    );
    let err = stderr_text(&b);
    assert_eq!(b.status.code(), Some(1), "{tag} stderr={err}");
    assert!(!out.exists(), "{tag} 不得留下产物");
    assert!(err.contains(token), "{tag} 缺少判词 {token} → {err}");
    assert!(!err.contains("panic"), "{tag} 不得 panic: {err}");
    assert!(
        !err.contains("C 编译器"),
        "{tag} 不得退化为 cc 失败: {err}"
    );
    assert_no_temp(d.path());
}

#[test]
fn l2g_negative_cases_are_rc1_and_fail_closed() {
    // L3a：Box<i8> 变量载荷已开放（原 n42 负控翻转）。
    assert_runs(
        "fn main() {\n    let x: i8 = 1;\n    let b: Box<i8> = box x;\n    println(*b);\n}\n",
        "n42",
        b"1\n",
    );
    // L3b：顶层整棵常量复合变量载荷已开放（原 x14c 负控翻转）。
    assert_runs(
        "struct S { x: i8 }\nfn main() { let s = S { x: 1 }; let b: Box<S> = box s; println(*b); }",
        "x14c",
        b"S { x: 1 }\n",
    );
    let cases: [(&str, &str, &str); 13] = [
        (
            "n36",
            "fn main() {\n    let b: Box<i8> = box 128;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n37",
            "fn main() {\n    let b: Box<i16> = box 32768;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n38",
            "fn main() {\n    let b: Box<i32> = box 2147483648;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n39",
            "fn main() {\n    let b: Box<u16> = box 65536;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n40",
            "fn main() {\n    let b: Box<u64> = box 18446744073709551616;\n    println(*b);\n}\n",
            "E1201",
        ),
        (
            "n41",
            "fn main() {\n    let b: Box<i8> = box -1;\n    println(*b);\n}\n",
            "box 载荷非整型字面量",
        ),
        (
            "n43",
            "fn main() {\n    let x: i8 = 1 + 1;\n    println(x);\n}\n",
            "E1201",
        ),
        (
            "n44",
            "fn main() {\n    let x: u64 = 1 + 1;\n    println(x);\n}\n",
            "E1201",
        ),
        (
            "n45",
            "fn f(b: Box<i8>) {\n    println(*b);\n}\nfn main() {\n    f(box 1);\n}\n",
            "额外函数",
        ),
        (
            "n46",
            "fn f() -> Box<i8> {\n    box 1\n}\nfn main() {\n    let b = f();\n    println(*b);\n}\n",
            "额外函数",
        ),
        (
            "n47",
            "struct Holder { b: Box<i8> }\nfn main() {\n    let h = Holder { b: box 1 };\n    println(h);\n}\n",
            "let 初始化",
        ),
        (
            "n48",
            "fn main() {\n    let b: Box<i32> = box 1;\n    println((*b, 1));\n}\n",
            "println 实参不可打印",
        ),
        (
            "n49",
            "fn main() {\n    let b: Box<i8> = box 1;\n    b = box 128;\n    println(*b);\n}\n",
            "E1201",
        ),
    ];
    for (tag, src, token) in cases {
        assert_rc1(tag, src, token);
    }
}

#[test]
fn zero_expansion_matrix_is_rc1() {
    // X-01～X-15：只开 Box 路径；非 Box 算术／比较、非 i64 Box 签名与复合打印保持 rc=1。
    let cases: [(&str, &str, &str); 22] = [
        ("x01", "fn main() { let x: i8 = 1 + 1; println(x); }", "E1201"),
        ("x02", "fn main() { let x: u64 = 1 + 1; println(x); }", "E1201"),
        ("x03", "fn main() { let b: Box<i8> = box 128; println(*b); }", "E1201"),
        ("x04", "fn main() { let b: Box<i16> = box 32768; println(*b); }", "E1201"),
        ("x05", "fn main() { let b: Box<i32> = box 2147483648; println(*b); }", "E1201"),
        ("x06", "fn main() { let b: Box<u16> = box 65536; println(*b); }", "E1201"),
        (
            "x07",
            "fn main() { let b: Box<u64> = box 18446744073709551616; println(*b); }",
            "E1201",
        ),
        (
            "x08",
            "fn main() { let b: Box<i8> = box -1; println(*b); }",
            "box 载荷非整型字面量",
        ),
        (
            "x09",
            "fn main() { let x: u8 = 1; let b: Box<i8> = box x; println(*b); }",
            "E1201",
        ),
        (
            "x10",
            "fn main() { let b: Box<i8> = box 1; println(b); }",
            "println 实参不可打印",
        ),
        (
            "x11",
            "fn main() { let b: Box<i8> = box 1; println((*b, 1)); }",
            "println 实参不可打印",
        ),
        (
            "x12",
            "fn main() { let b: Box<i8> = box 1; let v = *b; println(v + 1); }",
            "codegen",
        ),
        (
            "x13a",
            "fn main() { let b: Box<byte> = box 0; println(*b); }",
            "E1201",
        ),
        (
            "x13b",
            "fn main() { let b: Box<bool> = box true; println(*b); }",
            "let 类型标注",
        ),
        (
            "x13c",
            "fn main() { let b: Box<f32> = box 1; println(*b); }",
            "E1201",
        ),
        (
            "x13d",
            "fn main() { let b: Box<f64> = box 1; println(*b); }",
            "E1201",
        ),
        (
            "x13e",
            "fn main() { let b: Box<unit> = box (); println(*b); }",
            "let 类型标注",
        ),
        (
            "x14a",
            "fn main() { let b: Box<Box<i8>> = box 1; println(*b); }",
            "E1201",
        ),
        (
            "x14b",
            "fn main() { let b: Box<[i8; 3]> = box 1; println(*b); }",
            "E1201",
        ),
        (
            "x15a",
            "fn f(b: Box<i8>) { println(*b); }\nfn main() { f(box 1); }",
            "额外函数",
        ),
        (
            "x15b",
            "fn f() -> Box<i8> { box 1 }\nfn main() { let b = f(); println(*b); }",
            "额外函数",
        ),
        (
            "x15c",
            "struct H { b: Box<i8> }\nfn main() { let h = H { b: box 1 }; println(h); }",
            "let 初始化",
        ),
    ];
    for (tag, src, token) in cases {
        assert_rc1(tag, src, token);
    }
}
