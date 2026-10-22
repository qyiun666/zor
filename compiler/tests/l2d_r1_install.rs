//! L2d R-1：canonical 安装树 runtime 解析集成测试。
//!
//! 复制 zorcc 到临时 `prefix/bin/`，helper 放 `prefix/share/zor/runtime/`，
//! 在无关 cwd、无 `ZOR_RUNTIME_DIR` 下编译运行需要 helper 的程序；
//! 删除安装树 helper 后必须 rc=2、列出已搜索候选、零产物，
//! 且不得回退到构建机 `CARGO_MANIFEST_DIR` 路径。

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
            "zorcc-l2d-r1-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            n
        ));
        fs::create_dir_all(&p).expect("create temp dir");
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

fn repo_runtime() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime")
}

fn install_prefix(root: &Path) -> (PathBuf, PathBuf) {
    let prefix = root.join("prefix");
    let bin = prefix.join("bin");
    let share = prefix.join("share/zor/runtime");
    fs::create_dir_all(&bin).expect("mkdir bin");
    fs::create_dir_all(&share).expect("mkdir share");
    let zorcc = bin.join("zorcc");
    fs::copy(env!("CARGO_BIN_EXE_zorcc"), &zorcc).expect("copy zorcc");
    for f in ["zor_rt.c", "zor_rt.h"] {
        fs::copy(repo_runtime().join(f), share.join(f)).expect("copy runtime helper");
    }
    (prefix, zorcc)
}

fn run(zorcc: &Path, cwd: &Path, src: &Path, out: &Path) -> Output {
    Command::new(zorcc)
        .current_dir(cwd)
        .env_remove("ZOR_RUNTIME_DIR")
        .arg(src)
        .arg("-o")
        .arg(out)
        .output()
        .expect("spawn zorcc")
}

/// 需要 `zor_rt.h`／`zor_rt.c`（整型除法 helper）的最小程序。
const ARITH: &str = "fn main() { let a = 7; let b = 2; let c = a / b; println(c); }\n";

#[test]
fn install_tree_share_runtime_is_resolved_from_unrelated_cwd() {
    let d = TempDir::new("ok");
    let root = d.path().join("root");
    let (_prefix, zorcc) = install_prefix(&root);
    let work = d.path().join("work");
    fs::create_dir_all(&work).expect("mkdir work");
    let src = work.join("case.zor");
    fs::write(&src, ARITH).expect("write source");
    let out = work.join("case");

    let b = run(&zorcc, &work, &src, &out);
    assert_eq!(
        b.status.code(),
        Some(0),
        "build stderr={}",
        String::from_utf8_lossy(&b.stderr)
    );
    let r = Command::new(&out).output().expect("run artifact");
    assert_eq!(r.status.code(), Some(0));
    assert_eq!(r.stdout, b"3\n");
    assert!(r.stderr.is_empty(), "{:?}", r.stderr);
}

#[test]
fn missing_install_runtime_is_rc2_lists_candidates_and_leaves_no_artifact() {
    let d = TempDir::new("missing");
    let root = d.path().join("root");
    let (prefix, zorcc) = install_prefix(&root);
    fs::remove_dir_all(prefix.join("share/zor/runtime")).expect("remove helper");
    let work = d.path().join("work");
    fs::create_dir_all(&work).expect("mkdir work");
    let src = work.join("case.zor");
    fs::write(&src, ARITH).expect("write source");
    let out = work.join("case");

    let b = run(&zorcc, &work, &src, &out);
    assert_eq!(b.status.code(), Some(2), "缺 helper 必须 rc=2");
    assert!(!out.exists(), "失败不得留产物");
    let err = String::from_utf8_lossy(&b.stderr);
    assert!(err.contains("zorcc: codegen:"), "{err}");
    assert!(err.contains("已搜索"), "{err}");
    assert!(err.contains("share/zor/runtime"), "{err}");
    assert!(err.contains("ZOR_RUNTIME_DIR"), "{err}");
    // 不得回退到构建机源码树；诊断中不应出现 CARGO_MANIFEST_DIR 路径。
    let build_runtime = repo_runtime().display().to_string();
    assert!(!err.contains(&build_runtime), "{err}");
}
