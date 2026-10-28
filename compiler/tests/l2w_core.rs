//! L2w 核心集成测试：⑤a `Box` 常量复合树含 enum 叶子。
//!
//! 覆盖：`Box<S{e:E}>`／`Box<[E;N]>`／`Box<[S;N]>`／嵌套 struct／struct 内 enum
//! 数组／嵌套数组／多 enum 类型／零字段与限定变体／8 整型宽度与特殊值／
//! 空数组／常量重初始化／move／unbox／drop／`println(*b)` 与字面量逐字节一致／
//! 六档 `-O*` 稳定；以及变量复合叶子、表达式载荷、非标量 enum 字段、嵌套 Box、
//! 无标注、同类型重复、读形态与所有权错误等 fail-closed 面。只依赖宿主 `cc`。

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
            "zorcc-l2w-{}-{}-{}-{}",
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

/// 同构断言：每两行一组（Box 打印／同字面量打印），逐对相等。
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
        assert_eq!(pair[0], pair[1], "{tag} Box 与字面量须同字节: {out:?}");
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
fn box_struct_constant_enum_matches_literal() {
    assert_pairs_isomorphic(
        "struct",
        "enum E { V(i64), W(f64), N, P(i64, bool, f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let a: Box<S> = box S { e: V(1), x: 9 };\n    println(*a);\n    println(S { e: V(1), x: 9 });\n    let b: Box<S> = box S { e: N(), x: 2 };\n    println(*b);\n    println(S { e: N(), x: 2 });\n    let c: Box<S> = box S { e: E::W(1.5), x: 3 };\n    println(*c);\n    println(S { e: E::W(1.5), x: 3 });\n    let d: Box<S> = box S { e: P(1024, true, 1.5), x: 4 };\n    println(*d);\n    println(S { e: P(1024, true, 1.5), x: 4 });\n    let e: Box<S> = box S { e: V(f64::MAX_EXP), x: 5 };\n    println(*e);\n    println(S { e: V(f64::MAX_EXP), x: 5 });\n}\n",
    );
}

#[test]
fn box_array_constant_enum_matches_literal() {
    assert_pairs_isomorphic(
        "array",
        "enum E { V(i64), W(f64), N, B(bool) }\nfn main() {\n    let a: Box<[E; 2]> = box [V(1), W(2.5)];\n    println(*a);\n    println([V(1), W(2.5)]);\n    let b: Box<[E; 3]> = box [N(), B(true), V(4)];\n    println(*b);\n    println([N(), B(true), V(4)]);\n    let c: Box<[E; 1]> = box [E::V(5)];\n    println(*c);\n    println([E::V(5)]);\n    let d: Box<[E; 0]> = box [];\n    println(*d);\n    let empty: [E; 0] = [];\n    println(empty);\n}\n",
    );
}

#[test]
fn nested_struct_array_and_multi_enum() {
    assert_pairs_isomorphic(
        "nested",
        "enum E { V(i64), W(f64), N }\nstruct S { e: E, x: i64 }\nstruct S2 { inner: S }\nstruct SA { a: [E; 2] }\nfn main() {\n    let a: Box<[S; 2]> = box [S { e: V(1), x: 1 }, S { e: W(2.5), x: 2 }];\n    println(*a);\n    println([S { e: V(1), x: 1 }, S { e: W(2.5), x: 2 }]);\n    let b: Box<S2> = box S2 { inner: S { e: V(3), x: 3 } };\n    println(*b);\n    println(S2 { inner: S { e: V(3), x: 3 } });\n    let c: Box<SA> = box SA { a: [V(1), N()] };\n    println(*c);\n    println(SA { a: [V(1), N()] });\n    let d: Box<[[E; 1]; 2]> = box [[V(1)], [W(2.5)]];\n    println(*d);\n    println([[V(1)], [W(2.5)]]);\n}\n",
    );
    assert_runs(
        "enum E { V(i64) }\nenum F { Q(u8) }\nstruct S { e: E, f: F }\nfn main() {\n    let b: Box<S> = box S { e: V(1), f: Q(255) };\n    println(*b);\n    println(S { e: V(1), f: Q(255) });\n}\n",
        "multi-enum",
        b"S { e: V(1), f: Q(255) }\nS { e: V(1), f: Q(255) }\n",
    );
}

#[test]
fn scalar_widths_and_special_values_in_struct_enum() {
    assert_runs(
        "enum E { A(i8), B(i16), C(i32), D(i64), E(u8), F(u16), G(u32), H(u64), I(bool), J(f32), K(f64) }\nstruct S { e: E }\nfn main() {\n    let a: Box<S> = box S { e: A(-128) };\n    println(*a);\n    let b: Box<S> = box S { e: B(-32768) };\n    println(*b);\n    let c: Box<S> = box S { e: C(-2147483648) };\n    println(*c);\n    let d: Box<S> = box S { e: D(9223372036854775807) };\n    println(*d);\n    let e: Box<S> = box S { e: E(255) };\n    println(*e);\n    let f: Box<S> = box S { e: F(65535) };\n    println(*f);\n    let g: Box<S> = box S { e: G(4294967295) };\n    println(*g);\n    let h: Box<S> = box S { e: H(18446744073709551615) };\n    println(*h);\n    let i: Box<S> = box S { e: I(true) };\n    println(*i);\n    let j: Box<S> = box S { e: J(f32::MAX) };\n    println(*j);\n    let k: Box<S> = box S { e: K(f64::MIN_POSITIVE) };\n    println(*k);\n    let n: Box<S> = box S { e: K(f64::NAN) };\n    println(*n);\n    let ni: Box<S> = box S { e: K(f64::NEG_INFINITY) };\n    println(*ni);\n}\n",
        "widths",
        b"S { e: A(-128) }\nS { e: B(-32768) }\nS { e: C(-2147483648) }\nS { e: D(9223372036854775807) }\nS { e: E(255) }\nS { e: F(65535) }\nS { e: G(4294967295) }\nS { e: H(18446744073709551615) }\nS { e: I(true) }\nS { e: J(3.4028235e+38) }\nS { e: K(2.2250738585072014e-308) }\nS { e: K(nan) }\nS { e: K(-inf) }\n",
    );
}

#[test]
fn constant_reinit_move_unbox_and_scope_drop() {
    assert_runs(
        "enum E { V(i64), W(f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let b: Box<S> = box S { e: V(1), x: 1 };\n    b = box S { e: W(2.5), x: 2 };\n    println(*b);\n    let c = b;\n    println(*c);\n    let d: Box<S> = box S { e: V(3), x: 3 };\n    unbox d;\n    println(V(3));\n}\n",
        "ownership",
        b"S { e: W(2.5), x: 2 }\nS { e: W(2.5), x: 2 }\nV(3)\n",
    );
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(7) };\n    println(*b);\n}\n",
        "scope-drop",
        b"S { e: V(7) }\n",
    );
}

#[test]
fn variable_and_expression_leaves_stay_fail_closed() {
    // L2x：一层变量 enum 叶转正（struct 直接字段）；表达式／无标注仍 fail-closed。
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let e = V(1);\n    let b: Box<S> = box S { e: e };\n    println(*b);\n}\n",
        "variable-leaf",
        b"S { e: V(1) }\n",
    );
    // L3f：enum 构造表达式叶已开放（原负控翻转）。
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let x: i64 = 1;\n    let b: Box<S> = box S { e: V(x) };\n    println(*b);\n}\n",
        "expression-field",
        b"S { e: V(1) }\n",
    );
    assert_runs(
        "enum E { V(i64) }\nfn main() {\n    let b: Box<[E; 1]> = box [V(1 + 2)];\n    println(*b);\n}\n",
        "expression-arith",
        b"[V(3)]\n",
    );
    // L2x：一层变量复合重初始化转正。
    assert_runs(
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1) };\n    let e2 = V(2);\n    b = box S { e: e2 };\n    println(*b);\n}\n",
        "variable-reinit",
        b"S { e: V(2) }\n",
    );
    assert_rc1(
        "unannotated",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b = box S { e: V(1) };\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
}

#[test]
fn non_scalar_enum_fields_and_seen_repeat_stay_fail_closed() {
    assert_rc1(
        "enum-box-field",
        "enum E { A(Box<i64>) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(box 1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(P { x: 1 }) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-array-field",
        "enum E { V([i64; 2]) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V([1, 2]) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "seen-repeat",
        "enum E { V(i64) }\nstruct S { a: E, b: E }\nfn main() {\n    let b: Box<S> = box S { a: V(1), b: V(2) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "byte-array-field",
        "struct S { b: [byte; 2] }\nfn main() {\n    let b: Box<S> = box S { b: \"ab\" };\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn nested_box_and_read_forms_stay_fail_closed() {
    assert_rc1(
        "nested-box",
        "enum E { V(i64) }\nfn main() {\n    let b: Box<Box<E>> = box box V(1);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-array-of-box",
        "fn main() {\n    let b: Box<[Box<i64>; 1]> = box [box 1];\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "let-deref",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1) };\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
    assert_rc1(
        "print-box",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1) };\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "double-unbox",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1) };\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: V(1) };\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn constant_composite_enum_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(i64), W(f64) }\nstruct S { e: E, x: i64 }\nfn main() {\n    let a: Box<S> = box S { e: V(1), x: 1 };\n    println(*a);\n    let b: Box<[S; 2]> = box [S { e: V(2), x: 2 }, S { e: W(3.5), x: 3 }];\n    println(*b);\n    b = box [S { e: W(4.5), x: 4 }, S { e: V(5), x: 5 }];\n    println(*b);\n}\n",
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
