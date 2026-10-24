//! L2i 核心集成测试：浮点特殊值作 tuple／array／struct 的常量成员，含既有嵌套
//! 形态与投影打印、混合宽度、定义序、多实参与重复读取、绑定别名；枚举特殊载荷
//! 自 L2l 起放行单载荷直接形式（见 l2l_core），浮点算术／Box 浮点／投影绑定／
//! 非法下标维持 fail-closed。
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
            "zorcc-l2i-{}-{}-{}-{}",
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
fn tuple_specials_print_in_contract_format() {
    assert_runs(
        "fn main() {\n    println((f64::NAN, 1));\n    println((f32::INFINITY, f64::NEG_INFINITY));\n    println((f64::NAN, 1.5, -0.0));\n    println(((f64::NAN, 1), 2));\n}\n",
        "tuple",
        b"(nan, 1)\n(inf, -inf)\n(nan, 1.5, -0)\n((nan, 1), 2)\n",
    );
}

#[test]
fn tuple_projection_prints_special() {
    assert_runs(
        "fn main() {\n    let t = (f64::NAN, 1);\n    println(t[0]);\n    let u: (f64, f64) = (f64::NAN, 1.5);\n    println(u[1]);\n}\n",
        "tuple-proj",
        b"nan\n1.5\n",
    );
}

#[test]
fn array_specials_print_in_contract_format() {
    assert_runs(
        "fn main() {\n    let a: [f64; 2] = [f64::NAN, 1.5];\n    println(a);\n    let b: [f32; 2] = [f32::INFINITY, f32::NEG_INFINITY];\n    println(b);\n    println([[f64::NAN], [1.5]]);\n}\n",
        "array",
        b"[nan, 1.5]\n[inf, -inf]\n[[nan], [1.5]]\n",
    );
}

#[test]
fn array_projection_prints_special() {
    assert_runs(
        "fn main() {\n    let a: [f64; 2] = [f64::NAN, 1.5];\n    println(a[0]);\n    println(a[1]);\n}\n",
        "array-proj",
        b"nan\n1.5\n",
    );
}

#[test]
fn struct_specials_print_in_declaration_order() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nfn main() {\n    println(P { x: f64::NAN, y: 1.5 });\n    println(P { y: 1.5, x: f64::NAN });\n}\n",
        "struct-order",
        b"P { x: nan, y: 1.5 }\nP { x: nan, y: 1.5 }\n",
    );
}

#[test]
fn struct_field_projection_prints_special() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nfn main() {\n    let p = P { x: f64::NAN, y: 1.5 };\n    println(p.x);\n    println(p);\n}\n",
        "struct-proj",
        b"nan\nP { x: nan, y: 1.5 }\n",
    );
}

#[test]
fn mixed_width_fields_use_each_field_width() {
    assert_runs(
        "struct Q { a: f32, b: f64 }\nfn main() {\n    println(Q { b: f64::NEG_INFINITY, a: f32::INFINITY });\n    println((f64::NAN, f32::NEG_INFINITY));\n}\n",
        "mixed-width",
        b"Q { a: inf, b: -inf }\n(nan, -inf)\n",
    );
}

#[test]
fn nested_struct_and_struct_in_tuple_print_in_contract_format() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nstruct S { inner: P, tag: i64 }\nfn main() {\n    println(S { inner: P { x: f64::NAN, y: -0.0 }, tag: 7 });\n    println((P { x: f64::NEG_INFINITY, y: 0.0 }, 3));\n}\n",
        "nested",
        b"S { inner: P { x: nan, y: -0 }, tag: 7 }\n(P { x: -inf, y: 0 }, 3)\n",
    );
}

#[test]
fn multi_arg_and_repeated_read_are_stable() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nfn main() {\n    println(\"p\", P { x: f64::NAN, y: 2.5 });\n    let p = P { x: f64::NAN, y: 1.5 };\n    println(p);\n    println(p);\n}\n",
        "repeat",
        b"p P { x: nan, y: 2.5 }\nP { x: nan, y: 1.5 }\nP { x: nan, y: 1.5 }\n",
    );
}

#[test]
fn alias_bound_special_in_composite_is_positive() {
    assert_runs(
        "struct P { x: f64 }\nfn main() {\n    let n = f64::NAN;\n    println((n, 1));\n    let i = f32::INFINITY;\n    println([i, i]);\n    let p = P { x: n };\n    println(p);\n}\n",
        "alias",
        b"(nan, 1)\n[inf, inf]\nP { x: nan }\n",
    );
}

#[test]
fn finite_composites_zero_regression() {
    assert_runs(
        "struct P { x: f64, y: f64 }\nfn main() {\n    println((1.5, -0.0));\n    println([1.5, -0.0]);\n    println(P { x: 1.5, y: -0.0 });\n}\n",
        "finite",
        b"(1.5, -0)\n[1.5, -0]\nP { x: 1.5, y: -0 }\n",
    );
}

#[test]
fn enum_single_payload_special_literal_prints() {
    // L2l（O-4 批准）：单载荷变体的特殊值字面量直接形式转正；
    // 多字段／嵌套直接形式与绑定旁路边界见 l2l_core。
    assert_runs(
        "enum E { A(f64), B(f32) }\nfn main() {\n    println(A(f64::NAN));\n    println(B(f32::NEG_INFINITY));\n}\n",
        "enum",
        b"A(nan)\nB(-inf)\n",
    );
}

#[test]
fn float_arithmetic_and_box_special_stay_fail_closed() {
    assert_rc1(
        "arith",
        "fn main() {\n    println(f64::NAN + 1.5);\n}\n",
        "codegen",
    );
    assert_rc1(
        "box",
        "fn main() {\n    let b = box f64::NAN;\n    println(*b);\n}\n",
        "codegen",
    );
}

#[test]
fn projection_binding_and_index_boundaries_stay_fail_closed() {
    assert_rc1(
        "proj-let",
        "struct P { x: f64 }\nfn main() {\n    let p = P { x: f64::NAN };\n    let y = p.x;\n    println(y);\n}\n",
        "codegen",
    );
    assert_rc1(
        "idx-nonlit",
        "fn main() {\n    let a: [f64; 2] = [f64::NAN, 1.5];\n    let i = 0;\n    println(a[i]);\n}\n",
        "codegen",
    );
    assert_rc1(
        "idx-oob",
        "fn main() {\n    let a: [f64; 2] = [f64::NAN, 1.5];\n    println(a[5]);\n}\n",
        "codegen",
    );
}

#[test]
fn width_and_int_annotation_negatives_use_existing_codes() {
    // 跨宽：E1201（宽度严格，不新增 E 码）。
    assert_rc1(
        "width",
        "fn main() {\n    let x: f32 = f64::NAN;\n    println(x);\n}\n",
        "E1201",
    );
    // 特殊值→整型：直接标注与 struct 字段均 E1202。
    assert_rc1(
        "e1202-let",
        "fn main() {\n    let x: i64 = f64::NAN;\n    println(x);\n}\n",
        "E1202",
    );
    assert_rc1(
        "e1202-field",
        "struct P { x: i64 }\nfn main() {\n    println(P { x: f64::NAN });\n}\n",
        "E1202",
    );
    // 元组标注不向下传期望：整型标注报 E1201（与既有 tuple 契约一致）。
    assert_rc1(
        "tuple-ann",
        "fn main() {\n    let t: (i64, i64) = (f64::NAN, 1);\n    println(t);\n}\n",
        "E1201",
    );
    // 数组整型标注：既有 E1201＋E1202 双判词，先命中 E1201。
    assert_rc1(
        "array-ann",
        "fn main() {\n    let a: [i64; 1] = [f64::NAN];\n    println(a);\n}\n",
        "E1201",
    );
}

#[test]
fn composite_specials_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct P { x: f64, y: f32 }\nfn main() {\n    println((f64::NAN, 1));\n    println([f32::INFINITY, f32::NEG_INFINITY]);\n    println(P { x: f64::NEG_INFINITY, y: f32::NAN });\n}\n",
    );
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert_eq!(
            r.stdout,
            b"(nan, 1)\n[inf, -inf]\nP { x: -inf, y: nan }\n",
            "{flag}"
        );
        assert!(r.stderr.is_empty(), "{flag}");
    }
    assert_no_temp(d.path());
}
