#!/usr/bin/env bash
# T-4 golden 捕获 recipe（strict 扩项复验记录 §2.2 成文脚本化）
# 用法：bash ci/capture_golden.sh <zorcc路径>   （仓库根 cwd 执行，产出 tests/specs/golden/*.txt + sha256 前12位清单）
# 0.1.19 口径分列注记：本脚本＝捕获口径＝stdout+stderr 合并流写入基线文件；比对面＝ci/run_ci.sh stage_golden 的 stderr 单流。今日 38/38 全绿依赖「负例 stdout 实测 0 字节」之巧合；stdout 真分列＝codegen 窗动作（重捕全基线、改基线单独成拍经裁口位批准），本窗本脚本逻辑零改动。
set -euo pipefail
REPO_ROOT="$(git rev-parse --show-toplevel)"; cd "$REPO_ROOT"
Z="${1:?用法: capture_golden.sh <zorcc-binary-path>}"
[ -x "$Z" ] || { echo "[capture] 不可执行: $Z（先 cargo build --release）"; exit 2; }
GDIR="tests/specs/golden"; mkdir -p "$GDIR"
shopt -s nullglob
for f in tests/specs/negative/*.zor; do
  b="$(basename "$f" .zor)"
  # 负例预期 exit=1（set -e 下需显式放行）；0 或其他码=漏检/异常，中止防污染基线
  rc=0; "$Z" "$f" > "$GDIR/$b.txt" 2>&1 || rc=$?
  if [ "$rc" -ne 1 ]; then echo "[capture] ABORT: $b exit=$rc (负例预期1)"; exit 3; fi
done
echo "--- golden digest (sha256 前12位, 捕获序) ---"
for g in "$GDIR"/*.txt; do
  printf '%s %s\n' "$(shasum -a 256 "$g" | cut -c1-12)" "$(basename "$g" .txt)"
done
