# specs — ZOR 0.1.x 语言规范

四条规范初稿（R1–R4）合入即满足 0.1.0 首个 tag 条件（PRD §11）。

| 文件 | 需求 | 关键 AC | 状态 |
|---|---|---|---|
| `r1-type-system.md` | R1 最小类型系统 | AC-R1-1/2/3 | draft-0.1 初稿成文（V-2） |
| `r2-ownership.md` | R2 所有权/借用子集 | AC-R2-1/2/3/4 | draft-0.1 初稿成文（V-2） |
| `r3-concurrency.md` | R3 轻量并发（task+channel） | AC-R3-1/2/3/4 | 大纲（M2 前成文） |
| `r4-c-ffi.md` | R4 C FFI | AC-R4-1/2/3 | 大纲（M2 前成文） |

规范用例目录唯一基准见 `tests/README.md` 目录契约（M-1 终审：`tests/specs/`）。

约定：
- 每份 spec 含：形成规则/语义判定表、正负例测试用例映射（AC 编号）、未决 OQ 引用。
- R2 与 R3 的所有权交叉规则（channel 转移）须在联合评审中互检无矛盾（AC-R3-1）。
- 行为以 PRD 为需求来源，本目录为规范性来源（normative）；冲突时以群内评审结论为准并记决策日志。
