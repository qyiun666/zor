# tests/packaging — L2d 安装树 smoke（QA）

归属：ZOR质量保障（判据与证据）；安装器／CI job 归工具链。入口由 `ci/run_ci.sh --packaging-smoke` 直调。

## 入口

```bash
bash tests/packaging/run_packaging_smoke.sh --strict [--allow-nonlinux]
bash tests/packaging/run_packaging_smoke.sh --validate-only [--allow-nonlinux]
```

- env：`ZORC_BIN`、`CC`、`OUT_DIR`、`ZOR_INSTALLER`（默认仓库 `ci/install.sh`）；`ZOR_EXPECT_VERSION` 默认读取 `compiler/Cargo.toml`，账本升 0.3.3 后自动校验 0.3.3；显式覆写仅用于专项校验。
- 安装器接口（PR #112）：`bash ci/install.sh --prefix <绝对路径> [--destdir <根>] [--zorcc <路径>] [--runtime <目录>]`。
- 判词：Linux x86_64 严格模式恰一行 `PASS|packaging-smoke|cases=4 fail=0 ran=4` 或 `FAIL|packaging-smoke|...`；任一条失败 → rc=1。
- 非 Linux 或 `ZOR_EXPECT_VERSION` 覆写只用于本地预检，判词为 `PASS|packaging-smoke|local-precheck ...`，**不得作为门禁证据**；CI 不传 `--allow-nonlinux`。
- `--validate-only`：只做静态校验（TSV／fixture／expected／安装器／semver），不需 zorcc。

## 判据

| ID | 形态 | 判据 |
|---|---|---|
| P-01 | canonical 安装树 `prefix/bin/zorcc`＋`prefix/share/zor/runtime/{zor_rt.c,h}` | `--version` 与 `compiler/Cargo.toml` 一致（账本后自动为 0.3.3；显式 `ZOR_EXPECT_VERSION` 可专项断言）；从源码树外 cwd、`ZOR_RUNTIME_DIR` 未设构建并运行 hello → stdout `hello\n`；`-O0` E3001 样本 → rc=101、stdout `before\n`、stderr 含 `zor: runtime error[E3001]`；零 `.zorcc-tmp-*` 残留 |
| P-02 | 删除安装树 helper 后构建需要运行期 helper 的 `-O0` 样本 | rc=2、零产物、stderr 含 `zor_rt` 且列出 `share/zor/runtime` 候选；不得以链接失败代替 rc=2 |
| P-03 | `DESTDIR` 暂存安装（`--prefix /opt/zor-l2d`） | 文件集精确 3 个、目录集精确；`bin/zorcc` 755、helper 644；树清单 `p03-tree.sha256` 可复算；安装树文本（helper）无构建机绝对路径。二进制元数据（debug 路径等）不在本切片判据 |
| P-04 | `ZOR_RUNTIME_DIR` 指向缺件目录，cwd 放有效 helper 诱饵 | rc=2、零产物、stderr 含 `ZOR_RUNTIME_DIR`；不得回退 cwd／源码树 |

## 关键防假绿

- `hello.zor` 不引用 `zor_rt.h`，**不触发 helper 解析**；P-02／P-04 必须使用 `-O0` 下需要运行期 helper 的 `e3001.zor`，否则会得到假 rc=0。
- 安装后二进制远离源码树运行；P-02 删除安装树 helper 后，若仍成功即证明命中了构建机 `CARGO_MANIFEST_DIR/../runtime` 等回退候选，判 FAIL。P-01 成功本身不构成“不依赖构建机路径”的证明，P-02 才是该面的负控。
- 版本：默认与 `compiler/Cargo.toml` 一致；冻结 PRD §5 的 `0.3.3（账本后）` 由账本时 Cargo.toml 自动兑现。显式 `ZOR_EXPECT_VERSION` 只做专项断言（例如账本前验证 0.3.3 会按预期红）；是否门禁判词只由宿主闸决定，不由版本覆写降级。
- 计数固定 4；缺件／空集／跑不满一律 FAIL，不设 SKIP。

## 证据

`OUT_DIR` 下：`summary.txt`、`cases/P-01..04/{install,version,hello-build,hello-run,e3001-build,e3001-run,build}.{rc,out,err}`、`p03-tree.sha256`、`p03-tree.txt`、`cases/P-03/{files,dirs}.{expected,actual}`。工作目录默认在系统临时目录，退出即清理；`KEEP_WORK=1` 可保留。证据只含文本，二进制留在临时工作目录。

## 当前状态（PR #114 head `3b6546e`）

- 在 PR #112 `ci/install.sh` ＋ PR #114 `zorcc 0.3.2` 上本地预检：**P-01～P-04 4/4 PASS**（默认版本来源 `compiler/Cargo.toml` = 0.3.2；macOS local-precheck，不作门禁）。
- P-02 已转绿：构建树回退仅在 exe 位于 `CARGO_MANIFEST_DIR` 下时生效；删除安装树 helper 后 rc=2、零产物、stderr 列出 `share/zor/runtime` 等候选、无链接失败。
- 显式 `ZOR_EXPECT_VERSION=0.3.3` 在账本前按预期红（P-01 version），证明版本断言有效；账本升 0.3.3 后默认自动转绿。
- 未 PASS／未 Linux x86_64 终证前不宣称可安装。

—— ZOR质量保障｜2026-10-22｜L2d QA-01 packaging-smoke（4）
