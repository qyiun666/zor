//! codegen 后端边界（ADR-0001 §1）。
//!
//! 数据流：check 后的 `Prog` → subset 闸 → lower → typed IR → C emitter → cc。
//! 首片只有 C 后端；未来后端在同一 IR 接口后追加，不重写前端。

pub mod c;
pub mod ir;
pub mod lower;
pub mod subset;

use crate::parser::Prog;
use crate::type_table::TypeTable;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptLevel {
    O0,
    O2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildOptions {
    pub opt: OptLevel,
    pub debug: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        BuildOptions {
            opt: OptLevel::O2,
            debug: false,
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
    subset::check(prog).map_err(|e| BuildError::Unsupported {
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
    let c_src = c::emit::emit(&ir, source_str);
    c::cc::compile(&c_src, output, opts.opt, opts.debug)
        .map_err(|msg| BuildError::Toolchain { msg })
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
    use super::{build, BuildOptions};
    use crate::compile_typed;
    use std::path::Path;

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
        let src = super::c::emit::emit(&ir, "t.zor");
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
