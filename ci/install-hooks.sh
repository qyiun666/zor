#!/usr/bin/env bash
# 安装 git 直连钩子（pre-push 触发 CI 基线）。归属：工具链工程师｜skeleton-v1
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel)"; cd "$REPO_ROOT"
HOOKS_DIR="$(git rev-parse --git-path hooks)"
cp ci/hooks/pre-push "$HOOKS_DIR/pre-push"
chmod +x "$HOOKS_DIR/pre-push"
echo "[hooks] pre-push 已安装到 $HOOKS_DIR（本地直连模式；跳过方式：git push --no-verify，仅限已备案豁免）"
