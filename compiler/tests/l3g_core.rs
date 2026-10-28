//! L3g 核心集成测试：普通 let 算术 Field／Deref 操作数。
//!
//! 覆盖：`let y = p.x + 1`／`let y = *b0 + 1`（i64／u8／u32）、括号与任意既有
//! Bin／Un 组合、双字段、字段＋deref 混排、负号（i64）、取模、字面量／运行期绑定、
//! 常量／运行期 Box、链式、let 与 Box 载荷同表达式一致性、打印等价、六档；
//! 并锁定普通标量算术、Box 载荷表达式、普通枚举构造、裸 deref 读取不变，以及
//! 裸字段读取、println 算术、表达式语句、复合初值、嵌套字段／字段 of deref、
//! 数组索引、非运行期宽度／浮点、复合 `*b` 维持 fail-closed。

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
            "zorcc-l3g-{}-{}-{}-{}",
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

/// 打印等价：每两行一组（`println(同值临时/Box)`／`println(y)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 同表达式结果须同字节: {out:?}");
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
fn let_field_arithmetic() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "i64-literal",
        b"2\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "i64-runtime",
        b"4\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "u8",
        b"2\n",
    );
    assert_runs(
        "struct P { x: u32 }\nfn main() {\n    let p = P { x: 2 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "u32",
        b"3\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = (p.x + 1) * 2;\n    println(y);\n}\n",
        "paren",
        b"4\n",
    );
    assert_runs(
        "struct P { x: i64, y: i64 }\nfn main() {\n    let p = P { x: 1, y: 2 };\n    let z = p.x + p.y;\n    println(z);\n}\n",
        "two-fields",
        b"3\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = -p.x + 1;\n    println(y);\n}\n",
        "unary",
        b"0\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 3 };\n    let y = p.x % 2;\n    println(y);\n}\n",
        "mod",
        b"1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 2 };\n    let y = p.x + (1 + 2);\n    println(y);\n}\n",
        "const-subexpr",
        b"5\n",
    );
}

#[test]
fn let_deref_arithmetic() {
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "i64",
        b"8\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<u8> = box 7;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "u8",
        b"8\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<u32> = box 7;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "u32",
        b"8\n",
    );
    assert_runs(
        "fn main() {\n    let x = 7;\n    let b0: Box<i64> = box x;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "runtime-box",
        b"8\n",
    );
}

#[test]
fn combined_chains_and_consistency() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 7;\n    let y = p.x + *b0;\n    println(y);\n}\n",
        "mixed",
        b"8\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<u8> = box 3;\n    let y = p.x + *b0;\n    println(y);\n}\n",
        "u8-mixed",
        b"4\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 2;\n    let y = ((p.x + *b0) * 2 - 3) % 5;\n    println(y);\n}\n",
        "deep",
        b"3\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x + 1;\n    let y = t + 1;\n    println(y);\n}\n",
        "field-chain",
        b"3\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let t = *b0;\n    let y = t + 1;\n    println(y);\n}\n",
        "deref-read-chain",
        b"8\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 7;\n    let y = p.x + 1;\n    let z = *b0 + y;\n    println(z);\n}\n",
        "chain-deref",
        b"9\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let a = p.x + 1;\n    let b = p.x + 2;\n    println(a);\n    println(b);\n}\n",
        "independent",
        b"2\n3\n",
    );
}

#[test]
fn print_equivalence_and_regressions() {
    assert_pairs_isomorphic(
        "eq-let-box",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n    let b: Box<i64> = box (p.x + 1);\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-deref-box",
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let y = *b0 + 1;\n    println(y);\n    let b: Box<i64> = box (*b0 + 1);\n    println(*b);\n}\n",
    );
    // 普通标量算术回归。
    assert_runs(
        "fn main() {\n    let x = 1;\n    let y = x + 1;\n    println(y);\n}\n",
        "plain-arith",
        b"2\n",
    );
    // Box 载荷表达式回归（L3d／L3e）。
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box (p.x + 1);\n    println(*b);\n}\n",
        "box-field",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<i64> = box (*b0 + 1);\n    println(*b);\n}\n",
        "box-deref",
        b"8\n",
    );
    // 普通枚举构造不变（L3f）。
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let e = V(x + 1);\n    println(e);\n}\n",
        "ordinary-enum",
        b"V(2)\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    // 裸字段读取维持 fail-closed；裸 deref 读取为既有能力（见上）。
    assert_rc1(
        "bare-field-read",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "i8-field",
        "struct P { x: i8 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "i16-field",
        "struct P { x: i16 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "i32-field",
        "struct P { x: i32 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "u64-field",
        "struct P { x: u64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "f64-field",
        "struct P { x: f64 }\nfn main() {\n    let p = P { x: 1.5 };\n    let y = p.x + 1.0;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "f32-field",
        "struct P { x: f32 }\nfn main() {\n    let p = P { x: 1.0 };\n    let y = p.x + 1.0;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "i8-deref",
        "fn main() {\n    let b0: Box<i8> = box 1;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "u64-deref",
        "fn main() {\n    let b0: Box<u64> = box 1;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "bool-field",
        "struct P { x: bool }\nfn main() {\n    let p = P { x: true };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "E1201",
    );
    assert_rc1(
        "composite-deref",
        "struct P { x: i64 }\nfn main() {\n    let b0: Box<P> = box P { x: 1 };\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "E1201",
    );
    assert_rc1(
        "nested-field",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let o = O { i: I { x: 1 } };\n    let y = o.i.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "field-of-deref",
        "struct P { x: i64 }\nfn main() {\n    let b0: Box<P> = box P { x: 1 };\n    let y = (*b0).x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "array-index",
        "fn main() {\n    let a: [i64; 2] = [1, 2];\n    let y = a[0] + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "println-field-arith",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    println(p.x + 1);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "println-deref-arith",
        "fn main() {\n    let b0: Box<i64> = box 7;\n    println(*b0 + 1);\n}\n",
        "lower：算术操作数形态不在 L2b 子集",
    );
    assert_rc1(
        "expr-stmt-field-arith",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    p.x + 1;\n}\n",
        "整型算术表达式形态",
    );
    assert_rc1(
        "composite-init",
        "struct P { x: i64 }\nstruct S { n: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let s = S { n: p.x + 1 };\n    println(s);\n}\n",
        "codegen",
    );
    assert_rc1(
        "bool-add",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + true;\n    println(y);\n}\n",
        "E1201",
    );
    assert_rc1(
        "float-add",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1.0;\n    println(y);\n}\n",
        "E1202",
    );
    assert_rc1(
        "box-bool-deref",
        "fn main() {\n    let b0: Box<bool> = box true;\n    let y = *b0 + 1;\n    println(y);\n}\n",
        "E1201",
    );
}

#[test]
fn let_field_deref_arith_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct P { x: i64 }\nstruct U { x: u8 }\nstruct W { x: u32 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 7;\n    let y = p.x + 1;\n    println(y);\n    let z = *b0 + y;\n    println(z);\n    let t = p.x + *b0;\n    println(t);\n    let u = U { x: 2 };\n    let bu: Box<u8> = box 3;\n    let a = u.x + *bu;\n    println(a);\n    let w = W { x: 2 };\n    let bw: Box<u32> = box 3;\n    let b = w.x + *bw;\n    println(b);\n}\n",
    );
    let mut baseline: Option<Vec<u8>> = None;
    for flag in ["-O0", "-O1", "-O2", "-O3", "-Os", "-Og"] {
        let (b, r) = build_run(d.path(), &src, &[flag], &flag[1..]);
        assert_eq!(b.status.code(), Some(0), "{flag} build={}", stderr_text(&b));
        let r = r.expect("artifact");
        assert_eq!(r.status.code(), Some(0), "{flag}");
        assert!(r.stderr.is_empty(), "{flag} stderr={:?}", r.stderr);
        match &baseline {
            None => {
                assert_eq!(r.stdout, b"2\n9\n8\n5\n5\n", "{flag} 首档输出");
                baseline = Some(r.stdout);
            }
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
