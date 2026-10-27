//! L3a 核心集成测试：⑤e-1 顶层标量 Box 变量载荷＋重初始化。
//!
//! 覆盖：10 个 Box 标量宽度（运行期 i64／u8／u32；编译期 i8／i16／i32／u16／u64／
//! f32／f64）；变量来源（字面量／算术／`*b`／特殊值／命名常量）；Copy、多 Box、
//! move／unbox／drop；live 重初始化与 unbox 后重初始化；括号分组等价；六档 `-O*`；
//! 以及 bool、整棵变量载荷、表达式载荷、无标注、deref 窄宽度、嵌套 Box、enum 含 Box、
//! 读／所有权边界。

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
            "zorcc-l3a-{}-{}-{}-{}",
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

/// 同构断言：每两行一组（`println(*b)`／`println(x)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} Box 与变量须同字节: {out:?}");
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
fn all_widths_variable_payload_match_print() {
    assert_pairs_isomorphic(
        "widths",
        "fn main() {\n    let a: i8 = -5;\n    let b: i16 = -300;\n    let c: i32 = -70000;\n    let d: i64 = 7;\n    let e: u8 = 255;\n    let f: u16 = 65535;\n    let g: u32 = 4000000000;\n    let h: u64 = 42;\n    let i: f32 = 2.5;\n    let j: f64 = 1.5;\n    let ba: Box<i8> = box a;\n    let bb: Box<i16> = box b;\n    let bc: Box<i32> = box c;\n    let bd: Box<i64> = box d;\n    let be: Box<u8> = box e;\n    let bf: Box<u16> = box f;\n    let bg: Box<u32> = box g;\n    let bh: Box<u64> = box h;\n    let bi: Box<f32> = box i;\n    let bj: Box<f64> = box j;\n    println(*ba);\n    println(a);\n    println(*bb);\n    println(b);\n    println(*bc);\n    println(c);\n    println(*bd);\n    println(d);\n    println(*be);\n    println(e);\n    println(*bf);\n    println(f);\n    println(*bg);\n    println(g);\n    println(*bh);\n    println(h);\n    println(*bi);\n    println(i);\n    println(*bj);\n    println(j);\n}\n",
    );
}

#[test]
fn runtime_widths_from_arithmetic_and_deref() {
    assert_runs(
        "fn main() {\n    let x = 1 + 2;\n    let b: Box<i64> = box x;\n    println(*b);\n    println(x);\n    let a: u8 = 200;\n    let y = a + 40;\n    let c: Box<u8> = box y;\n    println(*c);\n    println(y);\n    let p: u32 = 100;\n    let z = p + 200;\n    let d: Box<u32> = box z;\n    println(*d);\n    println(z);\n    let b0: Box<i64> = box 9;\n    let w = *b0;\n    let e: Box<i64> = box w;\n    println(*e);\n    println(w);\n    let b1: Box<u8> = box 7;\n    let v = *b1;\n    let f: Box<u8> = box v;\n    println(*f);\n    println(v);\n}\n",
        "runtime",
        b"3\n3\n240\n240\n300\n300\n9\n9\n7\n7\n",
    );
    assert_pairs_isomorphic(
        "float-specials",
        "fn main() {\n    let a = f64::NAN;\n    let b: Box<f64> = box a;\n    println(*b);\n    println(a);\n    let c = f64::MAX;\n    let d: Box<f64> = box c;\n    println(*d);\n    println(c);\n    let e = f64::MIN_POSITIVE;\n    let f: Box<f64> = box e;\n    println(*f);\n    println(e);\n}\n",
    );
}

#[test]
fn copy_multiple_boxes_move_unbox_and_scope_drop() {
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b1: Box<i64> = box x;\n    let b2: Box<i64> = box x;\n    println(*b1);\n    println(*b2);\n    println(x);\n    let c = b1;\n    println(*c);\n    let d: Box<i64> = box 9;\n    unbox d;\n    println(x);\n}\n",
        "copy-move",
        b"1\n1\n1\n1\n1\n",
    );
}

#[test]
fn reinit_live_and_after_unbox_keep_originals() {
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x;\n    let y: i64 = 2;\n    b = box y;\n    println(*b);\n    println(y);\n    let a: Box<u8> = box 1;\n    let z: u8 = 2;\n    a = box z;\n    println(*a);\n    println(z);\n    let f: Box<f64> = box 1.5;\n    let g: f64 = 2.5;\n    f = box g;\n    println(*f);\n    println(g);\n}\n",
        "live-reinit",
        b"2\n2\n2\n2\n2.5\n2.5\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    unbox b;\n    let x: i64 = 3;\n    b = box x;\n    println(*b);\n    println(x);\n}\n",
        "unbox-reinit",
        b"3\n3\n",
    );
}

#[test]
fn paren_grouping_is_equivalent() {
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box (x);\n    let c: Box<i64> = box ((x));\n    println(*b);\n    println(*c);\n    println(x);\n}\n",
        "paren",
        b"1\n1\n1\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    assert_rc1(
        "bool-box",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    // L3b：顶层整棵常量复合变量载荷／重初始化已开放（原负控翻转）。
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<P> = box p;\n    println(*b);\n}\n",
        "whole-struct",
        b"P { x: 1 }\n",
    );
    assert_runs(
        "fn main() {\n    let a: [i64; 3] = [1, 2, 3];\n    let b: Box<[i64; 3]> = box a;\n    println(*b);\n}\n",
        "whole-array",
        b"[1, 2, 3]\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let p = P { x: 2 };\n    b = box p;\n    println(*b);\n}\n",
        "whole-reinit",
        b"P { x: 2 }\n",
    );
    assert_rc1(
        "neg-expression",
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box -x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "paren-expression",
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box (x + 1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "add-expression",
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x + 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "unannotated",
        "fn main() {\n    let x: i64 = 1;\n    let b = box x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "deref-narrow",
        "fn main() {\n    let b0: Box<i32> = box 7;\n    let x = *b0;\n    let b: Box<i32> = box x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "width-mismatch",
        "fn main() {\n    let x: u8 = 1;\n    let b: Box<i8> = box x;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "nested-box",
        "fn main() {\n    let b: Box<Box<i64>> = box box 1;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "println-box",
        "fn main() {\n    let b: Box<i64> = box 1;\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "fn main() {\n    let b: Box<i64> = box 1;\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "fn main() {\n    let b: Box<i64> = box 1;\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn scalar_box_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "fn main() {\n    let x: i64 = 1;\n    let f: f64 = 1.5;\n    let b: Box<i64> = box x;\n    println(*b);\n    let c: Box<f64> = box f;\n    println(*c);\n    let y: i64 = 2;\n    b = box y;\n    println(*b);\n}\n",
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
