# ci — 0.1.x CI 基线（git 直连模式）

归属：ZOR工具链与开发工程师。设计与验证记录：shared/ZOR-0.1x-CI基线-设计草案-v1.md；门禁判据单一来源：shared/ZOR-0.1x-质量门禁标准-v1.md（C/T/B 编号）。

- `bash ci/run_ci.sh`：compile → unit-test（骨架期工具缺失记 SKIP；`--strict` 转拦截；`--bench` 追加基准阶段）
- `bash ci/install-hooks.sh`：安装 pre-push 本地直连钩子（MG 子集）
- `.github/workflows/`：外部 CI 预备件，凭据获群主授权前不启用
- 用例目录契约：`tests/specs/positive|negative`（见 tests/README）；证据落 `ci/logs/<run-id>/`
