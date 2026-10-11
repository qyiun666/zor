# tests — 规范测试用例集

用例为 0.1.x 编译门禁的单一执行载体（对齐 QA 门禁草案 MG/RG/HG 三层；BG 口径已废，见 D-012）。

## 目录契约（M-1 终审定论，唯一基准，核心工程师）

规范用例根目录为 **`tests/specs/`**，不存在第二种口径：

- `tests/specs/positive/`：正例，命名 `r<N>-<序号>-<slug>.zor`（如 `r2-01-move-basic.zor`）
- `tests/specs/negative/`：负例，命名 `r<N>-n<序号>-<slug>.zor`；必须编译期失败并输出 错误码+行:列+错误类型
- `tests/specs/compare/`：AC-R2-4 的 Rust 同题对照源码
- `tests/runners/run_specs.sh`：runner 契约入口，实际逻辑在 `ci/run_ci.sh`

runner 匹配面：`ci/run_ci.sh` 常量 `TESTS_POS_DIR/TESTS_NEG_DIR` 单点定义（=tests/specs/*）；strict 下**负例数=0 即 FAIL**（防空套件假绿）。历史文档中 `specs/tests/…` 写法一律视为过时（PRD v1.1 勘误），不得进入任何脚本。

## 数量下限（PRD AC 摘录）

| 来源 | 正例 | 负例 |
|---|---|---|
| R1 (AC-R1-2) | ≥20 | ≥10 |
| R2 (AC-R2-2) | 覆盖可表达清单 | ≥15（借用违规） |
| R3 (AC-R3-2/4) | 3 类并发正例 | 全量拦截 |
| R4 (AC-R4-2) | strlen/qsort/零拷贝 | ≥5 |

通过判据：负例失败率 0、正例通过 0 豁免（PRD §10-1）。
