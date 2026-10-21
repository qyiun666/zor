# ci/asan — L2a Linux ASAN／LSAN 门禁骨架

- 归属：ZOR质量保障（判据／脚本）｜接线：ZOR工具链与开发工程师（L2A-CI-01）。
- 依据：QA v1.3 §6（`shared/ZOR-0.2.x-L2-门禁与计数delta-QA-v1.md`）、L2a 冻结件 §一／§四、核心 v1.1 §4.2。
- 性质：**门禁骨架与判据**；尚未接入 `ci/run_ci.sh`／GitHub Actions。CI-01 负责 `ubuntu-24.04`＋clang 18.1.3＋独立 `asan-linux` job、artifact 90 天与仓库侧快照。

## 文件

| 文件 | 作用 |
|---|---|
| `run_asan.sh` | 门禁入口；manifest 校验→7 条 `build-run` 构建／运行→两轮复算→UAF／泄漏／去 `-g` 探针 |
| `cc-asan` | `CC` wrapper：追加 `-fsanitize=address -fno-omit-frame-pointer -g` |
| `cc-nog` | 去 `-g` 对照 wrapper：与 `cc-asan` 唯一差异是不追加 `-g` |

## 用法

```sh
# 只校验 manifest（macOS 可预检）
bash ci/asan/run_asan.sh --validate-only

# Linux 全量门禁（strict；失败不 SKIP）
bash ci/asan/run_asan.sh --strict --out /path/to/evidence

# 指定编译器（CI 固定 clang 18.1.3）
CC_REAL=/usr/bin/clang-18 bash ci/asan/run_asan.sh --strict --out /path/to/evidence
```

环境变量：`ZORC_BIN`（默认 PATH `zorcc`）、`CC_REAL`（默认 `cc`）、`ASAN_TIMEOUT`（默认 30 秒）。

## 判据（任一不满足＝FAIL）

1. manifest 恰 48 行＝41 `compile-only`＋7 `build-run`；表头七列精确；无重复；路径存在且为普通文件、无绝对路径／`..`／symlink；`box_overwrite_drop` 在列。
2. 只读 manifest 取 `build-run` 集（R=7）；禁目录 glob 取运行集。
3. 逐条 ASAN／LSAN 构建 rc=0、运行 rc=0、stdout sha 匹配、stderr 0 B、`.debug_info` 在位、无 `ERROR: AddressSanitizer`／`ERROR: LeakSanitizer`。
4. UAF 探针必红、LSAN 泄漏探针必红；去 `-g` 构建必须被同一 `debug_info_present` 检查判 FAIL。
5. 同一宿主两轮，逐条 build_rc／run_rc／stdout sha／stderr 字节／sanitizer 结论一致。
6. 宿主必须 Linux；macOS 不得作泄漏零报告证据（脚本直接拒绝，退出 2）。manifest／`readelf`／`timeout`／C 编译器缺失＝FAIL。

## 证据

`--out` 目录（默认 `ci/logs/asan-<UTC>-<pid>/`）：

```text
commands.txt  versions.txt  manifest.tsv  manifest.sha256
summary.tsv  asan-verdict.tsv  probes.log
round1/  round2/   # 每轮 summary.tsv、asan-verdict.tsv、readelf-<id>.txt、<id>.{out,err,rc,...}
probes/            # UAF／泄漏／去 -g 对照的构建与运行日志
```

CI-01 应将 `--out` 指向 job 工作目录，上传 artifact（90 天）并在仓库侧留 `qa-outputs/ZOR/<UTC>/evidence/0.2.x-asan/<head>/` 快照；artifact 不得替代快照。

## 边界

- 零报告只覆盖 manifest 的 7 条 `build-run`；41 正例只承担既有七段 strict 的编译期零回归，不在本脚本构建。
- 不新增 E 码、不改 `r2:9`、不改 `tests/specs` 计数；manifest／`cases/`／探针不并入 41／45／45／3／86。
- 本脚本不是 CI 接线本身；工作流与 `ci/run_ci.sh` 的改动归 L2A-CI-01。
