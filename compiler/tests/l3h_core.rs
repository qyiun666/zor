//! L3h 核心集成测试：裸字段读取绑定。
//!
//! 覆盖：`let y = p.x`（i64／u8／u32，含同宽标注 `let y: i64 = p.x` 等）、
//! 字面量／运行期绑定、双字段／双读取／双 struct、读取后算术／复制／取模／乘法／
//! deref 混排、读取后复合叶／数组叶／Box 载荷、u8 算术、打印等价、读取不移动基座、
//! 六档；以及 i8／i16／i32／u16／u64／f32／f64／bool 裸读、嵌套字段、字段 of deref、
//! 数组索引／元素字段、复合／枚举／数组字段、struct 字面量直读、Box 基座直读、
//! 跨宽标注等 fail-closed 边界；并回归 println／Box 载荷／L3g 算术／裸 deref／
//! 普通标量算术／普通枚举构造。

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
            "zorcc-l3h-{}-{}-{}-{}",
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
fn field_read_binding_core_widths() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 7 };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "i64-literal",
        b"7\n7\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let x = 1 + 6;\n    let p = P { x: x };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "i64-runtime",
        b"7\n7\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 200 };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "u8",
        b"200\n200\n",
    );
    assert_runs(
        "struct P { x: u32 }\nfn main() {\n    let p = P { x: 3000000000 };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "u32",
        b"3000000000\n3000000000\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let x: u8 = 200;\n    let p = P { x: x };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "u8-runtime",
        b"200\n200\n",
    );
    assert_runs(
        "struct P { x: u32 }\nfn main() {\n    let x: u32 = 3000000000;\n    let p = P { x: x };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "u32-runtime",
        b"3000000000\n3000000000\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 + 2 };\n    let y = p.x;\n    println(y);\n    println(p.x);\n}\n",
        "binding-expr",
        b"3\n3\n",
    );
}

#[test]
fn same_width_annotations() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 7 };\n    let y: i64 = p.x;\n    println(y);\n}\n",
        "i64-ann",
        b"7\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 200 };\n    let y: u8 = p.x;\n    println(y);\n}\n",
        "u8-ann",
        b"200\n",
    );
    assert_runs(
        "struct P { x: u32 }\nfn main() {\n    let p = P { x: 3000000000 };\n    let y: u32 = p.x;\n    println(y);\n}\n",
        "u32-ann",
        b"3000000000\n",
    );
}

#[test]
fn downstream_use_and_no_move() {
    assert_runs(
        "struct P { x: i64, y: i64 }\nfn main() {\n    let p = P { x: 1, y: 2 };\n    let a = p.x;\n    let b = p.y;\n    println(a);\n    println(b);\n    println(p);\n}\n",
        "two-fields",
        b"1\n2\nP { x: 1, y: 2 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let a = p.x;\n    let b = p.x;\n    println(a);\n    println(b);\n}\n",
        "two-reads",
        b"1\n1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p1 = P { x: 1 };\n    let p2 = P { x: 2 };\n    let a = p1.x;\n    let b = p2.x;\n    println(a);\n    println(b);\n}\n",
        "two-structs",
        b"1\n2\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x;\n    let z = t + 1;\n    println(t);\n    println(z);\n}\n",
        "arith",
        b"1\n2\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x;\n    let z = t;\n    println(t);\n    println(z);\n}\n",
        "copy",
        b"1\n1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 5 };\n    let t = p.x;\n    let z = t % 2;\n    println(t);\n    println(z);\n}\n",
        "mod",
        b"5\n1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 2 };\n    let t = p.x;\n    let z = t * 3;\n    println(t);\n    println(z);\n}\n",
        "mul",
        b"2\n6\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b0: Box<i64> = box 1;\n    let t = p.x;\n    let z = t + *b0;\n    println(t);\n    println(z);\n}\n",
        "deref-mix",
        b"1\n2\n",
    );
    assert_runs(
        "struct P { x: u8 }\nfn main() {\n    let p = P { x: 200 };\n    let t = p.x;\n    let z = t + 1;\n    println(t);\n    println(z);\n}\n",
        "u8-arith",
        b"200\n201\n",
    );
    assert_runs(
        "struct P { x: i64 }\nstruct Q { y: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x;\n    let q = Q { y: t };\n    println(q);\n    println(Q { y: p.x });\n}\n",
        "comp-leaf",
        b"Q { y: 1 }\nQ { y: 1 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x;\n    let a: [i64; 1] = [t];\n    println(a);\n    println([p.x]);\n}\n",
        "array-leaf",
        b"[1]\n[1]\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let t = p.x;\n    let b: Box<i64> = box t;\n    println(*b);\n    let c: Box<i64> = box p.x;\n    println(*c);\n}\n",
        "box-use",
        b"1\n1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nstruct U { x: u8 }\nfn main() {\n    let p = P { x: 3 };\n    let u = U { x: 200 };\n    let a = p.x;\n    let b = u.x;\n    println(a);\n    println(p.x);\n    println(b);\n    println(u.x);\n}\n",
        "mixed-widths",
        b"3\n3\n200\n200\n",
    );
}

#[test]
fn existing_capabilities_unchanged() {
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    println(p.x);\n}\n",
        "println-field",
        b"1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let b: Box<i64> = box p.x;\n    println(*b);\n}\n",
        "box-field",
        b"1\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let a: [i64; 1] = [p.x];\n    println(a);\n}\n",
        "array-field",
        b"[1]\n",
    );
    assert_runs(
        "struct P { x: i64 }\nstruct Q { y: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let q = Q { y: p.x };\n    println(q);\n}\n",
        "comp-field",
        b"Q { y: 1 }\n",
    );
    assert_runs(
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "l3g-arith",
        b"2\n",
    );
    assert_runs(
        "fn main() {\n    let b0: Box<i64> = box 7;\n    let y = *b0;\n    println(y);\n}\n",
        "deref-read",
        b"7\n",
    );
    assert_runs(
        "fn main() {\n    let x = 1;\n    let y = x + 1;\n    println(y);\n}\n",
        "plain-arith",
        b"2\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let x = 1;\n    let e = V(x + 1);\n    println(e);\n}\n",
        "ordinary-enum",
        b"V(2)\n",
    );
}

#[test]
fn fail_closed_boundaries() {
    assert_rc1(
        "i8-read",
        "struct P { x: i8 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "i16-read",
        "struct P { x: i16 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "i32-read",
        "struct P { x: i32 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "u16-read",
        "struct P { x: u16 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "u64-read",
        "struct P { x: u64 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "f32-read",
        "struct P { x: f32 }\nfn main() {\n    let p = P { x: 1.0 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "f64-read",
        "struct P { x: f64 }\nfn main() {\n    let p = P { x: 1.5 };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "bool-read",
        "struct P { x: bool }\nfn main() {\n    let p = P { x: true };\n    let y = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "nested-field",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let o = O { i: I { x: 1 } };\n    let y = o.i.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "field-of-deref",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let y = (*b).x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "array-index",
        "fn main() {\n    let a: [i64; 2] = [1, 2];\n    let y = a[0];\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "array-elem-field",
        "struct S { x: i64 }\nfn main() {\n    let a: [S; 1] = [S { x: 1 }];\n    let y = a[0].x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "struct-field",
        "struct I { x: i64 }\nstruct O { i: I }\nfn main() {\n    let o = O { i: I { x: 1 } };\n    let q = o.i;\n    println(q);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "enum-field",
        "enum E { V(i64) }\nstruct P { e: E }\nfn main() {\n    let p = P { e: V(1) };\n    let q = p.e;\n    println(q);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "array-field",
        "struct P { a: [i64; 2] }\nfn main() {\n    let p = P { a: [1, 2] };\n    let q = p.a;\n    println(q);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "literal-base",
        "struct P { x: i64 }\nfn main() {\n    let y = P { x: 1 }.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "box-base",
        "struct P { x: i64 }\nfn main() {\n    let b: Box<P> = box P { x: 1 };\n    let y = b.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "cross-width-ann",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    let y: u8 = p.x;\n    println(y);\n}\n",
        "E1201",
    );
    assert_rc1(
        "f64-ann",
        "struct P { x: f64 }\nfn main() {\n    let p = P { x: 1.5 };\n    let y: f64 = p.x;\n    println(y);\n}\n",
        "let 初始化非可 codegen 形态",
    );
    assert_rc1(
        "i8-arith",
        "struct P { x: i8 }\nfn main() {\n    let p = P { x: 1 };\n    let y = p.x + 1;\n    println(y);\n}\n",
        "let 初始化非整型算术表达式",
    );
    assert_rc1(
        "println-arith",
        "struct P { x: i64 }\nfn main() {\n    let p = P { x: 1 };\n    println(p.x + 1);\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn field_read_binding_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "struct P { x: i64, y: i64 }\nstruct U { x: u8 }\nstruct W { x: u32 }\nfn main() {\n    let p = P { x: 1, y: 5 };\n    let a = p.x;\n    println(a);\n    let b: i64 = p.y;\n    println(b);\n    let u = U { x: 200 };\n    let c = u.x;\n    println(c);\n    let w = W { x: 3000000000 };\n    let e: u32 = w.x;\n    println(e);\n    let z = a + 1;\n    println(z);\n    println(p);\n}\n",
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
                assert_eq!(
                    r.stdout,
                    b"1\n5\n200\n3000000000\n2\nP { x: 1, y: 5 }\n",
                    "{flag} 首档输出"
                );
                baseline = Some(r.stdout);
            }
            Some(want) => assert_eq!(&r.stdout, want, "{flag} 各档输出须一致"),
        }
    }
    assert_no_temp(d.path());
}
