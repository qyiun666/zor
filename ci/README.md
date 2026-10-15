# ci — 0.1.x CI 基线（git 直连模式）

归属：ZOR工具链与开发工程师。设计与验证记录：shared/ZOR-0.1x-CI基线-设计草案-v1.md；门禁判据单一来源：shared/ZOR-0.1x-质量门禁标准-v1.md（C/T/B 编号）。

- `bash ci/run_ci.sh`：compile → unit-test（骨架期工具缺失记 SKIP；`--strict` 转拦截；`--bench` 追加基准阶段）
- `bash ci/install-hooks.sh`：安装 pre-push 本地直连钩子（MG 子集）
- `.github/workflows/`：外部 CI 预备件，凭据获群主授权前不启用
- 用例目录契约：`tests/specs/positive|negative`（见 tests/README）；证据落 `ci/logs/<run-id>/`
- 本地复现前置（0.1.16 实测）：先 `cd compiler && cargo build --release`，再 `ZORC_BIN=<仓库绝对路径>/compiler/target/release/zorcc bash ci/run_ci.sh --strict`；不注入时 `ci/run_ci.sh:34` 回退 `zorcc`，PATH 无该二进制则 compile/unit-test/golden 判 FAIL（环境坑，非仓库缺陷），口径同 `.github/workflows/ci.yml`

## 段名清单（0.1.18 起；本件只枚举段名、不写段数计数句——计数句随版本腐烂）
- `--strict` 常驻段名（`summary.txt` 行首列即段名，与 `record()` 同源）：`cargo`／`compile`／`unit-test`／`golden`／`code-table`
- `code-table`（0.1.18 项 2 新增）：诊断码表不变式校验段，单一实现处＝`ci/run_ci.sh` 的 `stage_code_table`（规范文本不另抄期望表）；口径源＝`specs/r1-type-system.md` §8／§8b 与 `specs/r2-ownership.md` §8 各自的取径注记；判定项＝A1–A7＋A8b1＋HDR（A8a 搬运闸与 A8b2 新行出处闸须对 base 侧旧文核对，属升格窗一次性执行，不入本段常驻面）；strict 下 FAIL 计入 `overall`
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
