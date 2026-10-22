#!/usr/bin/env bash
# ZOR 0.1.x CI 基线骨架（git 直连模式）
# 归属：ZOR工具链与开发工程师｜版本：skeleton-v2（V-3+M-1 收尾）｜2026-10-11
# 设计依据：shared/ZOR-0.1x-CI基线-设计草案-v1.md；QA 门禁 v2；排期验收单 V-3/M-1
#
# 阶段：cargo(可选,OQ-1方案B参考实现) → compile(debug/release 双模式,OQ-2) → unit-test → golden → code-table → rust-compare → run-smoke → [asan(Linux,--asan)] → [bench]
set -uo pipefail
shopt -s nullglob  # S-1：未命中 glob 展开为空数组，杜绝字面 glob 进循环的旁路

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[ -n "$REPO_ROOT" ] || { echo "[ci] 必须在 git 仓库内运行"; exit 2; }
cd "$REPO_ROOT"

# L2a CI-01：ASAN 宿主固定（ubuntu-24.04／x86_64／clang 18.1.3）
# 判据由 QA v1.3 §6 提供；本函数只做 CI 宿主身份闸，不替代 run_asan.sh 的 manifest／用例判据。
asan_host_pin_check() {
  local pin="$REPO_ROOT/ci/asan/host-pin.env"
  [ -f "$pin" ] || { echo "[ci][asan] ENV FAIL: 缺少 $pin"; return 1; }
  # shellcheck disable=SC1090
  . "$pin"
  local os arch os_id os_version clang_bin clang_version rt t f
  os="$(uname -s)"; arch="$(uname -m)"
  [ "$os" = "Linux" ] || {
    echo "[ci][asan] ENV FAIL: 宿主必须是 Linux（当前 $os）；macOS 无 LSAN，不得判 PASS"
    return 1
  }
  [ "$arch" = "${ZOR_ASAN_ARCH:-x86_64}" ] || {
    echo "[ci][asan] ENV FAIL: 架构必须是 ${ZOR_ASAN_ARCH:-x86_64}（当前 $arch）"
    return 1
  }
  [ -r /etc/os-release ] || { echo "[ci][asan] ENV FAIL: 缺少 /etc/os-release"; return 1; }
  os_id="$(. /etc/os-release 2>/dev/null; printf '%s' "${ID:-}")"
  os_version="$(. /etc/os-release 2>/dev/null; printf '%s' "${VERSION_ID:-}")"
  [ "$os_id" = "${ZOR_ASAN_OS_ID:-ubuntu}" ] || {
    echo "[ci][asan] ENV FAIL: OS ID=$os_id，要求 ${ZOR_ASAN_OS_ID:-ubuntu}"
    return 1
  }
  [ "$os_version" = "${ZOR_ASAN_OS_VERSION:-24.04}" ] || {
    echo "[ci][asan] ENV FAIL: OS VERSION_ID=$os_version，要求 ${ZOR_ASAN_OS_VERSION:-24.04}"
    return 1
  }
  clang_bin="${ZOR_ASAN_CLANG_BIN:-clang-18}"
  command -v "$clang_bin" >/dev/null 2>&1 || {
    echo "[ci][asan] ENV FAIL: 缺少 $clang_bin"
    return 1
  }
  clang_version="$("$clang_bin" --version 2>&1 | head -1)"
  printf '%s' "$clang_version" | grep -Fq -- "${ZOR_ASAN_CLANG:-18.1.3}" || {
    echo "[ci][asan] ENV FAIL: clang 版本不符：$clang_version（要求含 ${ZOR_ASAN_CLANG:-18.1.3}）"
    return 1
  }
  rt="$("$clang_bin" -print-file-name=libclang_rt.asan-x86_64.a 2>/dev/null || true)"
  if [ -z "$rt" ] || [ ! -f "$rt" ]; then
    rt="$("$clang_bin" -print-file-name=libclang_rt.asan-x86_64.so 2>/dev/null || true)"
  fi
  [ -n "$rt" ] && [ -f "$rt" ] || {
    echo "[ci][asan] ENV FAIL: ASAN runtime 缺失（$clang_bin -print-file-name=...a/.so）"
    return 1
  }
  for t in readelf timeout; do
    command -v "$t" >/dev/null 2>&1 || {
      echo "[ci][asan] ENV FAIL: 缺少工具 $t"
      return 1
    }
  done
  for f in ci/asan/cc-asan ci/asan/cc-nog; do
    [ -x "$REPO_ROOT/$f" ] || {
      echo "[ci][asan] ENV FAIL: 缺少或不可执行 $f"
      return 1
    }
  done
  echo "[ci][asan] host pin PASS os=${os_id} ${os_version} arch=${arch} clang=${clang_version}"
  echo "[ci][asan] asan_runtime=${rt} cc_real=$(command -v "$clang_bin")"
  return 0
}

STRICT=0; WITH_BENCH=0; WITH_ASAN=0; ASAN_HOST_CHECK=0
for arg in "$@"; do
  case "$arg" in
    --strict) STRICT=1 ;;
    --bench) WITH_BENCH=1 ;;
    --asan) WITH_ASAN=1 ;;
    --asan-host-check) WITH_ASAN=1; ASAN_HOST_CHECK=1 ;;
    *) echo "[ci] 未知参数 $arg (可用: --strict --bench --asan --asan-host-check)"; exit 2 ;;
  esac
done

if [ "$ASAN_HOST_CHECK" = 1 ]; then
  if asan_host_pin_check; then exit 0; fi
  exit 2
fi

# —— M-1 目录基准唯一定义处：核心工程师若定论为 specs/tests，仅改这两行（或注入同名环境变量）——
TESTS_POS_DIR="${TESTS_POS_DIR:-tests/specs/positive}"
TESTS_NEG_DIR="${TESTS_NEG_DIR:-tests/specs/negative}"
# T-4 黄金比对（strict 扩项复验记录 §2：golden 名=负例 basename 换 .txt；模式无关单份）
TESTS_GOLDEN_DIR="${TESTS_GOLDEN_DIR:-tests/specs/golden}"
# AC-R2-4 Rust 同题对照源（0.1.26 项 7：两态命名契约 *-annotated.rs / *-unannotated.rs）
TESTS_COMPARE_DIR="${TESTS_COMPARE_DIR:-tests/specs/compare}"

# —— OQ-2 双模式（debug=checked / release=wrap，flags 为 zorcc 落地的注入位）——
DEBUG_FLAGS="${ZORC_DEBUG_FLAGS:--Zoverflow=check}"
RELEASE_FLAGS="${ZORC_RELEASE_FLAGS:--O2 -Zoverflow=wrap}"
MODES="debug release"

ZORC="${ZORC_BIN:-zorcc}"
command -v "$ZORC" >/dev/null 2>&1 || ZORC=""
RUSTC_BIN="${RUSTC_BIN:-rustc}"
command -v "$RUSTC_BIN" >/dev/null 2>&1 || RUSTC_BIN=""

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
    echo "对称判据：strict 下 正例数=$n_pos 负例数=${n_neg}，任一侧=0 → FAIL（M-1/S-1）" >>"$log"
    record unit-test FAIL 1 "$log"; return
  fi
  if [ "$pos_fail" -gt 0 ] || [ "$neg_leak" -gt 0 ]; then record unit-test FAIL 1 "$log"
  elif [ -z "$ZORC" ] || { [ "$n_pos" -eq 0 ] && [ "$n_neg" -eq 0 ]; }; then
    [ "$STRICT" = 1 ] && record unit-test FAIL 1 "$log" || record unit-test SKIP 0 "$log"
  else record unit-test PASS 0 "$log"; fi
}

stage_golden() { # T-4 黄金比对（strict 扩项复验记录 §2.4：负例 exit+stderr==golden；正例 stderr 空即基线）
  # 0.1.19 口径分列注记：捕获面（ci/capture_golden.sh）＝stdout+stderr 合并流写入基线；本段比对面＝stderr 单流逐字节对平（正例 stdout 既不捕获也不比＝读数缺口）。
  local log; log="$(new_log golden)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：默认 SKIP（strict 判 FAIL）" > "$log"
    [ "$STRICT" = 1 ] && record golden FAIL 1 "$log" || record golden SKIP 0 "$log"; return
  fi
  local -a neg_files=("$TESTS_NEG_DIR"/*.zor) pos_files=("$TESTS_POS_DIR"/*.zor)
  local n_chk=0 n_pass=0 n_diff=0 n_missing=0 p_stderr_nonempty=0 rc
  local n_pos_rc=0 n_golden_empty=0
  local tmpf; tmpf="$(mktemp)"; trap 'rm -f "$tmpf"' RETURN
  for f in "${neg_files[@]}"; do
    local b g; b="$(basename "$f" .zor)"; g="$TESTS_GOLDEN_DIR/$b.txt"
    # shellcheck disable=SC2086
    "$ZORC" $DEBUG_FLAGS "$f" >/dev/null 2>"$tmpf"; rc=$?
    if [ ! -f "$g" ]; then
      n_missing=$((n_missing+1)); echo "MISSING golden=$b.txt (exit=$rc)" >>"$log"; continue
    fi
    n_chk=$((n_chk+1))
    [ -s "$g" ] || n_golden_empty=$((n_golden_empty+1))
    if [ "$rc" -ne 1 ]; then n_diff=$((n_diff+1)); echo "EXIT_MISMATCH $b rc=$rc (期望1)" >>"$log"
    elif cmp -s "$tmpf" "$g"; then n_pass=$((n_pass+1))
    else n_diff=$((n_diff+1)); echo "STDERR_DIFF $b" >>"$log"; diff "$g" "$tmpf" | head -6 >>"$log"; fi
  done
  for f in "${pos_files[@]}"; do
    # shellcheck disable=SC2086
    "$ZORC" $DEBUG_FLAGS "$f" >/dev/null 2>"$tmpf" || n_pos_rc=$((n_pos_rc+1))
    [ -s "$tmpf" ] && p_stderr_nonempty=$((p_stderr_nonempty+1))
  done
  echo "负例=${#neg_files[@]} 比对=$n_chk 一致=$n_pass 差异=$n_diff golden缺失=$n_missing 正例stderr非空=$p_stderr_nonempty 正例退出非零=$n_pos_rc golden基线空件=$n_golden_empty" >>"$log"
  if [ "$n_diff" -gt 0 ] || [ "$p_stderr_nonempty" -gt 0 ] || [ "$n_pos_rc" -gt 0 ] || [ "$n_golden_empty" -gt 0 ]; then record golden FAIL 1 "$log"
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
  # 形制（判据位 seq205 §二／§三，本段照此写）：末式必以 record 落账（非纯 echo），否则红不置 overall 且 summary.txt 无该行；
  # 调用行须为独立一行裸调用——record() 尾式在 status=PASS 时短路使本函数返回 1，取 $? 或接 &&／|| 会把 PASS 读成 FAIL。
  local log; log="$(new_log code-table)"
  # 环境面钉定（0.1.18 执笔席自抓，判据位 seq295/seq304、裁口位 seq298、工具链 seq302 各自机次证实）：同件实测定性分类命中 C 下 DECLEMIT=13、en_US.UTF-8 下=16
  # ⇒ 判定随执行者环境漂移，故段内钉 LC_ALL=C，函数返回即复原、不外溢他段（成因句三席定形为"删"，本注只留两枚实测值，不写可误读的机制表述）。
  # 对照注入值＝编期译（两态区分性对照；纪律正本见 B 件／七款 v1；判据源 裁口位 seq298 §一-3／判据位 seq304 §三），不用 保留X（两态同色，只算回归位）。
  local outer_lc="${LC_ALL-UNSET}"  # 取序须在钉定之前：local LC_ALL=C 之后同名变量已读回 C，放反则本行恒印 C 且无任何红信号（工具链 seq302 §三／判据位 seq304 §四）
  local LC_ALL=C; export LC_ALL
  echo "[code-table] 环境自述 外层LC_ALL=$outer_lc 段内钉=C（此行走 stdout、不入 code-table.log：入则两式对平的逐字节 SAME 必破，与裁口位 seq290 ④ 互斥）"
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
  # HDR＝§8b 新句四项（L2b 起实发）：新标题唯一、旧标题不命中、历史句在位、A8b1 另判。
  # 不做全文“未实发”缺席断言（正文历史句保留）。
  local hdr_new hdr_old
  hdr_new=$(grep -cE '^## 8b\. .*（L2b 起实发）' specs/r1-type-system.md || true)
  hdr_old=$(grep -E '^## 8b\.' specs/r1-type-system.md | grep -c '（规范登记、未实发）' || true)
  if [ "$hdr_new" = 1 ] \
    && [ "$hdr_old" = 0 ] \
    && grep -qF '自 L2b 起实发；0.2.x 及以前为规范登记、未实发' specs/r1-type-system.md; then
    : > "$td/v_hdr"
  else
    printf '§8b HDR 四项不符：新标题数=%s 旧标题数=%s（要求 1／0 且历史句在位）\n' "$hdr_new" "$hdr_old" > "$td/v_hdr"
  fi
  # A8b1＝新行出处闸的常驻半边（判据归口列非空）；另半边 A8b2 与搬运闸 A8a 须对 base 侧旧文核对，属升格窗一次性执行
  awk -F'\t' 'NF < 4 || $4 == "" { print $2 }' "$td/rows" > "$td/v_a8b1"
  {
    echo "[code-table] 取径=表结构（首格首枚 E[0-9]{4}），节区间标题谓词分类；口径源=specs/r1-type-system.md §8/§8b 与 specs/r2-ownership.md §8 取径注记；环境=段内钉 LC_ALL=C（外层原值只随本段 stdout 自述行出、不入本日志，入则两式对平逐字节 SAME 必破）、效应层实测 charmap=$(locale charmap 2>/dev/null || echo UNKNOWN)（与所钉一致即合格，不要求为 UTF-8）"
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

stage_rust_compare() { # AC-R2-4 两段判（0.1.26 项 7）：*-annotated.rs 可编 / *-unannotated.rs 必报 E0106
  # 命名契约：annotated=须含具名生命周期参数且可编；unannotated=同题最小去标注变体，须编译失败且诊断含
  # error[E0106] + missing lifetime specifier。两侧按 basename 成对，strict 下各≥3 对（防空套件假绿）。
  # 反空转三项：① annotated 含 ≥1 具名生命周期；② stripped 与 annotated 归一化后仅差生命周期标注；
  # ③ B 段须 rc≠0 与 E0106 同时命中。归一化只用于比对，不写回任何源件。
  local log; log="$(new_log rust-compare)"
  if [ -z "$RUSTC_BIN" ]; then
    echo "rustc 未安装：AC-R2-4 两段判不可执行（--strict 判 FAIL，非 PASS）" > "$log"
    [ "$STRICT" = 1 ] && record rust-compare FAIL 1 "$log" || record rust-compare SKIP 0 "$log"; return
  fi
  normalize_rs() {
    sed -e 's|//.*$||' \
        -e "s/<'[A-Za-z_][A-Za-z0-9_]*>//g" \
        -e "s/'[A-Za-z_][A-Za-z0-9_]*//g" \
        -e 's/&[[:space:]]*/\&/g' \
        -e 's/[[:space:]][[:space:]]*/ /g' \
        -e 's/^ //; s/ $//' "$1" | grep -v '^$'
  }
  local n_ann=0 n_unann=0 ann_fail=0 unann_fail=0 pair_fail=0 life_missing=0 strip_diff=0 f rc i=0 stem hits
  local td; td="$(mktemp -d)"; trap 'rm -rf "$td"' RETURN
  {
    echo "[rust-compare] rustc=$( "$RUSTC_BIN" --version 2>&1 | head -1 ) edition=2021 crate-type=lib emit=metadata"
    "$RUSTC_BIN" -vV 2>&1
    echo "[rust-compare] dir=$TESTS_COMPARE_DIR"
  } >> "$log"
  for f in "$TESTS_COMPARE_DIR"/*-annotated.rs; do
    [ -e "$f" ] || continue
    n_ann=$((n_ann+1)); i=$((i+1))
    stem="${f%-annotated.rs}"
    [ -f "$stem-unannotated.rs" ] || { pair_fail=$((pair_fail+1)); echo "PAIR_MISSING $(basename "$f") 无对应 $(basename "$stem")-unannotated.rs" >>"$log"; }
    if ! grep -Eq "<'[A-Za-z_]|&'[A-Za-z_]" "$f"; then
      life_missing=$((life_missing+1)); echo "ANNOTATED_NO_NAMED_LIFETIME ${f}（反空转①：须含 ≥1 具名生命周期）" >>"$log"
    fi
    if "$RUSTC_BIN" --edition=2021 --crate-type lib --emit=metadata -o "$td/ann-$i.rmeta" "$f" >>"$log" 2>&1; then
      echo "ANNOTATED_OK $(basename "$f") rc=0" >>"$log"
    else
      ann_fail=$((ann_fail+1)); echo "ANNOTATED_COMPILE_FAIL $(basename "$f")" >>"$log"
    fi
  done
  i=0
  for f in "$TESTS_COMPARE_DIR"/*-unannotated.rs; do
    [ -e "$f" ] || continue
    n_unann=$((n_unann+1)); i=$((i+1))
    stem="${f%-unannotated.rs}"
    [ -f "$stem-annotated.rs" ] || { pair_fail=$((pair_fail+1)); echo "PAIR_MISSING $(basename "$f") 无对应 $(basename "$stem")-annotated.rs" >>"$log"; }
    if [ -f "$stem-annotated.rs" ]; then
      normalize_rs "$stem-annotated.rs" > "$td/pair-$i-ann.norm"
      normalize_rs "$f" > "$td/pair-$i-unann.norm"
      if ! cmp -s "$td/pair-$i-ann.norm" "$td/pair-$i-unann.norm"; then
        strip_diff=$((strip_diff+1)); echo "STRIP_DIFF $(basename "$f") 与 $(basename "$stem")-annotated.rs 归一化后不等（差异不止生命周期标注）" >>"$log"
        diff "$td/pair-$i-ann.norm" "$td/pair-$i-unann.norm" | head -8 >>"$log"
      fi
    fi
    "$RUSTC_BIN" --edition=2021 --crate-type lib --emit=metadata -o "$td/unann-$i.rmeta" "$f" >/dev/null 2>"$td/unann-$i.err"; rc=$?
    if [ "$rc" -eq 0 ]; then
      unann_fail=$((unann_fail+1)); echo "UNANNOTATED_UNEXPECTED_PASS $(basename "$f") rc=0" >>"$log"
    elif ! grep -q 'error\[E0106\]' "$td/unann-$i.err"; then
      unann_fail=$((unann_fail+1)); echo "UNANNOTATED_WRONG_DIAG $(basename "$f") rc=${rc}（期望 error[E0106]）" >>"$log"
      head -8 "$td/unann-$i.err" >>"$log"
    else
      # D1（QA v5 §12）：判据只取 rc≠0＋E0106；旧措辞仅记日志、不计红。
      if ! grep -q 'missing lifetime specifier' "$td/unann-$i.err"; then
        echo "WORDING_DIFF $(basename "$f") 含 E0106 但无旧措辞（按 QA v5 §12 不计红）" >>"$log"
      fi
      hits="$(grep -c 'error\[E0106\]' "$td/unann-$i.err" || true)"
      echo "PAIR_OK annotated=$(basename "$stem")-annotated.rs unannotated=$(basename "$f") rc=$rc E0106=$hits" >>"$log"
    fi
  done
  echo "两段判 annotated=$n_ann annotated_fail=$ann_fail unannotated=$n_unann unannotated_fail=$unann_fail pair_fail=$pair_fail lifetime_missing=$life_missing strip_diff=$strip_diff strict_min_pairs=3" >> "$log"
  if [ "$ann_fail" -gt 0 ] || [ "$unann_fail" -gt 0 ] || [ "$pair_fail" -gt 0 ] || [ "$life_missing" -gt 0 ] || [ "$strip_diff" -gt 0 ]; then record rust-compare FAIL 1 "$log"
  elif [ "$n_ann" -eq 0 ] || [ "$n_unann" -eq 0 ]; then
    [ "$STRICT" = 1 ] && record rust-compare FAIL 1 "$log" || record rust-compare SKIP 0 "$log"
  elif [ "$STRICT" = 1 ] && { [ "$n_ann" -lt 3 ] || [ "$n_unann" -lt 3 ] || [ "$n_ann" -ne "$n_unann" ]; }; then
    echo "strict 下限：annotated=$n_ann unannotated=${n_unann}（要求各≥3 且成对）→ FAIL" >> "$log"
    record rust-compare FAIL 1 "$log"
  else record rust-compare PASS 0 "$log"; fi
}

stage_run_smoke() { # 020-CI-01：0.2.0 首个可执行闭环＋五类负控（QA v5）
  local log; log="$(new_log run-smoke)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：run-smoke strict 判 FAIL（不可 SKIP 洗绿）" > "$log"
    [ "$STRICT" = 1 ] && record run-smoke FAIL 1 "$log" || record run-smoke SKIP 0 "$log"
    return
  fi
  local CCBIN="${CC:-cc}"
  local missing=""
  command -v "$CCBIN" >/dev/null 2>&1 || missing="$missing cc"
  command -v file >/dev/null 2>&1 || missing="$missing file"
  if [ -n "$missing" ]; then
    echo "run-smoke 环境缺失：${missing}（strict 判 FAIL，不 SKIP）" > "$log"
    [ "$STRICT" = 1 ] && record run-smoke FAIL 1 "$log" || record run-smoke SKIP 0 "$log"
    return
  fi
  local td td2; td="$(mktemp -d)"; td2="$(mktemp -d)"
  trap 'rm -rf "$td" "$td2"' RETURN
  local hello="$REPO_ROOT/examples/hello.zor"
  local expected="$td/expected"; printf 'Hello, ZOR\n' > "$expected"
  local fail=0 rc ftype n4_files misuse src_hash_before src_hash_after
  local zorc_abs
  if printf '%s' "$ZORC" | grep -q '/'; then
    zorc_abs="$(cd "$(dirname "$ZORC")" && pwd)/$(basename "$ZORC")"
  else
    zorc_abs="$(command -v "$ZORC")"
  fi
  sha256_of() { if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'; else sha256sum "$1" | awk '{print $1}'; fi; }
  echo "[run-smoke] zorcc=$zorc_abs cc=$CCBIN host=$(uname -sm)" >> "$log"
  "$CCBIN" --version 2>&1 | head -1 >> "$log"
  echo "[run-smoke] hello=$hello" >> "$log"

  # S0-1 构建：显式 -o + 原生可执行
  "$zorc_abs" "$hello" -o "$td/hello" >> "$log" 2>&1; rc=$?
  echo "S0-1 BUILD rc=$rc target=$td/hello" >> "$log"
  [ "$rc" -eq 0 ] || { echo "S0-1 FAIL: build rc=${rc}（期望0）" >> "$log"; fail=1; }
  if [ ! -x "$td/hello" ]; then
    echo "S0-1 FAIL: 产物缺失或不可执行" >> "$log"; fail=1
  else
    ftype="$(file -b "$td/hello")"; echo "S0-1 FILE=$ftype" >> "$log"
    printf '%s' "$ftype" | grep -Eq 'Mach-O .* executable|ELF .* executable' \
      || { echo "S0-1 FAIL: 非宿主原生可执行: $ftype" >> "$log"; fail=1; }
  fi

  # S0-2 运行：rc=0、stdout 逐字节、stderr 空
  if [ -x "$td/hello" ]; then
    "$td/hello" > "$td/out" 2> "$td/err"; rc=$?
    echo "S0-2 RUN rc=$rc stdout_sha256=$(sha256_of "$td/out") stderr_bytes=$(wc -c < "$td/err" | tr -d ' ')" >> "$log"
    [ "$rc" -eq 0 ] || { echo "S0-2 FAIL: run rc=${rc}（期望0）" >> "$log"; fail=1; }
    cmp -s "$td/out" "$expected" || { echo "S0-2 FAIL: stdout 非 Hello, ZOR\\n" >> "$log"; fail=1; }
    [ -s "$td/err" ] && { echo "S0-2 FAIL: stderr 非空" >> "$log"; fail=1; }
  fi
  # F-CI-4：成功构建后临时件零残留
  if find "$td" -maxdepth 1 -name '.zorcc-tmp-*' -print -quit | grep -q .; then
    echo "S0-1 FAIL: 成功构建后残留 .zorcc-tmp-*" >> "$log"; fail=1
  else
    echo "TMP_CLEAN: 成功构建后无 .zorcc-tmp-*" >> "$log"
  fi

  # N-1 期望串错一位：比对器必须判不等
  printf 'Hello, ZOR\nX' > "$td/expected.wrong"
  if cmp -s "$td/out" "$td/expected.wrong"; then
    echo "N-1 FAIL: 比对器失明（错误期望串判等）" >> "$log"; fail=1
  else
    echo "N-1 PASS: 错一位期望串判不等" >> "$log"
  fi

  # N-2 前端负控：spawn → rc=1、E1101、无产物
  local ctl_spawn="$REPO_ROOT/tests/runtime/controls/unsupported-syntax.zor"
  if [ ! -f "$ctl_spawn" ]; then
    echo "N-2 FAIL: 缺少 $ctl_spawn" >> "$log"; fail=1
  else
    "$zorc_abs" "$ctl_spawn" -o "$td/spawn.out" > "$td/n2.out" 2> "$td/n2.err"; rc=$?
    echo "N-2 rc=$rc" >> "$log"
    if [ "$rc" -eq 1 ] && grep -q 'E1101' "$td/n2.err" && [ ! -e "$td/spawn.out" ]; then
      echo "N-2 PASS: rc=1/E1101/无产物" >> "$log"
    else
      echo "N-2 FAIL: 期望 rc=1/E1101/无产物（实得 rc=${rc}）" >> "$log"; fail=1
    fi
  fi

  # N-3 后端 fail-closed：最小 struct → rc=1、zorcc: codegen:、无产物
  local ctl_codegen="$REPO_ROOT/tests/runtime/controls/codegen-unsupported.zor"
  if [ ! -f "$ctl_codegen" ]; then
    echo "N-3 FAIL: 缺少 $ctl_codegen" >> "$log"; fail=1
  else
    "$zorc_abs" "$ctl_codegen" -o "$td/codegen.out" > "$td/n3.out" 2> "$td/n3.err"; rc=$?
    echo "N-3 rc=$rc" >> "$log"
    if [ "$rc" -eq 1 ] && grep -q 'zorcc: codegen:' "$td/n3.err" && [ ! -e "$td/codegen.out" ]; then
      echo "N-3 PASS: rc=1/zorcc: codegen:/无产物" >> "$log"
    else
      echo "N-3 FAIL: 期望 rc=1/zorcc: codegen:/无产物（实得 rc=${rc}）" >> "$log"; fail=1
    fi
  fi

  # N-4 无 -o：检查模式 rc=0、源目录零新增；负例仍 rc=1 且源目录零新增
  local nodir="$td/noout"; mkdir -p "$nodir"
  cp "$hello" "$nodir/hello.zor"
  ( cd "$nodir" && "$zorc_abs" "$nodir/hello.zor" ) > "$td/n4.out" 2> "$td/n4.err"; rc=$?
  n4_files="$(find "$nodir" -mindepth 1 | wc -l | tr -d ' ')"
  echo "N-4 rc=$rc source_dir_files=${n4_files}（期望1=仅源副本）" >> "$log"
  { [ "$rc" -eq 0 ] && [ "$n4_files" -eq 1 ] && [ -f "$nodir/hello.zor" ]; } \
    || { echo "N-4 FAIL: 无 -o 应 rc=0 且源目录仅源副本（实得 rc=$rc files=${n4_files}）" >> "$log"; fail=1; }
  local negdir="$td/negdir"; mkdir -p "$negdir"
  cp "$REPO_ROOT/tests/specs/negative/r1-n01-unknown-type.zor" "$negdir/"
  ( cd "$negdir" && "$zorc_abs" "$negdir/r1-n01-unknown-type.zor" ) > "$td/n4b.out" 2> "$td/n4b.err"; rc=$?
  local neg_files; neg_files="$(find "$negdir" -mindepth 1 | wc -l | tr -d ' ')"
  { [ "$rc" -eq 1 ] && [ "$neg_files" -eq 1 ]; } \
    || { echo "N-4b FAIL: 检查模式负例 rc=$rc files=${neg_files}（期望1/1）" >> "$log"; fail=1; }

  # N-5 -o 误用与原子性：每例独立子目录，断言 rc=2 且目录无新增/无残留
  src_hash_before="$(sha256_of "$hello")"
  misuse=0
  check_rc2_case() { # <desc> <dir> <cmd...>：rc=2、目录条目数不变、无 .zorcc-tmp-*
    local desc="$1" dir="$2"; shift 2
    mkdir -p "$dir"
    local before after ntmp
    before="$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')"
    "$@" > /dev/null 2>&1; local r=$?
    after="$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')"
    ntmp="$(find "$dir" -name '.zorcc-tmp-*' -print -quit)"
    if [ "$r" -eq 2 ] && [ "$after" -eq "$before" ] && [ -z "$ntmp" ]; then
      echo "N-5 $desc rc=2 entries=$before PASS" >> "$log"
    else
      echo "N-5 $desc rc=$r entries_before=$before entries_after=$after tmp=${ntmp:-none} FAIL（期望rc=2/零新增/无残留）" >> "$log"; misuse=1
    fi
  }
  check_rc2_case_in() { # <desc> <dir> <cmd...>：在 dir 内运行（捕 cwd 隐式产物）
    local desc="$1" dir="$2"; shift 2
    mkdir -p "$dir"
    local before after ntmp
    before="$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')"
    ( cd "$dir" && "$@" ) > /dev/null 2>&1; local r=$?
    after="$(find "$dir" -mindepth 1 | wc -l | tr -d ' ')"
    ntmp="$(find "$dir" -name '.zorcc-tmp-*' -print -quit)"
    if [ "$r" -eq 2 ] && [ "$after" -eq "$before" ] && [ -z "$ntmp" ]; then
      echo "N-5 $desc rc=2 entries=$before PASS（cwd 无隐式产物）" >> "$log"
    else
      echo "N-5 $desc rc=$r entries_before=$before entries_after=$after tmp=${ntmp:-none} FAIL（期望rc=2/零新增/无残留）" >> "$log"; misuse=1
    fi
  }
  check_rc2_case_in "missing-value" "$td/m5" "$zorc_abs" "$hello" -o
  check_rc2_case "duplicate" "$td/m6" "$zorc_abs" "$hello" -o "$td/m6/a" -o "$td/m6/b"
  check_rc2_case_in "empty-path" "$td/m7" "$zorc_abs" "$hello" -o ""
  check_rc2_case "parent-missing" "$td/m8" "$zorc_abs" "$hello" -o "$td/m8/missing/hello"
  mkdir -p "$td/m9"; : > "$td/m9/notdir"
  check_rc2_case "parent-is-file" "$td/m9" "$zorc_abs" "$hello" -o "$td/m9/notdir/hello"
  [ -f "$td/m9/notdir" ] || { echo "N-5 FAIL: parent-is-file 的 notdir 被改写" >> "$log"; fail=1; }
  mkdir -p "$td/m10/target"; printf 'keep' > "$td/m10/target/keep"
  check_rc2_case "target-is-dir" "$td/m10" "$zorc_abs" "$hello" -o "$td/m10/target"
  [ "$(cat "$td/m10/target/keep" 2>/dev/null)" = "keep" ] || { echo "N-5 FAIL: target-is-dir 既有内容被改写" >> "$log"; fail=1; }
  mkdir -p "$td/m11"; cp "$hello" "$td/m11/same.zor"
  local same_before same_after
  same_before="$(sha256_of "$td/m11/same.zor")"
  check_rc2_case "same-as-source" "$td/m11" "$zorc_abs" "$td/m11/same.zor" -o "$td/m11/same.zor"
  same_after="$(sha256_of "$td/m11/same.zor")"
  [ "$same_before" = "$same_after" ] || { echo "N-5 FAIL: 同路径源副本被改写" >> "$log"; fail=1; }
  if [ "$(id -u)" -ne 0 ]; then
    mkdir -p "$td/m12/ro"; chmod 500 "$td/m12/ro"
    check_rc2_case "unwritable-dir" "$td/m12" "$zorc_abs" "$hello" -o "$td/m12/ro/hello"
    chmod 700 "$td/m12/ro"
  fi
  src_hash_after="$(sha256_of "$hello")"
  [ "$src_hash_before" = "$src_hash_after" ] || { echo "N-5 FAIL: 源文件被改写" >> "$log"; fail=1; }
  [ "$misuse" -eq 0 ] || { echo "N-5 FAIL: -o 误用未全判 rc=2/零新增" >> "$log"; fail=1; }
  # N-5h 失败原子性：sentinel 目标 + 前端负例
  mkdir -p "$td/m13"
  printf 'sentinel-content' > "$td/m13/sentinel"
  local n5h_ok=1
  "$zorc_abs" "$REPO_ROOT/tests/specs/negative/r1-n01-unknown-type.zor" -o "$td/m13/sentinel" > "$td/n5h.out" 2>&1; rc=$?
  [ "$rc" -eq 1 ] || { echo "N-5h FAIL: 前端负例 rc=${rc}（期望1）" >> "$log"; fail=1; n5h_ok=0; }
  [ "$(cat "$td/m13/sentinel")" = "sentinel-content" ] || { echo "N-5h FAIL: sentinel 被改写" >> "$log"; fail=1; n5h_ok=0; }
  if find "$td" -name '.zorcc-tmp-*' -print -quit | grep -q .; then
    echo "N-5 FAIL: 残留 .zorcc-tmp-*" >> "$log"; fail=1; n5h_ok=0
  else
    echo "TMP_CLEAN: N-5h 后无 .zorcc-tmp-*" >> "$log"
  fi
  [ "$n5h_ok" -eq 1 ] && echo "N-5h PASS: rc=1、sentinel 未改写、无临时残留" >> "$log"

  # S0-3 换临时目录复算
  "$zorc_abs" "$hello" -o "$td2/hello" >> "$log" 2>&1; rc=$?
  [ "$rc" -eq 0 ] || { echo "S0-3 FAIL: 第二次构建 rc=$rc" >> "$log"; fail=1; }
  if [ -x "$td2/hello" ]; then
    "$td2/hello" > "$td2/out" 2> "$td2/err"; rc=$?
    [ "$rc" -eq 0 ] || { echo "S0-3 FAIL: 第二次运行 rc=$rc" >> "$log"; fail=1; }
    cmp -s "$td/out" "$td2/out" || { echo "S0-3 FAIL: 两次 stdout 不同" >> "$log"; fail=1; }
    [ -s "$td2/err" ] && { echo "S0-3 FAIL: 第二次 stderr 非空" >> "$log"; fail=1; }
  else
    echo "S0-3 FAIL: 第二次产物缺失" >> "$log"; fail=1
  fi
  if find "$td2" -maxdepth 1 -name '.zorcc-tmp-*' -print -quit | grep -q .; then
    echo "S0-3 FAIL: 第二次构建后残留 .zorcc-tmp-*" >> "$log"; fail=1
  else
    echo "TMP_CLEAN: 第二次构建后无 .zorcc-tmp-*" >> "$log"
  fi

  if [ "$fail" -eq 0 ]; then
    echo "[run-smoke] 判定=PASS（正例＋五类负控）" >> "$log"
    record run-smoke PASS 0 "$log"
  else
    echo "[run-smoke] 判定=FAIL" >> "$log"
    record run-smoke FAIL 1 "$log"
  fi
}

stage_asan() { # L2a CI-01：宿主固定 + QA-01 ASAN/LSAN 脚本；fail-closed；不并入七段常驻面
  local log; log="$(new_log asan)"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：asan strict 判 FAIL（不可 SKIP 洗绿）" > "$log"
    [ "$STRICT" = 1 ] && record asan FAIL 1 "$log" || record asan SKIP 0 "$log"
    return
  fi
  if ! asan_host_pin_check >>"$log" 2>&1; then
    echo "[asan] host pin FAIL" >> "$log"
    [ "$STRICT" = 1 ] && record asan FAIL 1 "$log" || record asan SKIP 0 "$log"
    return
  fi
  local cc_real; cc_real="$(command -v "$ZOR_ASAN_CLANG_BIN")"
  local rc=0
  CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
    bash ci/asan/run_asan.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan" >>"$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    echo "[asan] 判定=PASS" >> "$log"
    record asan PASS 0 "$log"
  else
    echo "[asan] 判定=FAIL rc=$rc" >> "$log"
    [ "$STRICT" = 1 ] && record asan FAIL 1 "$log" || record asan SKIP 0 "$log"
  fi
}

echo "[ci] run=$RUN_ID strict=$STRICT bench=$WITH_BENCH modes='$MODES' pos='$TESTS_POS_DIR' neg='$TESTS_NEG_DIR'"
stage_cargo
stage_compile
stage_unit_test
stage_golden
stage_code_table
stage_rust_compare
stage_run_smoke
[ "$WITH_ASAN" = 1 ] && stage_asan
[ "$WITH_BENCH" = 1 ] && stage_bench

{
  echo "--- stage summary ($RUN_ID) ---"
  for s in "${summary[@]}"; do echo "$s"; done
} | tee "$LOG_DIR/summary.txt"
n_skip="$(printf '%s\n' "${summary[@]}" | grep -c '^SKIP|' || true)"; [ "$STRICT" != 1 ] && [ "$n_skip" -gt 0 ] && echo "[ci][警告] 非 strict 且 SKIP=${n_skip}：存在被跳过的判定面，绿≠全绿（告警行不入任何 log、不改 rc 语义与 summary 段数）"
echo "[ci] 证据目录: $LOG_DIR"
exit "$overall"
