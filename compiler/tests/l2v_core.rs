//! L2v 核心集成测试：②a `Box<enum>` 变量载荷＋变量重初始化（Copy）。
//!
//! 覆盖：`box e` 单／多／零字段、Copy（e 仍可用）、变量重初始化（e2 仍可用）、
//! 移动／unbox／多 Box、`V(x)` 运行期整型与表达式先行求值、8 整型宽度、
//! bool／f32／f64／特殊值、限定与标注变体、`println(*b)` 与 `println(e)` 逐字节
//! 一致、六档 `-O*` 稳定；以及 `box V(x)`／`box V(1+2)`、无标注、非枚举变量、
//! 非枚举字段枚举变量、重初始化表达式、读形态与所有权错误等 fail-closed 面。
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
            "zorcc-l2v-{}-{}-{}-{}",
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

/// 同构断言：每两行一组（Box 打印／裸枚举打印），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} Box 与裸枚举须同字节: {out:?}");
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
fn box_enum_variable_matches_bare_variant() {
    assert_pairs_isomorphic(
        "copy",
        "enum E { A(i64), V(f64), N, B(bool) }\nfn main() {\n    let a = A(1);\n    let ba: Box<E> = box a;\n    println(*ba);\n    println(a);\n    let v = V(1.5);\n    let bv: Box<E> = box v;\n    println(*bv);\n    println(v);\n    let n = N();\n    let bn: Box<E> = box n;\n    println(*bn);\n    println(n);\n    let b = B(true);\n    let bb: Box<E> = box b;\n    println(*bb);\n    println(b);\n}\n",
    );
}

#[test]
fn runtime_fields_are_materialized_in_source_order() {
    assert_runs(
        "enum E { V(i64), P(i64, i64), Q(u8, u32), R(f64), S(bool) }\nfn main() {\n    let x: i64 = 7;\n    let y: i64 = 2;\n    let e = V(x);\n    let b: Box<E> = box e;\n    println(*b);\n    println(e);\n    let p = P(x, y);\n    let c: Box<E> = box p;\n    println(*c);\n    println(p);\n    let a: u8 = 255;\n    let z: u32 = 4294967295;\n    let q = Q(a, z);\n    let d: Box<E> = box q;\n    println(*d);\n    println(q);\n    let f: f64 = 1.5;\n    let r = R(f);\n    let g: Box<E> = box r;\n    println(*g);\n    println(r);\n    let t: bool = true;\n    let s = S(t);\n    let h: Box<E> = box s;\n    println(*h);\n    println(s);\n    let w = V(x + y);\n    let i: Box<E> = box w;\n    println(*i);\n    println(w);\n    let neg = V(-x);\n    let j: Box<E> = box neg;\n    println(*j);\n    println(neg);\n}\n",
        "runtime",
        b"V(7)\nV(7)\nP(7, 2)\nP(7, 2)\nQ(255, 4294967295)\nQ(255, 4294967295)\nR(1.5)\nR(1.5)\nS(true)\nS(true)\nV(9)\nV(9)\nV(-7)\nV(-7)\n",
    );
}

#[test]
fn variable_reinit_keeps_source_and_zero_field_variants() {
    assert_runs(
        "enum N { A(), B() }\nenum E { V(i64), W(f64) }\nfn main() {\n    let b: Box<E> = box V(1);\n    let e2 = W(2.5);\n    b = box e2;\n    println(*b);\n    println(e2);\n    b = box V(3);\n    println(*b);\n    let n: Box<N> = box A();\n    let z = B();\n    n = box z;\n    println(*n);\n    println(z);\n}\n",
        "reinit",
        b"W(2.5)\nW(2.5)\nV(3)\nB\nB\n",
    );
}

#[test]
fn move_unbox_and_multiple_boxes_from_same_copy() {
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let e = V(1);\n    let a: Box<E> = box e;\n    let b: Box<E> = box e;\n    let c = a;\n    unbox b;\n    println(*c);\n    println(e);\n}\n",
        "ownership",
        b"V(1)\nV(1)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let e = V(2);\n    let a: Box<E> = box e;\n    let b: Box<E> = box e;\n    println(*a);\n    println(*b);\n    println(e);\n}\n",
        "multi",
        b"V(2)\nV(2)\nV(2)\n",
    );
    // 移动后槽位重初始化：不得释放已移交的旧载荷，作用域各释放一次。
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let e = V(3);\n    let b: Box<E> = box V(1);\n    let c = b;\n    b = box e;\n    println(*b);\n    println(*c);\n    println(e);\n}\n",
        "revive",
        b"V(3)\nV(1)\nV(3)\n",
    );
}

#[test]
fn qualified_annotated_and_value_copy_sources() {
    assert_runs(
        "enum E { V(i64), W(i64) }\nfn main() {\n    let e: E = E::V(1);\n    let f = e;\n    let b: Box<E> = box f;\n    println(*b);\n    println(e);\n    println(f);\n    let w = E::W(2);\n    let c: Box<E> = box w;\n    println(*c);\n    println(w);\n}\n",
        "sources",
        b"V(1)\nV(1)\nV(1)\nW(2)\nW(2)\n",
    );
}

#[test]
fn eight_integer_widths_from_variables() {
    assert_runs(
        "enum E { V(i8), W(i16), X(i32), Y(i64), Z(u8), P(u16), Q(u32), R(u64) }\nfn main() {\n    let a8: i8 = -128;\n    let e = V(a8);\n    let b: Box<E> = box e;\n    println(*b);\n    println(e);\n    let a16: i16 = -32768;\n    let c = W(a16);\n    let d: Box<E> = box c;\n    println(*d);\n    println(c);\n    let a32: i32 = -2147483648;\n    let f = X(a32);\n    let g: Box<E> = box f;\n    println(*g);\n    println(f);\n    let a64: i64 = 9223372036854775807;\n    let h = Y(a64);\n    let i: Box<E> = box h;\n    println(*i);\n    println(h);\n    let u8v: u8 = 255;\n    let j = Z(u8v);\n    let k: Box<E> = box j;\n    println(*k);\n    println(j);\n    let u16v: u16 = 65535;\n    let l = P(u16v);\n    let m: Box<E> = box l;\n    println(*m);\n    println(l);\n    let u32v: u32 = 4294967295;\n    let n = Q(u32v);\n    let o: Box<E> = box n;\n    println(*o);\n    println(n);\n    let u64v: u64 = 18446744073709551615;\n    let q = R(u64v);\n    let r: Box<E> = box q;\n    println(*r);\n    println(q);\n}\n",
        "widths",
        b"V(-128)\nV(-128)\nW(-32768)\nW(-32768)\nX(-2147483648)\nX(-2147483648)\nY(9223372036854775807)\nY(9223372036854775807)\nZ(255)\nZ(255)\nP(65535)\nP(65535)\nQ(4294967295)\nQ(4294967295)\nR(18446744073709551615)\nR(18446744073709551615)\n",
    );
}

#[test]
fn bool_and_float_variables_including_special_values() {
    assert_runs(
        "enum E { V(bool), F(f32), D(f64) }\nfn main() {\n    let t: bool = false;\n    let e = V(t);\n    let b: Box<E> = box e;\n    println(*b);\n    println(e);\n    let fm = f32::MAX;\n    let f = F(fm);\n    let c: Box<E> = box f;\n    println(*c);\n    println(f);\n    let d = f64::MAX;\n    let g = D(d);\n    let h: Box<E> = box g;\n    println(*h);\n    println(g);\n    let n = f64::NAN;\n    let i = D(n);\n    let j: Box<E> = box i;\n    println(*j);\n    println(i);\n    let ni = f32::NEG_INFINITY;\n    let k = F(ni);\n    let l: Box<E> = box k;\n    println(*l);\n    println(k);\n}\n",
        "scalars",
        b"V(false)\nV(false)\nF(3.4028235e+38)\nF(3.4028235e+38)\nD(1.7976931348623157e+308)\nD(1.7976931348623157e+308)\nD(nan)\nD(nan)\nF(-inf)\nF(-inf)\n",
    );
}

#[test]
fn variable_payload_boundaries_stay_fail_closed() {
    assert_rc1(
        "direct-expression",
        "enum E { V(i64) }\nfn main() {\n    let x: i64 = 7;\n    let b: Box<E> = box V(x);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "direct-arith",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<E> = box V(1 + 2);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "unannotated",
        "enum E { V(i64) }\nfn main() {\n    let e = V(1);\n    let b = box e;\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
    // L3a：顶层标量 Box 变量载荷已开放（原 non-enum-variable 负控翻转）。
    assert_runs(
        "fn main() {\n    let x: i64 = 1;\n    let b: Box<i64> = box x;\n    println(*b);\n}\n",
        "non-enum-variable",
        b"1\n",
    );
    assert_rc1(
        "struct-variable",
        "struct S { x: i64 }\nfn main() {\n    let s = S { x: 1 };\n    let b: Box<S> = box s;\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nfn main() {\n    let e = V(P { x: 1 });\n    let b: Box<E> = box e;\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "reinit-expression",
        "enum E { V(i64) }\nfn main() {\n    let e = V(1);\n    let b: Box<E> = box e;\n    b = box V(1 + 2);\n    println(*b);\n}\n",
        "赋值语句",
    );
    assert_rc1(
        "let-deref",
        "enum E { V(i64) }\nfn main() {\n    let e = V(1);\n    let b: Box<E> = box e;\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
    assert_rc1(
        "print-box",
        "enum E { V(i64) }\nfn main() {\n    let e = V(1);\n    let b: Box<E> = box e;\n    println(b);\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn variable_payload_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i64), W(f64) }\nfn main() {\n    let x: i64 = 7;\n    let e = V(x);\n    let b: Box<E> = box e;\n    println(*b);\n    println(e);\n    let w = W(2.5);\n    b = box w;\n    println(*b);\n    println(w);\n}\n",
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
