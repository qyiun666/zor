# Changelog

本项目的所有重要变更记录在此文件。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循项目 0.1.x 顺序递进规则（见 README「版本规则」）。

## [0.1.1] - 2026-10-12

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。

### Added
- `compiler/`：`zorcc` R1 最小前端（lexer/parser/type check），R1 诊断码 E1101–E1601 全量实装（PR#5）。
- `zorcc` R2 所有权/借用/移动检查：E2101（移动后使用）、E2102（部分移动后整体使用）、E2201（独占借用期间再借用）、E2202（共享/独占共存冲突）、E2301（unbox 后再用/double free 类）、E1502（返回引用，定码；E2401 保留码位）（PR#10）。
- `tests/specs/positive/`：AC-R1-2 正例 20 项 `r1-01..r1-20`（slug 命名）（PR#9）。
- `tests/specs/negative/`：AC-R1-2 负例 11 项 `r1-n01..n11`（PR#7，slug 回填 PR#11）；AC-R2-2 所有权/借用负例 16 项 `r2-n01..n16`（PR#10）。
- `tests/specs/golden/`：T-4 黄金基线 28 份（负例 stderr 全文、双模式零差异）；`ci/run_ci.sh` 新增 `stage_golden`（strict 下 golden 缺失或零比对=FAIL）；`ci/capture_golden.sh` 捕获脚本（PR#12）。
- CI：GitHub Actions 外部通道发布并默认 `strict` 模式（CIQ-2，PR#4/#8）；`ZORC_BIN` 注入（PR#6）。

### Changed
- 规范 `specs/r2-ownership.md` §5 堆分配语法定稿（OI-3 关闭；拼写载文于 PR#2/D-020 团队自决，落码实装于 PR#10）：`box expr` / `*expr` / `b.field` 自动借用 / `unbox b`。
- CI push/PR 默认运行模式切换为 `strict`：空套件、负例/正例零计数、golden 缺失均为 FAIL（PR#8）。
- r1 负例文件命名对齐测试契约 `r1-n<序号>-<slug>.zor`，全库命名契约一致（PR#11）。

### Fixed
- parser：条件表达式上下文（`if`/`while` 块头）误按 record 结构体花括号糖解析导致吞块（PR#10）。
- CI runner：`stage_cargo` 日志相对路径在 `cd compiler` 子 shell 下失效导致主干门禁误红；`$REPO_ROOT` 绝对路径化修复（PR#6）。
- `run_ci.sh` spec 目录 glob 与 runner 目录契约不一致（M-1）、`stage_compile` 双模式空循环旁路（S-1）、n_pos 对称守卫（PR#4）。

### Known issues（跟踪中，非本版拦截）
- R3/R4 真交叉负例各 1 条待对应规范成文后补入（判据模板 §2 登记）。
- T-5 行为用例（运行期溢出 checked/wrap、除零）与 ASAN 对等校验：0.1.1 按门禁 v2 §6 修订行（D-033 会签）作"判据预置+不可执行留痕（文档级核对）"，可执行义务自运行链路落地后首版自动生效；0.1.2/0.1.3 仅为排期注记，不作期限。
- 循环引用导致的内存 leak 不属编译期门禁面（R2 §7.4 已知限制）。
- 文档项：spec R1 §8 E1501 释义扩写、r1-09 构造形态条款、`println` 内置归口、Copy 判定批次增量口径。

## [0.1.0] - 2026-10-11

内部验证里程碑，非可用语言版本（annotated tag 锚点 `16347de`）。

### Added
- 仓库骨架：README/LICENSE（MIT）/`specs`/`tests`/`bench`/`examples`/`compiler` 目录契约（PR#2 成文；glob 与 runner 不一致的 M-1 缺陷修复见 0.1.1 Fixed）。
- 规范草案：R1 类型系统、R2 所有权 draft-0.1 成文（PR#2）；吸收 QA V-4 裁定 F-1/F-1b/F-1c/F-2 勘误——R2 循环引用移出编译期负例清单、负例补足、MG/RG/HG 门禁层级命名更正（PR#3，tag 锚点即此 head）。
- CI 基线骨架：git 直连编译 + 单元测试流水线（PR#1）。

## 更早
- 0.1.x 需求包（PRD）与质量门禁标准（MG/RG/HG）在内部协作空间维护，不随仓库发布。
