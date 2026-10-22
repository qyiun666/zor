//! codegen 后端边界（ADR-0001 §1）。
//!
//! 数据流：check 后的 `Prog` → subset 闸 → lower → typed IR → C emitter → cc。
//! 首片只有 C 后端；未来后端在同一 IR 接口后追加，不重写前端。

pub mod c;
pub mod float_fmt;
pub mod ir;
pub mod lower;
pub mod subset;

use crate::parser::Prog;
use crate::type_table::TypeTable;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
    Os,
    Og,
}

impl OptLevel {
    /// 与 C 编译器同名旗标一一对应；`c::cc` 直接传给 cc，不做降级。
    pub fn cc_flag(self) -> &'static str {
        match self {
            OptLevel::O0 => "-O0",
            OptLevel::O1 => "-O1",
            OptLevel::O2 => "-O2",
            OptLevel::O3 => "-O3",
            OptLevel::Os => "-Os",
            OptLevel::Og => "-Og",
        }
    }

    /// 档位缺省 overflow：`O0`／`O1`／`Og` checked；`O2`／`O3`／`Os` wrap。
    pub fn default_overflow(self) -> OverflowMode {
        match self {
            OptLevel::O0 | OptLevel::O1 | OptLevel::Og => OverflowMode::Check,
            OptLevel::O2 | OptLevel::O3 | OptLevel::Os => OverflowMode::Wrap,
        }
    }
}

/// 算术模式：`O0`／`O1`／`Og` 缺省 checked；`O2`／`O3`／`Os` 及缺省 wrap；
/// `-Zoverflow` 显式覆盖。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowMode {
    Check,
    Wrap,
}

impl OverflowMode {
    pub fn from_opt(opt: OptLevel) -> Self {
        opt.default_overflow()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub opt: OptLevel,
    pub debug: bool,
    pub overflow: OverflowMode,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            opt: OptLevel::O2,
            debug: false,
            overflow: OverflowMode::Wrap,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// 前端通过、但不在当前 codegen 子集：rc=1，零产物，不占 E 码。
    Unsupported {
        line: usize,
        col: usize,
        what: String,
    },
    /// CLI／环境／C 工具链错误：rc=2，零残留。
    Toolchain { msg: String },
}

impl BuildError {
    pub fn rc(&self) -> i32 {
        match self {
            BuildError::Unsupported { .. } => 1,
            BuildError::Toolchain { .. } => 2,
        }
    }

    pub fn render(&self, source: &Path) -> String {
        match self {
            BuildError::Unsupported { line, col, what } => format!(
                "zorcc: codegen: {}:{}:{}: 当前 codegen 首片不支持 {}",
                source.display(),
                line,
                col,
                what
            ),
            BuildError::Toolchain { msg } => format!("zorcc: codegen: {msg}"),
        }
    }
}

/// 构建入口：仅在 `compile_typed()` 返回零诊断后调用。
/// `types` 来自 check（类型唯一来源）；lower 只读该表，不复制类型规则。
pub fn build(
    prog: &Prog,
    types: &TypeTable,
    source: &Path,
    output: &Path,
    opts: BuildOptions,
) -> Result<(), BuildError> {
    subset::check(prog, types).map_err(|e| BuildError::Unsupported {
        line: e.line,
        col: e.col,
        what: e.what,
    })?;
    let ir = lower::lower(prog, types).map_err(|e| BuildError::Unsupported {
        line: e.line,
        col: e.col,
        what: e.what,
    })?;
    let source_str = source.to_str().ok_or_else(|| BuildError::Toolchain {
        msg: "源路径不是合法 UTF-8，无法写入 #line".to_string(),
    })?;
    if same_file(source, output) {
        return Err(BuildError::Toolchain {
            msg: format!("输出路径与源文件相同：'{}'", output.display()),
        });
    }
    let c_src = c::emit::emit(&ir, source_str, opts.overflow);
    let runtime_dir = if c_src.contains("#include \"zor_rt.h\"") {
        Some(resolve_runtime_dir()?)
    } else {
        None
    };
    c::cc::compile(&c_src, output, opts, runtime_dir.as_deref())
        .map_err(|msg| BuildError::Toolchain { msg })
}

/// 运行期 helper 定位：环境变量优先；否则按可执行文件／仓根候选查找。
/// 缺件即 rc=2（Toolchain），不得退化为编译成功但链接失败。
fn resolve_runtime_dir() -> Result<std::path::PathBuf, BuildError> {
    fn valid(dir: &Path) -> bool {
        dir.join("zor_rt.c").is_file() && dir.join("zor_rt.h").is_file()
    }

    if let Ok(v) = std::env::var("ZOR_RUNTIME_DIR") {
        let dir = std::path::PathBuf::from(v);
        if valid(&dir) {
            return Ok(dir);
        }
        return Err(BuildError::Toolchain {
            msg: format!(
                "ZOR_RUNTIME_DIR='{}' 缺少 zor_rt.c／zor_rt.h",
                dir.display()
            ),
        });
    }

    let exe = std::env::current_exe().ok();
    let exe_dir = exe
        .as_ref()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let cwd = std::env::current_dir().ok();
    // 构建树回退仅对构建树内的 exe 生效：安装树外置时不得命中构建机绝对路径。
    let build_tree = exe.as_deref().is_some_and(exe_in_build_tree);
    let candidates = runtime_candidates(exe_dir.as_deref(), cwd.as_deref(), build_tree);
    for dir in &candidates {
        if valid(dir) {
            return Ok(dir.clone());
        }
    }
    let searched = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("，");
    Err(BuildError::Toolchain {
        msg: format!(
            "找不到运行期 helper zor_rt.c／zor_rt.h；已搜索：{}；请设置 ZOR_RUNTIME_DIR",
            searched
        ),
    })
}

/// exe 是否位于编译期记录的构建树（`CARGO_MANIFEST_DIR`）内。
fn exe_in_build_tree(exe: &Path) -> bool {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let manifest = manifest
        .canonicalize()
        .unwrap_or_else(|_| manifest.to_path_buf());
    exe.starts_with(manifest)
}

/// 候选顺序：exe 邻接（含 R-1 canonical `exe/../share/zor/runtime`）、cwd、
/// 仅在构建树内 exe 时追加源树回退。
fn runtime_candidates(
    exe_dir: Option<&Path>,
    cwd: Option<&Path>,
    build_tree_fallback: bool,
) -> Vec<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = exe_dir {
        candidates.push(dir.join("runtime"));
        // L2d R-1：canonical 安装树 prefix/bin/zorcc + prefix/share/zor/runtime。
        candidates.push(dir.join("../share/zor/runtime"));
        candidates.push(dir.join("../runtime"));
        candidates.push(dir.join("../../runtime"));
        candidates.push(dir.join("../../../runtime"));
    }
    if let Some(cwd) = cwd {
        candidates.push(cwd.join("runtime"));
    }
    if build_tree_fallback {
        candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime"));
    }
    candidates
}

fn same_file(source: &Path, output: &Path) -> bool {
    let src = match source.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let out = if output.exists() {
        match output.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        match (parent.canonicalize(), output.file_name()) {
            (Ok(p), Some(n)) => p.join(n),
            _ => return false,
        }
    };
    src == out
}

#[cfg(test)]
mod tests {
    use super::{build, runtime_candidates, BuildOptions, OverflowMode};
    use crate::compile_typed;
    use std::path::Path;

    #[test]
    fn r1_canonical_share_candidate_is_present() {
        let exe_dir = Path::new("/opt/zor/bin");
        let candidates = runtime_candidates(Some(exe_dir), None, false);
        let share = exe_dir.join("../share/zor/runtime");
        assert!(candidates.contains(&share), "{candidates:?}");
        // ZOR_RUNTIME_DIR 优先级不变：候选表只在 env 未设置时使用。
        assert_eq!(candidates[0], exe_dir.join("runtime"));
        // 安装树外置：不得命中构建机绝对路径。
        let manifest_fallback = Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime");
        assert!(!candidates.contains(&manifest_fallback), "{candidates:?}");
        let in_tree = runtime_candidates(Some(exe_dir), None, true);
        assert!(in_tree.contains(&manifest_fallback), "{in_tree:?}");
    }

    #[test]
    fn r1_missing_runtime_error_lists_searched_candidates() {
        let exe_dir = Path::new("/opt/zor/bin");
        let candidates = runtime_candidates(Some(exe_dir), Some(Path::new("/tmp")), false);
        let searched = candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("，");
        assert!(searched.contains("/opt/zor/bin/../share/zor/runtime"), "{searched}");
        assert!(searched.contains("/tmp/runtime"), "{searched}");
    }

    #[test]
    fn box_program_builds_runs_and_exits_zero() {
        let c = compile_typed("fn main() { let b = box 1; }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let out = std::env::temp_dir().join(format!(
            "zor-core04-build-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let built = build(
            &c.prog,
            &c.types,
            Path::new("t.zor"),
            &out,
            BuildOptions::default(),
        );
        assert!(built.is_ok(), "{built:?}");
        assert!(out.is_file(), "产物缺失");
        let run = std::process::Command::new(&out).output().expect("run artifact");
        assert_eq!(run.status.code(), Some(0));
        assert!(run.stdout.is_empty());
        assert!(run.stderr.is_empty(), "{:?}", run.stderr);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn hello_emit_has_no_alloc_or_free() {
        let c = compile_typed("fn main() { let s = \"hi\"; println(s); }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let ir = super::lower::lower(&c.prog, &c.types).expect("lower");
        let src = super::c::emit::emit(&ir, "t.zor", OverflowMode::Wrap);
        assert!(!src.contains("malloc"), "{src}");
        assert!(!src.contains("free("), "{src}");
    }

    #[test]
    fn six_runtime_samples_build_and_run_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for name in [
            "box_create_read_unbox",
            "box_scope_drop",
            "box_overwrite_drop",
            "box_move_between_bindings",
            "box_self_assign_no_drop",
            "box_unbox_then_reinit",
        ] {
            let src_path = root
                .join("tests/runtime/cases")
                .join(format!("{name}.zor"));
            let text = std::fs::read_to_string(&src_path).expect("read runtime sample");
            let c = compile_typed(&text);
            assert!(c.diags.is_empty(), "{name}: {:?}", c.diags);
            let out = std::env::temp_dir().join(format!(
                "zor-core04-{}-{}-{}",
                name,
                std::process::id(),
                nanos
            ));
            let built = build(
                &c.prog,
                &c.types,
                &src_path,
                &out,
                BuildOptions::default(),
            );
            assert!(built.is_ok(), "{name}: {built:?}");
            let run = std::process::Command::new(&out).output().expect("run artifact");
            assert_eq!(run.status.code(), Some(0), "{name}");
            assert!(run.stdout.is_empty(), "{name}: {:?}", run.stdout);
            assert!(run.stderr.is_empty(), "{name}: {:?}", run.stderr);
            let _ = std::fs::remove_file(&out);
        }
    }
}
