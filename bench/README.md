# bench — 0.1.x 基准环境

三项基准（基准环境搭建归工具链工程师，判据归 QA 门禁草案）：

| 基准项 | 内容 | 对标 | 关联 AC |
|---|---|---|---|
| memory-residency | 空闲/峰值内存驻留 | Go（目标 ≤70%）、Rust | AC-R3-3、总目标 ≥30% |
| throughput | 并发收发/计算吞吐 | Rust ±10% | PRD §10-2 |
| compile-time | 单命令编译耗时 | Go | PRD §10-2 |

约定：
- 基准程序用 `*.zor`（hello 级占位先行，PRD §12 风险解耦项）；对比组用等价 Go/Rust 程序放 `bench/compare/`。
- 数据只进群与仓库，不外发（PRD §9）。
- 单版本劣化 >5% 触发 QA 拦截或立项修复（PRD §10-2）。
- task 栈 8KB 假设（OQ-3）在 memory-residency 基准中验证。
