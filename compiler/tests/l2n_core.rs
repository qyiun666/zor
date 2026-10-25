//! L2n 核心集成测试：嵌套复合中的枚举值直接特殊值形式（表面形式对齐）。
//!
//! 覆盖：struct 字段／数组元素／元组元素、任意深度、附加成员、三种外层别名、
//! 多实参、单／多字段枚举、f32／f64 混排、直接↔绑定同构、有限值嵌套零回退；
//! 以及反方向（特殊值进入枚举自身复合载荷）、跨宽／整型／bool／错元数／`E::V`／
//! `Box<enum>`／扩展常量／算术比较的 fail-closed 红线与六档稳定性。
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
            "zorcc-l2n-{}-{}-{}-{}",
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

/// 同构断言：源码按「直接形式 → 绑定形式」成对打印，逐对行相等。
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
        assert_eq!(pair[0], pair[1], "{tag} 直接形式须与绑定形式同构: {out:?}");
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
fn struct_field_positions_and_extra_members() {
    assert_runs(
        "enum E { V(f64), W(f64, f64) }\nstruct S { e: E, n: i64 }\nfn main() {\n    println(S { e: V(f64::NAN), n: 1 });\n    println(S { e: W(1.5, f64::INFINITY), n: 2 });\n}\n",
        "struct",
        b"S { e: V(nan), n: 1 }\nS { e: W(1.5, inf), n: 2 }\n",
    );
}

#[test]
fn array_and_tuple_elements() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println([S { e: V(f64::NAN) }, S { e: V(1.5) }]);\n    println((S { e: V(f64::NEG_INFINITY) }, 2));\n}\n",
        "elements",
        b"[S { e: V(nan) }, S { e: V(1.5) }]\n(S { e: V(-inf) }, 2)\n",
    );
}

#[test]
fn composite_fields_with_enum_values() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { a: [E; 2] }\nfn main() {\n    println(S { a: [V(f64::NAN), V(1.5)] });\n}\n",
        "array-field",
        b"S { a: [V(nan), V(1.5)] }\n",
    );
    assert_runs(
        "enum E { V(f64, f64) }\nstruct S { t: (E, i64) }\nfn main() {\n    println(S { t: (V(f64::NAN, 1.5), 7) });\n}\n",
        "tuple-field",
        b"S { t: (V(nan, 1.5), 7) }\n",
    );
}

#[test]
fn arbitrary_depth() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nstruct T { s: S }\nfn main() {\n    println(T { s: S { e: V(f64::NAN) } });\n    println([(V(f64::NAN), 1)]);\n}\n",
        "deep",
        b"T { s: S { e: V(nan) } }\n[(V(nan), 1)]\n",
    );
}

#[test]
fn outer_aliases_for_struct_array_and_tuple() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    let s = S { e: V(f64::NAN) };\n    println(s, s);\n    let a = [V(f64::INFINITY)];\n    println(a);\n    let t = (V(f64::NEG_INFINITY), 1);\n    println(t);\n}\n",
        "alias",
        b"S { e: V(nan) } S { e: V(nan) }\n[V(inf)]\n(V(-inf), 1)\n",
    );
}

#[test]
fn multi_arg_with_nested_forms() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) }, [V(f64::INFINITY)], (V(1.5), 1));\n}\n",
        "multiarg",
        b"S { e: V(nan) } [V(inf)] (V(1.5), 1)\n",
    );
}

#[test]
fn direct_forms_are_isomorphic_with_binding_forms() {
    assert_pairs_isomorphic(
        "iso",
        "enum E { V(f64), W(f64, f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n    let x = f64::NAN;\n    println(S { e: V(x) });\n    println([V(1.5), W(f64::NAN, 2.5)]);\n    let y = f64::NAN;\n    println([V(1.5), W(y, 2.5)]);\n    println((W(1.5, f64::NEG_INFINITY), 1));\n    let z = f64::NEG_INFINITY;\n    println((W(1.5, z), 1));\n}\n",
    );
}

#[test]
fn mixed_width_and_multi_field_nested() {
    assert_runs(
        "enum E { A(f64), B(f32) }\nstruct S { e: E }\nfn main() {\n    println(S { e: A(f64::NAN) });\n    println([S { e: B(f32::NEG_INFINITY) }]);\n}\n",
        "widths",
        b"S { e: A(nan) }\n[S { e: B(-inf) }]\n",
    );
}

#[test]
fn finite_nested_zero_regression() {
    assert_runs(
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(1.5) });\n    println([V(1.5), V(2.5)]);\n    println((V(-0.0), 1));\n}\n",
        "finite",
        b"S { e: V(1.5) }\n[V(1.5), V(2.5)]\n(V(-0), 1)\n",
    );
}

#[test]
fn reverse_composite_payload_stays_fail_closed() {
    assert_rc1(
        "struct",
        "struct P { f: f64 }\nenum E { V(P, f64) }\nfn main() {\n    println(V(P { f: f64::NAN }, 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "array",
        "enum E { V([f64; 2], f64) }\nfn main() {\n    println(V([f64::NAN, 1.0], 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "tuple",
        "enum E { V((f64, f64), f64) }\nfn main() {\n    println(V((f64::NAN, 1.0), 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "enum",
        "enum W { A(f64) }\nenum E { V(W, f64) }\nfn main() {\n    println(V(A(f64::NAN), 1.5));\n}\n",
        "println 实参不可打印",
    );
    assert_rc1(
        "struct-enum",
        "enum W { A(f64) }\nstruct S { e: W }\nenum E { V(S, f64) }\nfn main() {\n    println(V(S { e: A(f64::NAN) }, 1.5));\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn nested_width_and_kind_are_strict() {
    assert_rc1(
        "cross",
        "enum E { V(f32) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n}\n",
        "E1201",
    );
    assert_rc1(
        "int",
        "enum E { V(i64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n}\n",
        "E1202",
    );
    assert_rc1(
        "bool",
        "enum E { V(bool) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n}\n",
        "E1201",
    );
}

#[test]
fn nested_arity_path_box_ext_and_arith_stay_fail_closed() {
    assert_rc1(
        "arity",
        "enum E { V(f64, f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN) });\n}\n",
        "E1601",
    );
    assert_rc1(
        "path",
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: E::V(f64::NAN) });\n}\n",
        "E1101",
    );
    assert_rc1(
        "box",
        "enum E { V(f64) }\nfn main() {\n    let b: Box<E> = box V(1.5);\n    println(*b);\n}\n",
        "let 类型标注",
    );
    assert_rc1(
        "max",
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::MAX) });\n}\n",
        "E1101",
    );
    assert_rc1(
        "arith",
        "enum E { V(f64) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN + 1.5) });\n}\n",
        "println 实参不可打印",
    );
}

#[test]
fn nested_forms_are_stable_across_opt_levels() {
    let d = TempDir::new("opt");
    let src = write(
        d.path(),
        "opt.zor",
        "enum E { V(f64, f64), W(f64, f32) }\nstruct S { e: E }\nfn main() {\n    println(S { e: V(f64::NAN, 1.5) });\n    println([W(1.5, f32::INFINITY)]);\n}\n",
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
