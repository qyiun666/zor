//! L3d 核心集成测试：⑤f-1 标量表达式载荷。
//!
//! 覆盖：`Box<i64|u8|u32>` 的常量表达式／`-1`／`-x`／算术（`+ - * / %`）／括号
//! 分组／单层字段投影 `p.x`（字面量绑定与运行期绑定）／单层标量 deref `*b0`／
//! 混合表达式；表达式在 box 时刻求值一次并快照；唯一临时局部（中间表达式不改变
//! 快照值）；表达式重初始化与自引用重初始化 `b = box (*b + 1)`（读旧值先于 free）；
//! Copy／move／unbox／drop；打印等价；六档；以及非运行期宽度表达式、窄宽度投影／
//! deref、复合 `*b`、复合／枚举表达式叶、bool 顶层、无标注、嵌套 Box、enum 含
//! Box／struct、`seen`、`box x + 1`、宽度溢出、读／所有权边界。

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
            "zorcc-l3d-{}-{}-{}-{}",
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

/// 打印等价：每两行一组（`println(t)`／`println(*b)`），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 表达式与同值临时须同字节: {out:?}");
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
fn constant_and_variable_expressions() {
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box (1 + 2);\n    println(*b);\n}\n",
        "const-add",
        b"3\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box -1;\n    println(*b);\n}\n",
        "neg-literal",
        b"-1\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box -x;\n    println(*b);\n}\n",
        "neg-var",
        b"-1\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box (x + 1);\n    println(*b);\n}\n",
        "var-add",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box ((x + 1) * 2);\n    println(*b);\n}\n",
        "nested-paren",
        b"4\n",
    );
    assert_runs(
        "fn main() {\n    let a = 1;\n    let b = 2;\n    let c: Box<i64> = box (a + b);\n    println(*c);\n}\n",
        "two-vars",
        b"3\n",
    );
    assert_runs(
        "fn main() {\n    let x = 7;\n    let b1: Box<i64> = box (x / 2);\n    let b2: Box<i64> = box (x % 2);\n    println(*b1);\n    println(*b2);\n}\n",
        "div-mod",
        b"3\n1\n",
    );
    assert_runs(
        "fn main() {\n    let x: u8 = 1;\n    let b: Box<u8> = box (x + 1);\n    println(*b);\n}\n",
        "u8-add",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let x: u8 = 2;\n    let b: Box<u8> = box (x * 2);\n    println(*b);\n}\n",
        "u8-mul",
        b"4\n",
    );
    assert_runs(
        "fn main() {\n    let x: u32 = 2;\n    let b: Box<u32> = box (x + 1);\n    println(*b);\n}\n",
        "u32-add",
        b"3\n",
    );
    assert_runs(
        "fn main() {\n    let x: u32 = 2;\n    let b: Box<u32> = box (x * 3);\n    println(*b);\n}\n",
        "u32-mul",
        b"6\n",
    );
}

#[test]
fn field_projection_and_scalar_deref() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
        "field-literal",
        b"1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box (p.x + 1);\n    println(*b);\n}\n",
        "field-expr",
        b"2\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<u8> = box p.x;\n    println(*b);\n}\n",
        "field-u8",
        b"1\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<u8> = box (p.x + 1);\n    println(*b);\n}\n",
        "field-u8-expr",
        b"2\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
        "field-runtime-bound",
        b"3\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let x: u8 = 1;\n    let p = P { x: x };\n    let b: Box<u8> = box p.x;\n    println(*b);\n}\n",
        "field-runtime-u8",
        b"1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 + 2 };\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
        "field-binding-expr",
        b"3\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box (p.x);\n    println(*b);\n}\n",
        "field-grouped",
        b"1\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<i64> = box *b0;\n    println(*b);\n}\n",
        "deref-i64",
        b"7\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<u8> = box 7;\n    let b: Box<u8> = box *b0;\n    println(*b);\n}\n",
        "deref-u8",
        b"7\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let b: Box<i64> = box (*b0 + 1);\n    println(*b);\n}\n",
        "deref-expr",
        b"8\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 7;\n    let b: Box<i64> = box (p.x + *b0);\n    println(*b);\n}\n",
        "field-deref-mixed",
        b"8\n",
    );
}

#[test]
fn reinit_self_reinit_and_ownership() {
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box (x + 1);\n    let y = 3 + 4;\n    println(*b);\n    println(y);\n    b = box (x + 1);\n    println(*b);\n}\n",
        "expr-reinit",
        b"2\n7\n2\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    unbox b;\n    let x = 1;\n    b = box -x;\n    println(*b);\n}\n",
        "unbox-reinit",
        b"-1\n",
    );
    // v1.1 §1／§2：自引用重初始化——先读旧值，再 free／alloc；旧载荷恰释放一次。
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 1;\n    b = box (*b + 1);\n    println(*b);\n}\n",
        "self-reinit-add",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<i64> = box 3;\n    b = box (*b * 2);\n    println(*b);\n}\n",
        "self-reinit-mul",
        b"6\n",
    );
    assert_runs(
        "fn main() {\n    let b: Box<u8> = box 1;\n    b = box (*b + 1);\n    println(*b);\n}\n",
        "self-reinit-u8",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box (x + 1);\n    let c = b;\n    println(*c);\n    let d: Box<i64> = box (x + 2);\n    unbox d;\n    println(*c);\n}\n",
        "copy-move-unbox",
        b"2\n2\n",
    );
}

#[test]
fn temp_local_stability_and_snapshot() {
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box (x + 1);\n    let y = 2 + 3;\n    println(*b);\n    println(y);\n    println(*b);\n}\n",
        "temp-stability",
        b"2\n5\n2\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b1: Box<i64> = box (x + 1);\n    let b2: Box<i64> = box (x + 1);\n    println(*b1);\n    println(*b2);\n}\n",
        "two-boxes",
        b"2\n2\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b1: Box<i64> = box (x + 1);\n    let y = 10;\n    let b2: Box<i64> = box (x + 2);\n    println(*b1);\n    println(*b2);\n    println(y);\n}\n",
        "independent-values",
        b"2\n3\n10\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let t = x + 1;\n    let b: Box<i64> = box (x + 1);\n    println(t);\n    println(*b);\n}\n",
        "temp-var",
        b"2\n2\n",
    );
}

#[test]
fn print_equivalence() {
    // 常量／变量／表达式三形态同值。
    assert_runs(
        "fn main() {\n    let b1: Box<i64> = box 1;\n    let x = 1;\n    let b2: Box<i64> = box x;\n    let b3: Box<i64> = box (1);\n    println(*b1);\n    println(*b2);\n    println(*b3);\n}\n",
        "three-forms",
        b"1\n1\n1\n",
    );
    assert_pairs_isomorphic(
        "eq-var",
        "fn main() {\n    let x = 2;\n    let t = x + 1;\n    println(t);\n    let b: Box<i64> = box (x + 1);\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-u8",
        "fn main() {\n    let x: u8 = 3;\n    let t: u8 = x * 2;\n    println(t);\n    let b: Box<u8> = box (x * 2);\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-u32",
        "fn main() {\n    let x: u32 = 2;\n    let t: u32 = x + 1;\n    println(t);\n    let b: Box<u32> = box (x + 1);\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-field",
        "struct P { x: i64 }\nfn main() {\n    let x = 1 + 2;\n    let p = P { x: x };\n    println(x);\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
    );
    assert_pairs_isomorphic(
        "eq-deref",
        "fn main() {\n    let b0: Box<u8> = box 7;\n    let t = *b0;\n    println(t);\n    let b: Box<u8> = box *b0;\n    println(*b);\n}\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    assert_rc1(
        "add-unparenthesized",
        "fn main() {\n    let x = 1;\n    let b: Box<i64> = box x + 1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "i8-neg-literal",
        "fn main() {\n    let b: Box<i8> = box -1;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "i8-expression",
        "fn main() {\n    let x: i8 = 1;\n    let b: Box<i8> = box (x + 1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "i16-expression",
        "fn main() {\n    let x: i16 = 1;\n    let b: Box<i16> = box (x + 1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "u64-expression",
        "fn main() {\n    let x: u64 = 1;\n    let b: Box<u64> = box (x + 1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "u8-neg-var",
        "fn main() {\n    let x: u8 = 1;\n    let b: Box<u8> = box -x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "u32-neg-literal",
        "fn main() {\n    let b: Box<u32> = box -1;\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "narrow-field",
        "struct P { x: i8 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i8> = box p.x;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "narrow-deref",
        "fn main() {\n    let b0: Box<i8> = box 1;\n    let b: Box<i8> = box *b0;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "composite-deref",
        "struct P { x: i64 }\nfn main() {\n    let b0: Box<P> = box P { x: 1 };\n    let b: Box<P> = box *b0;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    // L3e：struct 复合字面量表达式叶已开放（原负控翻转）。
    assert_runs(
        "struct S { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { x: x + 1 };\n    println(*b);\n}\n",
        "struct-expression-leaf",
        b"S { x: 2 }\n",
    );
    // L3e：array 复合字面量表达式叶已开放（原负控翻转）。
    assert_runs(
        "fn main() {\n    let x = 1;\n    let b: Box<[i64; 1]> = box [x + 1];\n    println(*b);\n}\n",
        "array-expression-leaf",
        b"[2]\n",
    );
    // L3f：顶层 enum 构造表达式载荷已开放（原负控翻转）。
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let b: Box<E> = box V(x);\n    println(*b);\n}\n",
        "enum-ctor-expression",
        b"V(1)\n",
    );
    // L3f：enum 构造表达式叶已开放（原负控翻转）。
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "enum-expression-leaf",
        b"S { e: V(1) }\n",
    );
    assert_rc1(
        "bool-top-level",
        "fn main() {\n    let b: Box<bool> = box true;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "unannotated",
        "fn main() {\n    let x = 1;\n    let b = box (x + 1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    assert_rc1(
        "nested-box",
        "fn main() {\n    let x = 1;\n    let b: Box<Box<i64>> = box box (x + 1);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(P { x: 1 }) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "seen-repeat",
        "struct I { x: i64 }\nstruct S { a: I, b: I }\nfn main() {\n    let b: Box<S> = box S { a: I { x: 1 }, b: I { x: 2 } };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "width-overflow",
        "fn main() {\n    let b: Box<u8> = box 256;\n    println(*b);\n}\n",
        "E1201",
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
fn scalar_expressions_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct P { x: i64 }\nfn main() {\n    let x = 1;\n    let b: Box<i64> = box (x + 1);\n    println(*b);\n    let y = 3 + 4;\n    println(*b);\n    let b0: Box<i64> = box 7;\n    let c: Box<i64> = box (*b0 + 1);\n    println(*c);\n    let p = P { x: x + 2 };\n    let d: Box<i64> = box p.x;\n    println(*d);\n    b = box (*b + 1);\n    println(*b);\n    let e: Box<i64> = box -1;\n    println(*e);\n    let f: Box<u8> = box 1;\n    f = box (*f + 1);\n    println(*f);\n}\n",
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
                assert_eq!(r.stdout, b"2\n2\n8\n3\n3\n-1\n2\n", "{flag} 首档输出");
                baseline = Some(r.stdout);
            }
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
