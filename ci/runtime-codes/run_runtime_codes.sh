#!/usr/bin/env bash
# ci/runtime-codes/run_runtime_codes.sh — L2b runtime-codes 判据（CI 侧）
# 归属：ZOR工具链与开发工程师；判据源：shared/ZOR-L2b-门禁与计数delta-QA-v1.3.md
# 只读三份 manifest；不执行 manifest 内任意 shell；无 sanitizer；失败 fail-closed。
set -uo pipefail

REPO_ROOT="${REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || true)}"
[ -n "$REPO_ROOT" ] || { echo "[runtime-codes] 必须在 git 仓库内运行" >&2; exit 2; }
cd "$REPO_ROOT"

OUT_DIR="${OUT_DIR:?OUT_DIR 未设置}"
ZORC="${ZORC:-}"
CC_BIN="${CC:-cc}"
RUNTIME_CODES_MANIFEST="${RUNTIME_CODES_MANIFEST:-tests/runtime/runtime-codes.tsv}"
WIDTH_FAILCLOSED_MANIFEST="${WIDTH_FAILCLOSED_MANIFEST:-tests/runtime/width-failclosed.tsv}"
CLI_CASES_MANIFEST="${CLI_CASES_MANIFEST:-tests/runtime/cli-cases.tsv}"
RUNTIME_EXPECTED_DIR="${RUNTIME_EXPECTED_DIR:-tests/runtime/expected}"
RUNTIME_CODES_BASE_REF="${RUNTIME_CODES_BASE_REF:-origin/main}"
RUNTIME_CODES_CLI_SRC="${RUNTIME_CODES_CLI_SRC:-tests/runtime/cases/l2b/no-overflow-ops-o0.zor}"
RUNTIME_CODES_ROWS=29
WIDTH_ROWS=6
CLI_ROWS=8

# 统一绝对路径：CLI 用例会在临时目录内执行，相对 ZORC 会失效
if [ -n "$ZORC" ] && command -v "$ZORC" >/dev/null 2>&1; then
  ZORC="$(command -v "$ZORC")"
  case "$ZORC" in
    /*) ;;
    *) ZORC="$(cd "$(dirname "$ZORC")" && pwd)/$(basename "$ZORC")" ;;
  esac
fi

RUNTIME_HEADER=$'case_id\tsource_path\topt\toverflow\textra_flags\texpect_rc\texpect_code\texpect_stdout_sha256\tkind\tnotes'
WIDTH_HEADER=$'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes'
CLI_HEADER=$'case_id\targs\texpect_rc\texpect_artifact\texpect_stdout_not_contains\texpect_stderr_contains\tnotes'
EMPTY_SHA="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
RC_PATTERN='^zor: runtime error\[E300[12]\] .+:[0-9]+:[0-9]+: (integer overflow|division by zero)$'

FAILS=0
WARNS=0
mkdir -p "$OUT_DIR/round1" "$OUT_DIR/round2"
: > "$OUT_DIR/commands.txt"
: > "$OUT_DIR/stdout.sha256"
: > "$OUT_DIR/stderr.sha256"
: > "$OUT_DIR/cli-controls.log"
: > "$OUT_DIR/width-controls.log"
printf '%s\n' "case_id	round	build_rc	run_rc	stdout_sha256	stderr_sha256	stderr_bytes	lines	code	verdict" > "$OUT_DIR/summary.tsv"

fail() {
  echo "[runtime-codes] FAIL: $*" >&2
  FAILS=$((FAILS + 1))
}
warn() {
  echo "[runtime-codes] WARN: $*" >&2
  WARNS=$((WARNS + 1))
}
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
sha256_stdin() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
  else
    shasum -a 256 | awk '{print $1}'
  fi
}
utc_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
add_cmd() { printf '%s\n' "$*" >> "$OUT_DIR/commands.txt"; }

check_no_temp_residue() { # <dir> <label>
  local hit
  hit="$(find "$1" -maxdepth 1 -name '.zorcc-tmp-*' -print -quit 2>/dev/null || true)"
  if [ -n "$hit" ]; then
    fail "F-RC-10 $2 存在临时残留：$hit"
    return 1
  fi
  return 0
}
check_source_path() { # <src> <label>
  local src="$1" label="$2"
  case "$src" in
    /*) fail "F-RC-4 $label source_path 为绝对路径：$src"; return 1 ;;
    *..*) fail "F-RC-4 $label source_path 含 ..：$src"; return 1 ;;
  esac
  case "$src" in
    tests/runtime/*) ;;
    *) fail "F-RC-4 $label source_path 不在 tests/runtime/：$src"; return 1 ;;
  esac
  if [ -L "$src" ]; then fail "F-RC-4 $label source_path 是 symlink：$src"; return 1; fi
  if [ ! -f "$src" ]; then fail "F-RC-4 $label source_path 不存在：$src"; return 1; fi
  return 0
}
validate_shape() { # <file> <header> <cols> <label>
  local file="$1" header="$2" cols="$3" label="$4"
  if [ ! -f "$file" ]; then fail "F-RC-4 $label 缺失：$file"; return 1; fi
  if grep -q $'\r' "$file"; then fail "F-RC-4 $label 含 CR"; return 1; fi
  if [ "$(head -n 1 "$file")" != "$header" ]; then fail "F-RC-4 $label 表头不精确"; return 1; fi
  if ! awk -F'\t' -v n="$cols" 'NF!=n{exit 1}' "$file"; then
    fail "F-RC-4 $label 存在非 $cols 列行"; return 1
  fi
  if grep -nE '^[[:space:]]*(#|$)' "$file" | head -1 | grep -q .; then
    fail "F-RC-4 $label 含空行或注释行"; return 1
  fi
  return 0
}
split_contains() { # <value> <label>；"-" 表示不判；"|" 分隔多个必须同时命中的子串
  local value="$1" label="$2" part
  if [ "$value" = "-" ]; then return 0; fi
  local IFS='|'
  for part in $value; do
    [ -n "$part" ] || continue
    if ! grep -Fq -- "$part" "$3"; then
      fail "$label 缺少子串：$part"; return 1
    fi
  done
  return 0
}
split_not_contains() { # <value> <label> <file>
  local value="$1" label="$2" file="$3" part
  if [ "$value" = "-" ]; then return 0; fi
  local IFS='|'
  for part in $value; do
    [ -n "$part" ] || continue
    if grep -Fq -- "$part" "$file"; then
      fail "$label 出现禁止子串：$part"; return 1
    fi
  done
  return 0
}

host_check() {
  local os arch kernel
  os="$(uname -s)"; arch="$(uname -m)"; kernel="$(uname -r)"
  [ "$os" = "Linux" ] || fail "F-RC-2 宿主必须是 Linux（当前 $os）"
  command -v timeout >/dev/null 2>&1 || fail "F-RC-3 缺少 timeout"
  if [ -z "$ZORC" ] || ! command -v "$ZORC" >/dev/null 2>&1; then
    fail "F-RC-1 ZORC 缺失或不可执行：${ZORC:-<empty>}"
  fi
  command -v "$CC_BIN" >/dev/null 2>&1 || fail "F-RC-3 缺少 CC：$CC_BIN"
  {
    echo "utc_start=$(utc_now)"
    echo "host=$os $kernel"
    echo "arch=$arch"
    echo "cc=$CC_BIN"
    if command -v "$CC_BIN" >/dev/null 2>&1; then "$CC_BIN" --version 2>&1 | head -1; fi
    echo "zorcc=$ZORC"
    if [ -n "$ZORC" ] && command -v "$ZORC" >/dev/null 2>&1; then "$ZORC" --version 2>&1 | head -1; fi
    echo "flags=no-sanitizer; runtime matrix only accepted flags (-O0|-O2|-Zoverflow=check|wrap|-g)"
    echo "runtime_manifest=$RUNTIME_CODES_MANIFEST"
    echo "width_manifest=$WIDTH_FAILCLOSED_MANIFEST"
    echo "cli_manifest=$CLI_CASES_MANIFEST"
  } > "$OUT_DIR/versions.txt"
}

validate_runtime_manifest() {
  local file="$1" rows case_id src opt overflow extra rc code sha kind notes
  validate_shape "$file" "$RUNTIME_HEADER" 10 "runtime manifest" || return 1
  rows=$(( $(awk 'END{print NR}' "$file") - 1 ))
  [ "$rows" -eq "$RUNTIME_CODES_ROWS" ] || fail "F-RC-4 runtime manifest 数据行=$rows，要求 $RUNTIME_CODES_ROWS"
  local dup
  dup="$(awk -F'\t' 'NR>1{print $1}' "$file" | sort | uniq -d)"
  [ -z "$dup" ] || fail "F-RC-4 runtime case_id 重复：$dup"
  dup="$(awk -F'\t' 'NR>1{print $2}' "$file" | sort | uniq -d)"
  [ -z "$dup" ] || fail "F-RC-4 runtime source_path 重复：$dup"

  while IFS=$'\034' read -r case_id src opt overflow extra rc code sha kind notes; do
    [ -n "$case_id" ] || { fail "F-RC-4 runtime 空 case_id"; continue; }
    check_source_path "$src" "$case_id" || continue
    case "$opt" in O0|O2|default) ;; *) fail "F-RC-4 $case_id opt 非法：$opt" ;; esac
    case "$overflow" in check|wrap|none) ;; *) fail "F-RC-4 $case_id overflow 非法：$overflow" ;; esac
    case "$extra" in ""|-g) ;; *) fail "F-RC-4 $case_id extra_flags 非法：$extra" ;; esac
    case "$rc" in 0|101) ;; *) fail "F-RC-4 $case_id expect_rc 非法：$rc" ;; esac
    case "$code" in E3001|E3002|none) ;; *) fail "F-RC-4 $case_id expect_code 非法：$code" ;; esac
    case "$kind" in checked|wrap|runtime-error|control) ;; *) fail "F-RC-4 $case_id kind 非法：$kind" ;; esac
    if [ "$sha" = "$EMPTY_SHA" ] || printf '%s' "$sha" | grep -Eq '^[0-9a-f]{64}$'; then :; else
      fail "F-RC-4 $case_id expect_stdout_sha256 非 64 位小写十六进制"
    fi
    if [ "$code" != "none" ]; then
      local exp="$RUNTIME_EXPECTED_DIR/$case_id.stderr"
      if [ ! -f "$exp" ] || [ -L "$exp" ]; then
        fail "F-RC-4 $case_id 缺少 expected stderr：$exp"
      else
        local lines last
        lines=$(wc -l < "$exp" | tr -d ' ')
        last=$(tail -c 1 "$exp" | od -An -tu1 | tr -d ' \n')
        [ "$lines" -eq 1 ] || fail "F-RC-7 $case_id expected stderr 行数=$lines，要求 1"
        [ "$last" = "10" ] || fail "F-RC-7 $case_id expected stderr 末字节非 LF"
        grep -Eq "$RC_PATTERN" "$exp" || fail "F-RC-7 $case_id expected stderr 不匹配运行期码形"
        local got_src
        got_src=$(sed -nE 's/^zor: runtime error\[E300[12]\] (.*):[0-9]+:[0-9]+: (integer overflow|division by zero)$/\1/p' "$exp")
        [ "$got_src" = "$src" ] || fail "F-RC-7 $case_id expected stderr 的 <file> 与 source_path 不一致"
      fi
    fi
  done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7,$8,$9,$10}' "$file")
}

flags_for() { # <opt> <overflow> <extra> ; 输出到全局 FLAGS 数组
  FLAGS=()
  case "$1/$2" in
    O0/check) FLAGS=(-O0) ;;
    O0/wrap) FLAGS=(-O0 -Zoverflow=wrap) ;;
    O2/check) FLAGS=(-O2 -Zoverflow=check) ;;
    O2/wrap) FLAGS=(-O2) ;;
    default/wrap) FLAGS=() ;;
    O0/none) FLAGS=(-O0) ;;
    O2/none) FLAGS=(-O2) ;;
    default/none) FLAGS=() ;;
    *) return 1 ;;
  esac
  [ "$3" = "-g" ] && FLAGS+=(-g)
  return 0
}

run_runtime_case() { # case_id src opt overflow extra expect_rc expect_code expect_sha kind
  local case_id="$1" src="$2" opt="$3" overflow="$4" extra="$5" expect_rc="$6" expect_code="$7" expect_sha="$8" kind="$9"
  local round bin rc brc out err rcf so se sb sl code verdict exp
  local -a flags
  if ! flags_for "$opt" "$overflow" "$extra"; then
    fail "F-RC-4 $case_id flags 映射失败（$opt/$overflow/$extra）"; return
  fi
  flags=()
  if [ ${#FLAGS[@]} -gt 0 ]; then flags=("${FLAGS[@]}"); fi
  local case_ok=1
  for round in 1 2; do
    bin="$OUT_DIR/round$round/$case_id"
    out="$OUT_DIR/round$round/$case_id.out"
    err="$OUT_DIR/round$round/$case_id.err"
    rcf="$OUT_DIR/round$round/$case_id.rc"
    brc=0
    if [ ${#flags[@]} -gt 0 ]; then
      add_cmd "timeout 120 $ZORC ${flags[*]} $src -o $bin"
      timeout 120 "$ZORC" "${flags[@]}" "$src" -o "$bin" > "$OUT_DIR/round$round/$case_id.build.log" 2>&1 || brc=$?
    else
      add_cmd "timeout 120 $ZORC $src -o $bin"
      timeout 120 "$ZORC" "$src" -o "$bin" > "$OUT_DIR/round$round/$case_id.build.log" 2>&1 || brc=$?
    fi
    if [ "$brc" -eq 124 ]; then fail "F-RC-11 $case_id build 超时"; case_ok=0; continue; fi
    if [ "$brc" -ne 0 ] || [ ! -x "$bin" ]; then
      fail "F-RC-5 $case_id round$round build rc=$brc 或产物缺失"
      case_ok=0; continue
    fi
    check_no_temp_residue "$OUT_DIR/round$round" "$case_id round$round" || case_ok=0
    add_cmd "env -u ASAN_OPTIONS -u ASAN_SYMBOLIZER_PATH timeout 30 $bin"
    rc=0
    env -u ASAN_OPTIONS -u ASAN_SYMBOLIZER_PATH timeout 30 "$bin" > "$out" 2> "$err" || rc=$?
    echo "$rc" > "$rcf"
    if [ "$rc" -eq 124 ]; then fail "F-RC-11 $case_id round$round run 超时"; case_ok=0; fi
    so="$(sha256_of "$out")"
    se="$(sha256_of "$err")"
    sb=$(wc -c < "$err" | tr -d ' ')
    sl=$(wc -l < "$err" | tr -d ' ')
    code="none"
    verdict="FAIL"
    if [ "$rc" -eq "$expect_rc" ] && [ "$so" = "$expect_sha" ]; then
      if [ "$expect_code" = "none" ]; then
        if [ "$sb" -eq 0 ]; then verdict="PASS"; else fail "F-RC-7 $case_id round$round 期望无 stderr，实得 ${sb} B"; fi
      else
        exp="$RUNTIME_EXPECTED_DIR/$case_id.stderr"
        if [ "$sl" -eq 1 ] && [ "$sb" -gt 0 ] && [ -f "$exp" ] && cmp -s "$err" "$exp"; then
          if grep -Eq "$RC_PATTERN" "$err"; then
            code="$expect_code"
            verdict="PASS"
          else
            fail "F-RC-7 $case_id round$round stderr 不匹配运行期码形"
          fi
        else
          fail "F-RC-7 $case_id round$round stderr 与 expected 不一致（行数=$sl，字节=$sb）"
        fi
      fi
    else
      fail "F-RC-6 $case_id round$round rc=$rc（期望 $expect_rc）或 stdout sha 不符"
    fi
    if grep -Eq 'ERROR: (AddressSanitizer|LeakSanitizer)|zorcc:' "$err"; then
      fail "F-RC-9 $case_id round$round stderr 出现 sanitizer 或 zorcc 诊断"
      verdict="FAIL"
    fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
      "$case_id" "round$round" "$brc" "$rc" "$so" "$se" "$sb" "$sl" "$code" "$verdict" >> "$OUT_DIR/summary.tsv"
    printf '%s  %s round%s\n' "$so" "$case_id" "$round" >> "$OUT_DIR/stdout.sha256"
    printf '%s  %s round%s\n' "$se" "$case_id" "$round" >> "$OUT_DIR/stderr.sha256"
    [ "$verdict" = "PASS" ] || case_ok=0
  done
  [ "$case_ok" -eq 1 ] || fail "F-RC-6 $case_id 两轮未全 PASS"
}

validate_width_manifest() {
  local file="$1" rows case_id src rc artifact contains not_contains notes
  validate_shape "$file" "$WIDTH_HEADER" 7 "width manifest" || return 1
  rows=$(( $(awk 'END{print NR}' "$file") - 1 ))
  [ "$rows" -eq "$WIDTH_ROWS" ] || fail "F-RC-4 width manifest 数据行=$rows，要求 $WIDTH_ROWS"
  local dup
  dup="$(awk -F'\t' 'NR>1{print $1}' "$file" | sort | uniq -d)"
  [ -z "$dup" ] || fail "F-RC-4 width case_id 重复：$dup"
  while IFS=$'\034' read -r case_id src rc artifact contains not_contains notes; do
    [ -n "$case_id" ] || { fail "F-RC-4 width 空 case_id"; continue; }
    check_source_path "$src" "$case_id" || continue
    [ "$rc" = "1" ] || fail "F-RC-4 $case_id width expect_rc=$rc，要求 1"
    [ "$artifact" = "none" ] || fail "F-RC-4 $case_id width expect_artifact=$artifact，要求 none"
    [ "$contains" = "zorcc: codegen:" ] || fail "F-RC-4 $case_id width expect_stderr_contains 不符"
    [ "$not_contains" = "zor: runtime error" ] || fail "F-RC-4 $case_id width expect_stderr_not_contains 不符"
  done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7}' "$file")
}

run_width_case() { # case_id src
  local case_id="$1" src="$2" opt bin brc rc out err artifact_ok
  local -a levels=(-O0 -O2)
  local pass=0 expanded=0 failed=0 detail
  detail="$case_id"
  for opt in "${levels[@]}"; do
    bin="$OUT_DIR/round1/width-$case_id-${opt#-}"
    out="$OUT_DIR/round1/width-$case_id-${opt#-}.out"
    err="$OUT_DIR/round1/width-$case_id-${opt#-}.err"
    rm -f "$bin" "$out" "$err"
    brc=0
    add_cmd "timeout 120 $ZORC $opt $src -o $bin"
    timeout 120 "$ZORC" "$opt" "$src" -o "$bin" > "$OUT_DIR/round1/width-$case_id-${opt#-}.build.log" 2>&1 || brc=$?
    if [ "$brc" -eq 124 ]; then
      fail "F-RC-11 width $case_id $opt build 超时"; failed=1; detail="$detail $opt=timeout"; continue
    fi
    artifact_ok=0
    [ -e "$bin" ] && artifact_ok=1
    check_no_temp_residue "$OUT_DIR/round1" "width $case_id $opt" || failed=1
    if [ "$brc" -eq 1 ] && [ "$artifact_ok" -eq 0 ] \
       && grep -Fq 'zorcc: codegen:' "$OUT_DIR/round1/width-$case_id-${opt#-}.build.log" \
       && ! grep -Fq 'zor: runtime error' "$OUT_DIR/round1/width-$case_id-${opt#-}.build.log"; then
      pass=$((pass + 1)); detail="$detail $opt=rejected"
      continue
    fi
    if [ "$brc" -eq 0 ] && [ "$artifact_ok" -eq 1 ]; then
      rc=0
      env -u ASAN_OPTIONS -u ASAN_SYMBOLIZER_PATH timeout 30 "$bin" > "$out" 2> "$err" || rc=$?
      if [ "$opt" = "-O0" ] && [ "$rc" -eq 101 ] && grep -Eq "$RC_PATTERN" "$err"; then
        expanded=$((expanded + 1)); WARNS=$((WARNS + 1)); detail="$detail $opt=expanded(O0/E3001)"
        echo "WARN $case_id $opt 已支持（运行期 E3001，范围扩展）" >> "$OUT_DIR/width-controls.log"
        continue
      fi
      if [ "$opt" = "-O2" ] && [ "$rc" -eq 0 ] && [ ! -s "$err" ]; then
        expanded=$((expanded + 1)); WARNS=$((WARNS + 1)); detail="$detail $opt=expanded(O2/wrap)"
        echo "WARN $case_id $opt 已支持（wrap，范围扩展）" >> "$OUT_DIR/width-controls.log"
        continue
      fi
      fail "F-RC-14 width $case_id $opt 构建成功但运行期行为不符 O-1"
      failed=1; detail="$detail $opt=bad-behavior"
      continue
    fi
    fail "F-RC-6 width $case_id $opt build rc=$brc，产物=$artifact_ok"
    failed=1; detail="$detail $opt=bad-rc"
  done
  if [ "$failed" -eq 1 ]; then
    echo "FAIL $case_id $detail" >> "$OUT_DIR/width-controls.log"
  elif [ "$expanded" -gt 0 ] && [ "$pass" -gt 0 ]; then
    fail "F-RC-14 width $case_id 两档结果不一致（rejected/expanded 混合）"
    echo "FAIL $case_id $detail（rejected/expanded 混合）" >> "$OUT_DIR/width-controls.log"
  elif [ "$expanded" -gt 0 ]; then
    echo "WARN $case_id $detail（范围扩展）" >> "$OUT_DIR/width-controls.log"
  else
    echo "PASS $case_id $detail" >> "$OUT_DIR/width-controls.log"
  fi
}

validate_cli_manifest() {
  local file="$1" rows case_id args rc artifact stdout_not stderr_contains notes
  validate_shape "$file" "$CLI_HEADER" 7 "cli manifest" || return 1
  rows=$(( $(awk 'END{print NR}' "$file") - 1 ))
  [ "$rows" -eq "$CLI_ROWS" ] || fail "F-RC-4 cli manifest 数据行=$rows，要求 $CLI_ROWS"
  local dup
  dup="$(awk -F'\t' 'NR>1{print $1}' "$file" | sort | uniq -d)"
  [ -z "$dup" ] || fail "F-RC-4 cli case_id 重复：$dup"
  if [ ! -f "$RUNTIME_CODES_CLI_SRC" ] || [ -L "$RUNTIME_CODES_CLI_SRC" ]; then
    fail "F-RC-4 cli 固定样本缺失或为 symlink：$RUNTIME_CODES_CLI_SRC"
  fi
  while IFS=$'\034' read -r case_id args rc artifact stdout_not stderr_contains notes; do
    [ -n "$case_id" ] || { fail "F-RC-4 cli 空 case_id"; continue; }
    case "$rc" in 0|2) ;; *) fail "F-RC-4 $case_id cli expect_rc 非法：$rc" ;; esac
    case "$artifact" in none|exists) ;; *) fail "F-RC-4 $case_id cli expect_artifact 非法：$artifact" ;; esac
  done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7}' "$file")
}

run_cli_case() { # case_id args expect_rc expect_artifact stdout_not stderr_contains
  local case_id="$1" args="$2" expect_rc="$3" expect_artifact="$4" stdout_not="$5" stderr_contains="$6"
  local tmp out stdoutf err rc before after token
  local out_exists=0
  local -a argv=()
  tmp="$(mktemp -d)"
  out="$tmp/out"
  stdoutf="$tmp/stdout"
  err="$tmp/stderr"
  if [ -n "$args" ]; then
    read -r -a argv <<< "$args"
  fi
  # 只替换精确 token；不执行 shell
  local -a final=()
  if [ ${#argv[@]} -gt 0 ]; then
    for token in "${argv[@]}"; do
      case "$token" in
        "<src>") final+=("$REPO_ROOT/$RUNTIME_CODES_CLI_SRC") ;;
        "out") final+=("$out") ;;
        *) final+=("$token") ;;
      esac
    done
  fi
  before=$(find "$tmp" -mindepth 1 -maxdepth 1 ! -name out ! -name stdout ! -name stderr | wc -l | tr -d ' ')
  rc=0
  if [ ${#final[@]} -gt 0 ]; then
    add_cmd "timeout 30 $ZORC ${final[*]} (cwd=$tmp)"
    ( cd "$tmp" && env -u ASAN_OPTIONS -u ASAN_SYMBOLIZER_PATH timeout 30 "$ZORC" "${final[@]}" > "$stdoutf" 2> "$err" ) || rc=$?
  else
    add_cmd "timeout 30 $ZORC (no args, cwd=$tmp)"
    ( cd "$tmp" && env -u ASAN_OPTIONS -u ASAN_SYMBOLIZER_PATH timeout 30 "$ZORC" > "$stdoutf" 2> "$err" ) || rc=$?
  fi
  after=$(find "$tmp" -mindepth 1 -maxdepth 1 ! -name out ! -name stdout ! -name stderr | wc -l | tr -d ' ')
  out_exists=0
  [ -e "$out" ] && out_exists=1
  [ ! -f "$err" ] && : > "$err"
  [ ! -f "$stdoutf" ] && : > "$stdoutf"
  local ok=1
  [ "$rc" -eq "$expect_rc" ] || { fail "F-RC-6 cli $case_id rc=$rc（期望 $expect_rc）"; ok=0; }
  if [ "$expect_artifact" = "none" ]; then
    [ "$out_exists" -eq 0 ] || { fail "F-RC-10 cli $case_id 期望零产物但 out 存在"; ok=0; }
  else
    [ "$out_exists" -eq 1 ] || { fail "F-RC-10 cli $case_id 期望产物但 out 缺失"; ok=0; }
  fi
  [ "$before" -eq "$after" ] || { fail "F-RC-10 cli $case_id 产生额外文件"; ok=0; }
  split_not_contains "$stdout_not" "cli $case_id stdout" "$stdoutf" || ok=0
  split_contains "$stderr_contains" "cli $case_id stderr" "$err" || ok=0
  if grep -Eq 'ERROR: (AddressSanitizer|LeakSanitizer)' "$err"; then
    fail "F-RC-9 cli $case_id stderr 出现 sanitizer 报告"; ok=0
  fi
  check_no_temp_residue "$tmp" "cli $case_id" || ok=0
  echo "$( [ "$ok" -eq 1 ] && echo PASS || echo FAIL ) $case_id rc=$rc artifact=$( [ "$out_exists" -eq 1 ] && echo exists || echo none )" >> "$OUT_DIR/cli-controls.log"
  rm -rf "$tmp"
}

check_helper_and_query() {
  local f token
  local -a existing=()
  for f in runtime/zor_rt.c runtime/zor_rt.h; do
    if [ -f "$f" ]; then
      existing+=("$f")
    else
      fail "F-RC-4 缺少运行期 helper：$f"
    fi
  done
  if [ ${#existing[@]} -gt 0 ]; then
    for token in E3001 E3002 'zor:' 'fflush' 'exit(101)'; do
      grep -Fq -- "$token" "${existing[@]}" || fail "F-RC-14 runtime helper 缺少 $token"
    done
    if grep -Eq 'abort\(|_exit\(' "${existing[@]}"; then
      fail "F-RC-14 runtime helper 含 abort(/_exit("
    fi
  fi
  if grep -R -Eq 'E300[12]' compiler/src 2>/dev/null; then
    fail "F-RC-14 compiler/src/** 出现 E30xx 串（应只在 runtime helper）"
  fi
}
check_visible_surface() {
  local added doc_scan
  if ! git rev-parse --verify "$RUNTIME_CODES_BASE_REF" >/dev/null 2>&1; then
    fail "F-RC-14 base ref 不可用：$RUNTIME_CODES_BASE_REF（文档新增行检查无法执行）"
    return
  fi
  doc_scan="$(mktemp)"
  git diff --unified=0 "$RUNTIME_CODES_BASE_REF" -- README.md CHANGELOG.md docs 2>/dev/null \
    | grep -E '^\+' | grep -v '^+++' > "$doc_scan" || true
  if grep -Fq -- '-Zoverflow' "$doc_scan"; then
    fail "F-RC-14 新增用户文档行泄漏 -Zoverflow"
  fi
  if grep -Eq -- '-Og|-O1|-O3|-Os' "$doc_scan"; then
    warn "新增文档行出现 -Og/-O1/-O3/-Os（需确认未写成可用档位）"
  fi
  rm -f "$doc_scan"
}

main() {
  host_check
  echo "[runtime-codes] host=${HOSTNAME:-unknown} $(uname -sm) $(utc_now)"
  validate_runtime_manifest "$RUNTIME_CODES_MANIFEST"
  validate_width_manifest "$WIDTH_FAILCLOSED_MANIFEST"
  validate_cli_manifest "$CLI_CASES_MANIFEST"
  check_helper_and_query
  check_visible_surface

  # 快照与 hash（即使后续用例失败也保留）
  cp "$RUNTIME_CODES_MANIFEST" "$OUT_DIR/runtime-codes.tsv" 2>/dev/null || true
  cp "$WIDTH_FAILCLOSED_MANIFEST" "$OUT_DIR/width-failclosed.tsv" 2>/dev/null || true
  cp "$CLI_CASES_MANIFEST" "$OUT_DIR/cli-cases.tsv" 2>/dev/null || true
  if [ -f "$OUT_DIR/runtime-codes.tsv" ]; then
    sha256_of "$OUT_DIR/runtime-codes.tsv" > "$OUT_DIR/manifest.sha256"
  fi

  if [ "$FAILS" -eq 0 ]; then
    local case_id src opt overflow extra rc code sha kind notes
    while IFS=$'\034' read -r case_id src opt overflow extra rc code sha kind notes; do
      run_runtime_case "$case_id" "$src" "$opt" "$overflow" "$extra" "$rc" "$code" "$sha" "$kind"
    done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7,$8,$9,$10}' "$RUNTIME_CODES_MANIFEST")

    local wcase wsrc wrc wart wcontains wnot wnotes
    while IFS=$'\034' read -r wcase wsrc wrc wart wcontains wnot wnotes; do
      run_width_case "$wcase" "$wsrc"
    done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7}' "$WIDTH_FAILCLOSED_MANIFEST")

    local ccase cargs crc cart cout cerr cnotes
    while IFS=$'\034' read -r ccase cargs crc cart cout cerr cnotes; do
      run_cli_case "$ccase" "$cargs" "$crc" "$cart" "$cout" "$cerr"
    done < <(awk -F'\t' -v OFS='\034' 'NR>1{print $1,$2,$3,$4,$5,$6,$7}' "$CLI_CASES_MANIFEST")
  fi

  {
    echo "utc_end=$(utc_now)"
    echo "cases=$(awk 'END{print NR-1}' "$RUNTIME_CODES_MANIFEST" 2>/dev/null || echo 0)"
    echo "width_cases=$(awk 'END{print NR-1}' "$WIDTH_FAILCLOSED_MANIFEST" 2>/dev/null || echo 0)"
    echo "cli_cases=$(awk 'END{print NR-1}' "$CLI_CASES_MANIFEST" 2>/dev/null || echo 0)"
    echo "fails=$FAILS"
    echo "warns=$WARNS"
    echo "verdict=$([ "$FAILS" -eq 0 ] && echo PASS || echo FAIL)"
  } >> "$OUT_DIR/versions.txt"

  if [ "$FAILS" -ne 0 ]; then
    echo "[runtime-codes] 判定=FAIL fails=$FAILS warns=$WARNS"
    return 1
  fi
  echo "[runtime-codes] 判定=PASS（29 runtime＋6 width＋8 CLI；warns=$WARNS）"
  return 0
}

main "$@"
