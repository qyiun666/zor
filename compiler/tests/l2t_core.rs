//! L2t 核心集成测试：`Box<enum>` 常量枚举载荷子集。
//!
//! 覆盖：零／单／多字段、标量字段（整型 8 宽／bool／f32／f64＋有限／特殊值／
//! 命名常量／整型成员含负值）、裸与限定变体、`println(*b)`、移动、常量重初始化、
//! `unbox`、多 Box、与裸枚举逐字节一致、六档 `-O*` 稳定；
//! 以及变量／表达式载荷、无标注、嵌套 Box、Box 复合含 enum、`Box<[E;N]>`、
//! enum 变体含 Box／struct／enum 字段、`println(b)`、`let v=*b`、所有权错误、
//! 类型／宽度不符等 fail-closed 面。只依赖宿主 `cc`；不访问网络。

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
            "zorcc-l2t-{}-{}-{}-{}",
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

/// 同构断言：源码按「Box 打印 → 裸枚举打印」成对输出，逐对行相等。
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
fn box_enum_constant_positions() {
    assert_runs(
        "enum E { A(i64), V(f64), N, B(bool), F(f32) }\nfn main() {\n    let a: Box<E> = box A(1);\n    println(*a);\n    let v: Box<E> = box V(1.5);\n    println(*v);\n    let n: Box<E> = box N();\n    println(*n);\n    let b: Box<E> = box B(true);\n    println(*b);\n    let f: Box<E> = box F(f32::MIN);\n    println(*f);\n    let q: Box<E> = box E::V(2.5);\n    println(*q);\n    let m: Box<E> = box A(f64::MAX_EXP);\n    println(*m);\n    let neg: Box<E> = box A(f64::MIN_EXP);\n    println(*neg);\n    let s: Box<E> = box V(f64::NAN);\n    println(*s);\n    let mx: Box<E> = box V(f64::MAX);\n    println(*mx);\n    let mp: Box<E> = box V(f64::MIN_POSITIVE);\n    println(*mp);\n}\n",
        "positions",
        b"A(1)\nV(1.5)\nN\nB(true)\nF(-3.4028235e+38)\nV(2.5)\nA(1024)\nA(-1021)\nV(nan)\nV(1.7976931348623157e+308)\nV(2.2250738585072014e-308)\n",
    );
}

#[test]
fn box_enum_multi_field_and_negative_values() {
    assert_runs(
        "enum M { P(f64, f64), Q(i64, bool), R(f64, i64), S(f32) }\nfn main() {\n    let a: Box<M> = box P(1.5, 2.5);\n    println(*a);\n    let b: Box<M> = box Q(7, true);\n    println(*b);\n    let c: Box<M> = box R(-1.5, -2);\n    println(*c);\n    let d: Box<M> = box S(-0.0);\n    println(*d);\n}\n",
        "multi",
        b"P(1.5, 2.5)\nQ(7, true)\nR(-1.5, -2)\nS(-0)\n",
    );
}

#[test]
fn box_enum_matches_bare_variant() {
    assert_pairs_isomorphic(
        "iso",
        "enum E { A(i64), V(f64), N }\nfn main() {\n    println(V(1.5));\n    let a: Box<E> = box V(1.5);\n    println(*a);\n    println(A(1024));\n    let b: Box<E> = box A(f64::MAX_EXP);\n    println(*b);\n    println(N());\n    let c: Box<E> = box N();\n    println(*c);\n}\n",
    );
}

#[test]
fn box_enum_ownership_forms() {
    assert_runs(
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    let c = b;\n    println(*c);\n    let d: Box<E> = box A(2);\n    d = box A(3);\n    println(*d);\n    let e: Box<E> = box A(4);\n    unbox e;\n}\n",
        "ownership",
        b"A(1)\nA(3)\n",
    );
    assert_runs(
        "enum E { A(i64) }\nfn main() {\n    let a: Box<E> = box A(1);\n    let b: Box<E> = box A(2);\n    println(*a);\n    println(*b);\n}\n",
        "multi-box",
        b"A(1)\nA(2)\n",
    );
}

#[test]
fn box_enum_constness_fail_closed() {
    // L2v：变量枚举载荷翻转（Copy 语义）；表达式／无标注仍 fail-closed。
    assert_runs(
        "enum E { A(i64) }\nfn main() {\n    let e = A(1);\n    let b: Box<E> = box e;\n    println(*b);\n    println(e);\n}\n",
        "variable",
        b"A(1)\nA(1)\n",
    );
    assert_rc1(
        "expression",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1 + 2);\n    println(*b);\n}\n",
        "box 载荷须为字面量常量",
    );
    assert_rc1(
        "unannotated",
        "enum E { A(i64) }\nfn main() {\n    let b = box A(1);\n    println(*b);\n}\n",
        "box 载荷非整型字面量",
    );
}

#[test]
fn box_enum_composite_boundaries_stay_fail_closed() {
    assert_rc1(
        "nested-box",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<Box<E>> = box box A(1);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-struct-enum-field",
        "enum E { A(i64) }\nstruct S { e: E }\nfn main() {\n    let b: Box<S> = box S { e: A(1) };\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "box-array-enum",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<[E; 1]> = box [A(1)];\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-box-field",
        "enum E { A(Box<i64>) }\nfn main() {\n    let b: Box<E> = box A(box 1);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-struct-field",
        "struct P { x: i64 }\nenum E { V(P) }\nfn main() {\n    let b: Box<E> = box V(P { x: 1 });\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "enum-enum-field",
        "enum I { A(i64) }\nenum E { V(I) }\nfn main() {\n    let b: Box<E> = box V(A(1));\n    println(*b);\n}\n",
        "let 类型标注",
    );
}

#[test]
fn box_enum_read_forms_stay_fail_closed() {
    assert_rc1(
        "print-box",
        "enum E { A(f64) }\nfn main() {\n    let b: Box<E> = box A(1.5);\n    println(b);\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "print-tuple",
        "enum E { A(f64) }\nfn main() {\n    let b: Box<E> = box A(1.5);\n    println((*b, 1));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "let-deref",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    let v = *b;\n    println(v);\n}\n",
        "解引用源不是 Box 绑定",
    );
}

#[test]
fn box_enum_type_and_ownership_errors() {
    assert_rc1(
        "wrong-enum",
        "enum E { A(i64) }\nenum F { A(i64) }\nfn main() {\n    let b: Box<F> = box E::A(1);\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "unknown-variant",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box E::X(1);\n    println(*b);\n}\n",
        "E1101",
    );
    assert_rc1(
        "arity",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A();\n    println(*b);\n}\n",
        "E1601",
    );
    assert_rc1(
        "cross-width",
        "enum E { V(f32) }\nfn main() {\n    let b: Box<E> = box V(f64::NAN);\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "int-to-float",
        "enum E { V(f64) }\nfn main() {\n    let b: Box<E> = box V(1);\n    println(*b);\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool-to-float",
        "enum E { V(f64) }\nfn main() {\n    let b: Box<E> = box V(true);\n    println(*b);\n}\n",
        "E1201",
    );
    assert_rc1(
        "double-unbox",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    unbox b;\n    unbox b;\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-unbox",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    unbox b;\n    println(*b);\n}\n",
        "E2301",
    );
    assert_rc1(
        "use-after-move",
        "enum E { A(i64) }\nfn main() {\n    let b: Box<E> = box A(1);\n    let c = b;\n    println(*c);\n    println(*b);\n}\n",
        "E2101",
    );
}

#[test]
fn box_enum_qualified_disambiguation() {
    assert_runs(
        "enum A { V(i64) }\nenum B { V(f64) }\nfn main() {\n    let b: Box<B> = box B::V(1.5);\n    println(*b);\n    let a: Box<A> = box A::V(2);\n    println(*a);\n}\n",
        "disambig",
        b"V(1.5)\nV(2)\n",
    );
}

#[test]
fn box_enum_is_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { A(i64), V(f64), N }\nfn main() {\n    let a: Box<E> = box A(f64::MAX_EXP);\n    println(*a);\n    let v: Box<E> = box E::V(f64::NAN);\n    println(*v);\n    let n: Box<E> = box N();\n    println(*n);\n}\n",
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
