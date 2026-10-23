# tests/stdout — L2f stdout／CLI 契约门禁（QA）

归属：ZOR质量保障。入口由 `ci/run_ci.sh --stdout-contract` 直调；CI 不维护第二套矩阵。L2f Stage-1 已纳入 S9-01～06 与 N-16～35；S9 expected 在核心实现 head 两轮捕获后由 Stage-2 回填，**Stage-1 阶段 `--validate-only` 会因 S9 expected 缺件而失败，这是预期**。

## 计数（L2f 冻结 124＝120 stdout＋4 packaging）

| 段 | 条数 | 矩阵 |
|---|---:|---|
| stdout runtime | **71** | `stdout-cases.tsv`（65；S1 6／S2 4／S3 29／S4 5／S5 11／S6 4／S7 3／S8 3）＋`l2f-cases.tsv`（6；S9-01～06） |
| stdout 编译负控 | **32** | `negative.tsv`（12；N-01～03、N-07～15）＋`l2f-negative.tsv`（20；N-16～35） |
| CLI | **17** | `cli-cases.tsv`（C1 6／C2 3／C3 3／C4 2（C4-02 显式变更）／C5 3 help） |
| stdout 合计 | **120** | `new-cases.tsv`（120；94＋26） |
| packaging | **4** | `tests/packaging/packaging-cases.tsv`（独立 job） |

- **S5-04 N/A**：1 元组无源形态，不建 case、不计数；不得写“1 元组已支持”。
- N-04／05／06 退役转正例，归档 `retired/l2c/`；不得继续 fail-closed。
- L2f 新 TSV：`l2f-cases.tsv`（6）＋`l2f-negative.tsv`（20）；旧 `stdout-cases.tsv`／`negative.tsv`／`cli-cases.tsv` **逐字节零改**。
- 负控固定：**E1201 11**（N-16／17／18、N-21～26、N-29、N-35）／**codegen 8**（N-19／20／27／28／30／31／32／33）／**E2301 1**（N-34）；runner 禁止 `contains_any`、宽松 `zorcc:`、cc rc=2。
- `l2c-baseline.tsv` 锁 68 条未变用例；`l2c-baseline.sha256` 105 行。
- `l2e-baseline` 162 条／209 行零改；`l2f-baseline` 在 Stage-2 生成（188 条／246 行／246 unique，含 `new-cases.tsv`＋`tests/packaging/expected/{hello,e3001}.stdout`），Stage-1 不提供。
- D-L2c-12：NaN 无端到端源形态；浮点契约由 `backend::float_fmt` 单测覆盖，S3-27 以 `1e-5` 替代。
- N-09／N-10 为 `&T`／`&mut T` 条件负控；`Handle<T>`／fn 无语法，记 N/A。

## 入口

```bash
bash tests/stdout/run_stdout_contract.sh --strict
```

- env：`ZORC_BIN`（zorcc 路径）、`CC`（C 编译器，CI 为 clang-18）、`OUT_DIR`（证据目录）。
- 输出：恰一行 `PASS|stdout-contract|…` 或 `FAIL|stdout-contract|…`，以及 `PASS|cli-contract|…` 或 `FAIL|cli-contract|…`；任一段 FAIL → rc=1。显式 FAIL 优先。
- 宿主：默认只允许 Linux x86_64；`--allow-nonlinux` 仅供本地预检，CI 不传。
- `--validate-only`：只做静态校验（矩阵／路径／expected／旧资产快照），不需 zorcc。Stage-1 因 S9 expected 缺件而红；Stage-2 后转绿。

## 判据

- runtime：两轮构建＋运行；stdout 逐字节 `cmp`；`run` 模式 stderr 0 B；`stdout-terminate` 模式只判 stdout 与 rc=101（E30xx 文案归 runtime-codes）。
- negative：两轮 rc=1、零产物、stderr 含固定唯一 token；禁止 cc rc=2／`error:`／`undeclared identifier`；不新增 E 码。
- CLI：NDJSON 六键固定序 `code/severity/message/file/line/col`、行序与文本模式一致、无诊断 stderr 0 B；`--version`／`-V` 短路；`-o` 零残留；usage 不泄露 `-Zoverflow`；`--help` 输出固定文本（C5）。
- 旧资产：`legacy-snapshot.sha256`（226 文件）逐文件 sha256；旧五值 41／53／53／3／94、manifest 48、runtime-codes 39＋6＋19 计数复核。

## 资产说明

- `expected/<case>.stdout`：逐字节期望；S6 只含终止前 stdout；S9-01～06 expected 在 Stage-2 捕获，**Stage-1 不预写**。
- `cases/S4-05.zor`：源文件内含原始 NUL 字节（`\0` 转义不是 NUL），按二进制资产处理。
- S6-03／S6-04 使用 `-O0` 触发 E3001；其余 runtime 用默认 `-O2`。
- `S5-09` 源写 `Red()`，输出 `Red`；`S5-10` `Circle(3)`；`S7` 为 `[u8;N]` 数组形态，与 S4 `[byte;N]` 原始字节严格区分。
- S9 三型：`Box<i64>`／`Box<u8>`／`Box<u32>` 注解构造；S9-01 为 `box 0`／`i64::MAX`，无 `i64::MIN`；六档为执行维度，不增 TSV 行。
