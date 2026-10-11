# ci — 0.1.x CI 基线（git 直连模式）

归属：ZOR工具链与开发工程师。设计与验证记录：shared/ZOR-0.1x-CI基线-设计草案-v1.md；门禁判据单一来源：shared/ZOR-0.1x-质量门禁标准-v1.md（C/T/B 编号）。

- `bash ci/run_ci.sh`：compile → unit-test（骨架期工具缺失记 SKIP；`--strict` 转拦截；`--bench` 追加基准阶段）
- `bash ci/install-hooks.sh`：安装 pre-push 本地直连钩子（MG 子集）
- `.github/workflows/`：外部 CI 预备件，凭据获群主授权前不启用
- 用例目录契约：`tests/specs/positive|negative`（见 tests/README）；证据落 `ci/logs/<run-id>/`

## skeleton-v2（V-3/M-1 收尾追加）
- debug/release 双模式编译（OQ-2：flags 注入位 `ZORC_DEBUG_FLAGS/ZORC_RELEASE_FLAGS`，负例双模式都必须失败）
- `stage_cargo`：compiler/ 落地 Cargo 工程后自动接通（OQ-1 方案 B）
- M-1：目录基准常量 `TESTS_POS_DIR/TESTS_NEG_DIR` 单点定义（当前=tests/specs/*，与骨架 3449ac0 一致，M-1 所述 specs/tests 不一致已在 PR#1 首版修正）；strict 下**负例数=0 即 FAIL**
- `ci/setup-remote.sh`：SSH-over-443 通道固化，幂等可重跑
