#!/usr/bin/env bash
# tests/stdout/run_stdout_contract.sh — L2c stdout／CLI 契约判据（QA 资产）
#
# 用法：
#   bash tests/stdout/run_stdout_contract.sh --strict [--allow-nonlinux]
#   bash tests/stdout/run_stdout_contract.sh --validate-only [--allow-nonlinux]
#
# 环境：
#   ZORC_BIN  zorcc 路径（默认 PATH 中 zorcc）
#   CC         C 编译器（默认 cc；CI 注入 clang-18）
#   OUT_DIR   证据目录（默认 ci/logs/stdout-contract-<UTC>-<pid>）
#
# 输出契约（ci/run_ci.sh stage_stdout_contract 依赖）：
#   恰一行 PASS|stdout-contract|... 或 FAIL|stdout-contract|...
#   恰一行 PASS|cli-contract|...   或 FAIL|cli-contract|...
#   显式 FAIL 优先；任一段 FAIL → 脚本 rc=1。
#
# 判据：
#   - 静态：48 runtime＋10 negative＋14 CLI＝72；new-cases 72；schema／路径／重复／expected 缺件。
#   - 旧资产快照：legacy-snapshot.sha256 逐文件 sha256；旧五值／runtime-codes／manifest 计数复核。
#   - runtime：两轮构建＋运行；stdout 逐字节 cmp；run 模式 stderr 0 B；terminate 模式 rc=101 且只判 stdout。
#   - negative：两轮 rc=1、零产物、stderr 含判词；不新增 E 码。
#   - CLI：NDJSON 六键固定序／行序／文本对照；版本短路；-o 零残留；usage 不泄露内部位。
#   - strict 不 SKIP；宿主默认只允许 Linux x86_64（--allow-nonlinux 仅本地预检，CI 不传）。

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPO_ROOT_REAL="$(cd "$REPO_ROOT" && pwd -P)"
STDOUT_DIR="$REPO_ROOT/tests/stdout"
RUNTIME_TSV="$STDOUT_DIR/stdout-cases.tsv"
NEGATIVE_TSV="$STDOUT_DIR/negative.tsv"
CLI_TSV="$STDOUT_DIR/cli-cases.tsv"
NEW_TSV="$STDOUT_DIR/new-cases.tsv"
SNAPSHOT="$STDOUT_DIR/legacy-snapshot.sha256"
NDJSON_CHECK="$STDOUT_DIR/ndjson_check.py"
ZORC="${ZORC_BIN:-zorcc}"
CC_REAL="${CC:-cc}"
OUT_DIR="${OUT_DIR:-}"
STRICT=0
VALIDATE_ONLY=0
ALLOW_NONLINUX=0
EXPECT_RUNTIME=48
EXPECT_NEGATIVE=10
EXPECT_CLI=14
EXPECT_TOTAL=72

while [ $# -gt 0 ]; do
  case "$1" in
    --strict) STRICT=1; shift ;;
    --validate-only) VALIDATE_ONLY=1; shift ;;
    --allow-nonlinux) ALLOW_NONLINUX=1; shift ;;
    --out)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[stdout-contract] --out 缺值" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,36p' "$0"; exit 0 ;;
    *) echo "[stdout-contract] 未知参数 '$1'" >&2; exit 2 ;;
  esac
done
if [ "$STRICT" = 0 ] && [ "$VALIDATE_ONLY" = 0 ]; then
  echo "[stdout-contract] 必须指定 --strict 或 --validate-only" >&2
  exit 2
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
utc_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
now_utc="$(utc_now)"
if [ -z "$OUT_DIR" ]; then
  OUT_DIR="$REPO_ROOT/ci/logs/stdout-contract-${now_utc//:/-}-$$"
fi
mkdir -p "$OUT_DIR" || { echo "[stdout-contract] 无法创建 OUT_DIR=$OUT_DIR" >&2; exit 2; }
OUT_DIR="$(cd "$OUT_DIR" && pwd -P)"

TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then TIMEOUT_BIN="gtimeout"
else echo "[stdout-contract] WARN: 无 timeout/gtimeout，本地预检改为直接执行（CI 必须有 timeout）"
fi

so_fail=(); so_n=0
cli_fail=(); cli_n=0
so_fail_msg() { so_fail+=("$1"); so_n=$((so_n+1)); }
cli_fail_msg() { cli_fail+=("$1"); cli_n=$((cli_n+1)); }

# ---------- 静态校验 ----------
validate_assets() {
  local bad=0
  for f in "$RUNTIME_TSV" "$NEGATIVE_TSV" "$CLI_TSV" "$NEW_TSV" "$SNAPSHOT" "$NDJSON_CHECK"; do
    if [ ! -f "$f" ]; then
      so_fail_msg "缺件:${f#$REPO_ROOT/}"
      bad=1
    fi
  done
  [ "$bad" = 0 ] || return 1

  local hdr rows
  hdr="$(head -n1 "$RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "stdout-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$CLI_TSV")"
  [ "$hdr" = $'case_id\tmode\targs\texpect_rc\tnotes' ] || { so_fail_msg "cli-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$NEW_TSV")"
  [ "$hdr" = $'case_id\tface\tkind\tsource\texpected\tnotes' ] || { so_fail_msg "new-cases.tsv 表头"; bad=1; }
  [ "$bad" = 0 ] || return 1

  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME" ] || { so_fail_msg "runtime 行数=$rows 期望 $EXPECT_RUNTIME"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE" ] || { so_fail_msg "negative 行数=$rows 期望 $EXPECT_NEGATIVE"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$CLI_TSV")"
  [ "$rows" = "$EXPECT_CLI" ] || { so_fail_msg "CLI 行数=$rows 期望 $EXPECT_CLI"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$NEW_TSV")"
  [ "$rows" = "$EXPECT_TOTAL" ] || { so_fail_msg "new-cases 行数=$rows 期望 $EXPECT_TOTAL"; bad=1; }

  local cid src mode erc exp flags note seen=" "
  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S[1-6]-*) ;; *) so_fail_msg "非法 runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$RUNTIME_TSV"

  local contains notc artifact
  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    [ -n "$contains" ] || { so_fail_msg "$cid 缺 stderr 判词"; bad=1; }
  done < "$NEGATIVE_TSV"

  while IFS=$'\t' read -r cid mode args erc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$mode" in ndjson-one|ndjson-two|ndjson-escape|ndjson-utf8|ndjson-o|ndjson-check|version|version-short|version-json|o-ok|o-fail|o-missing|usage|usage-flags) ;; *) so_fail_msg "$cid CLI mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|1|2) ;; *) so_fail_msg "$cid CLI expect_rc 非法"; bad=1 ;; esac
  done < "$CLI_TSV"
  [ "$bad" = 0 ] || return 1
  return 0
}

validate_snapshot() {
  if [ ! -f "$SNAPSHOT" ]; then
    so_fail_msg "缺 legacy-snapshot.sha256"
    return 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    ( cd "$REPO_ROOT" && sha256sum -c --quiet "$SNAPSHOT" ) >/dev/null 2>&1 || { so_fail_msg "旧资产快照不一致"; return 1; }
  else
    ( cd "$REPO_ROOT" && shasum -a 256 -c "$SNAPSHOT" ) >/dev/null 2>&1 || { so_fail_msg "旧资产快照不一致"; return 1; }
  fi
  return 0
}

validate_legacy_counts() {
  local bad=0 pos neg gold cmp specs manifest runtime width rcli
  pos="$(find "$REPO_ROOT/tests/specs/positive" -type f -name '*.zor' | wc -l | tr -d ' ')"
  neg="$(find "$REPO_ROOT/tests/specs/negative" -type f -name '*.zor' | wc -l | tr -d ' ')"
  gold="$(find "$REPO_ROOT/tests/specs/golden" -type f | wc -l | tr -d ' ')"
  cmp="$(find "$REPO_ROOT/tests/specs/compare" -type f -name '*-annotated.rs' | wc -l | tr -d ' ')"
  specs="$(find "$REPO_ROOT/tests/specs" -type f -name '*.zor' | wc -l | tr -d ' ')"
  [ "$pos" = 41 ] || { so_fail_msg "positive=$pos 期望 41"; bad=1; }
  [ "$neg" = 53 ] || { so_fail_msg "negative=$neg 期望 53"; bad=1; }
  [ "$gold" = 53 ] || { so_fail_msg "golden=$gold 期望 53"; bad=1; }
  [ "$cmp" = 3 ] || { so_fail_msg "compare=$cmp 期望 3"; bad=1; }
  [ "$specs" = 94 ] || { so_fail_msg "specs .zor=$specs 期望 94"; bad=1; }
  manifest="$(awk 'NR>1 && NF {n++} END{print n+0}' "$REPO_ROOT/tests/runtime/manifest.tsv")"
  [ "$manifest" = 48 ] || { so_fail_msg "manifest=$manifest 期望 48"; bad=1; }
  runtime="$(awk 'NR>1 && NF {n++} END{print n+0}' "$REPO_ROOT/tests/runtime/runtime-codes.tsv")"
  width="$(awk 'NR>1 && NF {n++} END{print n+0}' "$REPO_ROOT/tests/runtime/width-failclosed.tsv")"
  rcli="$(awk 'NR>1 && NF {n++} END{print n+0}' "$REPO_ROOT/tests/runtime/cli-cases.tsv")"
  [ "$runtime" = 29 ] || { so_fail_msg "runtime-codes=$runtime 期望 29"; bad=1; }
  [ "$width" = 6 ] || { so_fail_msg "width=$width 期望 6"; bad=1; }
  [ "$rcli" = 8 ] || { so_fail_msg "runtime CLI=$rcli 期望 8"; bad=1; }
  [ "$bad" = 0 ]
}

validate_host() {
  local os arch
  os="$(uname -s)"; arch="$(uname -m)"
  echo "[stdout-contract] host=${os}/${arch} allow_nonlinux=${ALLOW_NONLINUX}"
  if [ "$os" != "Linux" ] || [ "$arch" != "x86_64" ]; then
    if [ "$ALLOW_NONLINUX" = 1 ]; then
      echo "[stdout-contract] WARN: 非 Linux/x86_64 本地预检；终证必须 Linux x86_64（O-L2c-04）"
      return 0
    fi
    so_fail_msg "宿主 ${os}/${arch} 非 Linux/x86_64（终证平台）"
    cli_fail_msg "宿主 ${os}/${arch} 非 Linux/x86_64（终证平台）"
    return 1
  fi
  return 0
}

# ---------- 执行辅助 ----------
run_capture() { # run_capture <rcfile> <outfile> <errfile> <timeout> <cmd...>
  local rcfile="$1" outfile="$2" errfile="$3" tmo="$4"; shift 4
  local rc=0
  if [ -n "$TIMEOUT_BIN" ]; then
    "$TIMEOUT_BIN" "$tmo" "$@" >"$outfile" 2>"$errfile" || rc=$?
  else
    "$@" >"$outfile" 2>"$errfile" || rc=$?
  fi
  printf '%s' "$rc" >"$rcfile"
  return 0
}

# ---------- runtime ----------
run_runtime() {
  local cid src mode erc exp flags note round work bin rc stdout stderr
  local prev_rc="" prev_sha=""
  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    local ok=1
    for round in 1 2; do
      work="$OUT_DIR/runtime/$cid/round$round"
      mkdir -p "$work"
      bin="$work/case.bin"
      if [ -n "$flags" ] && [ "$flags" != "-" ]; then
        run_capture "$work/build.rc" "$work/build.out" "$work/build.err" 120 \
          env CC="$CC_REAL" "$ZORC" "$REPO_ROOT/$src" -o "$bin" "$flags"
      else
        run_capture "$work/build.rc" "$work/build.out" "$work/build.err" 120 \
          env CC="$CC_REAL" "$ZORC" "$REPO_ROOT/$src" -o "$bin"
      fi
      rc="$(cat "$work/build.rc")"
      if [ "$rc" != "0" ]; then
        so_fail_msg "$cid r$round 构建 rc=$rc"
        ok=0; break
      fi
      run_capture "$work/run.rc" "$work/stdout" "$work/stderr" 10 "$bin"
      rc="$(cat "$work/run.rc")"
      if [ "$mode" = "run" ]; then
        if [ "$rc" != "$erc" ]; then so_fail_msg "$cid r$round 运行 rc=$rc 期望 $erc"; ok=0; fi
        if [ -s "$work/stderr" ]; then so_fail_msg "$cid r$round stderr 非空"; ok=0; fi
      else
        if [ "$rc" != "101" ]; then so_fail_msg "$cid r$round 终止 rc=$rc 期望 101"; ok=0; fi
      fi
      stdout="$work/stdout"
      if ! cmp -s "$stdout" "$STDOUT_DIR/$exp"; then
        so_fail_msg "$cid r$round stdout 与 expected 不一致"
        ok=0
      fi
      local sha; sha="$(sha256_of "$stdout")"
      printf '%s  %s\n' "$sha" "${exp}" > "$work/stdout.sha256"
      if [ "$round" = 1 ]; then prev_rc="$rc"; prev_sha="$sha"; else
        [ "$rc" = "$prev_rc" ] || { so_fail_msg "$cid 两轮 rc 不一致"; ok=0; }
        [ "$sha" = "$prev_sha" ] || { so_fail_msg "$cid 两轮 stdout 不一致"; ok=0; }
      fi
      [ "$ok" = 1 ] || break
    done
    [ "$ok" = 1 ] && echo "[stdout-contract] PASS $cid"
  done < "$RUNTIME_TSV"
}

# ---------- negative ----------
run_negative() {
  local cid src erc artifact contains notc note round work out rc err
  local prev_rc="" prev_sha=""
  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    local ok=1
    for round in 1 2; do
      work="$OUT_DIR/negative/$cid/round$round"
      mkdir -p "$work"
      out="$work/out"
      run_capture "$work/rc" "$work/stdout" "$work/stderr" 120 \
        env CC="$CC_REAL" "$ZORC" "$REPO_ROOT/$src" -o "$out"
      rc="$(cat "$work/rc")"
      if [ "$rc" != "$erc" ]; then so_fail_msg "$cid r$round rc=$rc 期望 $erc"; ok=0; fi
      if [ -e "$out" ]; then so_fail_msg "$cid r$round 产物存在"; ok=0; fi
      if ! grep -qF -- "$contains" "$work/stderr"; then so_fail_msg "$cid r$round stderr 缺判词 '$contains'"; ok=0; fi
      if [ -n "$notc" ] && grep -qF -- "$notc" "$work/stderr"; then so_fail_msg "$cid r$round stderr 含禁词 '$notc'"; ok=0; fi
      local sha; sha="$(sha256_of "$work/stderr")"
      if [ "$round" = 1 ]; then prev_rc="$rc"; prev_sha="$sha"; else
        [ "$rc" = "$prev_rc" ] || { so_fail_msg "$cid 两轮 rc 不一致"; ok=0; }
        [ "$sha" = "$prev_sha" ] || { so_fail_msg "$cid 两轮 stderr 不一致"; ok=0; }
      fi
      [ "$ok" = 1 ] || break
    done
    [ "$ok" = 1 ] && echo "[stdout-contract] PASS $cid"
  done < "$NEGATIVE_TSV"
}

# ---------- CLI ----------
run_cli() {
  local W="$OUT_DIR/cli"
  mkdir -p "$W"
  local BAD1="$W/bad1.zor" BAD2="$W/bad2.zor" GOOD="$W/good.zor" NOFILE="$W/no-such.zor"
  printf 'fn main() { foo(); }\n' > "$BAD1"
  printf 'fn main() { foo(); bar(); }\n' > "$BAD2"
  printf 'fn main() { println("ok"); }\n' > "$GOOD"

  local ver
  ver="$(awk -F'"' '/^version = /{print $2; exit}' "$REPO_ROOT/compiler/Cargo.toml")"
  if ! printf '%s' "$ver" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    cli_fail_msg "Cargo.toml 版本不可解析"
    return
  fi

  local escape_dir="$W/escape" utf8_dir="$W/utf8"
  mkdir -p "$escape_dir" "$utf8_dir"
  local ESCAPE="$escape_dir/a\"b\\c.zor" UTF8="$utf8_dir/错误.zor"
  cp "$BAD1" "$ESCAPE"
  cp "$BAD1" "$UTF8"

  # C1-01 one JSON line + text-mode parity
  local d="$W/C1-01"; mkdir -p "$d"
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$BAD1"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD1"
  if [ "$(cat "$d/rc1")" != 1 ] || [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-01 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" one "$d/err1" "$d/err2" "$BAD1" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-01 NDJSON 校验失败"
  else echo "[cli-contract] PASS C1-01"; fi

  # C1-02 two lines, source order
  d="$W/C1-02"; mkdir -p "$d"
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$BAD2"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD2"
  if [ "$(cat "$d/rc1")" != 1 ] || [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-02 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" two "$d/err1" "$d/err2" "$BAD2" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-02 NDJSON 行序/字段失败"
  else echo "[cli-contract] PASS C1-02"; fi

  # C1-03 escaped path
  d="$W/C1-03"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$ESCAPE"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-03 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" path "$d/err" "$ESCAPE" escape >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-03 JSON 路径转义失败"
  else echo "[cli-contract] PASS C1-03"; fi

  # C1-04 UTF-8 path
  d="$W/C1-04"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$UTF8"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-04 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" path "$d/err" "$UTF8" utf8 >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-04 UTF-8 路径失败"
  else echo "[cli-contract] PASS C1-04"; fi

  # C1-05 with -o: zero artifact
  d="$W/C1-05"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$BAD1" -o "$d/artifact"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-05 rc 非 1"
  elif [ -e "$d/artifact" ]; then cli_fail_msg "C1-05 失败留产物"
  elif ! python3 "$NDJSON_CHECK" schema "$d/err" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-05 NDJSON schema 失败"
  else echo "[cli-contract] PASS C1-05"; fi

  # C1-06 check-mode success: rc0, stderr 0 B; failure still rc1
  d="$W/C1-06"; mkdir -p "$d"
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$GOOD"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" --diag=json "$BAD1"
  if [ "$(cat "$d/rc1")" != 0 ] || [ -s "$d/err1" ] || [ -s "$d/out1" ]; then
    cli_fail_msg "C1-06 成功检查态非静默 rc0"
  elif [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-06 失败检查态 rc 非 1"
  else echo "[cli-contract] PASS C1-06"; fi

  # C2 version cases
  d="$W/C2-01"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --version
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-01 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-01 stdout/ stderr 不符（期望 zorcc $ver）"
  else echo "[cli-contract] PASS C2-01"; fi

  d="$W/C2-02"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" -V --bogus "$NOFILE"
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-02 版本未短路 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-02 版本短路输出不符"
  else echo "[cli-contract] PASS C2-02"; fi

  d="$W/C2-03"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --version --diag=json
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-03 版本未短路 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-03 版本与 JSON 同现输出不符"
  else echo "[cli-contract] PASS C2-03"; fi

  # C3 -o cases
  d="$W/C3-01"; mkdir -p "$d/outdir"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" "$GOOD" -o "$d/outdir/out"
  local nfiles; nfiles="$(find "$d/outdir" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  if [ "$(cat "$d/rc")" != 0 ] || [ ! -x "$d/outdir/out" ] || [ "$nfiles" != 1 ]; then
    cli_fail_msg "C3-01 原子发布失败（rc/产物/残留）"
  else
    run_capture "$d/runrc" "$d/runout" "$d/runerr" 10 "$d/outdir/out"
    if [ "$(cat "$d/runrc")" != 0 ] || [ "$(cat "$d/runout")" != "ok" ]; then
      cli_fail_msg "C3-01 产物运行不符"
    else echo "[cli-contract] PASS C3-01"; fi
  fi

  d="$W/C3-02"; mkdir -p "$d"
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" "$BAD1" -o "$d/artifact"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD1"
  if [ "$(cat "$d/rc1")" != 1 ] || [ -e "$d/artifact" ]; then
    cli_fail_msg "C3-02 失败 rc/产物不符"
  elif ! cmp -s "$d/err1" "$d/err2"; then
    cli_fail_msg "C3-02 -o 改变文本诊断"
  else echo "[cli-contract] PASS C3-02"; fi

  d="$W/C3-03"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" "$GOOD" -o
  if [ "$(cat "$d/rc")" != 2 ]; then cli_fail_msg "C3-03 -o 缺值 rc 非 2"
  elif ! grep -qF -- "-o" "$d/err"; then cli_fail_msg "C3-03 stderr 未指认 -o"
  else echo "[cli-contract] PASS C3-03"; fi

  # C4 usage
  d="$W/C4-01"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC"
  if [ "$(cat "$d/rc")" != 2 ] || ! grep -qF -- "usage: zorcc" "$d/err"; then
    cli_fail_msg "C4-01 无参 usage 不符"
  else echo "[cli-contract] PASS C4-01"; fi

  d="$W/C4-02"; mkdir -p "$d"
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC"
  local leak=0 tok
  for tok in "-Zoverflow" "-Og" "-O1" "-O3" "-Os"; do
    grep -qF -- "$tok" "$d/err" && leak=1
  done
  if [ "$leak" = 1 ]; then cli_fail_msg "C4-02 usage 泄露内部/未开放旗标"
  else echo "[cli-contract] PASS C4-02"; fi
}

# ---------- main ----------
echo "[stdout-contract] time=$now_utc out=$OUT_DIR repo=$REPO_ROOT"
static_bad=0
validate_assets || static_bad=1
validate_snapshot || static_bad=1
validate_legacy_counts || static_bad=1
if [ "$static_bad" != 0 ]; then
  so_fail_msg "静态校验失败"
fi

if [ "$VALIDATE_ONLY" = 1 ]; then
  : > "$OUT_DIR/validate-only.log"
  echo "[stdout-contract] validate-only static_bad=$static_bad" | tee -a "$OUT_DIR/validate-only.log"
  if [ "$so_n" -gt 0 ]; then for m in "${so_fail[@]}"; do echo "[stdout-contract] STATIC_FAIL: $m" | tee -a "$OUT_DIR/validate-only.log"; done; fi
  if [ "$static_bad" = 0 ]; then
    echo "PASS|stdout-contract|validate-only runtime=$EXPECT_RUNTIME negative=$EXPECT_NEGATIVE"
    echo "PASS|cli-contract|validate-only cases=$EXPECT_CLI"
    exit 0
  fi
  echo "FAIL|stdout-contract|validate-only"
  echo "FAIL|cli-contract|validate-only"
  exit 1
fi

validate_host || true
if ! command -v "$ZORC" >/dev/null 2>&1 && [ ! -x "$ZORC" ]; then
  so_fail_msg "zorcc 不可执行: $ZORC"
  cli_fail_msg "zorcc 不可执行: $ZORC"
fi
if ! command -v "$CC_REAL" >/dev/null 2>&1; then
  so_fail_msg "CC 不可执行: $CC_REAL"
  cli_fail_msg "CC 不可执行: $CC_REAL"
fi
if [ "$VALIDATE_ONLY" = 0 ] && [ -z "$TIMEOUT_BIN" ] && [ "$ALLOW_NONLINUX" = 0 ]; then
  so_fail_msg "缺 timeout 命令"
  cli_fail_msg "缺 timeout 命令"
fi

if [ "$so_n" -eq 0 ]; then run_runtime; fi
if [ "$so_n" -eq 0 ]; then run_negative; fi
# CLI 独立于 stdout 段：即使 runtime 失败也继续，便于一次看全两段。

if [ "$so_n" -eq 0 ]; then
  echo "PASS|stdout-contract|runtime=$EXPECT_RUNTIME negative=$EXPECT_NEGATIVE fail=0"
  so_rc=0
else
  for m in "${so_fail[@]}"; do echo "[stdout-contract] FAIL_DETAIL: $m"; done
  echo "FAIL|stdout-contract|${so_fail[*]}"
  so_rc=1
fi
if [ "$cli_n" -eq 0 ]; then
  echo "PASS|cli-contract|cases=$EXPECT_CLI fail=0"
  cli_rc=0
else
  for m in "${cli_fail[@]}"; do echo "[cli-contract] FAIL_DETAIL: $m"; done
  echo "FAIL|cli-contract|${cli_fail[*]}"
  cli_rc=1
fi
{
  echo "--- summary $now_utc ---"
  echo "host=$(uname -s)/$(uname -m) explicit_allow_nonlinux=$ALLOW_NONLINUX"
  echo "stdout_fail=$so_n cli_fail=$cli_n"
  echo "out=$OUT_DIR"
} | tee "$OUT_DIR/summary.txt"
[ "$so_rc" -eq 0 ] && [ "$cli_rc" -eq 0 ] && exit 0
exit 1
