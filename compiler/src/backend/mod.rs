//! 0.2.0 codegen 后端边界（ADR-0001 §1）。
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
    // L2a A1 守卫：CORE-03 只产事件，不生成 malloc／free／drop；运行期归 CORE-04。
    if ir.needs_l2a_runtime() {
        let (line, col) = ir.first_l2a_pos().unwrap_or((1, 1));
        return Err(BuildError::Unsupported {
            line,
            col,
            what: "drop／Box 运行期（L2a CORE-04 前不生成）".to_string(),
        });
    }
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
    use super::{build, BuildError, BuildOptions};
    use crate::compile_typed;
    use std::path::Path;

    #[test]
    fn l2a_runtime_guard_rejects_box_program_before_emit() {
        let c = compile_typed("fn main() { let b = box 1; }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let e = build(
            &c.prog,
            &c.types,
            Path::new("t.zor"),
            Path::new("/tmp/zor-core03-guard-never"),
            BuildOptions::default(),
        )
        .expect_err("guard must reject");
        match e {
            BuildError::Unsupported { what, line, col } => {
                assert!(what.contains("CORE-04"), "{what}");
                assert_eq!((line, col), (1, 13));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn hello_ir_has_no_l2a_runtime() {
        let c = compile_typed("fn main() { let s = \"hi\"; println(s); }");
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let ir = super::lower::lower(&c.prog, &c.types).expect("lower");
        assert!(!ir.needs_l2a_runtime());
    }
}
