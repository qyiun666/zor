# tests/runtime — 0.2.0／L2a 运行期门禁用例

归属：ZOR质量保障（判据／manifest／样本）＋ZOR工具链与开发工程师（CI 接入）。本目录**不并入** `tests/specs` 的 golden／negative 计数（PRD §9；L2a 冻结件 §三）。

## `manifest.tsv`（L2a-QA-01；ASAN／LSAN 取数唯一来源）

- 冻结口径：**48 行＝41 `compile-only`＋7 `build-run`**；`cases=48`、`run_cases=7`。
- schema（七列，制表符分隔，LF）：
  `case_id  source_path  expect_rc  mode  expect_stdout_sha256  kind  notes`
- `compile-only`＝既有 41 枚 `tests/specs/positive/*.zor`（含 M1 夹具），只做编译期零回归，不进入 ASAN 运行集；每行 notes 必写 `codegen-subset` 理由，M1 另写“不计入 AC-R1-2”。
- `build-run`＝7 条（hello＋6 条 L2a 语义样本）；`expect_stdout_sha256` 必填 64 位十六进制；除 hello 外 stdout 为空串 sha。
- **禁目录 glob 取运行集**：ASAN 脚本只读 manifest；`tests/specs/positive/*.zor` 的枚举仅用于 manifest 完整性校验，不用于选运行用例。
- 空集／行数不符／路径不存在／symlink／`..`／重复 case_id＝FAIL（`ci/asan/run_asan.sh --validate-only` 可复算）。
- manifest 是 **L2a 新增 QA 资产**，不改动 41／45／45／3／86 五值；其 sha 与行数进 ASAN 证据。

## `cases/`（7 条 `build-run` 语义样本）

| 文件 | 语义 | 期望（Linux ASAN／LSAN） |
|---|---|---|
| `box_create_read_unbox.zor` | Box 构造→读→unbox | rc=0；stdout 空；无 UAF／泄漏 |
| `box_scope_drop.zor` | 作用域末逆序 drop（两枚 Box） | rc=0；无泄漏／double-free |
| `box_overwrite_drop.zor` | 覆盖旧值即 drop（W2 G-E-1 载体） | rc=0；旧值释放；LSAN 零泄漏 |
| `box_move_between_bindings.zor` | 绑定间移动；仅新 owner 释放 | rc=0；无 double-free |
| `box_self_assign_no_drop.zor` | 自赋值不 drop（W2 例外面） | rc=0；无 double-free／泄漏 |
| `box_unbox_then_reinit.zor` | unbox 后槽位无活值再重初始化 | rc=0；无 double-free |
| （`examples/hello.zor`，manifest 行 `hello`） | hello 基线 | rc=0；stdout `Hello, ZOR\n`；stderr 0 B |

- 样本**不打印非字符串值**：除 hello 外 stdout 为空，避免把 G-D-1 stdout 契约提前拉进 L2a；Box 读值正确性由单测／生成 C 断言覆盖。
- 样本须在前端 check 模式 rc=0；`-o` 构建／运行由 CORE-03／04 落地后转绿（此前按预期红）。

## `controls/`（负控源，不计数）

| 文件 | 前端行为 | 期望 | 证明 |
|---|---|---|---|
| `unsupported-syntax.zor` | `spawn` → E1101、rc=1 | rc=1、无产物 | 前端负控有牙 |
| `codegen-unsupported.zor` | 最小 `struct Mark {}` 前端通过 | rc=1、stderr 含 `zorcc: codegen:`、无产物、不新增 E 码 | 后端 fail-closed 有牙 |
| `asan_uaf_probe.c` | C 级故意 UAF | Linux ASAN 必红（`ERROR: AddressSanitizer`） | ASAN 探测器活 |
| `asan_leak_probe.c` | C 级故意泄漏 | Linux LSAN 必红（`ERROR: LeakSanitizer`） | LSAN 探测器活 |

- 两枚 `.zor` 源都**不是** `tests/specs/negative` 资产，不参与 0.1.x 计数；也不作为语言子集的正面示例。
- 两枚 `.c` 探针为 QA 判据资产，不属 ZOR 源码面，不参与 `tests/specs` 计数；固定路径、不入 manifest。
- 后端负控**不使用** `let x = 1 + 2`：i64 算术属 PRD §5.2 可选加速项，实现后该负控会失效（D-020-3／QA v5）。

## ASAN 门禁入口（L2a-QA-01；CI-01 接线）

```sh
# Linux 宿主；strict 判据；证据目录可指定
bash ci/asan/run_asan.sh --strict --out /path/to/evidence

# 只校验 manifest（可在 macOS 预检；不执行 ASAN）
bash ci/asan/run_asan.sh --validate-only
```

- 宿主：Linux x86_64；macOS **不得**作泄漏零报告证据（脚本直接拒绝非 Linux 运行）。
- 构建：`cc -std=c11 -O0 -g -fsanitize=address -fno-omit-frame-pointer`；`ASAN_OPTIONS=halt_on_error=1:detect_leaks=1`；通过 `CC=ci/asan/cc-asan` 注入，不改普通构建。
- 判据：7 条 `build-run` 构建／运行 rc=0、stdout sha 匹配、stderr 0 B、`.debug_info` 在位、无 sanitizer 报错；UAF／泄漏／去 `-g` 三负控按期望；两轮复算一致。
- fail-closed：manifest／`readelf`／`timeout`／ASAN runtime／宿主缺失＝FAIL，不 SKIP；BLOCKED≠PASS。
- 声明边界：只能写“41 正例**编译期零回归**＋7 条 ASAN 样本零报告”；不得写“41 枚运行零报告”或“语言级内存安全”。

## 负控编号映射（`run-smoke` 段内 ↔ W5 v6）

| `run-smoke` 段内 | W5 v6 | 内容 |
|---|---|---|
| N-1 | N-1 | 期望串错一位 |
| N-2 | N-2 | `spawn` 前端 E1101 |
| N-3 | N-2b | 最小 `struct` 后端 fail-closed |
| N-4 | N-3a | 无 `-o` 检查模式零新增（源目录） |
| N-5 | N-3b | `-o` 误用 rc=2／零新增 |
| N-5h | N-3c | 失败原子性（sentinel／无 `.zorcc-tmp-*`） |
| S0-3 | N-4 | 换临时目录复算 |

ASAN 段的 N-5／N-LS／N-6 编号见 QA v1.3 §6.4，由 `ci/asan/run_asan.sh` 执行。


