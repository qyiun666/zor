#!/usr/bin/env bash
# ZOR 0.1.x CI 基线骨架（git 直连模式）
# 归属：ZOR工具链与开发工程师｜版本：skeleton-v2（V-3+M-1 收尾）｜2026-10-11
# 设计依据：shared/ZOR-0.1x-CI基线-设计草案-v1.md；QA 门禁 v2；排期验收单 V-3/M-1
#
# 阶段：cargo(可选,OQ-1方案B参考实现) → compile(debug/release 双模式,OQ-2) → unit-test → golden → code-table → rust-compare → run-smoke → [asan(Linux,--asan)] → [bench]
# L2b：--runtime-codes 为独立模式（单段 summary），只执行宿主检查＋runtime-codes 判据；不改既有七段／八段语义。
# L2c：--stdout-contract 为独立模式（stdout／cli 两段 summary），直调 QA 判据脚本；不改既有七段／八段／runtime-codes 语义。
# L2d：--packaging-smoke 为独立模式（单段 summary），直调 QA 安装树判据脚本；不改既有七段／八段／runtime-codes／stdout 语义。
# L2e：stdout 判词 65／12／17、runtime-codes 39／6／19；仅同步计数门禁，不建 opt-matrix，不改 job 结构。
# L2f：stdout 判词 71／32／17；stdout-contract 内加 l2e／l2f 双基线 gate；runtime-codes 39／6／19；不建新 job。
# L2g：stdout 判词 80／41／17；stdout-contract 内加 l2e／l2f／l2g 三基线 gate（l2g 206／275）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2h：stdout 判词 89／73／17；stdout-contract 内加 l2e／l2f／l2g／l2h 四基线 gate（l2h 247／327）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2i：stdout 判词 108／103／17；stdout-contract 内加第五基线 l2i（297／398），N-81 仅执行面退休；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2j：stdout 判词 126／132／17；stdout-contract 内加第六基线 l2j（345／466），N-94 仅执行面退休；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2k：stdout 判词 175／143／17；stdout-contract 内加第七基线 l2k（417／590），Ret=12 仅执行面退休；ASAN 第二 manifest 6 条（同 asan 段）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2l：stdout 判词 199／163／17；stdout-contract 内加第八基线 l2l（461／660），Ret=0；ASAN K=0（默认 manifest 48／41／7 与 stage_asan 零改）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2m：stdout 判词 226／180／17；stdout-contract 内加第九基线 l2m（506／734），Ret=1（N-183 仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2n：stdout 判词 258／194／17；stdout-contract 内加第十基线 l2n（554／816），Ret=2（N-184／N-200 仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2o：stdout 判词 286／202／17；stdout-contract 内加第十一基线 l2o（599／891），Ret=9（九 ID 跨代仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2p：stdout 判词 310／221／17；stdout-contract 内加第十二基线 l2p（645／963），Ret=3（N-220／N-233／N-235 仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2q：stdout 判词 350／228／17；stdout-contract 内加第十三基线 l2q（701／1062），Ret=9（N-221／222／223＋N-237～242 仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2r：stdout 判词 390／239／17；stdout-contract 内加第十四基线 l2r（759／1163），Ret=7（N-96／172／173／195／211／230／246 仅执行面）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2s：stdout 判词 430／267／17；stdout-contract 内加第十五基线 l2s（827／1274），Ret=0（定义切片，无编译器行为变化）；ASAN K=0；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2t：stdout 判词 462／286／17；stdout-contract 内加第十六基线 l2t（884／1367），Ret=6（N-146／203／231／254／287／303 仅执行面）；ASAN 第二 manifest 组 6+3=9（K=3；l2k 原 6 例字节零改）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2u：stdout 判词 462／296／17；stdout-contract 内加第十七基线 l2u（894／1379），P=0／Q=10／Ret=0（诊断文本层；三条单 ID 执行面覆盖，旧 16 条零改）；ASAN 不变；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2v：stdout 判词 486／312／17；stdout-contract 内加第十八基线 l2v（937／1450），P=24／Q=19／Ret=3（N-290／N-321／N-323 仅执行面）；ASAN 新增 l2v 独立 K=2 判词；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2w：stdout 判词 510／324／17；stdout-contract 内加第十九基线 l2w（979／1520），P=24／Q=18／Ret=6（N-145／N-288／N-304／N-305／N-325／N-326 仅执行面，N-360／N-361 单 ID 判词覆盖）；ASAN 新增 l2w 独立 K=2 判词；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2x：stdout 判词 534／340／17；stdout-contract 内加第二十基线 l2x（1021／1590），P=24／Q=18／Ret=2（N-360／N-361 仅执行面；移除 L2w 单 ID token 覆盖）；ASAN 新增 l2x 独立 K=2 判词（合并 15）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2y：stdout 判词 558／354／17；stdout-contract 内加第二十一基线 l2y（1062／1659），P=24／Q=17／Ret=3（N-396／N-397／N-398 仅执行面）；ASAN 新增 l2y 独立 K=2 判词（合并 17）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L2z：stdout 判词 582／365／17；stdout-contract 内加第二十二基线 l2z（1104／1729），P=24／Q=18／Ret=7（N-149／N-150／N-294／N-296／N-414／N-415／N-427 仅执行面）；ASAN 新增 l2z 独立 K=2 判词（合并 19）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3a：stdout 判词 606／372／17；stdout-contract 内加第二十三基线 l3a（1146／1799），P=24／Q=18／Ret=11（N-20／N-42／N-255／N-293／N-295／N-368／N-399／N-401／N-417／N-428／N-429 仅执行面）；ASAN 新增 l3a 独立 K=2 判词（合并 21）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3b：stdout 判词 630／381／17；stdout-contract 内加第二十四基线 l3b（1188／1869），P=24／Q=18／Ret=9（N-163／N-164／N-400／N-402／N-416／N-430／N-448／N-449／N-450 仅执行面）；ASAN 新增 l3b 独立 K=2 判词（合并 23）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3c：stdout 判词 654／391／17；stdout-contract 内加第二十五基线 l3c（1230／1939），P=24／Q=18／Ret=8（N-464～N-471 仅执行面）；ASAN 新增 l3c 独立 K=2 判词（合并 25）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3d：stdout 判词 678／399／17；stdout-contract 内加第二十六基线 l3d（1272／2009），P=24／Q=18／Ret=10（N-19／N-297／N-299／N-452／N-453／N-474／N-475／N-483／N-484／N-488 仅执行面）；ASAN 新增 l3d 独立 K=2 判词（合并 27）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3e：stdout 判词 702／407／17；stdout-contract 内加第二十七基线 l3e（1314／2079），P=24／Q=18／Ret=10（N-298／N-431／N-432／N-443／N-444／N-445／N-485／N-486／N-508／N-509 仅执行面）；ASAN 新增 l3e 独立 K=2 判词（合并 29）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
# L3f：stdout 判词 726／405／17；stdout-contract 内加第二十八基线 l3f（1356／2149），P=24／Q=18／Ret=20（N-322／N-356／N-357／N-375／N-376／N-377／N-393／N-394／N-395／N-411／N-412／N-413／N-426／N-489／N-510／N-511／N-518／N-519／N-520／N-521 仅执行面）；ASAN 新增 l3f 独立 K=2 判词（合并 31）；runtime-codes 39／6／19、packaging 4 不变；不建新 job。
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

STRICT=0; WITH_BENCH=0; WITH_ASAN=0; ASAN_HOST_CHECK=0; WITH_RUNTIME_CODES=0; WITH_STDOUT_CONTRACT=0; WITH_PACKAGING_SMOKE=0
for arg in "$@"; do
  case "$arg" in
    --strict) STRICT=1 ;;
    --bench) WITH_BENCH=1 ;;
    --asan) WITH_ASAN=1 ;;
    --asan-host-check) WITH_ASAN=1; ASAN_HOST_CHECK=1 ;;
    --runtime-codes) WITH_RUNTIME_CODES=1 ;;
    --stdout-contract) WITH_STDOUT_CONTRACT=1 ;;
    --packaging-smoke) WITH_PACKAGING_SMOKE=1 ;;
    *) echo "[ci] 未知参数 $arg (可用: --strict --bench --asan --asan-host-check --runtime-codes --stdout-contract --packaging-smoke)"; exit 2 ;;
  esac
done

if [ "$WITH_RUNTIME_CODES" = 1 ] && { [ "$STRICT" = 1 ] || [ "$WITH_ASAN" = 1 ] || [ "$WITH_BENCH" = 1 ]; }; then
  echo "[ci] --runtime-codes 为独立模式，不可与 --strict／--asan／--bench 组合" >&2
  exit 2
fi

if [ "$WITH_STDOUT_CONTRACT" = 1 ] && { [ "$STRICT" = 1 ] || [ "$WITH_ASAN" = 1 ] || [ "$WITH_BENCH" = 1 ] || [ "$WITH_RUNTIME_CODES" = 1 ]; }; then
  echo "[ci] --stdout-contract 为独立模式，不可与 --strict／--asan／--bench／--runtime-codes 组合" >&2
  exit 2
fi

if [ "$WITH_PACKAGING_SMOKE" = 1 ] && { [ "$STRICT" = 1 ] || [ "$WITH_ASAN" = 1 ] || [ "$WITH_BENCH" = 1 ] || [ "$WITH_RUNTIME_CODES" = 1 ] || [ "$WITH_STDOUT_CONTRACT" = 1 ]; }; then
  echo "[ci] --packaging-smoke 为独立模式，不可与 --strict／--asan／--bench／--runtime-codes／--stdout-contract 组合" >&2
  exit 2
fi

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

  # N-3 后端 fail-closed：复合内 Box（L2d 不可打印）→ rc=1、zorcc: codegen:、无产物
  # 旧 codegen-unsupported.zor 的 struct Mark {} 在 L2d 已合法，按 legacy-snapshot 零改保留。
  local ctl_codegen="$REPO_ROOT/tests/runtime/controls/codegen-unsupported-l2d.zor"
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
    # L2t CI-01：第二 manifest 组 6+3=9（l2k 原 6 例字节零改 + 新 l2t 3 例，K=3）；默认 manifest 48／41／7 与 run_asan.sh 零改。
    local rc_l2k=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2k.sh --validate-only >>"$log" 2>&1 || rc_l2k=$?
    if [ "$rc_l2k" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2k.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2k" >>"$log" 2>&1 || rc_l2k=$?
    fi
    [ "$rc_l2k" -eq 0 ] || rc="$rc_l2k"
    # L2v CI-01：独立 l2v ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3 零改。
    local rc_l2v=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2v.sh --validate-only >>"$log" 2>&1 || rc_l2v=$?
    if [ "$rc_l2v" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2v.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2v" >>"$log" 2>&1 || rc_l2v=$?
    fi
    [ "$rc_l2v" -eq 0 ] || rc="$rc_l2v"
    # L2w CI-01：独立 l2w ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2 零改。
    local rc_l2w=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2w.sh --validate-only >>"$log" 2>&1 || rc_l2w=$?
    if [ "$rc_l2w" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2w.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2w" >>"$log" 2>&1 || rc_l2w=$?
    fi
    [ "$rc_l2w" -eq 0 ] || rc="$rc_l2w"
    # L2x CI-01：独立 l2x ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2 零改。
    local rc_l2x=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2x.sh --validate-only >>"$log" 2>&1 || rc_l2x=$?
    if [ "$rc_l2x" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2x.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2x" >>"$log" 2>&1 || rc_l2x=$?
    fi
    [ "$rc_l2x" -eq 0 ] || rc="$rc_l2x"
    # L2y CI-01：独立 l2y ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2 零改。
    local rc_l2y=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2y.sh --validate-only >>"$log" 2>&1 || rc_l2y=$?
    if [ "$rc_l2y" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2y.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2y" >>"$log" 2>&1 || rc_l2y=$?
    fi
    [ "$rc_l2y" -eq 0 ] || rc="$rc_l2y"
    # L2z CI-01：独立 l2z ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2 零改。
    local rc_l2z=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l2z.sh --validate-only >>"$log" 2>&1 || rc_l2z=$?
    if [ "$rc_l2z" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l2z.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l2z" >>"$log" 2>&1 || rc_l2z=$?
    fi
    [ "$rc_l2z" -eq 0 ] || rc="$rc_l2z"
    # L3a CI-01：独立 l3a ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2 零改。
    local rc_l3a=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3a.sh --validate-only >>"$log" 2>&1 || rc_l3a=$?
    if [ "$rc_l3a" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3a.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3a" >>"$log" 2>&1 || rc_l3a=$?
    fi
    [ "$rc_l3a" -eq 0 ] || rc="$rc_l3a"
    # L3b CI-01：独立 l3b ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2、l3a 2 零改。
    local rc_l3b=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3b.sh --validate-only >>"$log" 2>&1 || rc_l3b=$?
    if [ "$rc_l3b" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3b.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3b" >>"$log" 2>&1 || rc_l3b=$?
    fi
    [ "$rc_l3b" -eq 0 ] || rc="$rc_l3b"
    # L3c CI-01：独立 l3c ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2、l3a 2、l3b 2 零改。
    local rc_l3c=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3c.sh --validate-only >>"$log" 2>&1 || rc_l3c=$?
    if [ "$rc_l3c" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3c.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3c" >>"$log" 2>&1 || rc_l3c=$?
    fi
    [ "$rc_l3c" -eq 0 ] || rc="$rc_l3c"
    # L3d CI-01：独立 l3d ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2、l3a 2、l3b 2、l3c 2 零改。
    local rc_l3d=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3d.sh --validate-only >>"$log" 2>&1 || rc_l3d=$?
    if [ "$rc_l3d" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3d.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3d" >>"$log" 2>&1 || rc_l3d=$?
    fi
    [ "$rc_l3d" -eq 0 ] || rc="$rc_l3d"
    # L3e CI-01：独立 l3e ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2、l3a 2、l3b 2、l3c 2、l3d 2 零改。
    local rc_l3e=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3e.sh --validate-only >>"$log" 2>&1 || rc_l3e=$?
    if [ "$rc_l3e" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3e.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3e" >>"$log" 2>&1 || rc_l3e=$?
    fi
    [ "$rc_l3e" -eq 0 ] || rc="$rc_l3e"
    # L3f CI-01：独立 l3f ASAN manifest（K=2）；默认 48／41／7、l2k 6、l2t 3、l2v 2、l2w 2、l2x 2、l2y 2、l2z 2、l3a 2、l3b 2、l3c 2、l3d 2、l3e 2 零改。
    local rc_l3f=0
    CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
      bash ci/asan/run_asan_l3f.sh --validate-only >>"$log" 2>&1 || rc_l3f=$?
    if [ "$rc_l3f" -eq 0 ]; then
      CC_REAL="$cc_real" ZORC_BIN="$ZORC" \
        bash ci/asan/run_asan_l3f.sh --strict --zorc "$ZORC" --out "$LOG_DIR/asan-l3f" >>"$log" 2>&1 || rc_l3f=$?
    fi
    [ "$rc_l3f" -eq 0 ] || rc="$rc_l3f"
  fi
  if [ "$rc" -eq 0 ]; then
    echo "[asan] 判定=PASS" >> "$log"
    record asan PASS 0 "$log"
  else
    echo "[asan] 判定=FAIL rc=$rc" >> "$log"
    [ "$STRICT" = 1 ] && record asan FAIL 1 "$log" || record asan SKIP 0 "$log"
  fi
}

stage_runtime_codes() { # L2b CI-01：独立 Linux job；直调 QA 判据脚本；无 sanitizer；fail-closed
  local log; log="$(new_log runtime-codes)"
  local out="$LOG_DIR/runtime-codes"
  mkdir -p "$out"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：runtime-codes 判 FAIL（不可 SKIP 洗绿）" > "$log"
    record runtime-codes FAIL 1 "$log"
    return
  fi
  if [ ! -f tests/runtime/run_runtime_codes.sh ]; then
    echo "runtime-codes 判据脚本缺失：tests/runtime/run_runtime_codes.sh（QA-01 未合入）" > "$log"
    record runtime-codes FAIL 1 "$log"
    return
  fi
  local rc=0
  CC="${CC:-cc}" ZORC_BIN="$ZORC" OUT_DIR="$out" \
    bash tests/runtime/run_runtime_codes.sh --strict --out "$out" --zorc "$ZORC" >>"$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    # L2e CI-01：冻结 runtime_rows=39／width_rows=6／cli_rows=19／fail=0；缺行或计数不符同样 FAIL。
    local counts_ok=1 kv
    for kv in runtime_rows=39 width_rows=6 cli_rows=19 fail=0; do
      grep -Fxq "$kv" "$out/runtime-codes.log" 2>/dev/null || counts_ok=0
    done
    if [ "$counts_ok" -eq 1 ]; then
      echo "[runtime-codes] 判定=PASS（39／6／19）" >> "$log"
      record runtime-codes PASS 0 "$log"
    else
      echo "[runtime-codes] 计数不符：期望 runtime_rows=39 width_rows=6 cli_rows=19 fail=0（L2e 冻结）" >> "$log"
      record runtime-codes FAIL 1 "$log"
    fi
  else
    echo "[runtime-codes] 判定=FAIL rc=$rc" >> "$log"
    record runtime-codes FAIL 1 "$log"
  fi
}

stdout_section_verdict() { # 显式 FAIL 判词优先（即使脚本 rc=0）；否则 rc=0 即两段 PASS；缺判词按 FAIL（fail-closed）
  local section="$1" log="$2" rc="$3"
  if grep -q "^FAIL|${section}|" "$log" 2>/dev/null; then printf 'FAIL'; return; fi
  if [ "$rc" -eq 0 ]; then printf 'PASS'; return; fi
  if grep -q "^PASS|${section}|" "$log" 2>/dev/null; then printf 'PASS'; return; fi
  printf 'FAIL'
}

verify_baseline() { # L2f CI-01：verify_baseline <l2e|l2f|l2g|l2h|l2i|l2j|l2k|l2l|l2m|l2n|l2o|l2p|l2q|l2r|l2s|l2t|l2u|l2v|l2w|l2x|l2y|l2z|l3a|l3b|l3c|l3d|l3e|l3f> <outdir> <cases> <files>
  local name="$1" out="$2" cases_expected="$3" files_expected="$4"
  local base="tests/stdout/${name}-baseline.sha256"
  local tsv="tests/stdout/${name}-baseline.tsv"
  local log="$out/${name}-baseline.log"
  : > "$log"
  local files=0 unique=0 cases=0
  if [ ! -f "$base" ] || [ ! -f "$tsv" ]; then
    echo "FAIL|${name}-baseline|cases=0 files=0 unique=0 fail=1 reason=missing" >> "$log"
    return 1
  fi
  files="$(awk 'NF{n++} END{print n+0}' "$base")"
  unique="$(awk 'NF{print $2}' "$base" | sort -u | wc -l | tr -d ' ')"
  cases="$(awk 'NR>1 && NF{n++} END{print n+0}' "$tsv")"
  # 路径卫生：仅 tests/ 相对路径，含 64hex ＋ 两空格格式；拒绝绝对路径／..／空格／反斜杠／glob。
  if ! awk 'NF<2 {bad=1; next} {p=$2; if (p !~ /^tests\/[A-Za-z0-9._\/-]+$/ || p ~ /\.\./) bad=1} END{exit bad?1:0}' "$base"; then
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=unsafe_path" >> "$log"
    return 1
  fi
  if [ "$files" -le 0 ] || [ "$unique" -ne "$files" ]; then
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=duplicate_path" >> "$log"
    return 1
  fi
  # 必须锁定自身 TSV（防删条目规避）。
  if ! grep -Fq " ${tsv}" "$base"; then
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=self_not_locked" >> "$log"
    return 1
  fi
  local hrc=0 check_out=""
  if command -v sha256sum >/dev/null 2>&1; then
    ( cd "$REPO_ROOT" && sha256sum -c --quiet "$base" ) >> "$log" 2>&1 || hrc=1
  elif command -v shasum >/dev/null 2>&1; then
    check_out="$( cd "$REPO_ROOT" && shasum -a 256 -c "$base" 2>&1 )" || hrc=1
    [ "$hrc" -eq 0 ] || printf '%s\n' "$check_out" >> "$log"
  else
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=no_sha256_tool" >> "$log"
    return 1
  fi
  if [ "$hrc" -ne 0 ]; then
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=hash_mismatch" >> "$log"
    return 1
  fi
  if [ "$cases" -ne "$cases_expected" ] || [ "$files" -ne "$files_expected" ]; then
    echo "FAIL|${name}-baseline|cases=$cases files=$files unique=$unique fail=1 reason=count_mismatch" >> "$log"
    return 1
  fi
  echo "PASS|${name}-baseline|cases=$cases files=$files unique=$unique fail=0" >> "$log"
  if command -v sha256sum >/dev/null 2>&1; then
    echo "${name}-baseline.sha256=$(sha256sum "$base" | awk '{print $1}')" >> "$log"
  else
    echo "${name}-baseline.sha256=$(shasum -a 256 "$base" | awk '{print $1}')" >> "$log"
  fi
  cp "$base" "$out/${name}-baseline.sha256"
  cp "$tsv" "$out/${name}-baseline.tsv"
  return 0
}

stage_stdout_contract() { # L2c CI-01：独立 Linux job；直调 QA 判据脚本；stdout／cli 两段 summary；fail-closed
  local log; log="$(new_log stdout-contract)"
  local out="$LOG_DIR/stdout-contract"
  mkdir -p "$out"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：stdout-contract 判 FAIL（不可 SKIP 洗绿）" > "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  if [ ! -f tests/stdout/run_stdout_contract.sh ]; then
    echo "stdout-contract 判据脚本缺失：tests/stdout/run_stdout_contract.sh（QA-01 未合入）" > "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  # L3f CI-01：二十八基线 gate（先于两轮用例）。l2e 162／209；l2f 188／246；l2g 206／275；l2h 247／327；l2i 297／398；l2j 345／466；l2k 417／590；l2l 461／660；l2m 506／734；l2n 554／816；l2o 599／891；l2p 645／963；l2q 701／1062；l2r 759／1163；l2s 827／1274；l2t 884／1367；l2u 894／1379；l2v 937／1450；l2w 979／1520；l2x 1021／1590；l2y 1062／1659；l2z 1104／1729；l3a 1146／1799；l3b 1188／1869；l3c 1230／1939；l3d 1272／2009；l3e 1314／2079；l3f 全矩阵 1356／2149。
  # 任一失败即两段 FAIL 并返回；缺件／路径不安全／重复／hash 不符／计数不符一律 FAIL，不可 SKIP。
  if ! verify_baseline l2e "$out" 162 209; then
    grep -hE '^(PASS|FAIL)\|l2e-baseline' "$out/l2e-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2e-baseline] 校验失败（见 l2e-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2e-baseline' "$out/l2e-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2f "$out" 188 246; then
    grep -hE '^(PASS|FAIL)\|l2f-baseline' "$out/l2f-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2f-baseline] 校验失败（见 l2f-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2f-baseline' "$out/l2f-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2g "$out" 206 275; then
    grep -hE '^(PASS|FAIL)\|l2g-baseline' "$out/l2g-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2g-baseline] 校验失败（见 l2g-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2g-baseline' "$out/l2g-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2h "$out" 247 327; then
    grep -hE '^(PASS|FAIL)\|l2h-baseline' "$out/l2h-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2h-baseline] 校验失败（见 l2h-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2h-baseline' "$out/l2h-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2i "$out" 297 398; then
    grep -hE '^(PASS|FAIL)\|l2i-baseline' "$out/l2i-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2i-baseline] 校验失败（见 l2i-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2i-baseline' "$out/l2i-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2j "$out" 345 466; then
    grep -hE '^(PASS|FAIL)\|l2j-baseline' "$out/l2j-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2j-baseline] 校验失败（见 l2j-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2j-baseline' "$out/l2j-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2k "$out" 417 590; then
    grep -hE '^(PASS|FAIL)\|l2k-baseline' "$out/l2k-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2k-baseline] 校验失败（见 l2k-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2k-baseline' "$out/l2k-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2l "$out" 461 660; then
    grep -hE '^(PASS|FAIL)\|l2l-baseline' "$out/l2l-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2l-baseline] 校验失败（见 l2l-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2l-baseline' "$out/l2l-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2m "$out" 506 734; then
    grep -hE '^(PASS|FAIL)\|l2m-baseline' "$out/l2m-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2m-baseline] 校验失败（见 l2m-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2m-baseline' "$out/l2m-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2n "$out" 554 816; then
    grep -hE '^(PASS|FAIL)\|l2n-baseline' "$out/l2n-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2n-baseline] 校验失败（见 l2n-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2n-baseline' "$out/l2n-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2o "$out" 599 891; then
    grep -hE '^(PASS|FAIL)\|l2o-baseline' "$out/l2o-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2o-baseline] 校验失败（见 l2o-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2o-baseline' "$out/l2o-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2p "$out" 645 963; then
    grep -hE '^(PASS|FAIL)\|l2p-baseline' "$out/l2p-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2p-baseline] 校验失败（见 l2p-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2p-baseline' "$out/l2p-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2q "$out" 701 1062; then
    grep -hE '^(PASS|FAIL)\|l2q-baseline' "$out/l2q-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2q-baseline] 校验失败（见 l2q-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2q-baseline' "$out/l2q-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2r "$out" 759 1163; then
    grep -hE '^(PASS|FAIL)\|l2r-baseline' "$out/l2r-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2r-baseline] 校验失败（见 l2r-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2r-baseline' "$out/l2r-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2s "$out" 827 1274; then
    grep -hE '^(PASS|FAIL)\|l2s-baseline' "$out/l2s-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2s-baseline] 校验失败（见 l2s-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2s-baseline' "$out/l2s-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2t "$out" 884 1367; then
    grep -hE '^(PASS|FAIL)\|l2t-baseline' "$out/l2t-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2t-baseline] 校验失败（见 l2t-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2t-baseline' "$out/l2t-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2u "$out" 894 1379; then
    grep -hE '^(PASS|FAIL)\|l2u-baseline' "$out/l2u-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2u-baseline] 校验失败（见 l2u-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2u-baseline' "$out/l2u-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2v "$out" 937 1450; then
    grep -hE '^(PASS|FAIL)\|l2v-baseline' "$out/l2v-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2v-baseline] 校验失败（见 l2v-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2v-baseline' "$out/l2v-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2w "$out" 979 1520; then
    grep -hE '^(PASS|FAIL)\|l2w-baseline' "$out/l2w-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2w-baseline] 校验失败（见 l2w-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2w-baseline' "$out/l2w-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2x "$out" 1021 1590; then
    grep -hE '^(PASS|FAIL)\|l2x-baseline' "$out/l2x-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2x-baseline] 校验失败（见 l2x-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2x-baseline' "$out/l2x-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2y "$out" 1062 1659; then
    grep -hE '^(PASS|FAIL)\|l2y-baseline' "$out/l2y-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2y-baseline] 校验失败（见 l2y-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2y-baseline' "$out/l2y-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l2z "$out" 1104 1729; then
    grep -hE '^(PASS|FAIL)\|l2z-baseline' "$out/l2z-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l2z-baseline] 校验失败（见 l2z-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l2z-baseline' "$out/l2z-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3a "$out" 1146 1799; then
    grep -hE '^(PASS|FAIL)\|l3a-baseline' "$out/l3a-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3a-baseline] 校验失败（见 l3a-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3a-baseline' "$out/l3a-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3b "$out" 1188 1869; then
    grep -hE '^(PASS|FAIL)\|l3b-baseline' "$out/l3b-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3b-baseline] 校验失败（见 l3b-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3b-baseline' "$out/l3b-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3c "$out" 1230 1939; then
    grep -hE '^(PASS|FAIL)\|l3c-baseline' "$out/l3c-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3c-baseline] 校验失败（见 l3c-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3c-baseline' "$out/l3c-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3d "$out" 1272 2009; then
    grep -hE '^(PASS|FAIL)\|l3d-baseline' "$out/l3d-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3d-baseline] 校验失败（见 l3d-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3d-baseline' "$out/l3d-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3e "$out" 1314 2079; then
    grep -hE '^(PASS|FAIL)\|l3e-baseline' "$out/l3e-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3e-baseline] 校验失败（见 l3e-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3e-baseline' "$out/l3e-baseline.log" >> "$log" 2>/dev/null || true
  if ! verify_baseline l3f "$out" 1356 2149; then
    grep -hE '^(PASS|FAIL)\|l3f-baseline' "$out/l3f-baseline.log" >> "$log" 2>/dev/null || true
    echo "[l3f-baseline] 校验失败（见 l3f-baseline.log）；stdout-contract／cli-contract 判 FAIL" >> "$log"
    record stdout-contract FAIL 1 "$log"
    record cli-contract FAIL 1 "$log"
    return
  fi
  grep -hE '^(PASS|FAIL)\|l3f-baseline' "$out/l3f-baseline.log" >> "$log" 2>/dev/null || true
  local rc=0
  CC="${CC:-cc}" ZORC_BIN="$ZORC" OUT_DIR="$out" \
    bash tests/stdout/run_stdout_contract.sh --strict >>"$log" 2>&1 || rc=$?
  # D-L2c-16：把 QA 两段判词（含 ran 计数）回显到 job 日志，便于直接核对执行数。
  grep -E '^(PASS|FAIL)\|(stdout|cli)-contract' "$log" || true
  local so_v cli_v so_rc=0 cli_rc=0
  so_v="$(stdout_section_verdict stdout-contract "$log" "$rc")"
  cli_v="$(stdout_section_verdict cli-contract "$log" "$rc")"
  # L3f CI-01：冻结计数 726／405／17（P=24／Q=18／Ret=20）；判定 PASS 但判词计数不符（如旧资产 702／407／17）同样 FAIL，防静默洗绿。
  if [ "$so_v" = PASS ] && ! grep -Fq 'PASS|stdout-contract|runtime=726 negative=405 fail=0 ran=726/405' "$log"; then
    echo "[stdout-contract] 计数不符：期望 runtime=726 negative=405 ran=726/405（L3f 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$cli_v" = PASS ] && ! grep -Fq 'PASS|cli-contract|cases=17 fail=0 ran=17' "$log"; then
    echo "[cli-contract] 计数不符：期望 cases=17 ran=17（L3f 冻结，CLI 不变）" >> "$log"
    cli_v=FAIL
  fi
  # L3f CI-01：二十八基线精确判词（含 fail=0）；计数不符同样 FAIL。
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2e-baseline|cases=162 files=209 unique=209 fail=0' "$out/l2e-baseline.log"; then
    echo "[l2e-baseline] 计数不符：期望 cases=162 files=209 unique=209（历史冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2f-baseline|cases=188 files=246 unique=246 fail=0' "$out/l2f-baseline.log"; then
    echo "[l2f-baseline] 计数不符：期望 cases=188 files=246 unique=246（L2f 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2g-baseline|cases=206 files=275 unique=275 fail=0' "$out/l2g-baseline.log"; then
    echo "[l2g-baseline] 计数不符：期望 cases=206 files=275 unique=275（L2g 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2h-baseline|cases=247 files=327 unique=327 fail=0' "$out/l2h-baseline.log"; then
    echo "[l2h-baseline] 计数不符：期望 cases=247 files=327 unique=327（L2h 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2i-baseline|cases=297 files=398 unique=398 fail=0' "$out/l2i-baseline.log"; then
    echo "[l2i-baseline] 计数不符：期望 cases=297 files=398 unique=398（L2i 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2j-baseline|cases=345 files=466 unique=466 fail=0' "$out/l2j-baseline.log"; then
    echo "[l2j-baseline] 计数不符：期望 cases=345 files=466 unique=466（L2j 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2k-baseline|cases=417 files=590 unique=590 fail=0' "$out/l2k-baseline.log"; then
    echo "[l2k-baseline] 计数不符：期望 cases=417 files=590 unique=590（L2k 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2l-baseline|cases=461 files=660 unique=660 fail=0' "$out/l2l-baseline.log"; then
    echo "[l2l-baseline] 计数不符：期望 cases=461 files=660 unique=660（L2l 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2m-baseline|cases=506 files=734 unique=734 fail=0' "$out/l2m-baseline.log"; then
    echo "[l2m-baseline] 计数不符：期望 cases=506 files=734 unique=734（L2m 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2n-baseline|cases=554 files=816 unique=816 fail=0' "$out/l2n-baseline.log"; then
    echo "[l2n-baseline] 计数不符：期望 cases=554 files=816 unique=816（L2n 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2o-baseline|cases=599 files=891 unique=891 fail=0' "$out/l2o-baseline.log"; then
    echo "[l2o-baseline] 计数不符：期望 cases=599 files=891 unique=891（L2o 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2p-baseline|cases=645 files=963 unique=963 fail=0' "$out/l2p-baseline.log"; then
    echo "[l2p-baseline] 计数不符：期望 cases=645 files=963 unique=963（L2p 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2q-baseline|cases=701 files=1062 unique=1062 fail=0' "$out/l2q-baseline.log"; then
    echo "[l2q-baseline] 计数不符：期望 cases=701 files=1062 unique=1062（L2q 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2r-baseline|cases=759 files=1163 unique=1163 fail=0' "$out/l2r-baseline.log"; then
    echo "[l2r-baseline] 计数不符：期望 cases=759 files=1163 unique=1163（L2r 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2s-baseline|cases=827 files=1274 unique=1274 fail=0' "$out/l2s-baseline.log"; then
    echo "[l2s-baseline] 计数不符：期望 cases=827 files=1274 unique=1274（L2s 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2t-baseline|cases=884 files=1367 unique=1367 fail=0' "$out/l2t-baseline.log"; then
    echo "[l2t-baseline] 计数不符：期望 cases=884 files=1367 unique=1367（L2t 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2u-baseline|cases=894 files=1379 unique=1379 fail=0' "$out/l2u-baseline.log"; then
    echo "[l2u-baseline] 计数不符：期望 cases=894 files=1379 unique=1379（L2u 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2v-baseline|cases=937 files=1450 unique=1450 fail=0' "$out/l2v-baseline.log"; then
    echo "[l2v-baseline] 计数不符：期望 cases=937 files=1450 unique=1450（L2v 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2w-baseline|cases=979 files=1520 unique=1520 fail=0' "$out/l2w-baseline.log"; then
    echo "[l2w-baseline] 计数不符：期望 cases=979 files=1520 unique=1520（L2w 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2x-baseline|cases=1021 files=1590 unique=1590 fail=0' "$out/l2x-baseline.log"; then
    echo "[l2x-baseline] 计数不符：期望 cases=1021 files=1590 unique=1590（L2x 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2y-baseline|cases=1062 files=1659 unique=1659 fail=0' "$out/l2y-baseline.log"; then
    echo "[l2y-baseline] 计数不符：期望 cases=1062 files=1659 unique=1659（L2y 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l2z-baseline|cases=1104 files=1729 unique=1729 fail=0' "$out/l2z-baseline.log"; then
    echo "[l2z-baseline] 计数不符：期望 cases=1104 files=1729 unique=1729（L2z 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3a-baseline|cases=1146 files=1799 unique=1799 fail=0' "$out/l3a-baseline.log"; then
    echo "[l3a-baseline] 计数不符：期望 cases=1146 files=1799 unique=1799（L3a 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3b-baseline|cases=1188 files=1869 unique=1869 fail=0' "$out/l3b-baseline.log"; then
    echo "[l3b-baseline] 计数不符：期望 cases=1188 files=1869 unique=1869（L3b 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3c-baseline|cases=1230 files=1939 unique=1939 fail=0' "$out/l3c-baseline.log"; then
    echo "[l3c-baseline] 计数不符：期望 cases=1230 files=1939 unique=1939（L3c 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3d-baseline|cases=1272 files=2009 unique=2009 fail=0' "$out/l3d-baseline.log"; then
    echo "[l3d-baseline] 计数不符：期望 cases=1272 files=2009 unique=2009（L3d 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3e-baseline|cases=1314 files=2079 unique=2079 fail=0' "$out/l3e-baseline.log"; then
    echo "[l3e-baseline] 计数不符：期望 cases=1314 files=2079 unique=2079（L3e 冻结）" >> "$log"
    so_v=FAIL
  fi
  if [ "$so_v" = PASS ] && ! grep -Fxq 'PASS|l3f-baseline|cases=1356 files=2149 unique=2149 fail=0' "$out/l3f-baseline.log"; then
    echo "[l3f-baseline] 计数不符：期望 cases=1356 files=2149 unique=2149（L3f 冻结）" >> "$log"
    so_v=FAIL
  fi
  [ "$so_v" = FAIL ] && so_rc=1
  [ "$cli_v" = FAIL ] && cli_rc=1
  echo "[stdout-contract] 判定=$so_v rc=$rc" >> "$log"
  echo "[cli-contract] 判定=$cli_v rc=$rc" >> "$log"
  record stdout-contract "$so_v" "$so_rc" "$log"
  record cli-contract "$cli_v" "$cli_rc" "$log"
}

stage_packaging_smoke() { # L2d CI-01：独立 Linux job；直调 QA 安装树脚本；单段 summary；fail-closed
  local log; log="$(new_log packaging-smoke)"
  local out="$LOG_DIR/packaging-smoke"
  mkdir -p "$out"
  if [ -z "$ZORC" ]; then
    echo "zorcc 未安装：packaging-smoke 判 FAIL（不可 SKIP 洗绿）" > "$log"
    record packaging-smoke FAIL 1 "$log"
    return
  fi
  if [ ! -f tests/packaging/run_packaging_smoke.sh ]; then
    echo "packaging-smoke 判据脚本缺失：tests/packaging/run_packaging_smoke.sh（QA-L2d-02 未合入）" > "$log"
    record packaging-smoke FAIL 1 "$log"
    return
  fi
  local rc=0
  CC="${CC:-cc}" ZORC_BIN="$ZORC" OUT_DIR="$out" ZOR_INSTALLER="$REPO_ROOT/ci/install.sh" \
    bash tests/packaging/run_packaging_smoke.sh --strict >>"$log" 2>&1 || rc=$?
  grep -E '^(PASS|FAIL)\|packaging-smoke' "$log" || true
  # L2d 冻结：显式 FAIL 优先；必须 rc=0 且判词为 cases=4 fail=0 ran=4；缺判词／计数不符／rc≠0 一律 FAIL。
  local v=FAIL
  if ! grep -q '^FAIL|packaging-smoke|' "$log" 2>/dev/null \
     && [ "$rc" -eq 0 ] \
     && grep -Fq 'PASS|packaging-smoke|cases=4 fail=0 ran=4' "$log"; then
    v=PASS
  elif [ "$rc" -eq 0 ] && ! grep -q '^FAIL|packaging-smoke|' "$log" 2>/dev/null; then
    echo "[packaging-smoke] 缺少冻结判词或计数不符：期望 cases=4 fail=0 ran=4（L2d 冻结）" >> "$log"
  fi
  local prc=0; [ "$v" = FAIL ] && prc=1
  echo "[packaging-smoke] 判定=$v rc=$rc" >> "$log"
  record packaging-smoke "$v" "$prc" "$log"
}

if [ "$WITH_STDOUT_CONTRACT" = 1 ]; then
  stage_stdout_contract
  {
    echo "--- stage summary ($RUN_ID) ---"
    for s in "${summary[@]}"; do echo "$s"; done
  } | tee "$LOG_DIR/summary.txt"
  echo "[ci] 证据目录: $LOG_DIR"
  exit "$overall"
fi

if [ "$WITH_PACKAGING_SMOKE" = 1 ]; then
  stage_packaging_smoke
  {
    echo "--- stage summary ($RUN_ID) ---"
    for s in "${summary[@]}"; do echo "$s"; done
  } | tee "$LOG_DIR/summary.txt"
  echo "[ci] 证据目录: $LOG_DIR"
  exit "$overall"
fi

if [ "$WITH_RUNTIME_CODES" = 1 ]; then
  stage_runtime_codes
  {
    echo "--- stage summary ($RUN_ID) ---"
    for s in "${summary[@]}"; do echo "$s"; done
  } | tee "$LOG_DIR/summary.txt"
  echo "[ci] 证据目录: $LOG_DIR"
  exit "$overall"
fi

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
