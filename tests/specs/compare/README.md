# tests/specs/compare — AC-R2-4 Rust 同题对照源

本目录只放 Rust 侧对照源；ZOR 侧正例在 `tests/specs/positive/r2-ref-*.zor`。两态命名契约：

- `*-annotated.rs`：须含 ≥1 个具名生命周期参数且可编（`rustc --edition=2021 --crate-type lib --emit=metadata` exit=0）。
- `*-unannotated.rs`：同题**最小去标注**变体（只删生命周期参数／标注，不改题面其他 token），须 rc≠0 且 stderr 同时含 `error[E0106]` 与 `missing lifetime specifier`。

映射表：

| ZOR 正例 | Rust annotated | Rust unannotated | Rust 侧预期 |
|---|---|---|---|
| `positive/r2-ref-01-two-input-select-owned.zor`（T-a 二输入择一返回，改返回拥有值） | `t-a-pick-annotated.rs` | `t-a-pick-unannotated.rs` | annotated rc=0；unannotated rc≠0 且 `E0106` |
| `positive/r2-ref-02-struct-hold-value.zor`（T-b struct 持引用字段，改持值） | `t-b-holder-ref-field-annotated.rs` | `t-b-holder-ref-field-unannotated.rs` | annotated rc=0；unannotated rc≠0 且 `E0106` |
| `positive/r2-ref-03-array-hold-value.zor`（T-c 容器/数组元素持引用，改持值） | `t-c-holder-ref-array-annotated.rs` | `t-c-holder-ref-array-unannotated.rs` | annotated rc=0；unannotated rc≠0 且 `E0106` |

纪律：

- **同任务、非同签名**：两侧源码不同题拼写；ZOR 侧按 R2 §4 返回拥有值、按 R1 §3／R2 §7 In2 容器持值，Rust 侧才使用 ZOR 不可表达的返回引用／引用字段形态。等价性只在「同输入得同输出口」层成立，不得表述为「逐字等价」或「同签名对照」。
- **同引纪律**：引用 AC-R2-4 结果必须同引 R2 §7 不可表达清单，不得单引。
- 载荷限 Copy 集（`i64`）；本集不覆盖非 Copy 绑定重赋值形态。
- 门禁：`ci/run_ci.sh` 的 `rust-compare` 段执行 A/B 两段判＋反空转三项（具名生命周期在位／差异仅去标注／rc≠0 与 E0106 同时命中）；strict 下 ≥3 对且按 basename 成对。
- 本目录对照源在本机 `rustc 1.99.0 (b940084d7 2026-09-28)` 下实测；CI 每次在 `ci/logs/<run>/rust-compare.log` 记录 `rustc -vV`、edition 与逐对 rc／E0106 命中数。
