//! 0.2.0 codegen 后端边界（ADR-0001 §1）。
//!
//! 数据流：check 后的 `Prog` → subset 闸 → lower → typed IR → C emitter → cc。
//! 首片只有 C 后端；未来后端在同一 IR 接口后追加，不重写前端。

pub mod c;
pub mod ir;
pub mod lower;
pub mod subset;

use crate::parser::Prog;
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
    /// 前端通过、但不在 0.2.0 首片子集：rc=1，零产物，不占 E 码。
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
                "zorcc: codegen: {}:{}:{}: 0.2.0 首片不支持 {}",
                source.display(),
                line,
                col,
                what
            ),
            BuildError::Toolchain { msg } => format!("zorcc: codegen: {msg}"),
        }
    }
}

/// 构建入口：仅在 `compile()` 返回零诊断后调用。
pub fn build(
    prog: &Prog,
    source: &Path,
    output: &Path,
    opts: BuildOptions,
) -> Result<(), BuildError> {
    subset::check(prog).map_err(|e| BuildError::Unsupported {
        line: e.line,
        col: e.col,
        what: e.what,
    })?;
    let ir = lower::lower(prog).map_err(|e| BuildError::Unsupported {
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
