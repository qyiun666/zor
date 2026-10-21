#!/usr/bin/env bash
# ci/asan/run_asan.sh — L2a Linux ASAN／LSAN 门禁骨架（QA 判据；CI-01 接线）
#
# 归属：ZOR质量保障（判据／脚本）＋ZOR工具链与开发工程师（CI job 接线）。
# 依据：QA v1.3 §6（ASAN／LSAN 证据协议）、L2a 冻结件 §一／§四、核心 v1.1 §4.2。
#
# 判据（任一不满足＝FAIL，strict 不 SKIP）：
#   1. manifest 恰 48 行＝41 compile-only＋7 build-run；schema 七列；无重复；路径存在且为普通文件；
#      41 枚 tests/specs/positive/*.zor 全部在 manifest（完整性校验，不用于取运行集）。
#   2. 只读 manifest 取 build-run 集（R=7），禁目录 glob 取运行集。
#   3. 逐条 ASAN／LSAN 构建 rc=0、运行 rc=0、stdout sha 匹配、stderr 0 B、无 sanitizer 报错。
#   4. 每条 build-run 二进制含 .debug_info（readelf -S）；去 -g 对照必须被同一检查函数判 FAIL。
#   5. UAF 探针必红、LSAN 泄漏探针必红；探针固定路径、不入 manifest。
#   6. 同一宿主两轮，逐条 rc／stdout sha／sanitizer 结论一致；不一致＝FAIL。
#   7. 宿主必须 Linux；macOS 不得作泄漏证据（本脚本直接拒绝非 Linux 运行）。
#
# 用法：
#   bash ci/asan/run_asan.sh --strict [--out <dir>] [--manifest <path>] [--zorc <path>]
#   bash ci/asan/run_asan.sh --validate-only        # 只校验 manifest（可在 macOS 预检）
#
# 环境：
#   ZORC_BIN       zorcc 路径（默认 PATH 中 zorcc）
#   CC_REAL        wrapper 实际调用的 C 编译器（默认 cc；CI 可钉 clang-18）
#   ASAN_TIMEOUT   单次运行超时秒数（默认 30）
#
# 证据（--out 指定；默认 ci/logs/asan-<UTC>-<pid>）：
#   commands.txt versions.txt manifest.tsv manifest.sha256 summary.tsv
#   asan-verdict.tsv probes.log
#   round1/ round2/：每轮 summary.tsv、asan-verdict.tsv、readelf-<id>.txt、
#                    <id>.build.{out,err}、<id>.{out,err}
#   probes/：UAF／泄漏／去 -g 对照的构建与运行日志

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO_ROOT_REAL="$(cd "$REPO_ROOT" && pwd -P)"

MANIFEST="$REPO_ROOT/tests/runtime/manifest.tsv"
ZORC="${ZORC_BIN:-zorcc}"
OUT_DIR=""
VALIDATE_ONLY=0
ASAN_TIMEOUT="${ASAN_TIMEOUT:-30}"

while [ $# -gt 0 ]; do
  case "$1" in
    --strict) shift ;;
    --validate-only) VALIDATE_ONLY=1; shift ;;
    --out)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan] --out 缺值" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    --manifest)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan] --manifest 缺值" >&2; exit 2; }
      MANIFEST="$2"; shift 2 ;;
    --zorc)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[asan] --zorc 缺值" >&2; exit 2; }
      ZORC="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,40p' "$0"; exit 0 ;;
    *)
      echo "[asan] 未知参数 '$1'（可用: --strict --validate-only --out --manifest --zorc）" >&2
      exit 2 ;;
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

HOST_OS="$(uname -s)"
if [ "$VALIDATE_ONLY" != 1 ] && [ "$HOST_OS" != "Linux" ]; then
  echo "[asan] HOST_UNSUPPORTED os=${HOST_OS}：L2a ASAN／LSAN 零报告只认 Linux 宿主；macOS 不得作泄漏证据（QA v1.3 §6.1／§6.7）。" >&2
  exit 2
fi

fail=0
note_fail() { echo "[asan] FAIL: $*" >&2; fail=1; }

# ---------- manifest 校验 ----------
expected_header=$'case_id\tsource_path\texpect_rc\tmode\texpect_stdout_sha256\tkind\tnotes'
if [ ! -f "$MANIFEST" ]; then
  note_fail "manifest 缺失：$MANIFEST"
else
  IFS= read -r header < "$MANIFEST" || header=""
  [ "$header" = "$expected_header" ] || note_fail "manifest 表头不精确（须七列制表符分隔）"
  if grep -q $'\r' "$MANIFEST"; then note_fail "manifest 含 CR（须 LF）"; fi
  [ -z "$(tail -c 1 "$MANIFEST")" ] || note_fail "manifest 末尾缺 LF"
fi

n_rows=0; n_compile=0; n_run=0
if [ "$fail" -eq 0 ]; then
  n_rows=$(tail -n +2 "$MANIFEST" | wc -l | tr -d ' ')
  n_empty=$(tail -n +2 "$MANIFEST" | grep -c '^$' || true)
  [ "$n_rows" -eq 48 ] || note_fail "数据行=$n_rows（期望 48）"
  [ "$n_empty" -eq 0 ] || note_fail "含空行=$n_empty（禁止）"
  n_compile=$(awk -F'\t' '$4=="compile-only"' "$MANIFEST" | wc -l | tr -d ' ')
  n_run=$(awk -F'\t' '$4=="build-run"' "$MANIFEST" | wc -l | tr -d ' ')
  [ "$n_compile" -eq 41 ] || note_fail "compile-only=$n_compile（期望 41）"
  [ "$n_run" -eq 7 ] || note_fail "build-run=$n_run（期望 7）"

  # 逐行 schema／路径／取值（IFS 制表符会把连续空列折叠，故用 cut 保列）
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
    case "$case_id" in *[[:space:]]*) note_fail "L${line_no}: case_id 含空白";; esac
    case "$src" in
      /*) note_fail "L${line_no}: source_path 为绝对路径：$src" ;;
      *..*) note_fail "L${line_no}: source_path 含 '..'：$src" ;;
    esac
    case "$mode" in compile-only|build-run) ;; *) note_fail "L${line_no}: mode 非法：$mode";; esac
    case "$kind" in positive|fixture|control) ;; *) note_fail "L${line_no}: kind 非法：$kind";; esac
    [ "$expect_rc" = "0" ] || note_fail "L${line_no}: expect_rc=$expect_rc（L2a 冻结为 0）"
    [ -n "$notes" ] || note_fail "L${line_no}: notes 为空（compile-only 必须写理由）"
    if [ "$mode" = "compile-only" ]; then
      printf '%s' "$notes" | grep -q 'codegen-subset' || note_fail "L${line_no}: compile-only 未写 codegen-subset 理由"
    fi
    if [ "$case_id" = "m1-s1-passthrough" ]; then
      printf '%s' "$notes" | grep -q '不计入 AC-R1-2' || note_fail "M1 行 notes 未写“不计入 AC-R1-2”"
    fi
    if [ "$mode" = "build-run" ]; then
      printf '%s' "$sha" | grep -Eq '^[0-9a-f]{64}$' || note_fail "L${line_no}: build-run stdout sha 非法"
    fi
    full="$REPO_ROOT/$src"
    if [ ! -f "$full" ]; then note_fail "L${line_no}: source_path 不存在：$src"; continue; fi
    if [ -L "$full" ]; then note_fail "L${line_no}: source_path 为 symlink：$src"; fi
    rp="$(cd "$(dirname "$full")" && pwd -P)/$(basename "$full")"
    case "$rp" in "$REPO_ROOT_REAL"/*) ;; *) note_fail "L${line_no}: source_path 逃出仓库：$src";; esac
  done < <(tail -n +2 "$MANIFEST")

  dup_ids=$(tail -n +2 "$MANIFEST" | cut -f1 | sort | uniq -d)
  [ -z "$dup_ids" ] || note_fail "case_id 重复：$dup_ids"
  dup_paths=$(tail -n +2 "$MANIFEST" | cut -f2 | sort | uniq -d)
  [ -z "$dup_paths" ] || note_fail "source_path 重复：$dup_paths"

  # 完整性校验（仅校验，不用于取运行集）：41 正例逐枚在 manifest
  awk -F'\t' '$4=="compile-only"{print $2}' "$MANIFEST" | sort > /tmp/zor-asan-manifest-pos.$$
  find "$REPO_ROOT/tests/specs/positive" -maxdepth 1 -type f -name '*.zor' | sed "s|^$REPO_ROOT/||" | sort > /tmp/zor-asan-dir-pos.$$
  if ! diff -q /tmp/zor-asan-manifest-pos.$$ /tmp/zor-asan-dir-pos.$$ >/dev/null; then
    note_fail "manifest 的 compile-only 集与 tests/specs/positive/*.zor 不一致"
    diff /tmp/zor-asan-manifest-pos.$$ /tmp/zor-asan-dir-pos.$$ | head -10 >&2
  fi
  rm -f /tmp/zor-asan-manifest-pos.$$ /tmp/zor-asan-dir-pos.$$

  awk -F'\t' '$4=="build-run" && $1=="box_overwrite_drop"{found=1} END{exit found?0:1}' "$MANIFEST" \
    || note_fail "build-run 缺 box_overwrite_drop（W2 立即 drop 载体）"
fi

if [ "$VALIDATE_ONLY" = 1 ]; then
  if [ "$fail" -eq 0 ]; then
    echo "[asan] manifest OK cases=48 compile_only=${n_compile} run_cases=${n_run} sha256=$(sha256_of "$MANIFEST" || echo NA)"
    exit 0
  fi
  echo "[asan] manifest FAIL" >&2
  exit 1
fi

# ---------- 环境 ----------
WRAPPER_ASAN="$REPO_ROOT/ci/asan/cc-asan"
WRAPPER_NOG="$REPO_ROOT/ci/asan/cc-nog"
UAF_PROBE="$REPO_ROOT/tests/runtime/controls/asan_uaf_probe.c"
LEAK_PROBE="$REPO_ROOT/tests/runtime/controls/asan_leak_probe.c"

for f in "$WRAPPER_ASAN" "$WRAPPER_NOG" "$UAF_PROBE" "$LEAK_PROBE"; do
  [ -f "$f" ] || note_fail "缺少脚本／探针：$f"
done
[ -x "$WRAPPER_ASAN" ] || note_fail "cc-asan 不可执行：$WRAPPER_ASAN"
[ -x "$WRAPPER_NOG" ] || note_fail "cc-nog 不可执行：$WRAPPER_NOG"
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
  echo "[asan] 环境／manifest 前置失败，未执行构建" >&2
  exit 1
fi

if [ -z "$OUT_DIR" ]; then
  OUT_DIR="$REPO_ROOT/ci/logs/asan-$(date -u +%Y%m%dT%H%M%SZ)-$$"
fi
mkdir -p "$OUT_DIR/round1" "$OUT_DIR/round2" "$OUT_DIR/probes"
cp "$MANIFEST" "$OUT_DIR/manifest.tsv"
sha256_of "$MANIFEST" > "$OUT_DIR/manifest.sha256" || note_fail "无法计算 manifest sha256"

: > "$OUT_DIR/commands.txt"
: > "$OUT_DIR/versions.txt"
: > "$OUT_DIR/probes.log"

{
  echo "utc_start=$(utc_now)"
  echo "host=$(uname -srm)"
  echo "repo=$REPO_ROOT_REAL"
  echo "head=$(git -C "$REPO_ROOT" rev-parse HEAD 2>/dev/null || echo NA)"
  echo "zorcc=$ZORC_ABS"
  echo "manifest=$MANIFEST"
  echo "manifest_sha256=$(cat "$OUT_DIR/manifest.sha256" 2>/dev/null || echo NA)"
  echo "CC_REAL=$REAL_CC"
  echo "ASAN_OPTIONS=halt_on_error=1:detect_leaks=1"
  echo "ASAN_TIMEOUT=$ASAN_TIMEOUT"
  echo "flags=-std=c11 -O0 -g -fsanitize=address -fno-omit-frame-pointer"
  echo "--- versions ---"
  "$ZORC_ABS" --version 2>&1 | head -1 || true
  "$REAL_CC" --version 2>&1 | head -1 || true
  if command -v clang >/dev/null 2>&1; then clang --version 2>&1 | head -1; fi
  ldd --version 2>&1 | head -1 || true
  readelf --version 2>&1 | head -1 || true
  uname -a
} > "$OUT_DIR/versions.txt"

debug_info_present() { readelf -S "$1" 2>/dev/null | grep -q '\.debug_info'; }

run_case() { # <round_dir> <case_id> <src> <expect_sha> <expect_rc>
  local rd="$1" id="$2" src="$3" expect_sha="$4" expect_rc="$5"
  local bin="$rd/$id.bin" log="$rd/$id" sumf="$rd/summary.tsv" vf="$rd/asan-verdict.tsv"
  local build_rc=0 debug="absent" run_rc="" out_sha="" err_bytes="" verdict="FAIL" san="none"
  [ -f "$sumf" ] || printf 'case_id\tbuild_rc\tdebug_info\trun_rc\tstdout_sha256\tstderr_bytes\tasan_verdict\tverdict\tnotes\n' > "$sumf"
  {
    printf 'CC=%s %s %s -O0 -o %s\n' "$WRAPPER_ASAN" "$ZORC_ABS" "$src" "$bin"
  } >> "$OUT_DIR/commands.txt"
  ( cd "$REPO_ROOT" && CC="$WRAPPER_ASAN" "$ZORC_ABS" "$src" -O0 -o "$bin" ) > "$log.build.out" 2> "$log.build.err"
  build_rc=$?
  if [ "$build_rc" -eq 0 ] && [ -x "$bin" ]; then
    if debug_info_present "$bin"; then debug="present"; else debug="absent"; fi
    readelf -S "$bin" > "$rd/readelf-$id.txt" 2>&1 || true
    ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$bin" > "$log.out" 2> "$log.err"
    run_rc=$?
    out_sha="$(sha256_of "$log.out" || echo NA)"
    err_bytes="$(wc -c < "$log.err" | tr -d ' ')"
    if grep -q 'ERROR: AddressSanitizer' "$log.err"; then san="asan-error";
    elif grep -q 'ERROR: LeakSanitizer' "$log.err"; then san="lsan-error"; fi
    if [ "$run_rc" -eq "$expect_rc" ] && [ "$out_sha" = "$expect_sha" ] \
       && [ "$err_bytes" -eq 0 ] && [ "$san" = "none" ] && [ "$debug" = "present" ]; then
      verdict="PASS"
    fi
  else
    verdict="FAIL"; run_rc="NA"
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$id" "$build_rc" "$debug" "$run_rc" "$out_sha" "$err_bytes" "$san" "$verdict" \
    "asan-build+run" >> "$sumf"
  printf '%s\tbuild_rc=%s\tdebug_info=%s\trun_rc=%s\tstdout_sha=%s\tstderr_bytes=%s\tsan=%s\tverdict=%s\n' \
    "$id" "$build_rc" "$debug" "$run_rc" "$out_sha" "$err_bytes" "$san" "$verdict" >> "$vf"
  [ "$verdict" = "PASS" ] || note_fail "case ${id}: build_rc=${build_rc} debug=${debug} run_rc=${run_rc} stdout_sha=${out_sha} stderr=${err_bytes} san=${san}"
}

# ---------- round 1 / round 2：只读 manifest 取 build-run 集 ----------
while IFS=$'\t' read -r case_id src expect_rc sha; do
  run_case "$OUT_DIR/round1" "$case_id" "$src" "$sha" "$expect_rc"
done < <(awk -F'\t' '$4=="build-run"{print $1"\t"$2"\t"$3"\t"$5}' "$MANIFEST")

# 复算：同一宿主第二轮；逐条 rc／stdout sha／sanitizer 结论必须一致
while IFS=$'\t' read -r case_id src expect_rc sha; do
  run_case "$OUT_DIR/round2" "$case_id" "$src" "$sha" "$expect_rc" >/dev/null 2>&1
done < <(awk -F'\t' '$4=="build-run"{print $1"\t"$2"\t"$3"\t"$5}' "$MANIFEST")

cp "$OUT_DIR/round1/summary.tsv" "$OUT_DIR/summary.tsv"
cp "$OUT_DIR/round1/asan-verdict.tsv" "$OUT_DIR/asan-verdict.tsv"
tail -n +2 "$OUT_DIR/round1/summary.tsv" | sort > "$OUT_DIR/round1.compare"
tail -n +2 "$OUT_DIR/round2/summary.tsv" | sort > "$OUT_DIR/round2.compare"
if ! diff -q "$OUT_DIR/round1.compare" "$OUT_DIR/round2.compare" >/dev/null; then
  note_fail "两轮复算不一致（rc／stdout sha／stderr 字节／sanitizer 结论）"
  diff "$OUT_DIR/round1.compare" "$OUT_DIR/round2.compare" | head -20 >> "$OUT_DIR/probes.log"
fi

# ---------- 探针：UAF 必红、LSAN 泄漏必红、去 -g 必 FAIL ----------
uaf_bin="$OUT_DIR/probes/uaf_probe"
if "$WRAPPER_ASAN" -O0 -o "$uaf_bin" "$UAF_PROBE" > "$OUT_DIR/probes/uaf.build.out" 2> "$OUT_DIR/probes/uaf.build.err"; then
  ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$uaf_bin" > "$OUT_DIR/probes/uaf.out" 2> "$OUT_DIR/probes/uaf.err"
  uaf_rc=$?
  if [ "$uaf_rc" -ne 0 ] && grep -q 'ERROR: AddressSanitizer' "$OUT_DIR/probes/uaf.err"; then
    echo "UAF_PROBE=PASS rc=$uaf_rc AddressSanitizer=hit" >> "$OUT_DIR/probes.log"
  else
    echo "UAF_PROBE=FAIL rc=$uaf_rc（期望 rc≠0 且命中 AddressSanitizer）" >> "$OUT_DIR/probes.log"
    note_fail "UAF 探针静默（ASAN 探测器死）"
  fi
else
  echo "UAF_PROBE=FAIL 构建失败" >> "$OUT_DIR/probes.log"; note_fail "UAF 探针构建失败"
fi

leak_bin="$OUT_DIR/probes/leak_probe"
if "$WRAPPER_ASAN" -O0 -o "$leak_bin" "$LEAK_PROBE" > "$OUT_DIR/probes/leak.build.out" 2> "$OUT_DIR/probes/leak.build.err"; then
  ASAN_OPTIONS=halt_on_error=1:detect_leaks=1 timeout "$ASAN_TIMEOUT" "$leak_bin" > "$OUT_DIR/probes/leak.out" 2> "$OUT_DIR/probes/leak.err"
  leak_rc=$?
  if [ "$leak_rc" -ne 0 ] && grep -q 'ERROR: LeakSanitizer' "$OUT_DIR/probes/leak.err"; then
    echo "LEAK_PROBE=PASS rc=$leak_rc LeakSanitizer=hit" >> "$OUT_DIR/probes.log"
  else
    echo "LEAK_PROBE=FAIL rc=$leak_rc（期望 rc≠0 且命中 LeakSanitizer）" >> "$OUT_DIR/probes.log"
    note_fail "LSAN 泄漏探针静默（泄漏检测死）"
  fi
else
  echo "LEAK_PROBE=FAIL 构建失败" >> "$OUT_DIR/probes.log"; note_fail "泄漏探针构建失败"
fi

# 去 -g：与正例同一检查函数；必须判 FAIL（无 .debug_info）
nog_bin="$OUT_DIR/probes/hello_nog"
( cd "$REPO_ROOT" && CC="$WRAPPER_NOG" "$ZORC_ABS" examples/hello.zor -O0 -o "$nog_bin" ) \
  > "$OUT_DIR/probes/nog.build.out" 2> "$OUT_DIR/probes/nog.build.err"
nog_build_rc=$?
if [ "$nog_build_rc" -eq 0 ] && [ -x "$nog_bin" ]; then
  if debug_info_present "$nog_bin"; then
    echo "NOG_CONTROL=FAIL（无 -g 仍含 .debug_info：检查函数失明）" >> "$OUT_DIR/probes.log"
    note_fail "去 -g 对照未被判 FAIL（debug-info 检查失明）"
  else
    echo "NOG_CONTROL=PASS（无 -g 无 .debug_info；同一检查函数判 FAIL）" >> "$OUT_DIR/probes.log"
  fi
else
  echo "NOG_CONTROL=FAIL 构建失败 rc=$nog_build_rc" >> "$OUT_DIR/probes.log"
  note_fail "去 -g 对照构建失败"
fi

# ---------- 收尾 ----------
{
  echo "utc_end=$(utc_now)"
  echo "cases=48 compile_only=${n_compile} run_cases=${n_run}"
  echo "manifest_sha256=$(cat "$OUT_DIR/manifest.sha256" 2>/dev/null || echo NA)"
  echo "verdict=$([ "$fail" -eq 0 ] && echo PASS || echo FAIL)"
} >> "$OUT_DIR/versions.txt"

if [ "$fail" -eq 0 ]; then
  echo "[asan] PASS（R=${n_run} ASAN/LSAN 零报告；UAF/泄漏/去-g 对照均按期望）"
  echo "[asan] 证据：$OUT_DIR"
  exit 0
fi
echo "[asan] FAIL；证据：$OUT_DIR" >&2
exit 1
