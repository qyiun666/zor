#!/usr/bin/env bash
# SSH-over-443 通道固化（群内已核实：github.com HTTPS 间歇超时，ssh.github.com:443 稳定）
# 幂等；IdentityFile 可用 ZOR_SSH_KEY 覆盖。归属：工具链工程师｜skeleton-v1
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel)"; cd "$REPO_ROOT"
KEY="${ZOR_SSH_KEY:-$HOME/.ssh/id_ed25519_meowagent}"
[ -f "$KEY" ] || { echo "[remote] 缺 SSH 私钥: $KEY（向核心工程师/群主确认凭据来源）"; exit 1; }
git config remote.origin.url ssh://git@ssh.github.com:443/qyiun666/zor.git
git config core.sshCommand "ssh -i $KEY -o IdentitiesOnly=yes"
git config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'
git ls-remote --symref origin HEAD >/dev/null && echo "[remote] SSH-over-443 通道连通，origin 已固化"
