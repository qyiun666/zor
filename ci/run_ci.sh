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
# T-4 黄金比对（strict 扩项复验记录 §2：golden 名=负例 basename 换 .txt；模式无关单份）
TESTS_GOLDEN_DIR="${TESTS_GOLDEN_DIR:-tests/specs/golden}"

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

stage_golden() { # T-4 黄金比对（strict 扩项复验记录 §2.4：负例 exit+stderr==golden；正例 stderr 空即基线）
  local log; log="$(new_log golden)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：默认 SKIP（strict 判 FAIL）" > "$log"
    [ "$STRICT" = 1 ] && record golden FAIL 1 "$log" || record golden SKIP 0 "$log"; return
  fi
  local -a neg_files=("$TESTS_NEG_DIR"/*.zor) pos_files=("$TESTS_POS_DIR"/*.zor)
  local n_chk=0 n_pass=0 n_diff=0 n_missing=0 p_stderr_nonempty=0 rc
  local tmpf; tmpf="$(mktemp)"; trap 'rm -f "$tmpf"' RETURN
  for f in "${neg_files[@]}"; do
    local b g; b="$(basename "$f" .zor)"; g="$TESTS_GOLDEN_DIR/$b.txt"
    # shellcheck disable=SC2086
    "$ZORC" $DEBUG_FLAGS "$f" >/dev/null 2>"$tmpf"; rc=$?
    if [ ! -f "$g" ]; then
      n_missing=$((n_missing+1)); echo "MISSING golden=$b.txt (exit=$rc)" >>"$log"; continue
    fi
    n_chk=$((n_chk+1))
    if [ "$rc" -ne 1 ]; then n_diff=$((n_diff+1)); echo "EXIT_MISMATCH $b rc=$rc (期望1)" >>"$log"
    elif cmp -s "$tmpf" "$g"; then n_pass=$((n_pass+1))
    else n_diff=$((n_diff+1)); echo "STDERR_DIFF $b" >>"$log"; diff "$g" "$tmpf" | head -6 >>"$log"; fi
  done
  for f in "${pos_files[@]}"; do
    # shellcheck disable=SC2086
    "$ZORC" $DEBUG_FLAGS "$f" >/dev/null 2>"$tmpf" || true
    [ -s "$tmpf" ] && p_stderr_nonempty=$((p_stderr_nonempty+1))
  done
  echo "负例=${#neg_files[@]} 比对=$n_chk 一致=$n_pass 差异=$n_diff golden缺失=$n_missing 正例stderr非空=$p_stderr_nonempty" >>"$log"
  if [ "$n_diff" -gt 0 ] || [ "$p_stderr_nonempty" -gt 0 ]; then record golden FAIL 1 "$log"
  elif [ "$STRICT" = 1 ] && { [ "$n_missing" -gt 0 ] || [ "$n_chk" -eq 0 ]; }; then
    echo "strict 防空口径：golden缺失=$n_missing 比对数=$n_chk → FAIL（§2.1 缺失语义）" >>"$log"; record golden FAIL 1 "$log"
  elif [ "$n_chk" -eq 0 ] || [ "$n_missing" -gt 0 ]; then record golden SKIP 0 "$log"
  else record golden PASS 0 "$log"; fi
}

stage_bench() {
  local log; log="$(new_log bench)"
  if [ -x ci/bench/run_bench.sh ]; then
    if ci/bench/run_bench.sh --quick >>"$log" 2>&1; then record bench PASS 0 "$log"
    else record bench FAIL 1 "$log"; fi
  else record bench SKIP 0 "$log"; fi
}

stage_code_table() { # 0.1.18 项2 诊断码表不变式 A1–A7（单一实现处＝本函数，规范文本不另抄期望表）
  # 取径（在册声明）：条目界=表结构，非全角 U+FF5C；节区间标题谓词分类（^## 8. 含「编译期」→声明集，^## 8b. 含「运行期」→DECLRT，
  # 其余→散文面入 A6 分子）；区间内按每行码位列首枚 E[0-9]{4} 取，禁对区间正文全文 grep。判定只用 0/非 0 与集合差，不含任何 rc=1 通过条件。
  local log; log="$(new_log code-table)"
  if ! command -v awk >/dev/null 2>&1 || ! command -v grep >/dev/null 2>&1; then
    echo "awk/grep 缺失：本段不执行（--strict 判 FAIL，非 PASS）" > "$log"
    [ "$STRICT" = 1 ] && record code-table FAIL 1 "$log" || record code-table SKIP 0 "$log"; return
  fi
  local td; td="$(mktemp -d)"; trap 'rm -rf "$td"' RETURN
  local -a SP=(specs/r1-type-system.md specs/r2-ownership.md)
  local f rc_v=0
  for f in "${SP[@]}"; do
    [ -f "$f" ] || { echo "MISSING specs 件=$f" >>"$log"; record code-table FAIL 1 "$log"; return; }
    awk -v ROWS="$td/rows" -v OTH="$td/other" '
      BEGIN { s = "other" }
      function codes(line, out,   r) {
        r = line
        while (match(r, /E[0-9][0-9][0-9][0-9]/)) { print substr(r, RSTART, 5) >> out; r = substr(r, RSTART + 5) }
      }
      /^## / {
        s = "other"; t = substr($0, 4)
        if (t ~ /^8(\.|b)/) { if (t ~ /编译期/) s = "decl"; else if (t ~ /运行期/) s = "rt" }
        next
      }
      /^\|/ && (s == "decl" || s == "rt") {
        cell = $0; n = split(cell, a, /\|/)
        if (n < 3) next
        c = a[2]; gsub(/^[ \t]+|[ \t]+$/, "", c)
        if (match(c, /E[0-9][0-9][0-9][0-9]/) == 0) next
        q = a[3]; gsub(/^[ \t]+|[ \t]+$/, "", q)
        r = a[4]; gsub(/^[ \t]+|[ \t]+$/, "", r)
        print s "\t" substr(c, RSTART, 5) "\t" q "\t" r >> ROWS
        next
      }
      { if (s == "other") codes($0, OTH) }
    ' "$f"
  done
  touch "$td/rows" "$td/other"
  awk -F'\t' '$3 == "实发"  { print $2 }' "$td/rows" | sort -u > "$td/decl_emit"
  awk -F'\t' '$3 == "保留"  { print $2 }' "$td/rows" | sort -u > "$td/decl_res"
  awk -F'\t' '$3 ~ /^别名→/ { print $2 }' "$td/rows" | sort -u > "$td/decl_alias"
  awk -F'\t' '$1 == "rt"    { print $2 }' "$td/rows" | sort -u > "$td/decl_rt"
  awk -F'\t' '$3 ~ /^别名→/ && match($3, /E[0-9][0-9][0-9][0-9]/) { print substr($3, RSTART, 5) }' "$td/rows" | sort -u > "$td/alias_tgt"
  awk -F'\t' '$3 != "实发" && $3 != "保留" && $3 != "运行期" && $3 !~ /^别名→/ { print $2 "=" $3 }' "$td/rows" > "$td/bad_qual"
  cat "$td/decl_emit" "$td/decl_res" "$td/decl_alias" "$td/decl_rt" | sort -u > "$td/decl_all"
  grep -hoE "E[0-9]{4}" compiler/src/*.rs      | sort -u > "$td/emit"
  grep -hoE "E[0-9]{4}" tests/specs/golden/*.txt | sort -u > "$td/golden"
  sort -u "$td/other" > "$td/other_u"
  cat "$td/decl_emit" "$td/decl_alias" "$td/decl_res" | sort -u > "$td/decl_ct"
  comm -23 "$td/decl_emit" "$td/emit"        > "$td/v_a1"
  comm -23 "$td/emit" "$td/decl_ct"          > "$td/v_a2"
  comm -23 "$td/alias_tgt" "$td/emit"        > "$td/v_a3"
  cat "$td/emit" "$td/golden" | sort -u      > "$td/emit_gld"
  comm -12 "$td/decl_rt" "$td/emit_gld"      > "$td/v_a4"
  comm -23 "$td/other_u" "$td/decl_all"      > "$td/v_a6"
  cat "$td/decl_res" "$td/decl_rt" | sort -u > "$td/res_rt"
  comm -12 "$td/res_rt" "$td/emit"           > "$td/v_a7"
  local n_row n_uniq; n_row=$(grep -c '' "$td/rows" || true); n_uniq=$(cut -f2 "$td/rows" | sort -u | grep -c '' || true)
  if [ "$n_row" = "$n_uniq" ]; then : > "$td/v_a5"; else printf '码位列总行=%s 去重枚数=%s\n' "$n_row" "$n_uniq" > "$td/v_a5"; fi
  # HDR＝§8b 表头诚实句在位（冻结件 §五-3 强制句的常驻对应形；不依赖历史文本）
  if grep -q '规范登记、未实发' specs/r1-type-system.md; then : > "$td/v_hdr"; else printf '§8b 缺"规范登记、未实发"表头句\n' > "$td/v_hdr"; fi
  # A8b1＝新行出处闸的常驻半边（判据归口列非空）；另半边 A8b2 与搬运闸 A8a 须对 base 侧旧文核对，属升格窗一次性执行
  awk -F'\t' 'NF < 4 || $4 == "" { print $2 }' "$td/rows" > "$td/v_a8b1"
  {
    echo "[code-table] 取径=表结构（首格首枚 E[0-9]{4}），节区间标题谓词分类；口径源=specs/r1-type-system.md §8/§8b 与 specs/r2-ownership.md §8 取径注记"
    printf '[code-table] 声明行=%s 去重=%s DECLEMIT=%s DECLRES=%s DECLALIAS=%s DECLRT=%s EMIT=%s GOLDEN=%s 散文码位=%s\n' \
      "$n_row" "$n_uniq" "$(grep -c '' "$td/decl_emit" || true)" "$(grep -c '' "$td/decl_res" || true)" \
      "$(grep -c '' "$td/decl_alias" || true)" "$(grep -c '' "$td/decl_rt" || true)" \
      "$(grep -c '' "$td/emit" || true)" "$(grep -c '' "$td/golden" || true)" "$(grep -c '' "$td/other_u" || true)"
  } >> "$log"
  local a la
  for a in A1 A2 A3 A4 A5 A6 A7 A8b1 HDR; do
    la="$(printf '%s' "$a" | tr 'A-Z' 'a-z')"
    if [ -s "$td/v_$la" ]; then
      printf '[code-table] %s 违规=FAIL\n' "$a" >> "$log"
      sed 's/^/    /' "$td/v_$la" >> "$log"; rc_v=1
    else
      printf '[code-table] %s 违规=0 PASS\n' "$a" >> "$log"
    fi
  done
  if [ -s "$td/bad_qual" ]; then
    printf '[code-table] 定性列越闭集=FAIL（闭集=实发/保留/别名→<码>/运行期）\n' >> "$log"
    sed 's/^/    /' "$td/bad_qual" >> "$log"; rc_v=1
  else printf '[code-table] 定性列闭集=PASS\n' >> "$log"; fi
  if [ "$rc_v" = 0 ]; then echo "[code-table] 判定=PASS" >> "$log"; record code-table PASS 0 "$log"
  else echo "[code-table] 判定=FAIL" >> "$log"; record code-table FAIL 1 "$log"; fi
}

echo "[ci] run=$RUN_ID strict=$STRICT bench=$WITH_BENCH modes='$MODES' pos='$TESTS_POS_DIR' neg='$TESTS_NEG_DIR'"
stage_cargo
stage_compile
stage_unit_test
stage_golden
stage_code_table
[ "$WITH_BENCH" = 1 ] && stage_bench

{
  echo "--- stage summary ($RUN_ID) ---"
  for s in "${summary[@]}"; do echo "$s"; done
} | tee "$LOG_DIR/summary.txt"
echo "[ci] 证据目录: $LOG_DIR"
exit "$overall"
