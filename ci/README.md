# ci — 0.1.x CI 基线（git 直连模式）

归属：ZOR工具链与开发工程师。设计与验证记录：shared/ZOR-0.1x-CI基线-设计草案-v1.md；门禁判据单一来源：shared/ZOR-0.1x-质量门禁标准-v1.md（C/T/B 编号）。

- `bash ci/run_ci.sh`：compile → unit-test（骨架期工具缺失记 SKIP；`--strict` 转拦截；`--bench` 追加基准阶段）
- `bash ci/run_ci.sh --strict --asan`：仅 Linux x86_64；在七段之后追加 `asan`（QA-01 `ci/asan/run_asan.sh`）；macOS 不传 `--asan`，保持七段
- `bash ci/run_ci.sh --asan-host-check`：仅校验 ubuntu-24.04／x86_64／clang 18.1.3 宿主固定；不读 manifest、不跑用例
- `bash ci/install-hooks.sh`：安装 pre-push 本地直连钩子（MG 子集）
- `.github/workflows/`：外部 CI 预备件，凭据获群主授权前不启用
- 用例目录契约：`tests/specs/positive|negative`（见 tests/README）；证据落 `ci/logs/<run-id>/`
- 本地复现前置（0.1.16 实测）：先 `cd compiler && cargo build --release`，再 `ZORC_BIN=<仓库绝对路径>/compiler/target/release/zorcc bash ci/run_ci.sh --strict`；不注入时 `ci/run_ci.sh:34` 回退 `zorcc`，PATH 无该二进制则 compile/unit-test/golden 判 FAIL（环境坑，非仓库缺陷），口径同 `.github/workflows/ci.yml`

## 段名清单（0.1.18 起；本件只枚举段名、不写段数计数句——计数句随版本腐烂）
- `--strict` 常驻段名（`summary.txt` 每行形如 `状态|段名|exit=N|log=路径`，首列为状态、第二列为段名）：`cargo`／`compile`／`unit-test`／`golden`／`code-table`／`rust-compare`／`run-smoke`
- `code-table`（0.1.18 项 2 新增）：诊断码表不变式校验段，单一实现处＝`ci/run_ci.sh` 的 `stage_code_table`（规范文本不另抄期望表）；口径源＝`specs/r1-type-system.md` §8／§8b 与 `specs/r2-ownership.md` §8 各自的取径注记；判定项＝A1–A7＋A8b1＋HDR（A8a 搬运闸与 A8b2 新行出处闸须对 base 侧旧文核对，属升格窗一次性执行，不入本段常驻面）；strict 下 FAIL 计入 `overall`
- `rust-compare`（0.1.26 项 7 新增；D1 处置 @0.2.0）：AC-R2-4 Rust 同题两段判，单一实现处＝`ci/run_ci.sh` 的 `stage_rust_compare`；`tests/specs/compare/*-annotated.rs` 须 `rustc --edition=2021 --crate-type lib --emit=metadata` exit=0，`*-unannotated.rs` 须非零退出且 stderr 含 `error[E0106]`（**D1：旧措辞 `missing lifetime specifier` 仅记日志、不计红**，0.1.x 历史件不回改）；反空转三项＝annotated 含 ≥1 具名生命周期／stripped 归一化后仅差生命周期标注／B 段 rc≠0 与 E0106 同时命中；strict 下两侧各≥3 且按 basename 成对，否则 FAIL；rustc 缺失时 strict 判 FAIL、默认 SKIP（同 compile 段环境口径）；`RUSTC_BIN` 可注入指定 rustc
- `run-smoke`（0.2.0／020-CI-01 新增）：首个可执行闭环——`zorcc examples/hello.zor -o <tmp>/hello` → rc=0、宿主原生可执行、运行 stdout 逐字节 `Hello, ZOR\n`（11 B）／stderr 0 B／rc=0；五类负控每次运行（期望串错一位／前端 `spawn` E1101／后端 fail-closed 复合内 `Box`（L2d 起改用 `tests/runtime/controls/codegen-unsupported-l2d.zor`；旧 `struct Mark {}` 在 L2d 已合法，按 legacy-snapshot 零改保留）／无 `-o` 检查模式零新增文件／`-o` 误用与失败原子性）；strict 下 `cc`／`file` 缺失判 FAIL；无 `-o` 不进入 codegen，保持 0.1.x 检查态（D-020-1 A）；日志 `ci/logs/<run>/run-smoke.log`
- `asan`（L2a／L2A-CI-01 接线；判据脚本＝QA-01 `ci/asan/run_asan.sh`）：仅 `--asan` 且 Linux x86_64；manifest 48 行（41 `compile-only`＋7 `build-run`）、两轮构建／运行、UAF／LSAN 泄漏／去 `-g` 三探针；manifest 缺位／行数不符／路径不存在＝FAIL；证据 `ci/logs/<run>/asan/`；**不并入 macOS 七段常驻面，八段口径只在 Linux gate 成立**
- `bench` 仅在 `--bench` 下追加，不属常驻段名
- 历史口径出清：0.1.17 及以前在册的「四段全 PASS」句现存于 `CHANGELOG.md` 的 `[0.1.17]` 节（历史节，零触碰红线覆盖、不改），凡引段数一律绑所检 head ＋节名锚，**不得把该句读作现态**

## skeleton-v2（V-3/M-1 收尾追加）
- debug/release 双模式编译（OQ-2：flags 注入位 `ZORC_DEBUG_FLAGS/ZORC_RELEASE_FLAGS`，负例双模式都必须失败）
- `stage_cargo`：compiler/ 落地 Cargo 工程后自动接通（OQ-1 方案 B）
- M-1：目录基准常量 `TESTS_POS_DIR/TESTS_NEG_DIR` 单点定义（当前=tests/specs/*，与骨架 3449ac0 一致，M-1 所述 specs/tests 不一致已在 PR#1 首版修正）；strict 下**负例数=0 即 FAIL**
- `ci/setup-remote.sh`：SSH-over-443 通道固化，幂等可重跑

## M1 前置（S-1/n_pos 对称 + 外部 CI 发布）
- S-1：`shopt -s nullglob` + 双模式各侧用例数入证据日志；strict 下任一模式正例数=0 即 FAIL
- n_pos 对称判据：strict 下正/负任一侧用例集为空即 FAIL（M-1 双侧对称）
- 回归夹具：tests/specs/{positive,m1-s1-passthrough.zor|negative,m1-s1-negative.zor}（AC 用例批量落码前保持套件非空，不计入 AC-R1-2 计数）
- 外部 CI（D-020）：workflow 发布——push/PR 默认模式；strict/bench 经 workflow_dispatch 演练；ci/logs 以 artifact 上传（30 天）

## L2b runtime-codes（0.3.1 起）
- `bash ci/run_ci.sh --runtime-codes`：独立模式（不可与 `--strict／--asan／--bench` 组合），只执行宿主检查＋`stage_runtime_codes`，输出单段 `summary.txt`；baseline 七段、asan-linux 八段零改
- job：`ubuntu-24.04`＋clang-18、timeout 20、无 sanitizer（rc=101 只在此判）；29 runtime＋6 width fail-closed＋8 CLI；artifact `runtime-codes-logs-<run_id>-<attempt>` 90 天；fail-closed F-RC-1～14，不得 SKIP／`continue-on-error`／重跑洗绿
- 资产与判据脚本由 QA 提供：`tests/runtime/runtime-codes.tsv`、`width-failclosed.tsv`、`cli-cases.tsv`、`expected/`、`run_runtime_codes.sh`、`cc-capture`；`--runtime-codes` 直调 QA 脚本，CI 不自维护第二套判据；缺件即 FAIL

## L2c stdout-contract（0.3.2 起）
- `bash ci/run_ci.sh --stdout-contract`：独立模式（不可与 `--strict／--asan／--bench／--runtime-codes` 组合），只执行 `stage_stdout_contract`，输出 `stdout-contract`＋`cli-contract` 两段 `summary.txt`；baseline 七段、asan-linux 八段、runtime-codes 单段零改
- job：`ubuntu-24.04` x86_64、timeout 20、无 sanitizer；确定性环境 `LC_ALL=C`／`TZ=UTC`／`TERM=dumb`／`NO_COLOR=1`，并 `env -u ZOR_RUNTIME_DIR`；artifact `stdout-contract-logs-<run_id>-<attempt>` 90 天
- 资产与判据脚本由 QA 提供：`tests/stdout/`（`manifest.tsv`、`negative.tsv`、`cli-cases.tsv`、`cases/`、`controls/`、`expected/`、`run_stdout_contract.sh`、`new-cases.tsv`）；`--stdout-contract` 直调 QA 脚本（env：`ZORC_BIN`／`CC`／`OUT_DIR`），CI 不自维护第二套矩阵／expected；缺件或空集即 FAIL，不得 SKIP／`continue-on-error`／重跑洗绿
- 判据边界：rc=101／E30xx／stderr 仍归 `runtime-codes`；本 job 只判 stdout 字节与顺序（S6 不复制 E30xx 判据）；stdout 失败按脚本显式 `PASS|FAIL|` 判词分段落账，缺判词按 FAIL
- required 时机：实施 PR 内 advisory；QA GATE PASS 与 0.3.2 账本合入后再设为 required

## L2d 复合输出扩展与 packaging-smoke（0.3.3 起）
- `--stdout-contract` 计数冻结为 **86**：runtime **62**（L2c 48＋S5 11＋S7 3）＋负控 **10**＋CLI **14**；CI 除分段落账外，还校验两段判词原文 `PASS|stdout-contract|runtime=62 negative=10 fail=0 ran=62/10` 与 `PASS|cli-contract|cases=14 fail=0 ran=14`；判定 PASS 但计数不符（如 QA 旧资产 48）一律 FAIL，防静默洗绿；QA 脚本仍是唯一判据来源，CI 不自维护矩阵
- `bash ci/run_ci.sh --packaging-smoke`：独立模式（不可与 `--strict／--asan／--bench／--runtime-codes／--stdout-contract` 组合），只执行 `stage_packaging_smoke`，输出单段 `summary.txt`；直调 QA `tests/packaging/run_packaging_smoke.sh --strict`（env：`ZORC_BIN`／`CC`／`OUT_DIR`／`ZOR_INSTALLER`）；缺 zorcc／脚本／判词、或 `PASS|packaging-smoke|cases=4 fail=0 ran=4` 计数不符即 FAIL，不得 SKIP／`continue-on-error`／重跑洗绿
- `bash ci/install.sh --prefix <绝对前缀> [--destdir <根>] [--zorcc <路径>] [--runtime <目录>]`：安装树构造（CI smoke 用，非用户安装承诺）；canonical 布局 `<DESTDIR><prefix>/bin/zorcc`＋`<DESTDIR><prefix>/share/zor/runtime/{zor_rt.c,h}`；缺 zorcc／helper 或前缀非绝对路径 rc=2
- job：`packaging-smoke`（`ubuntu-24.04`、timeout 20、无 sanitizer、`env -u ZOR_RUNTIME_DIR`），artifact `packaging-logs-<run_id>-<attempt>` 90 天、仅文本；实施期 advisory，required 仍仅 `stdout-contract`；baseline 七段、asan-linux 八段、runtime-codes、stdout-contract 结构零改
- `-O*`／G-D-2 不属本版（L2e）；不建 `opt-matrix` job

## L2e `-O*` 与 CLI 扩展（0.3.4 起）
- `--stdout-contract` 判词同步为 **runtime 65／negative 12／CLI 17**（stdout 94）；CI 校验 `PASS|stdout-contract|runtime=65 negative=12 fail=0 ran=65/12` 与 `PASS|cli-contract|cases=17 fail=0 ran=17`，不符即 FAIL
- `--runtime-codes` 判词同步为 **runtime_rows=39／width_rows=6／cli_rows=19／fail=0**；CI 在 QA 脚本 rc=0 后校验 `ci/logs/<run>/runtime-codes/runtime-codes.log` 的这四行，缺行或计数不符即 FAIL
- `-O*` 六档接受、缺省 `-O2`、last-wins；overflow：`-O0/-O1/-Og` checked、`-O2/-O3/-Os` wrap；`--help` 仅长旗标（`-h` rc=2）；`-Zoverflow` 仍为内部位
- **不建 `opt-matrix` job**：接受／拒绝／cc 映射／输出不变性由现有 `stdout-contract` CLI 段与 `runtime-codes` CLI 段覆盖
- `packaging-smoke` 4 条不变；required 切换只在 0.3.4 账本合并＋连续两次 main 全绿后 PATCH ruleset `25633324`，回退一条 PATCH；切换前保持 advisory
