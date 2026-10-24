#!/usr/bin/env bash
# ci/asan/run_asan_l2k.sh — L2k ASAN／LSAN 补充门禁（S2 独立脚本）
#
# 归属：ZOR工具链与开发工程师（接线／脚本）＋ZOR质量保障（判据口径）。
# 依据：L2k 范围冻结件 v1（ff7259b9…d2db）、QA 计数 v1.2（24ca7f3c…1922）、
#       L2k CI 影响工具链 v1.1（第七基线 417／590，ASAN K=6，δ=1）。
#
# 性质：既有 asan-linux job 内的补充判据；**不改** tests/runtime/manifest.tsv（48／41／7）
#       与 ci/asan/run_asan.sh，不新增 job／workflow YAML 结构／ruleset。
#
# 判据（任一不满足＝FAIL，strict 不 SKIP）：
#   1. manifest 恰 6 行；七列 schema；无 CR／空行；case_id／source_path 唯一；
#      mode 仅 build-run；expect_rc=0；source 为 tests/ 下普通相对文件（非 symlink）。
#   2. expect_stdout_sha256 必须是 64hex；空值判 FAIL reason=expected_pending
#      （Stage-1 尚未捕获 expected，fail-closed；禁止占位哈希／静默通过）。
#   3. 两轮 ASAN／LSAN 构建＋运行：build rc=0；二进制含 .debug_info；run rc=0；
#      stdout sha 匹配；stderr 0 B；无 AddressSanitizer／LeakSanitizer 报错。
#   4. UAF 探针必红、LSAN 泄漏探针必红、去 -g 对照必无 .debug_info。
#   5. 同一宿主两轮逐条一致；宿主仅 Linux（validate-only 可在 macOS 预检）。
#
# 用法：
#   bash ci/asan/run_asan_l2k.sh --strict [--out <dir>] [--manifest <path>] [--zorc <path>]
#   bash ci/asan/run_asan_l2k.sh --validate-only [--manifest <path>]
#
# 环境：
#   ZORC_BIN      zorcc 路径（默认 PATH 中 zorcc）
#   CC_REAL       wrapper 实际调用的 C 编译器（默认 cc；CI 钉 clang-18）
#   ASAN_TIMEOUT  单次运行超时秒数（默认 30）
#
# 输出契约：
#   PASS|asan-l2k|validate-only cases=6 static=ok
#   PASS|asan-l2k|cases=6 fail=0 ran=6/6
#   FAIL|asan-l2k|...（显式失败优先）

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO_ROOT_REAL="$(cd "$REPO_ROOT" && pwd -P)"
DEFAULT_MANIFEST="$REPO_ROOT/tests/runtime/l2k-asan-manifest.tsv"
EXPECT_ROWS=6
EXPECT_RUN=6
MANIFEST="$DEFAULT_MANIFEST"
ZORC="${ZORC_BIN:-zorcc}"
OUT_DIR=""
VALIDATE_ONLY=0
ASAN_TIMEOUT="${ASAN_TIMEOUT:-30}"

while [ $# -gt 0 ]; do
  case "$1" in
    --strict) shift ;;
    --validate-only) VALIDATE_ONLY=1; shift ;;
    --manifest)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan-l2k] --manifest 缺值" >&2; exit 2; }
      MANIFEST="$2"; shift 2 ;;
    --zorc)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan-l2k] --zorc 缺值" >&2; exit 2; }
      ZORC="$2"; shift 2 ;;
    --out)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan-l2k] --out 缺值" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,34p' "$0"; exit 0 ;;
    *)
      echo "[asan-l2k] 未知参数 '$1'（可用: --strict --validate-only --manifest --out --zorc）" >&2
      exit 2 ;;
  esac
done

case "$MANIFEST" in
  /*) ;;
  *) MANIFEST="$REPO_ROOT/$MANIFEST" ;;
esac

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    return 1
  fi
}
utc_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

fail=0; fail_msgs=(); pending_ids=(); run_ids=0
note_fail() { fail=1; fail_msgs+=("$1"); }

if [ "$VALIDATE_ONLY" != 1 ]; then
  HOST_OS="$(uname -s)"
  if [ "$HOST_OS" != "Linux" ]; then
    echo "[asan-l2k] HOST_UNSUPPORTED os=${HOST_OS}：ASAN／LSAN 只认 Linux 宿主；macOS 不得作泄漏证据" >&2
    exit 2
  fi
elif [ "$(uname -s)" != "Linux" ]; then
  echo "[asan-l2k] WARN: 非 Linux 仅做 manifest 静态预检（--validate-only）" >&2
fi

# ---------- manifest 校验 ----------
expected_header=$'case_id\tsource_path\texpect_rc\tmode\texpect_stdout_sha256\tkind\tnotes'
if [ ! -f "$MANIFEST" ]; then
  note_fail "manifest 缺失：$MANIFEST"
else
  IFS= read -r header < "$MANIFEST" || header=""
  [ "$header" = "$expected_header" ] || note_fail "manifest 表头不精确（须七列制表符分隔）"
  grep -q $'\r' "$MANIFEST" && note_fail "manifest 含 CR（须 LF）"
  [ -z "$(tail -c 1 "$MANIFEST")" ] || note_fail "manifest 末尾缺 LF"
fi

n_rows=0; n_run=0; n_empty=0
if [ "$fail" -eq 0 ]; then
  n_rows=$(tail -n +2 "$MANIFEST" | wc -l | tr -d ' ')
  n_empty=$(tail -n +2 "$MANIFEST" | grep -c '^$' || true)
  n_run=$(awk -F'\t' '$4=="build-run"' "$MANIFEST" | wc -l | tr -d ' ')
  [ "$n_rows" -eq "$EXPECT_ROWS" ] || note_fail "数据行=$n_rows（期望 $EXPECT_ROWS）"
  [ "$n_empty" -eq 0 ] || note_fail "含空行=$n_empty（禁止）"
  [ "$n_run" -eq "$EXPECT_RUN" ] || note_fail "build-run=$n_run（期望 $EXPECT_RUN）"

  line_no=1
  while IFS= read -r row; do
    line_no=$((line_no+1))
    nf=$(printf '%s\n' "$row" | awk -F'\t' '{print NF}')
    [ "$nf" -eq 7 ] || note_fail "L${line_no}: 列数=$nf（期望 7）"
    case_id=$(printf '%s\n' "$row" | cut -f1)
    src=$(printf '%s\n' "$row" | cut -f2)
    expect_rc=$(printf '%s\n' "$row" | cut -f3)
    mode=$(printf '%s\n' "$row" | cut -f4)
    sha=$(printf '%s\n' "$row" | cut -f5)
    kind=$(printf '%s\n' "$row" | cut -f6)
    notes=$(printf '%s\n' "$row" | cut -f7)
    [ -n "$case_id" ] || note_fail "L${line_no}: case_id 为空"
    [ -n "$src" ] || note_fail "L${line_no}: source_path 为空"
    [ -n "$notes" ] || note_fail "L${line_no}: notes 为空"
    case "$case_id" in *[[:space:]]*) note_fail "L${line_no}: case_id 含空白";; esac
    case "$src" in
      /*) note_fail "L${line_no}: source_path 为绝对路径：$src" ;;
      tests/*) ;;
      *) note_fail "L${line_no}: source_path 不在 tests/ 下：$src" ;;
    esac
    case "$src" in *..*) note_fail "L${line_no}: source_path 含 '..'：$src" ;; esac
    case "$mode" in build-run) ;; *) note_fail "L${line_no}: mode 非法（仅 build-run）：$mode" ;; esac
    case "$kind" in positive|fixture|control) ;; *) note_fail "L${line_no}: kind 非法：$kind" ;; esac
    [ "$expect_rc" = "0" ] || note_fail "L${line_no}: expect_rc=$expect_rc（期望 0）"
    if [ -z "$sha" ]; then
      pending_ids+=("$case_id")
    else
      printf '%s' "$sha" | grep -Eq '^[0-9a-f]{64}$' || note_fail "L${line_no}: stdout sha 非法（64hex）"
    fi
    full="$REPO_ROOT/$src"
    if [ ! -f "$full" ]; then note_fail "L${line_no}: source_path 不存在：$src"; continue; fi
    [ -L "$full" ] && note_fail "L${line_no}: source_path 为 symlink：$src"
    rp="$(cd "$(dirname "$full")" && pwd -P)/$(basename "$full")"
    case "$rp" in "$REPO_ROOT_REAL"/*) ;; *) note_fail "L${line_no}: source_path 逃出仓库：$src" ;; esac
  done < <(tail -n +2 "$MANIFEST")

  dup_ids=$(tail -n +2 "$MANIFEST" | cut -f1 | sort | uniq -d)
  [ -z "$dup_ids" ] || note_fail "case_id 重复：$dup_ids"
  dup_paths=$(tail -n +2 "$MANIFEST" | cut -f2 | sort | uniq -d)
  [ -z "$dup_paths" ] || note_fail "source_path 重复：$dup_paths"
fi

if [ "${#pending_ids[@]}" -gt 0 ]; then
  note_fail "expected_pending=${#pending_ids[@]}（${pending_ids[*]}；Stage-2 捕获后填 sha）"
fi

if [ "$VALIDATE_ONLY" = 1 ]; then
  if [ "$fail" -eq 0 ]; then
    echo "PASS|asan-l2k|validate-only cases=${n_rows} static=ok"
    exit 0
  fi
  echo "[asan-l2k] STATIC_FAIL: ${fail_msgs[*]}" >&2
  echo "FAIL|asan-l2k|validate-only ${fail_msgs[*]}"
  exit 1
fi

# ---------- 环境 ----------
WRAPPER_ASAN="$REPO_ROOT/ci/asan/cc-asan"
WRAPPER_NOG="$REPO_ROOT/ci/asan/cc-nog"
UAF_PROBE="$REPO_ROOT/tests/runtime/controls/asan_uaf_probe.c"
LEAK_PROBE="$REPO_ROOT/tests/runtime/controls/asan_leak_probe.c"
[ -f "$WRAPPER_ASAN" ] && [ -x "$WRAPPER_ASAN" ] || note_fail "cc-asan 不可执行：$WRAPPER_ASAN"
[ -f "$WRAPPER_NOG" ] && [ -x "$WRAPPER_NOG" ] || note_fail "cc-nog 不可执行：$WRAPPER_NOG"
[ -f "$UAF_PROBE" ] || note_fail "缺 UAF 探针：$UAF_PROBE"
[ -f "$LEAK_PROBE" ] || note_fail "缺泄漏探针：$LEAK_PROBE"
command -v readelf >/dev/null 2>&1 || note_fail "readelf 缺失（strict 不得 SKIP）"
command -v timeout >/dev/null 2>&1 || note_fail "timeout 缺失（strict 不得 SKIP）"
REAL_CC="${CC_REAL:-cc}"
command -v "$REAL_CC" >/dev/null 2>&1 || note_fail "C 编译器缺失：$REAL_CC"
export CC_REAL="$REAL_CC"

if [ -n "$ZORC" ] && printf '%s' "$ZORC" | grep -q '/'; then
  ZORC_ABS="$(cd "$(dirname "$ZORC")" && pwd -P)/$(basename "$ZORC")"
else
  ZORC_ABS="$(command -v "$ZORC" 2>/dev/null || true)"
fi
[ -n "$ZORC_ABS" ] && [ -x "$ZORC_ABS" ] || note_fail "zorcc 不可执行：$ZORC"

if [ "$fail" -ne 0 ]; then
  echo "[asan-l2k] 环境／manifest 前置失败，未执行构建" >&2
  echo "FAIL|asan-l2k|${fail_msgs[*]}"
  exit 1
fi

if [ -z "$OUT_DIR" ]; then
  OUT_DIR="$REPO_ROOT/ci/logs/asan-l2k-$(date -u +%Y%m%dT%H%M%SZ)-$$"
fi
mkdir -p "$OUT_DIR/round1" "$OUT_DIR/round2" "$OUT_DIR/probes"
cp "$MANIFEST" "$OUT_DIR/manifest.tsv"
sha256_of "$MANIFEST" > "$OUT_DIR/manifest.sha256" || note_fail "无法计算 manifest sha256"

: > "$OUT_DIR/commands.txt"
{
  echo "utc_start=$(utc_now)"
  echo "host=$(uname -srm)"
  echo "repo=$REPO_ROOT_REAL"
  echo "head=$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo NA)"
  echo "zorcc=$ZORC_ABS"
  echo "manifest=$MANIFEST"
  echo "manifest_sha256=$(cat "$OUT_DIR/manifest.sha256" 2>/dev/null || echo NA)"
  echo "CC_REAL=$REAL_CC"
  echo "cases=${n_rows} run_cases=${n_run}"
  echo "ASAN_OPTIONS=halt_on_error=1:detect_leaks=1"
  echo "--- versions ---"
  "$ZORC_ABS" --version 2>&1 | head -1 || true
  "$REAL_CC" --version 2>&1 | head -1 || true
  readelf --version 2>&1 | head -1 || true
} > "$OUT_DIR/versions.txt"

debug_info_present() { readelf -S "$1" 2>/dev/null | grep -q '\.debug_info'; }

run_case() { # <round_dir> <case_id> <src> <expect_sha> <expect_rc>
  local rd="$1" id="$2" src="$3" expect_sha="$4" expect_rc="$5"
  local bin="$rd/$id.bin" log="$rd/$id" sumf="$rd/summary.tsv" vf="$rd/asan-verdict.tsv"
  local build_rc=0 debug="absent" run_rc="NA" out_sha="NA" err_bytes="NA" verdict="FAIL" san="none"
  [ -f "$sumf" ] || printf 'case_id\tbuild_rc\tdebug_info\trun_rc\tstdout_sha256\tstderr_bytes\tasan_verdict\tverdict\tnotes\n' > "$sumf"
  printf 'CC=%s %s %s -O0 -o %s\n' "$WRAPPER_ASAN" "$ZORC_ABS" "$src" "$bin" >> "$OUT_DIR/commands.txt"
  ( cd "$REPO_ROOT" && CC="$WRAPPER_ASAN" "$ZORC_ABS" "$src" -O0 -o "$bin" ) > "$log.build.out" 2> "$log.build.err"
  build_rc=$?
  if [ "$build_rc" -eq 0 ] && [ -x "$bin" ]; then
    debug_info_present "$bin" && debug="present"
    readelf -S "$bin" > "$rd/readelf-$id.txt" 2>&1 || true
    ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$bin" > "$log.out" 2> "$log.err"
    run_rc=$?
    out_sha="$(sha256_of "$log.out" || echo NA)"
    err_bytes="$(wc -c < "$log.err" | tr -d ' ')"
    if grep -q 'ERROR: AddressSanitizer' "$log.err"; then san="asan-error"
    elif grep -q 'ERROR: LeakSanitizer' "$log.err"; then san="lsan-error"; fi
    if [ "$run_rc" -eq "$expect_rc" ] && [ "$out_sha" = "$expect_sha" ] \
       && [ "$err_bytes" -eq 0 ] && [ "$san" = "none" ] && [ "$debug" = "present" ]; then
      verdict="PASS"
    fi
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$id" "$build_rc" "$debug" "$run_rc" "$out_sha" "$err_bytes" "$san" "$verdict" "asan-l2k-build+run" >> "$sumf"
  printf '%s\tbuild_rc=%s\tdebug_info=%s\trun_rc=%s\tstdout_sha=%s\tstderr_bytes=%s\tsan=%s\tverdict=%s\n' \
    "$id" "$build_rc" "$debug" "$run_rc" "$out_sha" "$err_bytes" "$san" "$verdict" >> "$vf"
  [ "$verdict" = "PASS" ] || note_fail "case ${id}: build_rc=${build_rc} debug=${debug} run_rc=${run_rc} stdout_sha=${out_sha} stderr=${err_bytes} san=${san}"
}

while IFS=$'\t' read -r case_id src expect_rc sha; do
  run_ids=$((run_ids+1))
  run_case "$OUT_DIR/round1" "$case_id" "$src" "$sha" "$expect_rc"
done < <(awk -F'\t' '$4=="build-run"{print $1"\t"$2"\t"$3"\t"$5}' "$MANIFEST")

while IFS=$'\t' read -r case_id src expect_rc sha; do
  run_case "$OUT_DIR/round2" "$case_id" "$src" "$sha" "$expect_rc" >/dev/null 2>&1
done < <(awk -F'\t' '$4=="build-run"{print $1"\t"$2"\t"$3"\t"$5}' "$MANIFEST")

cp "$OUT_DIR/round1/summary.tsv" "$OUT_DIR/summary.tsv"
cp "$OUT_DIR/round1/asan-verdict.tsv" "$OUT_DIR/asan-verdict.tsv"
tail -n +2 "$OUT_DIR/round1/summary.tsv" | sort > "$OUT_DIR/round1.compare"
tail -n +2 "$OUT_DIR/round2/summary.tsv" | sort > "$OUT_DIR/round2.compare"
if ! diff -q "$OUT_DIR/round1.compare" "$OUT_DIR/round2.compare" >/dev/null; then
  note_fail "两轮复算不一致（rc／stdout sha／stderr 字节／sanitizer 结论）"
  diff "$OUT_DIR/round1.compare" "$OUT_DIR/round2.compare" | head -20 > "$OUT_DIR/round2.diff" || true
fi
[ "$run_ids" -eq "$EXPECT_RUN" ] || note_fail "实际执行数=$run_ids（期望 $EXPECT_RUN）"

# ---------- 探针：UAF 必红、LSAN 泄漏必红、去 -g 必无 .debug_info ----------
: > "$OUT_DIR/probes.log"
uaf_bin="$OUT_DIR/probes/uaf_probe"
if "$WRAPPER_ASAN" -O0 -o "$uaf_bin" "$UAF_PROBE" > "$OUT_DIR/probes/uaf.build.out" 2> "$OUT_DIR/probes/uaf.build.err"; then
  ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$uaf_bin" > "$OUT_DIR/probes/uaf.out" 2> "$OUT_DIR/probes/uaf.err"
  uaf_rc=$?
  if [ "$uaf_rc" -ne 0 ] && grep -q 'ERROR: AddressSanitizer' "$OUT_DIR/probes/uaf.err"; then
    echo "UAF_PROBE=PASS rc=$uaf_rc AddressSanitizer=hit" >> "$OUT_DIR/probes.log"
  else
    echo "UAF_PROBE=FAIL rc=$uaf_rc" >> "$OUT_DIR/probes.log"
    note_fail "UAF 探针静默（ASAN 探测器死）"
  fi
else
  echo "UAF_PROBE=FAIL 构建失败" >> "$OUT_DIR/probes.log"
  note_fail "UAF 探针构建失败"
fi

leak_bin="$OUT_DIR/probes/leak_probe"
if "$WRAPPER_ASAN" -O0 -o "$leak_bin" "$LEAK_PROBE" > "$OUT_DIR/probes/leak.build.out" 2> "$OUT_DIR/probes/leak.build.err"; then
  ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$leak_bin" > "$OUT_DIR/probes/leak.out" 2> "$OUT_DIR/probes/leak.err"
  leak_rc=$?
  if [ "$leak_rc" -ne 0 ] && grep -q 'ERROR: LeakSanitizer' "$OUT_DIR/probes/leak.err"; then
    echo "LEAK_PROBE=PASS rc=$leak_rc LeakSanitizer=hit" >> "$OUT_DIR/probes.log"
  else
    echo "LEAK_PROBE=FAIL rc=$leak_rc" >> "$OUT_DIR/probes.log"
    note_fail "LSAN 泄漏探针静默（泄漏检测死）"
  fi
else
  echo "LEAK_PROBE=FAIL 构建失败" >> "$OUT_DIR/probes.log"
  note_fail "泄漏探针构建失败"
fi

nog_bin="$OUT_DIR/probes/hello_nog"
( cd "$REPO_ROOT" && CC="$WRAPPER_NOG" "$ZORC_ABS" examples/hello.zor -O0 -o "$nog_bin" ) \
  > "$OUT_DIR/probes/nog.build.out" 2> "$OUT_DIR/probes/nog.build.err"
nog_build_rc=$?
if [ "$nog_build_rc" -eq 0 ] && [ -x "$nog_bin" ]; then
  if debug_info_present "$nog_bin"; then
    echo "NOG_CONTROL=FAIL（无 -g 仍含 .debug_info）" >> "$OUT_DIR/probes.log"
    note_fail "去 -g 对照未被判 FAIL（debug-info 检查失明）"
  else
    echo "NOG_CONTROL=PASS（无 -g 无 .debug_info）" >> "$OUT_DIR/probes.log"
  fi
else
  echo "NOG_CONTROL=FAIL 构建失败 rc=$nog_build_rc" >> "$OUT_DIR/probes.log"
  note_fail "去 -g 对照构建失败"
fi

# ---------- 收尾 ----------
{
  echo "utc_end=$(utc_now)"
  echo "cases=${n_rows} run_cases=${n_run}"
  echo "manifest_sha256=$(cat "$OUT_DIR/manifest.sha256" 2>/dev/null || echo NA)"
  echo "verdict=$([ "$fail" -eq 0 ] && echo PASS || echo FAIL)"
} >> "$OUT_DIR/versions.txt"

if [ "$fail" -eq 0 ]; then
  echo "[asan-l2k] PASS（cases=${n_rows}；ASAN/LSAN 零报告；探针与去-g 对照按期望）"
  echo "[asan-l2k] 证据：$OUT_DIR"
  echo "PASS|asan-l2k|cases=${n_rows} fail=0 ran=${run_ids}/${EXPECT_RUN}"
  exit 0
fi
echo "[asan-l2k] FAIL：${fail_msgs[*]}" >&2
echo "FAIL|asan-l2k|${fail_msgs[*]}"
exit 1
