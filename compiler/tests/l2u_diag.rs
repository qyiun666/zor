//! L2u（0.3.20）④b-A 诊断回归：D1 文案与 D2 `Box<Bad>` 派生收敛。
//!
//! 覆盖：一元负号新文案；`Box<i64> ← 浮点特殊值` 仅 E1202；未知变体仅 E1101；
//! 赋值位同口径；合法 Box 不匹配 E1201 保留（整型→浮点 Box、跨 enum、跨宽）。
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
            "zorcc-l2u-{}-{}-{}-{}",
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

fn assert_diag(tag: &str, src: &str, must: &[&str], must_not: &[&str]) {
    let d = TempDir::new(tag);
    let path = write(d.path(), &format!("{tag}.zor"), src);
    let out = d.path().join(format!("out-{tag}"));
    let b = run_in(d.path(), &[path.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    let err = stderr_text(&b);
    assert_eq!(b.status.code(), Some(1), "{tag} stderr={err}");
    assert!(!out.exists(), "{tag} 不得留下产物");
    assert!(!err.contains("panic"), "{tag} 不得 panic: {err}");
    assert!(!err.contains("C 编译器"), "{tag} 不得退化为 cc 失败: {err}");
    for token in must {
        assert!(err.contains(token), "{tag} 缺少判词 {token} → {err}");
    }
    for token in must_not {
        assert!(!err.contains(token), "{tag} 不应包含 {token} → {err}");
    }
    assert_no_temp(d.path());
}

#[test]
fn unary_minus_message_is_updated() {
    assert_diag(
        "d1",
        "fn main() {\n    println(-f64::NAN);\n}\n",
        &["一元 '-' 不接受特殊值", "（含绑定后读取）"],
        &["仅直接打印"],
    );
}

#[test]
fn box_int_special_only_e1202() {
    assert_diag(
        "d2-special-nan",
        "fn main() {\n    let b: Box<i64> = box f64::NAN;\n    println(*b);\n}\n",
        &["E1202"],
        &["E1201", "Box<?"],
    );
    assert_diag(
        "d2-special-max",
        "fn main() {\n    let b: Box<i64> = box f64::MAX;\n    println(*b);\n}\n",
        &["E1202"],
        &["E1201", "Box<?"],
    );
}

#[test]
fn box_unknown_variant_only_e1101() {
    assert_diag(
        "d2-unknown",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box W(1);\n    println(*b);\n}\n",
        &["E1101"],
        &["E1201", "Box<?"],
    );
}

#[test]
fn box_assignment_special_only_e1202() {
    assert_diag(
        "d2-assign",
        "fn main() {\n    let a: Box<i64> = box 1;\n    a = box f64::NAN;\n    println(*a);\n}\n",
        &["E1202"],
        &["E1201", "Box<?"],
    );
}

#[test]
fn legal_box_mismatches_stay_e1201() {
    assert_diag(
        "d2-int-float",
        "fn main() {\n    let b: Box<f64> = box 1024;\n    println(*b);\n}\n",
        &["E1201"],
        &["Box<?"],
    );
    assert_diag(
        "d2-cross-enum",
        "enum A { V(i64) }\nenum B { V(i64) }\nfn main() {\n    let b: Box<A> = box B::V(1);\n    println(*b);\n}\n",
        &["E1201"],
        &["Box<?"],
    );
    assert_diag(
        "d2-cross-width",
        "fn main() {\n    let b: Box<f32> = box f64::NAN;\n    println(*b);\n}\n",
        &["E1201"],
        &["Box<?"],
    );
}

#[test]
fn enum_field_cross_width_stays_e1201() {
    assert_diag(
        "d2-enum-field",
        "enum E { V(f32) }\nfn main() {\n    let b: Box<E> = box V(f64::NAN);\n    println(*b);\n}\n",
        &["E1201"],
        &["Box<?"],
    );
}
