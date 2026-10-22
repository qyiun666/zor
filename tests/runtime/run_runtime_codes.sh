#!/usr/bin/env bash
# tests/runtime/run_runtime_codes.sh — L2b runtime-codes 判据脚本（QA 资产）
#
# 归属：ZOR质量保障（判据／manifest／样本）；ZOR工具链与开发工程师负责
#       `ci/run_ci.sh --runtime-codes` 与 workflow job 接线。
# 依据：QA v1.3 §三～§五；工具链 v1 §2.4～§2.6；核心 v1.1 §八。
#
# 用法：
#   bash tests/runtime/run_runtime_codes.sh --strict [--out <dir>] [--zorc <path>]
#   bash tests/runtime/run_runtime_codes.sh --validate-only   # 只校验清单（可在 macOS 预检）
#
# 环境：
#   ZORC_BIN    zorcc 路径（默认 PATH 中 zorcc）
#   CC          真实 C 编译器（默认 cc；CI 注入 clang-18）
#   OUT_DIR     证据目录（默认 ci/logs/runtime-codes-<UTC>-<pid>）
#
# 判据（任一不满足＝FAIL，strict 不 SKIP）：
#   - runtime-codes.tsv 恰 29 行；width-failclosed.tsv 恰 6 行；cli-cases.tsv 恰 8 行。
#   - 每行 schema／取值／路径／expected 一致；禁空行、注释、重复、绝对路径、`..`、symlink。
#   - 29 条 runtime：两轮构建 rc=0、运行 rc 严格 101/0、stdout sha、stderr 逐字节；
#     终止样本恰 1 行＋1 LF；无 sanitizer／zorcc 前缀；u8/u32 生成 C 宽度抽检。
#   - 6 条宽度负控：-O0/-O2 双档 rc=1、零产物、含 `zorcc: codegen:`；缺 codegen 不判 FAIL。
#   - 8 条 CLI 负控：非法 -Zoverflow rc=2；-Og/-O1/-O3/-Os 拒判；usage 不含 -Zoverflow。
#   - F-RC-1～14；宿主必须 Linux；rc=101 只在无 sanitizer 普通构建判。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO_ROOT_REAL="$(cd "$REPO_ROOT" && pwd -P)"
RUNTIME_TSV="$REPO_ROOT/tests/runtime/runtime-codes.tsv"
WIDTH_TSV="$REPO_ROOT/tests/runtime/width-failclosed.tsv"
CLI_TSV="$REPO_ROOT/tests/runtime/cli-cases.tsv"
CC_CAPTURE="$REPO_ROOT/tests/runtime/cc-capture"
ZORC="${ZORC_BIN:-zorcc}"
CC_REAL="${CC:-cc}"
OUT_DIR=""
VALIDATE_ONLY=0
STRICT=0
RUN_TIMEOUT="${RUNTIME_CODES_TIMEOUT:-30}"
BUILD_TIMEOUT="${RUNTIME_CODES_BUILD_TIMEOUT:-120}"

while [ $# -gt 0 ]; do
  case "$1" in
    --strict) STRICT=1; shift ;;
    --validate-only) VALIDATE_ONLY=1; shift ;;
    --out)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[runtime-codes] --out 缺值" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    --zorc)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[runtime-codes] --zorc 缺值" >&2; exit 2; }
      ZORC="$2"; shift 2 ;;
    -h|--help) sed -n '2,34p' "$0"; exit 0 ;;
    *) echo "[runtime-codes] 未知参数 '$1'" >&2; exit 2 ;;
  esac
done

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
EMPTY_SHA="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

fail=0
frc_fail() { # frc_fail <id> <msg>
  echo "[runtime-codes][F-RC-$1] FAIL: $2" >&2
  fail=1
}
safe_rel() { # safe_rel <path>
  case "$1" in
    ""|/*|*..*) return 1 ;;
  esac
  return 0
}
is_sha() { printf '%s' "$1" | grep -Eq '^[0-9a-f]{64}$'; }

# 精确 case_id 集合（防静默丢行／换行）
EXPECTED_RUNTIME="e3001-add-o0 e3001-sub-o0 e3001-mul-o0 e3001-min-div-neg1-o0 e3001-const-add-o0 e3002-div-zero-o0 e3002-mod-zero-o0 e3001-flush-o0 e3001-wrap-o2 e3001-wrap-default e3001-wrap-o0-explicit e3001-check-o2-explicit no-overflow-ops-o0 no-overflow-ops-o2 min-mod-neg1-o0 e3001-g-o0 e3001-g-o2 e3001-u8-add-o0 e3001-u8-sub-o0 e3001-u8-mul-o0 e3001-u8-wrap-o2 e3001-u8-wrap-o0-explicit e3001-u32-add-o0 e3001-u32-wrap-o2 e3002-u8-div-zero-o0 e3002-u32-mod-zero-o2 no-overflow-unsigned-o0 e3001-const-add-o2 e3002-const-div-zero-o2"
EXPECTED_WIDTH="width-i8-rejected width-i16-rejected width-i32-rejected width-u16-rejected width-u64-rejected width-byte-rejected"
EXPECTED_CLI="zoverflow-invalid zoverflow-empty zoverflow-check-mode og-rejected o1-rejected o3-rejected os-rejected usage-no-internal-flag"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/zor-runtime-codes.XXXXXX")" || exit 2
trap 'rm -rf "$TMP"' EXIT

check_case_set() { # check_case_set <tsv> <expected words> <label>
  local tsv="$1" expected="$2" label="$3"
  awk -F'\t' 'NR>1{print $1}' "$tsv" | LC_ALL=C sort > "$TMP/actual.ids"
  printf '%s\n' $expected | LC_ALL=C sort > "$TMP/expected.ids"
  if ! cmp -s "$TMP/actual.ids" "$TMP/expected.ids"; then
    frc_fail 4 "$label case_id 集合与冻结清单不符"
    diff -u "$TMP/expected.ids" "$TMP/actual.ids" >&2 || true
  fi
}

validate_runtime() {
  local header line_no nf cid src opt ov extra rc code sha kind notes
  local exp path ln col msg expected_file
  expected_file=''
  [ -f "$RUNTIME_TSV" ] || { frc_fail 4 "runtime-codes.tsv 缺失"; return; }
  IFS= read -r header < "$RUNTIME_TSV" || header=""
  [ "$header" = $'case_id\tsource_path\topt\toverflow\textra_flags\texpect_rc\texpect_code\texpect_stdout_sha256\tkind\tnotes' ] \
    || frc_fail 4 "runtime-codes.tsv 表头不精确（须十列）"
  grep -q $'\r' "$RUNTIME_TSV" && frc_fail 4 "runtime-codes.tsv 含 CR"
  [ -z "$(tail -c 1 "$RUNTIME_TSV")" ] || frc_fail 4 "runtime-codes.tsv 末尾缺 LF"
  [ "$(tail -n +2 "$RUNTIME_TSV" | wc -l | tr -d ' ')" -eq 29 ] || frc_fail 4 "runtime 数据行≠29"
  [ "$(tail -n +2 "$RUNTIME_TSV" | grep -c '^$' || true)" -eq 0 ] || frc_fail 4 "runtime 含空行"
  check_case_set "$RUNTIME_TSV" "$EXPECTED_RUNTIME" runtime

  line_no=1
  while IFS= read -r line; do
    line_no=$((line_no+1))
    nf=$(printf '%s\n' "$line" | awk -F'\t' '{print NF}')
    [ "$nf" -eq 10 ] || { frc_fail 4 "runtime L$line_no 列数=$nf"; continue; }
    cid=$(printf '%s\n' "$line" | cut -f1)
    src=$(printf '%s\n' "$line" | cut -f2)
    opt=$(printf '%s\n' "$line" | cut -f3)
    ov=$(printf '%s\n' "$line" | cut -f4)
    extra=$(printf '%s\n' "$line" | cut -f5)
    rc=$(printf '%s\n' "$line" | cut -f6)
    code=$(printf '%s\n' "$line" | cut -f7)
    sha=$(printf '%s\n' "$line" | cut -f8)
    kind=$(printf '%s\n' "$line" | cut -f9)
    notes=$(printf '%s\n' "$line" | cut -f10)
    [ -n "$cid" ] || frc_fail 4 "runtime L$line_no case_id 为空"
    [ -n "$notes" ] || frc_fail 4 "$cid: notes 为空"
    case "$opt" in O0|O2|default) ;; *) frc_fail 4 "$cid: opt=$opt" ;; esac
    case "$ov" in check|wrap|none) ;; *) frc_fail 4 "$cid: overflow=$ov" ;; esac
    case "$extra" in ""|-g) ;; *) frc_fail 4 "$cid: extra_flags=$extra" ;; esac
    case "$rc" in 101|0) ;; *) frc_fail 4 "$cid: expect_rc=$rc" ;; esac
    case "$code" in E3001|E3002|none) ;; *) frc_fail 4 "$cid: expect_code=$code" ;; esac
    case "$kind" in checked|wrap|runtime-error|control) ;; *) frc_fail 4 "$cid: kind=$kind" ;; esac
    is_sha "$sha" || frc_fail 4 "$cid: stdout sha 非 64 hex"
    if [ "$code" = none ]; then
      [ "$rc" = 0 ] || frc_fail 4 "$cid: expect_code=none 但 rc=$rc"
    else
      [ "$rc" = 101 ] || frc_fail 4 "$cid: expect_code=$code 但 rc=$rc"
    fi
    [ "$kind" = control ] && [ "$code" != none ] && frc_fail 4 "$cid: control 却 expect_code=$code"
    [ "$kind" = runtime-error ] && [ "$code" = none ] && frc_fail 4 "$cid: runtime-error 却 expect_code=none"
    [ "$kind" = wrap ] && [ "$code" != none ] && frc_fail 4 "$cid: wrap 却 expect_code=$code"
    [ "$opt" = default ] && [ "$ov" = check ] && frc_fail 4 "$cid: default+check 无映射"
    safe_rel "$src" || frc_fail 4 "$cid: source_path 不安全（$src）"
    [ -f "$REPO_ROOT/$src" ] || frc_fail 4 "$cid: source 不存在（$src）"
    [ -L "$REPO_ROOT/$src" ] && frc_fail 4 "$cid: source 是 symlink"
    if [ "$code" = none ]; then
      [ -e "$REPO_ROOT/tests/runtime/expected/$cid.stderr" ] && frc_fail 4 "$cid: expect_code=none 却有 expected 文件"
    else
      exp="$REPO_ROOT/tests/runtime/expected/$cid.stderr"
      [ -f "$exp" ] || { frc_fail 4 "$cid: expected 缺失"; continue; }
      [ "$(grep -c '^' "$exp" || true)" -eq 1 ] || frc_fail 4 "$cid: expected 行数≠1"
      [ -z "$(tail -c 1 "$exp")" ] || frc_fail 4 "$cid: expected 末尾缺 LF"
      path=$(sed -E 's/^zor: runtime error\[(E300[12])\] (.+):([0-9]+):([0-9]+): (integer overflow|division by zero)$/\2/' "$exp")
      ln=$(sed -E 's/^zor: runtime error\[(E300[12])\] (.+):([0-9]+):([0-9]+): (integer overflow|division by zero)$/\3/' "$exp")
      col=$(sed -E 's/^zor: runtime error\[(E300[12])\] (.+):([0-9]+):([0-9]+): (integer overflow|division by zero)$/\4/' "$exp")
      msg=$(sed -E 's/^zor: runtime error\[(E300[12])\] (.+):([0-9]+):([0-9]+): (integer overflow|division by zero)$/\5/' "$exp")
      [ "$path" = "$src" ] || frc_fail 4 "$cid: expected 形制／path 不符（$path）"
      case "$code:$msg" in
        E3001:integer\ overflow|E3002:division\ by\ zero) ;;
        *) frc_fail 4 "$cid: expected code/message 不符" ;;
      esac
      case "$ln:$col" in *[!0-9:]*|:*|*:) frc_fail 4 "$cid: expected line/col 非法" ;; esac
    fi
  done < <(tail -n +2 "$RUNTIME_TSV")
}

validate_width() {
  local header line_no nf cid src rc art inc exc notes
  [ -f "$WIDTH_TSV" ] || { frc_fail 4 "width-failclosed.tsv 缺失"; return; }
  IFS= read -r header < "$WIDTH_TSV" || header=""
  [ "$header" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] \
    || frc_fail 4 "width-failclosed.tsv 表头不精确"
  grep -q $'\r' "$WIDTH_TSV" && frc_fail 4 "width-failclosed.tsv 含 CR"
  [ -z "$(tail -c 1 "$WIDTH_TSV")" ] || frc_fail 4 "width-failclosed.tsv 末尾缺 LF"
  [ "$(tail -n +2 "$WIDTH_TSV" | wc -l | tr -d ' ')" -eq 6 ] || frc_fail 4 "width 数据行≠6"
  check_case_set "$WIDTH_TSV" "$EXPECTED_WIDTH" width
  line_no=1
  while IFS= read -r line; do
    line_no=$((line_no+1))
    nf=$(printf '%s\n' "$line" | awk -F'\t' '{print NF}')
    [ "$nf" -eq 7 ] || { frc_fail 4 "width L$line_no 列数=$nf"; continue; }
    cid=$(printf '%s\n' "$line" | cut -f1); src=$(printf '%s\n' "$line" | cut -f2)
    rc=$(printf '%s\n' "$line" | cut -f3); art=$(printf '%s\n' "$line" | cut -f4)
    inc=$(printf '%s\n' "$line" | cut -f5); exc=$(printf '%s\n' "$line" | cut -f6); notes=$(printf '%s\n' "$line" | cut -f7)
    [ "$rc" = 1 ] || frc_fail 4 "$cid: expect_rc=$rc（须 1）"
    [ "$art" = none ] || frc_fail 4 "$cid: expect_artifact=$art（须 none）"
    [ "$inc" = 'zorcc: codegen:' ] || frc_fail 4 "$cid: stderr_contains 非冻结句"
    [ "$exc" = 'zor: runtime error' ] || frc_fail 4 "$cid: stderr_not_contains 非冻结句"
    [ -n "$notes" ] || frc_fail 4 "$cid: notes 为空"
    safe_rel "$src" || frc_fail 4 "$cid: source_path 不安全"
    [ -f "$REPO_ROOT/$src" ] || frc_fail 4 "$cid: source 不存在"
    [ -L "$REPO_ROOT/$src" ] && frc_fail 4 "$cid: source 是 symlink"
  done < <(tail -n +2 "$WIDTH_TSV")
}

validate_cli() {
  local header line_no nf cid args rc art notc inc notes
  [ -f "$CLI_TSV" ] || { frc_fail 4 "cli-cases.tsv 缺失"; return; }
  IFS= read -r header < "$CLI_TSV" || header=""
  [ "$header" = $'case_id\targs\texpect_rc\texpect_artifact\texpect_stdout_not_contains\texpect_stderr_contains\tnotes' ] \
    || frc_fail 4 "cli-cases.tsv 表头不精确"
  grep -q $'\r' "$CLI_TSV" && frc_fail 4 "cli-cases.tsv 含 CR"
  [ -z "$(tail -c 1 "$CLI_TSV")" ] || frc_fail 4 "cli-cases.tsv 末尾缺 LF"
  [ "$(tail -n +2 "$CLI_TSV" | wc -l | tr -d ' ')" -eq 8 ] || frc_fail 4 "cli 数据行≠8"
  check_case_set "$CLI_TSV" "$EXPECTED_CLI" cli
  line_no=1
  while IFS= read -r line; do
    line_no=$((line_no+1))
    nf=$(printf '%s\n' "$line" | awk -F'\t' '{print NF}')
    [ "$nf" -eq 7 ] || { frc_fail 4 "cli L$line_no 列数=$nf"; continue; }
    cid=$(printf '%s\n' "$line" | cut -f1); args=$(printf '%s\n' "$line" | cut -f2)
    rc=$(printf '%s\n' "$line" | cut -f3); art=$(printf '%s\n' "$line" | cut -f4)
    notc=$(printf '%s\n' "$line" | cut -f5); inc=$(printf '%s\n' "$line" | cut -f6); notes=$(printf '%s\n' "$line" | cut -f7)
    case "$rc" in 0|2) ;; *) frc_fail 4 "$cid: expect_rc=$rc" ;; esac
    case "$art" in none|exists) ;; *) frc_fail 4 "$cid: expect_artifact=$art" ;; esac
    [ -n "$notc" ] || frc_fail 4 "$cid: stdout_not_contains 为空"
    [ -n "$inc" ] || frc_fail 4 "$cid: stderr_contains 为空"
    [ -n "$notes" ] || frc_fail 4 "$cid: notes 为空"
    case "$cid" in
      zoverflow-invalid|zoverflow-empty|zoverflow-check-mode|og-rejected|o1-rejected|o3-rejected|os-rejected) ;;
      usage-no-internal-flag) [ -z "$args" ] || frc_fail 4 "$cid: args 须空" ;;
    esac
    [ "$cid" = usage-no-internal-flag ] || [ -n "$args" ] || frc_fail 4 "$cid: args 为空"
  done < <(tail -n +2 "$CLI_TSV")
}

# ---- validate-only：清单／样本／expected 静态校验（可在 macOS 预检）----
validate_runtime
validate_width
validate_cli
if [ "$fail" -ne 0 ]; then
  echo "VALIDATE FAIL|runtime-codes|exit=1" >&2
  exit 1
fi
if [ "$VALIDATE_ONLY" = 1 ]; then
  echo "VALIDATE PASS|runtime-codes|runtime=29|width=6|cli=8"
  exit 0
fi

# ---- 完整运行：宿主／工具链 fail-closed ----
case "$(uname -s)" in
  Linux) ;;
  *) frc_fail 2 "宿主必须是 Linux（当前 $(uname -s)）；macOS 不得作 rc=101 终证" ;;
esac
command -v timeout >/dev/null 2>&1 || frc_fail 3 "缺少 timeout"
[ -n "$(command -v "$ZORC" 2>/dev/null || true)" ] && [ -x "$(command -v "$ZORC" 2>/dev/null || true)" ] \
  || [ -x "$ZORC" ] || frc_fail 1 "ZORC 缺失／不可执行：$ZORC"
CC_REAL_PATH="$(command -v "$CC_REAL" 2>/dev/null || true)"
[ -n "$CC_REAL_PATH" ] && [ -x "$CC_REAL_PATH" ] || frc_fail 5 "CC 缺失／不可执行：$CC_REAL"
[ -x "$CC_CAPTURE" ] || frc_fail 5 "CC capture wrapper 缺失／不可执行：$CC_CAPTURE"
[ -f "$REPO_ROOT/runtime/zor_rt.c" ] && [ -f "$REPO_ROOT/runtime/zor_rt.h" ] \
  || frc_fail 5 "helper runtime/zor_rt.c/.h 缺失"
if [ "$fail" -ne 0 ]; then
  echo "FAIL|runtime-codes|exit=1|reason=host_or_toolchain" >&2
  exit 1
fi
ZORC_PATH="$(command -v "$ZORC" 2>/dev/null || true)"; [ -n "$ZORC_PATH" ] || ZORC_PATH="$ZORC"

# helper 触达／E30xx 字面位置
helper_log="$TMP/helper.log"
{
  grep -Fq 'E3001' "$REPO_ROOT/runtime/zor_rt.c" && echo 'E3001 present'
  grep -Fq 'E3002' "$REPO_ROOT/runtime/zor_rt.c" && echo 'E3002 present'
  grep -Fq 'zor:' "$REPO_ROOT/runtime/zor_rt.c" && echo 'zor: present'
  grep -Fq 'fflush' "$REPO_ROOT/runtime/zor_rt.c" && echo 'fflush present'
  grep -Fq 'exit(101)' "$REPO_ROOT/runtime/zor_rt.c" && echo 'exit(101) present'
} > "$helper_log" 2>&1 || true
for pat in 'E3001' 'E3002' 'zor:' 'fflush' 'exit(101)'; do
  grep -Fq "$pat" "$REPO_ROOT/runtime/zor_rt.c" || frc_fail 5 "helper 缺 $pat"
done
grep -Fq 'abort(' "$REPO_ROOT/runtime/zor_rt.c" && frc_fail 5 "helper 含 abort("
grep -Fq '_exit(' "$REPO_ROOT/runtime/zor_rt.c" && frc_fail 5 "helper 含 _exit("
if grep -R -n -E 'E3001|E3002' "$REPO_ROOT/compiler/src" > "$TMP/compiler-e30xx.log" 2>&1; then
  frc_fail 5 "compiler/src/** 出现 E30xx 字面"
fi

# ---- 证据目录 ----
if [ -z "$OUT_DIR" ]; then OUT_DIR="$REPO_ROOT/ci/logs/runtime-codes-$(date -u +%Y%m%dT%H%M%SZ)-$$"; fi
mkdir -p "$OUT_DIR" || { echo "[runtime-codes] 无法创建证据目录 $OUT_DIR" >&2; exit 2; }
OUT_DIR="$(cd "$OUT_DIR" && pwd -P)"
mkdir -p "$OUT_DIR/round1" "$OUT_DIR/round2" "$OUT_DIR/generated-c"
: > "$OUT_DIR/commands.txt"; : > "$OUT_DIR/summary.tsv"
printf 'case_id\tround\tbuild_rc\trun_rc\tstdout_sha256\tstderr_sha256\tstderr_bytes\tlines\tcode\tverdict\n' > "$OUT_DIR/summary.tsv"
: > "$OUT_DIR/stdout.sha256"; : > "$OUT_DIR/stderr.sha256"
: > "$OUT_DIR/cli-controls.log"; : > "$OUT_DIR/width-controls.log"; : > "$OUT_DIR/generated-c.log"; : > "$OUT_DIR/runtime-codes.log"
cp "$RUNTIME_TSV" "$OUT_DIR/manifest.tsv"
sha256_of "$RUNTIME_TSV" > "$OUT_DIR/manifest.sha256" || frc_fail 4 "manifest sha 计算失败"
{
  echo "host=$(uname -s)"; echo "arch=$(uname -m)"; echo "kernel=$(uname -r)"
  echo "cc=$CC_REAL"; echo "cc_path=$CC_REAL_PATH"; echo "zorcc=$ZORC_PATH"
  echo "zorcc_version=$("$ZORC_PATH" --version 2>/dev/null || echo unknown)"
  echo "utc_start=$(utc_now)"; echo "flags=O0/O2/default,+check/wrap/none,+(-g)"
} > "$OUT_DIR/versions.txt"

flags_for() { # flags_for <opt> <overflow>
  case "$1/$2" in
    O0/check) printf '%s' '-O0' ;;
    O2/check) printf '%s' '-O2 -Zoverflow=check' ;;
    O0/wrap) printf '%s' '-O0 -Zoverflow=wrap' ;;
    O2/wrap) printf '%s' '-O2' ;;
    default/wrap) printf '%s' '' ;;
    O0/none) printf '%s' '-O0' ;;
    O2/none) printf '%s' '-O2' ;;
    default/none) printf '%s' '' ;;
    *) return 1 ;;
  esac
}

record_summary() { # case round build run outsha errsha errbytes lines code verdict
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" "${10}" >> "$OUT_DIR/summary.tsv"
}

run_runtime_round() { # run_runtime_round <round>
  local round="$1" line_no cid src opt ov extra erc ecode esha kind notes flags bin bdir
  local build_rc run_rc out_sha err_sha err_bytes lines code exp exp_sha verdict cap
  bdir="$OUT_DIR/$round/bin"; mkdir -p "$bdir"
  line_no=1
  while IFS= read -r line; do
    line_no=$((line_no+1))
    cid=$(printf '%s\n' "$line" | cut -f1); src=$(printf '%s\n' "$line" | cut -f2)
    opt=$(printf '%s\n' "$line" | cut -f3); ov=$(printf '%s\n' "$line" | cut -f4)
    extra=$(printf '%s\n' "$line" | cut -f5); erc=$(printf '%s\n' "$line" | cut -f6)
    ecode=$(printf '%s\n' "$line" | cut -f7); esha=$(printf '%s\n' "$line" | cut -f8)
    kind=$(printf '%s\n' "$line" | cut -f9); notes=$(printf '%s\n' "$line" | cut -f10)
    flags="$(flags_for "$opt" "$ov" || true)"
    bin="$bdir/$cid"
    cap=""
    case "$cid" in
      e3001-u8-*|e3002-u8-*|e3001-u32-*|e3002-u32-*|no-overflow-unsigned-*) cap="$OUT_DIR/generated-c/$cid" ;;
    esac
    [ -n "$cap" ] && mkdir -p "$cap"
    printf '[%s] cd %s && CC=%s timeout %s %s %s %s %s -o %s\n' \
      "$round" "$REPO_ROOT_REAL" "$CC_CAPTURE" "$BUILD_TIMEOUT" "$ZORC_PATH" "$flags" "$extra" "$src" "$bin" \
      >> "$OUT_DIR/commands.txt"
    # 以仓库根为 cwd，传 manifest 的相对 source_path；保证 __FILE__ 与 expected 一致
    # shellcheck disable=SC2086
    ( cd "$REPO_ROOT_REAL" && CC="$CC_CAPTURE" ZOR_REAL_CC="$CC_REAL_PATH" ZOR_CAPTURE_DIR="$cap" \
      timeout "$BUILD_TIMEOUT" "$ZORC_PATH" $flags $extra "$src" -o "$bin" ) \
      > "$OUT_DIR/$round/$cid.build.out" 2> "$OUT_DIR/$round/$cid.build.err"
    build_rc=$?
    : > "$OUT_DIR/$round/$cid.out"; : > "$OUT_DIR/$round/$cid.err"; run_rc=""
    if [ "$build_rc" -eq 0 ] && [ -x "$bin" ]; then
      timeout "$RUN_TIMEOUT" "$bin" > "$OUT_DIR/$round/$cid.out" 2> "$OUT_DIR/$round/$cid.err"
      run_rc=$?
    fi
    [ -n "$run_rc" ] || run_rc="build-fail"
    printf '%s\n' "$run_rc" > "$OUT_DIR/$round/$cid.rc"
    out_sha=$(sha256_of "$OUT_DIR/$round/$cid.out" 2>/dev/null || echo unknown)
    err_sha=$(sha256_of "$OUT_DIR/$round/$cid.err" 2>/dev/null || echo unknown)
    err_bytes=$(wc -c < "$OUT_DIR/$round/$cid.err" | tr -d ' ')
    lines=$(wc -l < "$OUT_DIR/$round/$cid.err" | tr -d ' ')
    code=$(grep -o 'E300[12]' "$OUT_DIR/$round/$cid.err" | head -1 || true)
    [ -n "$code" ] || code=none
    verdict=PASS
    [ "$build_rc" -eq 0 ] || verdict=FAIL
    [ "$run_rc" = "$erc" ] || verdict=FAIL
    [ "$out_sha" = "$esha" ] || verdict=FAIL
    if [ "$ecode" = none ]; then
      [ "$err_bytes" -eq 0 ] || verdict=FAIL
      [ "$err_sha" = "$EMPTY_SHA" ] || verdict=FAIL
      [ "$code" = none ] || verdict=FAIL
    else
      exp="$REPO_ROOT/tests/runtime/expected/$cid.stderr"
      exp_sha=$(sha256_of "$exp" 2>/dev/null || echo unknown)
      [ "$err_sha" = "$exp_sha" ] || verdict=FAIL
      [ "$lines" -eq 1 ] || verdict=FAIL
      [ "$code" = "$ecode" ] || verdict=FAIL
      if grep -Eq 'zorcc:|AddressSanitizer|LeakSanitizer' "$OUT_DIR/$round/$cid.err"; then verdict=FAIL; fi
    fi
    [ "$run_rc" = "124" ] && verdict=FAIL
    if [ -n "$cap" ]; then
      if [ ! -f "$cap/out.c" ]; then
        verdict=FAIL
        printf '%s\t%s\n' "$cid" "missing generated C" >> "$OUT_DIR/generated-c.log"
      else
        case "$cid" in
          e3001-u8-*|e3002-u8-*) grep -q 'uint8_t' "$cap/out.c" || verdict=FAIL ;;
          e3001-u32-*|e3002-u32-*) grep -q 'uint32_t' "$cap/out.c" || verdict=FAIL ;;
          no-overflow-unsigned-*)
            grep -q 'uint8_t' "$cap/out.c" || verdict=FAIL
            grep -q 'uint32_t' "$cap/out.c" || verdict=FAIL ;;
        esac
        case "$cid" in
          e3001-u8-add-o0|e3001-u8-sub-o0|e3001-u8-mul-o0|e3001-u32-add-o0)
            grep -q '__builtin_' "$cap/out.c" || verdict=FAIL ;;
        esac
        if grep -Eq 'E3001|E3002' "$cap/out.c"; then verdict=FAIL; fi
        printf '%s\t%s\n' "$cid" "$cap/out.c" >> "$OUT_DIR/generated-c.log"
      fi
    fi
    printf '%s  %s/%s\n' "$out_sha" "$round" "$cid" >> "$OUT_DIR/stdout.sha256"
    printf '%s  %s/%s\n' "$err_sha" "$round" "$cid" >> "$OUT_DIR/stderr.sha256"
    record_summary "$cid" "$round" "$build_rc" "$run_rc" "$out_sha" "$err_sha" "$err_bytes" "$lines" "$code" "$verdict"
    [ "$verdict" = PASS ] || frc_fail 6 "$cid [$round]: build=$build_rc run=$run_rc out=$out_sha err=$err_sha code=$code"
  done < <(tail -n +2 "$RUNTIME_TSV")
}

run_runtime_round round1
run_runtime_round round2
# 两轮逐条比较（去掉 round 列）
awk -F'\t' '$2=="round1"{print}' "$OUT_DIR/summary.tsv" | cut -f1,3- | LC_ALL=C sort > "$OUT_DIR/round1/rows.sorted"
awk -F'\t' '$2=="round2"{print}' "$OUT_DIR/summary.tsv" | cut -f1,3- | LC_ALL=C sort > "$OUT_DIR/round2/rows.sorted"
cmp -s "$OUT_DIR/round1/rows.sorted" "$OUT_DIR/round2/rows.sorted" \
  || frc_fail 6 "两轮 summary 不一致"
cmp -s "$OUT_DIR/round1/rows.sorted" "$OUT_DIR/round2/rows.sorted" \
  && echo SAME > "$OUT_DIR/round1/round2.compare" || echo DIFF > "$OUT_DIR/round1/round2.compare"

# ---- 宽度 fail-closed：双档 ----
line_no=1
while IFS= read -r line; do
  line_no=$((line_no+1))
  cid=$(printf '%s\n' "$line" | cut -f1); src=$(printf '%s\n' "$line" | cut -f2)
  for optflag in -O0 -O2; do
    bin="$TMP/width-$cid-$optflag"
    ( cd "$REPO_ROOT_REAL" && timeout "$BUILD_TIMEOUT" "$ZORC_PATH" "$optflag" "$src" -o "$bin" ) \
      > "$TMP/width-$cid-$optflag.out" 2> "$TMP/width-$cid-$optflag.err"
    rc=$?
    art=no; [ -e "$bin" ] && art=yes
    v=PASS
    [ "$rc" -eq 1 ] || v=FAIL
    [ "$art" = no ] || v=FAIL
    grep -Fq 'zorcc: codegen:' "$TMP/width-$cid-$optflag.err" || v=FAIL
    grep -Fq 'zor: runtime error' "$TMP/width-$cid-$optflag.err" && v=FAIL
    if [ "$rc" -eq 0 ] && [ "$art" = yes ]; then
      # 范围扩展：只有行为符合 O-1 才记 WARN；静默 wrap／漏检＝FAIL
      timeout "$RUN_TIMEOUT" "$bin" > "$TMP/width-$cid-$optflag.run.out" 2> "$TMP/width-$cid-$optflag.run.err"
      wrc=$?
      if [ "$optflag" = "-O0" ]; then
        if [ "$wrc" -eq 101 ] && grep -Eq '^zor: runtime error\[E300(1|2)\] .+:[0-9]+:[0-9]+: (integer overflow|division by zero)$' "$TMP/width-$cid-$optflag.run.err"; then
          v=WARN
        else
          v=FAIL
        fi
      else
        if [ "$wrc" -eq 0 ] && [ ! -s "$TMP/width-$cid-$optflag.run.err" ]; then
          v=WARN
        else
          v=FAIL
        fi
      fi
    fi
    printf '%s\t%s\trc=%s\tartifact=%s\tverdict=%s\n' "$cid" "$optflag" "$rc" "$art" "$v" >> "$OUT_DIR/width-controls.log"
    [ "$v" = FAIL ] && frc_fail 6 "$cid $optflag: rc=$rc artifact=$art"
    rm -f "$bin"
  done
done < <(tail -n +2 "$WIDTH_TSV")

# ---- CLI 负控 ----
line_no=1
while IFS= read -r line; do
  line_no=$((line_no+1))
  cid=$(printf '%s\n' "$line" | cut -f1); args=$(printf '%s\n' "$line" | cut -f2)
  erc=$(printf '%s\n' "$line" | cut -f3); art=$(printf '%s\n' "$line" | cut -f4)
  notc=$(printf '%s\n' "$line" | cut -f5); inc=$(printf '%s\n' "$line" | cut -f6)
  cli_tmp="$TMP/cli-$cid"; mkdir -p "$cli_tmp"
  # 词法替换 <src> 与 out；不做 shell eval
  argv=(); if [ -n "$args" ]; then read -r -a argv <<< "$args"; fi
  i=0
  while [ "$i" -lt "${#argv[@]}" ]; do
    case "${argv[$i]}" in
      '<src>') argv[$i]="tests/runtime/cases/l2b/e3001-add-o0.zor" ;;
      out) argv[$i]="$cli_tmp/out" ;;
    esac
    i=$((i+1))
  done
  if [ "${#argv[@]}" -gt 0 ]; then
    ( cd "$REPO_ROOT_REAL" && timeout "$RUN_TIMEOUT" "$ZORC_PATH" "${argv[@]}" ) \
      > "$cli_tmp/stdout" 2> "$cli_tmp/stderr"
    rc=$?
  else
    timeout "$RUN_TIMEOUT" "$ZORC_PATH" > "$cli_tmp/stdout" 2> "$cli_tmp/stderr"
    rc=$?
  fi
  v=PASS
  [ "$rc" = "$erc" ] || v=FAIL
  [ "$art" = none ] && [ -e "$cli_tmp/out" ] && v=FAIL
  [ "$art" = exists ] && [ ! -e "$cli_tmp/out" ] && v=FAIL
  [ "$notc" != '-' ] && grep -Fq -- "$notc" "$cli_tmp/stdout" && v=FAIL
  [ "$inc" != '-' ] && ! grep -Fq -- "$inc" "$cli_tmp/stderr" && v=FAIL
  if find "$cli_tmp" -maxdepth 1 -name '.zorcc-tmp-*' | grep -q .; then v=FAIL; fi
  printf '%s\trc=%s\tartifact=%s\tverdict=%s\n' "$cid" "$rc" "$([ -e "$cli_tmp/out" ] && echo yes || echo no)" "$v" >> "$OUT_DIR/cli-controls.log"
  [ "$v" = FAIL ] && frc_fail 12 "$cid: rc=$rc expected=$erc"
done < <(tail -n +2 "$CLI_TSV")

# 用户可见面（F-RC-14）
if "$ZORC_PATH" > "$TMP/usage.out" 2> "$TMP/usage.err"; then :; fi
grep -Fq 'usage: zorcc' "$TMP/usage.err" || frc_fail 14 "无参 usage 缺失"
grep -Fq -- '-Zoverflow' "$TMP/usage.out" "$TMP/usage.err" && frc_fail 14 "usage 泄漏 -Zoverflow"
if [ -f "$REPO_ROOT/README.md" ] && grep -Fq -- '-Zoverflow' "$REPO_ROOT/README.md"; then
  frc_fail 14 "README.md 泄漏 -Zoverflow"
fi
if [ -f "$REPO_ROOT/CHANGELOG.md" ]; then
  # 只看最新一节（第二个 `## ` 前）；在册历史行不追改
  awk 'BEGIN{n=0} /^## /{n++; if(n==2) exit} {print}' "$REPO_ROOT/CHANGELOG.md" \
    | grep -Fq -- '-Zoverflow' && frc_fail 14 "CHANGELOG.md 最新节泄漏 -Zoverflow"
fi
if [ -d "$REPO_ROOT/docs" ] && grep -R -Fq -- '-Zoverflow' "$REPO_ROOT/docs"; then
  frc_fail 14 "docs/** 泄漏 -Zoverflow"
fi

# ---- 汇总 ----
{
  echo "runtime_rows=29"
  echo "width_rows=6"
  echo "cli_rows=8"
  echo "fail=$fail"
  echo "utc_end=$(utc_now)"
} >> "$OUT_DIR/runtime-codes.log"
if [ "$fail" -eq 0 ]; then
  echo "PASS|runtime-codes|exit=0|log=$OUT_DIR/runtime-codes.log" | tee "$OUT_DIR/summary.txt"
  exit 0
else
  echo "FAIL|runtime-codes|exit=1|log=$OUT_DIR/runtime-codes.log" | tee "$OUT_DIR/summary.txt"
  exit 1
fi
