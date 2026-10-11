#!/usr/bin/env bash
# ZOR 0.1.x CI 基线骨架（git 直连模式）
# 归属：ZOR工具链与开发工程师｜版本：skeleton-v2（V-3+M-1 收尾）｜2026-10-11
# 设计依据：shared/ZOR-0.1x-CI基线-设计草案-v1.md；QA 门禁 v2；排期验收单 V-3/M-1
#
# 阶段：cargo(可选,OQ-1方案B参考实现) → compile(debug/release 双模式,OQ-2) → unit-test → [bench]
set -uo pipefail
shopt -s nullglob  # S-1：未命中 glob 展开为空数组，杜绝字面 glob 进循环的旁路

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

# —— M-1 目录基准唯一定义处：核心工程师若定论为 specs/tests，仅改这两行（或注入同名环境变量）——
TESTS_POS_DIR="${TESTS_POS_DIR:-tests/specs/positive}"
TESTS_NEG_DIR="${TESTS_NEG_DIR:-tests/specs/negative}"

# —— OQ-2 双模式（debug=checked / release=wrap，flags 为 zorcc 落地的注入位）——
DEBUG_FLAGS="${ZORC_DEBUG_FLAGS:--Zoverflow=check}"
RELEASE_FLAGS="${ZORC_RELEASE_FLAGS:--O2 -Zoverflow=wrap}"
MODES="debug release"

ZORC="${ZORC_BIN:-zorcc}"
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

compile_one() { # compile_one <file> <flags> <log> → 0/1
  local f="$1" fl="$2" lg="$3"
  # shellcheck disable=SC2086
  "$ZORC" $fl "$f" >>"$lg" 2>&1
}

stage_cargo() { # OQ-1 方案B：compiler/ 落地 Cargo 工程后接通；未落地前不入门禁判定
  local log; log="$(new_log cargo)"
  if [ -f compiler/Cargo.toml ] && command -v cargo >/dev/null 2>&1; then
    # 子 shell cd compiler 后相对路径失效，重定向须用绝对路径（PR#6 修复）
    local abs="$REPO_ROOT/$log"
    if (cd compiler && cargo build --all-targets >>"$abs" 2>&1 && cargo test >>"$abs" 2>&1); then
      record cargo PASS 0 "$log"
    else record cargo FAIL 1 "$log"; fi
  else
    echo "compiler/ 尚无 Cargo 工程或 cargo 缺失：OQ-1 落地前 SKIP（strict 亦不判）" > "$log"
    record cargo SKIP 0 "$log"
  fi
}

stage_compile() {
  local log; log="$(new_log compile)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：骨架期 SKIP（--strict 将判 FAIL）" > "$log"
    [ "$STRICT" = 1 ] && record compile FAIL 1 "$log" || record compile SKIP 0 "$log"
    return
  fi
  local pos_ok=0 pos_fail=0 empty_modes="" mode
  for mode in $MODES; do
    local fl; [ "$mode" = debug ] && fl="$DEBUG_FLAGS" || fl="$RELEASE_FLAGS"
    local mlog="$LOG_DIR/compile-$mode.log"
    local -a pos_files=("$TESTS_POS_DIR"/*.zor)
    echo "mode=$mode 正例文件数=${#pos_files[@]}" >>"$mlog"
    [ "${#pos_files[@]}" -eq 0 ] && empty_modes="$empty_modes $mode"
    for f in "${pos_files[@]}"; do
      if compile_one "$f" "$fl" "$mlog"; then pos_ok=$((pos_ok+1)); else pos_fail=$((pos_fail+1)); fi
    done
  done
  echo "双模式正例编译 ok=$pos_ok fail=$pos_fail 空模式循环:${empty_modes:-无}" >>"$log"
  # C-2 告警基线对比、C-4 编译耗时趋势在此接入（设计草案 §3）
  if [ "$pos_fail" -gt 0 ]; then record compile FAIL 1 "$log"
  elif [ "$STRICT" = 1 ] && [ -n "$empty_modes" ]; then
    # S-1：strict 下任一模式零用例即 FAIL（双模式空循环旁路封堵）
    echo "S-1：模式${empty_modes} 正例数=0 → FAIL" >>"$log"; record compile FAIL 1 "$log"
  elif [ "$pos_ok" -eq 0 ]; then echo "无正例可编译：骨架期 SKIP" >>"$log"; record compile SKIP 0 "$log"
  else record compile PASS 0 "$log"; fi
}

stage_unit_test() {
  local log; log="$(new_log unit-test)"
  local n_pos=0 n_neg=0 pos_fail=0 neg_leak=0 mode
  for f in "$TESTS_POS_DIR"/*.zor; do
    [ -e "$f" ] || continue; n_pos=$((n_pos+1))
    if [ -n "$ZORC" ]; then compile_one "$f" "$DEBUG_FLAGS" "$log" || pos_fail=$((pos_fail+1)); fi
  done
  for f in "$TESTS_NEG_DIR"/*.zor; do
    [ -e "$f" ] || continue; n_neg=$((n_neg+1))
    if [ -n "$ZORC" ]; then
      # 负例必须在 debug 与 release 双模式下均编译期失败（OQ-2/T-5：溢出类负例模式敏感）
      for mode in $MODES; do
        local fl; [ "$mode" = debug ] && fl="$DEBUG_FLAGS" || fl="$RELEASE_FLAGS"
        compile_one "$f" "$fl" "$log" && neg_leak=$((neg_leak+1))
      done
    fi
  done
  echo "正例数=$n_pos 正例失败=$pos_fail 负例数=$n_neg 负例漏检(双模式合计)=$neg_leak" >>"$log"
  # M-1/n_pos 对称判据：strict 下任一侧用例集为空即 FAIL（防空套件假绿，两侧对称）
  if [ "$STRICT" = 1 ] && { [ "$n_pos" -eq 0 ] || [ "$n_neg" -eq 0 ]; }; then
    echo "对称判据：strict 下 正例数=$n_pos 负例数=$n_neg，任一侧=0 → FAIL（M-1/S-1）" >>"$log"
    record unit-test FAIL 1 "$log"; return
  fi
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

echo "[ci] run=$RUN_ID strict=$STRICT bench=$WITH_BENCH modes='$MODES' pos='$TESTS_POS_DIR' neg='$TESTS_NEG_DIR'"
stage_cargo
stage_compile
stage_unit_test
[ "$WITH_BENCH" = 1 ] && stage_bench

{
  echo "--- stage summary ($RUN_ID) ---"
  for s in "${summary[@]}"; do echo "$s"; done
} | tee "$LOG_DIR/summary.txt"
echo "[ci] 证据目录: $LOG_DIR"
exit "$overall"
