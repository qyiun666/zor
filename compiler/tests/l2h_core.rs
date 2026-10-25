//! L2h 核心集成测试：六个特殊值源形态（`f64|f32::NAN|INFINITY|NEG_INFINITY`）的
//! 解析、宽度严格检查、`println` 输出、fail-closed 红线与既有有限浮点零回退。
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
            "zorcc-l2h-{}-{}-{}-{}",
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
fn six_special_forms_print_nan_inf_neg_inf() {
    for (tag, form, want) in [
        ("f64nan", "f64::NAN", b"nan\n".as_slice()),
        ("f64inf", "f64::INFINITY", b"inf\n"),
        ("f64ninf", "f64::NEG_INFINITY", b"-inf\n"),
        ("f32nan", "f32::NAN", b"nan\n"),
        ("f32inf", "f32::INFINITY", b"inf\n"),
        ("f32ninf", "f32::NEG_INFINITY", b"-inf\n"),
    ] {
        assert_runs(&format!("fn main() {{ println({form}); }}\n"), tag, want);
    }
}

#[test]
fn specials_mix_with_finite_and_integers_in_one_call() {
    assert_runs(
        "fn main() {\n    let a = f64::NAN;\n    let b: f32 = f32::INFINITY;\n    let c = f64::NEG_INFINITY;\n    let d: f32 = f32::NEG_INFINITY;\n    println(a, b, c, d, 1, -0.0);\n}\n",
        "mixed",
        b"nan inf -inf -inf 1 -0\n",
    );
}

#[test]
fn repeated_read_and_copy_of_special_are_stable() {
    assert_runs(
        "fn main() {\n    let x = f64::NAN;\n    println(x);\n    println(x);\n    let nan = 1.0;\n    println(nan);\n}\n",
        "repeat",
        b"nan\nnan\n1\n",
    );
    // 特殊值经 Copy 绑定后仍是直接打印面（非复合内嵌）。
    assert_runs(
        "fn main() {\n    let x = f64::NAN;\n    let y = x;\n    println(y);\n}\n",
        "copy",
        b"nan\n",
    );
}

#[test]
fn finite_zero_and_unary_regressions_stay_rc0() {
    // S11-09：零／-0 双宽、有限非字面量 -x、有限浮点复合均保持 0.3.6 行为。
    assert_runs(
        "fn main() {\n    let a: f32 = -0.0;\n    let b: f64 = 0.0;\n    let x: f64 = 1.5;\n    println(-0.0, 0.0, a, b);\n    println(-x);\n    println((1.5, 1));\n}\n",
        "finite",
        b"-0 0 -0 0\n-1.5\n(1.5, 1)\n",
    );
}

#[test]
fn existing_overflow_literals_are_preserved() {
    assert_runs(
        "fn main() { println(1e999, -1e999, 1e-999); }\n",
        "overflow",
        b"inf -inf 0\n",
    );
    // 溢出字面量经绑定后的一元负号仍走既有有限路径（非特殊值来源）。
    assert_runs(
        "fn main() {\n    let x = 1e999;\n    println(-x);\n}\n",
        "overflow-alias",
        b"-inf\n",
    );
}

#[test]
fn specials_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "fn main() {\n    let a = f64::NAN;\n    let b: f32 = f32::INFINITY;\n    let c = f64::NEG_INFINITY;\n    let d: f32 = f32::NEG_INFINITY;\n    println(a, b, c, d);\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(r.stdout, b"nan inf -inf -inf\n", "{flag}");
        assert!(r.stderr.is_empty(), "{flag}");
    }
    assert_no_temp(d.path());
}

#[test]
fn width_is_strict_both_directions() {
    assert_rc1(
        "w1",
        "fn main() {\n    let x: f32 = f64::NAN;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "w2",
        "fn main() {\n    let x: f64 = f32::NAN;\n    println(x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "w3",
        "fn main() {\n    let x: i64 = f64::NAN;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "w4",
        "fn main() {\n    let x: bool = f32::INFINITY;\n    println(x);\n}\n",
        "E1201",
    );
}

#[test]
fn unary_minus_rejects_specials_but_not_finite_bindings() {
    for (tag, form) in [
        ("u1", "f64::NAN"),
        ("u2", "f32::NAN"),
        ("u3", "f64::INFINITY"),
        ("u4", "f32::INFINITY"),
        ("u5", "f64::NEG_INFINITY"),
    ] {
        let src = format!("fn main() {{\n    let x = -{form};\n    println(x);\n}}\n");
        assert_rc1(tag, &src, "E1201");
    }
    // 裁定 3：特殊值经绑定后的一元负号同报 E1201（Stage-1 固定判词）。
    assert_rc1(
        "u6",
        "fn main() {\n    let x = f64::NAN;\n    println(-x);\n}\n",
        "E1201",
    );
    assert_rc1(
        "u7",
        "fn main() {\n    let x = f32::INFINITY;\n    let y = -x;\n    println(y);\n}\n",
        "E1201",
    );
    assert_rc1(
        "u8",
        "fn main() {\n    let x = f64::NAN;\n    let y = x;\n    println(-y);\n}\n",
        "E1201",
    );
}

#[test]
fn other_float_members_and_bare_names_are_e1101() {
    // L2o：`MAX`／`MIN`／`EPSILON` 已转正（见 l2o_core）；其余成员与裸名仍 E1101。
    for (tag, src) in [
        ("m3", "fn main() {\n    let x: f64 = f64::nan;\n    println(x);\n}\n"),
        ("m4", "fn main() {\n    let x: f64 = f64::NEG_NAN;\n    println(x);\n}\n"),
        ("m5", "fn main() {\n    let x: f64 = f64::NAN(1);\n    println(x);\n}\n"),
        ("m6", "fn main() {\n    let x = nan;\n    println(x);\n}\n"),
        ("m7", "fn main() {\n    let x = inf;\n    println(x);\n}\n"),
    ] {
        assert_rc1(tag, src, "E1101");
    }
}

#[test]
fn special_after_literal_string_is_plain_multi_arg_print() {
    // QA Stage-1 订正：`println` 为多实参字面打印，无格式串语义。
    assert_runs(
        "fn main() {\n    println(\"{}\", f64::NAN);\n}\n",
        "fmtlike",
        b"{} nan\n",
    );
}

#[test]
fn float_arithmetic_comparison_and_box_stay_fail_closed() {
    for (tag, src) in [
        ("a1", "fn main() {\n    let x: f64 = 1.0 + 2.0;\n    println(x);\n}\n"),
        ("a2", "fn main() {\n    let b: bool = 1.0 < 2.0;\n    println(b);\n}\n"),
        ("a3", "fn main() {\n    if 1.0 < 2.0 { println(1); }\n}\n"),
        ("a4", "fn main() {\n    while 2.0 < 1.0 { }\n}\n"),
        ("a5", "fn main() {\n    let b = box 1.5;\n    println(*b);\n}\n"),
    ] {
        assert_rc1(tag, src, "codegen");
    }
    // 浮点 unbox 命中既有 E1201（非新码）。
    assert_rc1(
        "a6",
        "fn main() {\n    let x = 1.5;\n    unbox x;\n}\n",
        "E1201",
    );
}
