//! L2j 核心集成测试：`Box<f32>`／`Box<f64>` 承载六个浮点特殊值源形态。
//! 覆盖注解构造、`println(*b)`、多 Box／重复读、移动、局部重赋值、`unbox`／作用域 drop；
//! 有限浮点载荷、无注解、嵌套 Box、复合 Box、非 `println(*b)` 读法维持 fail-closed。
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
            "zorcc-l2j-{}-{}-{}-{}",
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
fn six_special_forms_in_boxes_print_nan_inf_neg_inf() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f64> = box f64::INFINITY;\n    let c: Box<f64> = box f64::NEG_INFINITY;\n    let d: Box<f32> = box f32::NAN;\n    let e: Box<f32> = box f32::INFINITY;\n    let f: Box<f32> = box f32::NEG_INFINITY;\n    println(*a, *b, *c);\n    println(*d, *e, *f);\n}\n",
        "six",
        b"nan inf -inf\nnan inf -inf\n",
    );
}

#[test]
fn multi_box_and_repeated_read_do_not_consume() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f32> = box f32::NEG_INFINITY;\n    println(*a, *b, *a);\n    println(*b);\n    println(*a);\n}\n",
        "multi",
        b"nan -inf nan\n-inf\nnan\n",
    );
}

#[test]
fn local_move_keeps_payload_and_prints() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b = a;\n    println(*b);\n}\n",
        "move",
        b"nan\n",
    );
}

#[test]
fn reassign_drops_old_value_and_prints_new() {
    assert_runs(
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    b = box f64::INFINITY;\n    println(*b);\n    b = box f64::NEG_INFINITY;\n    println(*b);\n}\n",
        "reassign",
        b"inf\n-inf\n",
    );
}

#[test]
fn box_move_and_overwrite_assign_print_current_payload() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f64> = box f64::INFINITY;\n    b = a;\n    println(*b);\n    let c: Box<f32> = box f32::INFINITY;\n    let d: Box<f32> = box f32::NAN;\n    c = d;\n    println(*c);\n}\n",
        "assign",
        b"nan\nnan\n",
    );
}

#[test]
fn self_assign_is_noop_and_keeps_payload() {
    assert_runs(
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    b = b;\n    println(*b);\n}\n",
        "self",
        b"nan\n",
    );
}

#[test]
fn unbox_and_scope_drop_are_single_release() {
    assert_runs(
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f32> = box f32::NEG_INFINITY;\n    unbox a;\n    unbox b;\n    println(1);\n}\n",
        "unbox",
        b"1\n",
    );
}

#[test]
fn double_unbox_and_use_after_unbox_stay_e2301() {
    assert_rc1(
        "double-unbox",
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-unbox",
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    unbox b;\n    println(*b);\n}\n",
        "E2301",
    );
}

/// L2k：`println(*b)` 与同值普通复合打印逐字节同构（不写死期望字节）。
fn assert_isomorphic(tag: &str, src: &str) {
    let d = TempDir::new(tag);
    let path = write(d.path(), "m.zor", src);
    let (b, r) = build_run(d.path(), &path, &[], tag);
    assert_eq!(b.status.code(), Some(0), "{tag} build={}", stderr_text(&b));
    let r = r.expect("artifact");
    assert_eq!(r.status.code(), Some(0), "{tag}");
    let out = String::from_utf8_lossy(&r.stdout).into_owned();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{tag} stdout={out:?}");
    assert_eq!(lines[0], lines[1], "{tag} 解引用打印须与普通打印同构");
    assert_no_temp(d.path());
}

#[test]
fn finite_float_payloads_are_supported_and_print_like_plain_values() {
    // L2k：有限浮点字面量载荷（宽度按标注收窄；上溢按既有常量求值）。
    assert_isomorphic(
        "finite-f64",
        "fn main() {\n    let b: Box<f64> = box 1.5;\n    println(1.5);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "neg-zero",
        "fn main() {\n    let b: Box<f64> = box -0.0;\n    println(-0.0);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "pos-zero",
        "fn main() {\n    let b: Box<f64> = box 0.0;\n    println(0.0);\n    println(*b);\n}\n",
    );
    assert_isomorphic(
        "finite-f32",
        "fn main() {\n    let b: Box<f32> = box 1.5;\n    let x: f32 = 1.5;\n    println(x);\n    println(*b);\n}\n",
    );
    // 红线：整型载荷进浮点 Box 仍是 E1201，不得变 E1202。
    assert_rc1(
        "int-into-f32",
        "fn main() {\n    let b: Box<f32> = box 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "int-into-f64",
        "fn main() {\n    let b: Box<f64> = box 1;\n    println(*b);\n}\n",
        "E1201",
    );
}

#[test]
fn unannotated_and_nested_box_forms_stay_fail_closed() {
    assert_rc1(
        "unannotated",
        "fn main() {\n    let b = box f64::NAN;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "box-box",
        "fn main() {\n    let b: Box<Box<i8>> = box box 1;\n    println(*b);\n}\n",
        "E1201",
    );
    // L2k：字面量复合载荷已支持；变量载荷维持 fail-closed（见新 token 负控）。
    // L3b：顶层整棵常量复合变量载荷已开放（原负控翻转）。
    assert_runs(
        "fn main() {\n    let a: [i8; 3] = [1, 2, 3];\n    let b: Box<[i8; 3]> = box a;\n    println(*b);\n}\n",
        "box-array-variable",
        b"[1, 2, 3]\n",
    );
    assert_runs(
        "struct S { x: i8 }\nfn main() {\n    let s = S { x: 1 };\n    let b: Box<S> = box s;\n    println(*b);\n}\n",
        "box-struct-variable",
        b"S { x: 1 }\n",
    );
    assert_rc1(
        "box-bool",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn width_is_strict_and_int_payload_uses_existing_codes() {
    assert_rc1(
        "cross-f64-to-f32",
        "fn main() {\n    let b: Box<f32> = box f64::NAN;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "cross-f32-to-f64",
        "fn main() {\n    let b: Box<f64> = box f32::NAN;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "int-payload",
        "fn main() {\n    let b: Box<i64> = box f64::NAN;\n    println(*b);\n}\n",
        "E1202",
    );
}

#[test]
fn non_deref_print_and_composite_reads_stay_fail_closed() {
    assert_rc1(
        "print-box",
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "tuple-deref",
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    println((*b, 1));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "let-deref",
        "fn main() {\n    let b: Box<f64> = box f64::NAN;\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
}

#[test]
fn eight_int_width_boxes_zero_regression() {
    assert_runs(
        "fn main() {\n    let a: Box<i8> = box 127;\n    let b: Box<i16> = box 32767;\n    let c: Box<i32> = box 2147483647;\n    let d: Box<i64> = box 9223372036854775807;\n    let e: Box<u8> = box 255;\n    let f: Box<u16> = box 65535;\n    let g: Box<u32> = box 4294967295;\n    let h: Box<u64> = box 18446744073709551615;\n    println(*a, *b, *c, *d);\n    println(*e, *f, *g, *h);\n}\n",
        "ints",
        b"127 32767 2147483647 9223372036854775807\n255 65535 4294967295 18446744073709551615\n",
    );
}

#[test]
fn l2i_composite_specials_zero_regression() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nfn main() {\n    println((f64::NAN, 1));\n    println([f32::INFINITY, f32::NEG_INFINITY]);\n    println(P { x: f64::NEG_INFINITY, y: 1.5 });\n}\n",
        "l2i",
        b"(nan, 1)\n[inf, -inf]\nP { x: -inf, y: 1.5 }\n",
    );
}

#[test]
fn special_boxes_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "fn main() {\n    let a: Box<f64> = box f64::NAN;\n    let b: Box<f32> = box f32::INFINITY;\n    println(*a, *b);\n    let c = a;\n    println(*c);\n    b = box f32::NEG_INFINITY;\n    println(*b);\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(r.stdout, b"nan inf\nnan\n-inf\n", "{flag}");
        assert!(r.stderr.is_empty(), "{flag}");
    }
    assert_no_temp(d.path());
}
