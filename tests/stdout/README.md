# tests/stdout — L2d stdout／CLI 契约门禁（QA）

归属：ZOR质量保障。入口由 `ci/run_ci.sh --stdout-contract` 直调；CI 不维护第二套矩阵。S5 11＋S7 3 expected 已在 PR #114 head `2a92902` 两轮捕获并 byte-SAME；本地 `--strict --allow-nonlinux` 得 62/10/14 PASS，**Linux x86_64 CI 与产品会签为门禁前提**。

## 计数（冻结 90＝86 stdout＋4 packaging）

| 段 | 条数 | 矩阵 |
|---|---:|---|
| stdout runtime | **62** | `stdout-cases.tsv`（S1 6／S2 4／S3 29／S4 5／S5 11／S6 4／S7 3） |
| stdout 编译负控 | **10** | `negative.tsv`（N-01～03、N-07～13；N-04～06 退役归档） |
| CLI | **14** | `cli-cases.tsv`（C1 6／C2 3／C3 3／C4 2，零改） |
| stdout 合计 | **86** | `new-cases.tsv` |
| packaging | **4** | `tests/packaging/packaging-cases.tsv`（独立 job） |

- **S5-04 N/A**：1 元组无源形态，不建 case、不计数；不得写“1 元组已支持”。
- N-04／05／06 退役转正例，归档 `retired/l2c/`；不得继续 fail-closed。
- `l2c-baseline.tsv` 锁 69 条未变用例（48＋7＋14）；`l2c-baseline.sha256` 为标准清单（105 行），`shasum -a 256 -c tests/stdout/l2c-baseline.sha256` 可复算。
- D-L2c-12：NaN 无端到端源形态；浮点契约由 `backend::float_fmt` 单测覆盖，S3-27 以 `1e-5` 替代。
- N-09／N-10 为 `&T`／`&mut T` 条件负控；`Handle<T>`／fn 无语法，记 N/A。

## 入口

```bash
bash tests/stdout/run_stdout_contract.sh --strict
```

- env：`ZORC_BIN`（zorcc 路径）、`CC`（C 编译器，CI 为 clang-18）、`OUT_DIR`（证据目录）。
- 输出：恰一行 `PASS|stdout-contract|…` 或 `FAIL|stdout-contract|…`，以及 `PASS|cli-contract|…` 或 `FAIL|cli-contract|…`；任一段 FAIL → rc=1。显式 FAIL 优先。
- 宿主：默认只允许 Linux x86_64；`--allow-nonlinux` 仅供本地预检，CI 不传。
- `--validate-only`：只做静态校验（矩阵／路径／expected／旧资产快照），不需 zorcc。

## 判据

- runtime：两轮构建＋运行；stdout 逐字节 `cmp`；`run` 模式 stderr 0 B；`stdout-terminate` 模式只判 stdout 与 rc=101（E30xx 文案归 runtime-codes）。
- negative：两轮 rc=1、零产物、stderr 含判词；不新增 E 码。
- CLI：NDJSON 六键固定序 `code/severity/message/file/line/col`、行序与文本模式一致、无诊断 stderr 0 B；`--version`／`-V` 短路；`-o` 零残留；usage 不泄露 `-Zoverflow` 与未开放 `-O*`。
- 旧资产：`legacy-snapshot.sha256`（226 文件）逐文件 sha256；旧五值 41／53／53／3／94、manifest 48、runtime-codes 29＋6＋8 计数复核。

## 资产说明

- `expected/<case>.stdout`：逐字节期望；S6 只含终止前 stdout；L2d 新 expected 14 条已按 PR #114 head 捕获（待产品会签）。
- `cases/S4-05.zor`：源文件内含原始 NUL 字节（`\0` 转义不是 NUL），按二进制资产处理。
- S6-03／S6-04 使用 `-O0` 触发 E3001；其余 runtime 用默认 `-O2`。
- `S5-09` 源写 `Red()`，输出 `Red`；`S5-10` `Circle(3)`；`S7` 为 `[u8;N]` 数组形态，与 S4 `[byte;N]` 原始字节严格区分。
