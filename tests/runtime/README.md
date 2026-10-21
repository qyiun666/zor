# tests/runtime — 0.2.0 运行期门禁用例

归属：ZOR工具链与开发工程师（CI 接入）＋ZOR质量保障（判据）。本目录**不并入** `tests/specs` 的 golden／negative 计数（PRD §9）。

## `controls/`（run-smoke 负控源）

| 文件 | 前端行为 | 期望（`-o` 构建模式） | 证明 |
|---|---|---|---|
| `unsupported-syntax.zor` | `spawn` → E1101、rc=1 | rc=1、无产物 | 前端负控有牙 |
| `codegen-unsupported.zor` | 最小 `struct Mark {}` 前端通过 | rc=1、stderr 含 `zorcc: codegen:`、无产物、不新增 E 码 | 后端 fail-closed 有牙 |

- 两枚源都**不是** `tests/specs/negative` 资产，不参与 0.1.x 计数；也不作为 0.2.0 语言子集的正面示例。
- `run-smoke` 段在每次 CI 运行中执行两枚负控；任一枚不按期望变红即判 FAIL（门禁空转）。
- 后端负控**不使用** `let x = 1 + 2`：i64 算术属 PRD §5.2 可选加速项，实现后该负控会失效（D-020-3／QA v4）。
