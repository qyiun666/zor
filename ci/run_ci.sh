#!/usr/bin/env bash
# ZOR 0.1.x CI 基线骨架（git 直连模式）
# 归属：ZOR工具链与开发工程师｜版本：skeleton-v1｜2026-10-11
# 设计依据：shared/ZOR-0.1x-CI基线-设计草案-v1.md；QA 门禁 shared/ZOR-0.1x-质量门禁标准-v1.md
#
# 阶段：compile(C-1/C-2/C-3/C-4) → unit-test(T-1/T-2/T-4) → bench(B-1/B-4, 仅 --bench)
# 工具缺失策略：0.1.x 骨架期 zorcc 未就绪时阶段记 SKIP；--strict 下 SKIP 视为 FAIL（供 MG 强制后用）。
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[ -n "$REPO_ROOT" ] || { echo "[ci] 必须在 git 仓库内运行"; exit 2; }
cd "$REPO_ROOT"

STRICT=0; WITH_BENCH=0
for arg in "$@"; do
  case "$arg" in
    --strict) STRICT=1 ;;
    --bench) WITH_BENCH=1 ;;
    *) echo "[ci] 未知参数 $arg (可用: --strict --bench)"; exit 2 ;;
  esac
done

ZORC="${ZORC_BIN:-zorcc}"          # 原型编译器入口，核心工程师产出后接入（OQ-1 选型待定）
command -v "$ZORC" >/dev/null 2>&1 || ZORC=""

RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)-$$"
LOG_DIR="ci/logs/$RUN_ID"; mkdir -p "$LOG_DIR"
echo "$RUN_ID" > ci/logs/LATEST
overall=0; declare -a summary=()

record() { # record <stage> <status> <exit> <log>
  summary+=("$2|$1|exit=$3|log=$4")
  [ "$2" = "FAIL" ] && overall=1
}

new_log() { echo "$LOG_DIR/$1.log"; }

stage_compile() {
  local log; log="$(new_log compile)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：骨架期 SKIP（--strict 将判 FAIL）" > "$log"
    [ "$STRICT" = 1 ] && record compile FAIL 1 "$log" || record compile SKIP 0 "$log"
    return
  fi
  local pos=0 fail=0
  for f in tests/specs/positive/*.zor; do
    [ -e "$f" ] || continue
    if "$ZORC" "$f" >>"$log" 2>&1; then pos=$((pos+1)); else fail=$((fail+1)); fi
  done
  # C-2 告警基线对比、C-4 编译耗时回归在此接入（见设计草案 §3 插入点）
  if [ "$fail" -gt 0 ] || { [ "$pos" -eq 0 ] && [ "$STRICT" = 1 ]; }; then
    record compile FAIL 1 "$log"
  elif [ "$pos" -eq 0 ]; then
    echo "无正例可编译：骨架期 SKIP" >> "$log"; record compile SKIP 0 "$log"
  else
    record compile PASS 0 "$log"
  fi
}

stage_unit_test() {
  local log; log="$(new_log unit-test)"
  local pos_fail=0 neg_leak=0 n_pos=0 n_neg=0
  for f in tests/specs/positive/*.zor; do
    [ -e "$f" ] || continue; n_pos=$((n_pos+1))
    if [ -n "$ZORC" ]; then "$ZORC" "$f" >>"$log" 2>&1 || pos_fail=$((pos_fail+1)); fi
  done
  for f in tests/specs/negative/*.zor; do
    [ -e "$f" ] || continue; n_neg=$((n_neg+1))
    if [ -n "$ZORC" ]; then
      "$ZORC" "$f" >>"$log" 2>&1 && neg_leak=$((neg_leak+1))  # 负例必须编译失败
    fi
  done
  echo "正例数=$n_pos 正例失败=$pos_fail 负例数=$n_neg 负例漏检=$neg_leak" >> "$log"
  if [ "$pos_fail" -gt 0 ] || [ "$neg_leak" -gt 0 ]; then record unit-test FAIL 1 "$log"
  elif [ -z "$ZORC" ] || { [ "$n_pos" -eq 0 ] && [ "$n_neg" -eq 0 ]; }; then
    [ "$STRICT" = 1 ] && record unit-test FAIL 1 "$log" || record unit-test SKIP 0 "$log"
  else record unit-test PASS 0 "$log"; fi
}

stage_bench() {
  local log; log="$(new_log bench)"
  if [ -x ci/bench/run_bench.sh ]; then
    if ci/bench/run_bench.sh --quick >>"$log" 2>&1; then record bench PASS 0 "$log"
    else record bench FAIL 1 "$log"; fi
  else record bench SKIP 0 "$log"; fi
}

echo "[ci] run=$RUN_ID strict=$STRICT bench=$WITH_BENCH repo=$REPO_ROOT"
stage_compile
stage_unit_test
[ "$WITH_BENCH" = 1 ] && stage_bench

{
  echo "--- stage summary ($RUN_ID) ---"
  for s in "${summary[@]}"; do echo "$s"; done
} | tee "$LOG_DIR/summary.txt"
echo "[ci] 证据目录: $LOG_DIR"
exit "$overall"
