//! L2d 复合输出集成测试：stdout 契约 v1.2（＋v1.2.1 补丁）逐字节；
//! 类型级 fail-closed（N-11～13）、Q-3 常量元素、Q-4 字面量下标。
//! 测试只依赖宿主 `cc`，不访问网络。

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
            "zorcc-l2d-{}-{}-{}-{}",
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

fn write(dir: &Path, name: &str, src: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, src).expect("write source");
    p
}

fn build(src_path: &Path, out: &Path) -> Output {
    Command::new(zorcc())
        .arg(src_path)
        .arg("-o")
        .arg(out)
        .output()
        .expect("spawn zorcc")
}

fn run_case(tag: &str, src: &str) -> Output {
    let d = TempDir::new(tag);
    let src_path = write(d.path(), "case.zor", src);
    let out = d.path().join("case");
    let b = build(&src_path, &out);
    assert_eq!(
        b.status.code(),
        Some(0),
        "{tag}: build stderr={}",
        String::from_utf8_lossy(&b.stderr)
    );
    Command::new(&out).output().expect("run artifact")
}

fn build_and_run(tag: &str, src: &str) -> Output {
    let r = run_case(tag, src);
    assert!(r.stderr.is_empty(), "{tag}: stderr={:?}", r.stderr);
    r
}

fn assert_stdout(tag: &str, src: &str, want: &[u8]) {
    let r = build_and_run(tag, src);
    assert_eq!(r.status.code(), Some(0), "{tag}");
    assert_eq!(r.stdout, want, "{tag}");
}

fn assert_codegen_fail(tag: &str, src: &str) {
    let d = TempDir::new(tag);
    let src_path = write(d.path(), "case.zor", src);
    let out = d.path().join("case");
    let b = build(&src_path, &out);
    assert_eq!(b.status.code(), Some(1), "{tag}: codegen 必须 rc=1");
    let err = String::from_utf8_lossy(&b.stderr);
    assert!(err.contains("zorcc: codegen:"), "{tag}: {err}");
    assert!(
        !err.contains("E1") && !err.contains("E2") && !err.contains("E3"),
        "{tag}: codegen 诊断不占 E 码: {err}"
    );
    assert!(!out.exists(), "{tag}: 失败不得留产物");
    let left: Vec<String> = fs::read_dir(d.path())
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(left, vec!["case.zor".to_string()], "{tag}");
}

// —— 契约 v1.2 §3.2／§3.3：tuple 与 array ——

#[test]
fn tuple_bytes_match_contract() {
    assert_stdout(
        "tuple",
        "fn main() {
    println(());
    println((1, true));
    println((1, true, 2));
    println(((), 1));
    println((1.5, -0.0));
    println((\"\", 1));
    println((\"\", \"\"));
}
",
        b"()\n(1, true)\n(1, true, 2)\n((), 1)\n(1.5, -0)\n(, 1)\n(, )\n",
    );
}

#[test]
fn array_and_raw_byte_array_bytes_match_contract() {
    assert_stdout(
        "array",
        "struct S { b: [byte; 2] }

fn main() {
    let a: [i64; 0] = [];
    println(a);
    let b: [i64; 3] = [1, 2, 3];
    println(b);
    let c: [u8; 2] = [1, 2];
    println(c);
    let s: [byte; 2] = \"Hi\";
    println(s);
    let e: [[byte; 0]; 2] = [\"\", \"\"];
    println(e);
    let n: [[i64; 2]; 2] = [[1, 2], [3, 4]];
    println(n);
    println((\"a,b\", 1));
    println(S { b: \"hi\" });
}
",
        b"[]\n[1, 2, 3]\n[1, 2]\nHi\n[, ]\n[[1, 2], [3, 4]]\n(a,b, 1)\nS { b: hi }\n",
    );
}

// —— 契约 v1.2 §3.4／§3.5：struct 与 enum ——

#[test]
fn struct_bytes_match_contract() {
    assert_stdout(
        "struct",
        "struct Empty {}
struct Point { x: i64, y: i64 }
struct S { b: [byte; 0] }
struct Outer { p: Point, tags: [i64; 2] }

fn main() {
    println(Empty {});
    println(Point { x: 1, y: 2 });
    println(Point { y: 2, x: 1 });
    println(S { b: \"\" });
    println(Outer { p: Point { x: 1, y: 2 }, tags: [3, 4] });
}
",
        b"Empty {}\nPoint { x: 1, y: 2 }\nPoint { x: 1, y: 2 }\nS { b:  }\nOuter { p: Point { x: 1, y: 2 }, tags: [3, 4] }\n",
    );
}

#[test]
fn enum_bytes_match_contract() {
    assert_stdout(
        "enum",
        "enum Color { Red(), Green() }
enum E { A(), B(i64, bool) }
enum V { V([byte; 0]) }
enum Node { Leaf(), Pair(i64, bool) }

fn main() {
    println(Red());
    println(B(1, true));
    println(V(\"\"));
    println(Pair(1, false));
}
",
        b"Red\nB(1, true)\nV()\nPair(1, false)\n",
    );
}

// —— 契约 v1.2 §3.6：嵌套与多实参 ——

#[test]
fn nested_and_multi_arg_bytes_match_contract() {
    assert_stdout(
        "nested",
        "struct Point { x: i64, y: i64 }

fn main() {
    println((Point { x: 1, y: 2 }, 3));
    println([Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]);
    println(\"p\", Point { x: 1, y: 2 });
}
",
        b"(Point { x: 1, y: 2 }, 3)\n[Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]\np Point { x: 1, y: 2 }\n",
    );
}

// —— 契约 v1.2 §3.9：只读投影（Q-4） ——

#[test]
fn read_only_projection_bytes_match_contract() {
    assert_stdout(
        "projection",
        "struct Point { x: i64, y: i64 }
struct Wrap { p: Point }

fn main() {
    let p = Point { x: 1, y: 2 };
    println(p.x);
    let t = (1, true, 3);
    println(t[0]);
    println(t[1]);
    let a: [u8; 3] = [10, 20, 30];
    println(a[1]);
    let w = Wrap { p: Point { x: 7, y: 8 } };
    println(w.p.y);
    println(p);
}
",
        b"1\n1\ntrue\n20\n8\nPoint { x: 1, y: 2 }\n",
    );
}

// —— 契约 v1.2 §5：原子性 ——

#[test]
fn composite_argument_termination_writes_zero_bytes_for_that_call() {
    let r = run_case(
        "atomic",
        "fn main() {
    println(\"before\");
    println(\"x\", (1 / 0, 2));
}
",
    );
    assert_eq!(r.status.code(), Some(101));
    assert_eq!(r.stdout, b"before\n", "失败调用必须零字节");
    assert!(!r.stderr.is_empty());
}

// —— N-11～13：类型级 fail-closed ——

#[test]
fn n11_tuple_with_box_is_fail_closed() {
    assert_codegen_fail("n11", "fn main() { let b = box 1; println((b, 1)); }\n");
}

#[test]
fn n12_nested_tuple_box_is_fail_closed() {
    assert_codegen_fail(
        "n12",
        "struct P { t: (i64, Box<i64>) }\nfn main() { let p = P { t: (1, box 1) }; println(p); }\n",
    );
}

#[test]
fn n13_enum_type_level_box_is_fail_closed() {
    // Q-2：enum 任一变体含不可打印成员 → 整类型不可打印，即使待打印值属于其他变体。
    assert_codegen_fail(
        "n13",
        "enum E { A(i64), B(Box<i64>) }\nfn main() { println(A(1)); }\n",
    );
}

// —— Q-3／Q-4：常量元素与字面量下标 ——

#[test]
fn non_constant_array_element_is_fail_closed() {
    assert_codegen_fail(
        "q3",
        "fn main() { let x: u8 = 1; let a: [u8; 2] = [x, 2]; println(a); }\n",
    );
}

#[test]
fn non_literal_index_is_fail_closed() {
    assert_codegen_fail(
        "q4var",
        "fn main() { let a: [u8; 3] = [1, 2, 3]; let i: i64 = 1; println(a[i]); }\n",
    );
}

#[test]
fn out_of_bounds_index_is_fail_closed() {
    assert_codegen_fail(
        "q4oob",
        "fn main() { let a: [u8; 3] = [1, 2, 3]; println(a[5]); }\n",
    );
}

// —— 零回归：L2c 面在多实参旁保持逐字节 ——

#[test]
fn l2c_scalar_faces_unchanged_next_to_composites() {
    assert_stdout(
        "mixed",
        "fn main() {
    let s: [byte; 2] = \"ok\";
    println(s, 42, true, 1.5);
}
",
        b"ok 42 true 1.5\n",
    );
}
