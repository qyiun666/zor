#!/usr/bin/env bash
# tests/packaging/run_packaging_smoke.sh — L2d QA-01 安装树最小切片（P-01～P-04）
#
# 用法：
#   bash tests/packaging/run_packaging_smoke.sh --strict [--allow-nonlinux]
#   bash tests/packaging/run_packaging_smoke.sh --validate-only [--allow-nonlinux]
#
# 环境：
#   ZORC_BIN           zorcc 路径（默认 PATH 中 zorcc）
#   CC                 C 编译器（默认 cc；CI 注入 clang-18）
#   OUT_DIR            证据目录（默认 /tmp/zor-packaging-smoke-<UTC>-<pid>）
#   ZOR_INSTALLER      安装器（默认 <repo>/ci/install.sh）
#   ZOR_EXPECT_VERSION 期望 --version（默认读取 compiler/Cargo.toml；账本后自动为 0.3.3；
#                      显式覆写用于专项校验，不改变宿主闸）
#
# 输出契约（ci/run_ci.sh stage_packaging_smoke 依赖）：
#   Linux x86_64 严格模式恰一行 PASS|packaging-smoke|cases=4 fail=0 ran=4 或 FAIL|packaging-smoke|...
#   非 Linux 或版本覆写（--allow-nonlinux 本地预检）输出 PASS|packaging-smoke|local-precheck ...；
#   该行不得作为门禁证据。显式 FAIL 优先；任一条失败 → rc=1；不得 SKIP。
#
# 判据：
#   P-01 canonical 安装（prefix/bin/zorcc＋prefix/share/zor/runtime/{zor_rt.c,h}）；
#        从源码树外 cwd、ZOR_RUNTIME_DIR 未设运行 --version／构建并运行 hello；-O0 E3001 样本。
#   P-02 删除安装树 helper → rc=2、零产物、stderr 列出已搜索路径、不得链接失败。
#   P-03 DESTDIR 暂存：文件集精确 3 个、目录集精确、可执行位／helper 模式正确、树清单可复算、
#        安装树文本无构建机绝对路径泄漏。
#   P-04 ZOR_RUNTIME_DIR 指向缺件目录 → rc=2、零产物、不得回退 cwd／源码树候选。
#   全部判据不依赖单元测试；任何缺件／空集／计数不符一律 FAIL。

set -uo pipefail

PKG_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "$PKG_DIR/../.." && pwd -P)"
CASES_TSV="$PKG_DIR/packaging-cases.tsv"
HELLO_SRC="$PKG_DIR/cases/hello.zor"
E3001_SRC="$PKG_DIR/cases/e3001.zor"
HELLO_EXPECTED="$PKG_DIR/expected/hello.stdout"
E3001_EXPECTED="$PKG_DIR/expected/e3001.stdout"
RUNTIME_DIR="$REPO_ROOT/runtime"
ZORC="${ZORC_BIN:-zorcc}"
ZORC_REAL=""
CC_REAL="${CC:-cc}"
INSTALLER="${ZOR_INSTALLER:-$REPO_ROOT/ci/install.sh}"
OUT_DIR="${OUT_DIR:-}"
EXPECT_VERSION="${ZOR_EXPECT_VERSION:-}"
EXPECT_VERSION_SOURCE="cargo"
STRICT=0
VALIDATE_ONLY=0
ALLOW_NONLINUX=0
LOCAL_PRECHECK=0
EXPECT_CASES=4
CASES_OK="P-01 P-02 P-03 P-04"

while [ $# -gt 0 ]; do
  case "$1" in
    --strict) STRICT=1; shift ;;
    --validate-only) VALIDATE_ONLY=1; shift ;;
    --allow-nonlinux) ALLOW_NONLINUX=1; shift ;;
    --out)
      [ $# -ge 2 ] && [ -n "$2" ] || { echo "[packaging-smoke] --out 缺值" >&2; exit 2; }
      OUT_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "[packaging-smoke] 未知参数 '$1'" >&2; exit 2 ;;
  esac
done
if [ "$STRICT" = 0 ] && [ "$VALIDATE_ONLY" = 0 ]; then
  echo "[packaging-smoke] 必须指定 --strict 或 --validate-only" >&2
  exit 2
fi
if [ "$STRICT" = 1 ] && [ "$VALIDATE_ONLY" = 1 ]; then
  echo "[packaging-smoke] --strict 与 --validate-only 不可同时使用" >&2
  exit 2
fi

utc_now() { date -u +%Y-%m-%dT%H:%M:%SZ; }
now_utc="$(utc_now)"
if [ -z "$OUT_DIR" ]; then
  OUT_DIR="${TMPDIR:-/tmp}/zor-packaging-smoke-${now_utc//:/-}-$$"
fi
mkdir -p "$OUT_DIR" || { echo "[packaging-smoke] 无法创建 OUT_DIR=$OUT_DIR" >&2; exit 2; }
OUT_DIR="$(cd "$OUT_DIR" && pwd -P)"
SUMMARY="$OUT_DIR/summary.txt"
: > "$SUMMARY"

TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then TIMEOUT_BIN="gtimeout"
else echo "[packaging-smoke] WARN: 无 timeout/gtimeout，本地预检改为直接执行（CI 必须有 timeout）"
fi

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}
file_mode() {
  if stat -c '%a' "$1" >/dev/null 2>&1; then stat -c '%a' "$1"; else stat -f '%Lp' "$1"; fi
}
file_size() {
  if stat -c '%s' "$1" >/dev/null 2>&1; then stat -c '%s' "$1"; else stat -f '%z' "$1"; fi
}

CASE_ID=""
pc_case_ok=1
pc_case_fail=0
pc_ran=0
pc_n=0
pc_fail_msg() {
  pc_case_ok=0
  pc_n=$((pc_n+1))
  echo "[packaging-smoke] $CASE_ID FAIL: $*" | tee -a "$SUMMARY" >&2
}
run_capture() { # run_capture <rc_file> <out_file> <err_file> <secs> <cmd...>
  local rc_f="$1" out_f="$2" err_f="$3" secs="$4"
  shift 4
  local rc=0
  if [ -n "$TIMEOUT_BIN" ]; then
    "$TIMEOUT_BIN" "$secs" "$@" >"$out_f" 2>"$err_f" || rc=$?
  else
    "$@" >"$out_f" 2>"$err_f" || rc=$?
  fi
  printf '%s\n' "$rc" > "$rc_f"
  return 0
}
check_rc() { # check_rc <rc_file> <want> <label>
  local got
  got="$(cat "$1" 2>/dev/null || echo missing)"
  [ "$got" = "$2" ] || pc_fail_msg "$3 rc=$got 期望 $2"
}
check_bytes() { # check_bytes <actual> <expected> <label>
  cmp -s "$1" "$2" || pc_fail_msg "$3 字节不符（actual=$(file_size "$1" 2>/dev/null || echo 0)B expected=$(file_size "$2" 2>/dev/null || echo 0)B）"
}
check_empty() { # check_empty <file> <label>
  [ -s "$1" ] && pc_fail_msg "$2 非空（$(file_size "$1")B）：$(head -c 300 "$1" | tr '\n' ' ')"
  return 0
}
check_contains() { # check_contains <file> <literal> <label>
  grep -qF -- "$2" "$1" 2>/dev/null || pc_fail_msg "$3 未包含 '$2'"
}
check_not_link_failure() { # check_not_link_failure <stderr_file> <label>
  if grep -Eq 'undefined reference|collect2|linker command failed|ld: ' "$1" 2>/dev/null; then
    pc_fail_msg "$2 出现链接失败（不得以链接失败代替 rc=2）"
  fi
}
check_no_tmp_residue() { # check_no_tmp_residue <dir> <label>
  local hits
  hits="$(find "$1" -name '.zorcc-tmp-*' -print 2>/dev/null | head -n 20)"
  [ -z "$hits" ] || pc_fail_msg "$2 残留临时件：$hits"
}
check_zero_compile_artifact() { # check_zero_compile_artifact <dir> <out_path> <label>
  [ ! -e "$2" ] || pc_fail_msg "$3 产物仍存在：$2"
  local hits
  hits="$(find "$1" -maxdepth 1 -type f \( -name '*.c' -o -name '*.o' -o -name '*.s' \) -print 2>/dev/null | head -n 20)"
  [ -z "$hits" ] || pc_fail_msg "$3 残留中间件：$hits"
}
resolve_zorc() {
  local p="$ZORC"
  [ -n "$p" ] || return 1
  if [ -f "$p" ]; then
    :
  elif command -v "$p" >/dev/null 2>&1; then
    p="$(command -v "$p")"
  else
    return 1
  fi
  case "$p" in
    /*) ;;
    *) p="$(cd "$(dirname "$p")" && pwd -P)/$(basename "$p")" ;;
  esac
  [ -f "$p" ] && [ -x "$p" ] || return 1
  ZORC_REAL="$p"
  return 0
}

# ---------- 版本期望 ----------
# 冻结 P-01 判据不含硬编码版本；PRD §5“0.3.3（账本后）”由 Cargo.toml 在账本时自动兑现。
derive_expect_version() {
  if [ -z "$EXPECT_VERSION" ]; then
    local manifest="$REPO_ROOT/compiler/Cargo.toml"
    [ -f "$manifest" ] || { echo "[packaging-smoke] STATIC FAIL: 缺 $manifest，无法确定期望版本" >&2; return 1; }
    EXPECT_VERSION="$(awk -F'"' '/^version = /{print $2; exit}' "$manifest")"
    EXPECT_VERSION_SOURCE="cargo"
  else
    EXPECT_VERSION_SOURCE="override"
  fi
  printf '%s\n' "$EXPECT_VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || {
    echo "[packaging-smoke] STATIC FAIL: 期望版本 '$EXPECT_VERSION' 非 semver（来源 $EXPECT_VERSION_SOURCE）" >&2
    return 1
  }
  return 0
}

# ---------- 静态校验 ----------
validate_assets() {
  local bad=0 f hdr ids rows
  for f in "$CASES_TSV" "$HELLO_SRC" "$E3001_SRC" "$HELLO_EXPECTED" "$E3001_EXPECTED" "$RUNTIME_DIR/zor_rt.c" "$RUNTIME_DIR/zor_rt.h"; do
    if [ ! -f "$f" ]; then
      echo "[packaging-smoke] STATIC FAIL: 缺件 ${f#$REPO_ROOT/}" >&2
      bad=1
    fi
  done
  if [ ! -f "$INSTALLER" ]; then
    echo "[packaging-smoke] STATIC FAIL: 缺安装器 $INSTALLER（PR #112 ci/install.sh 接口）" >&2
    bad=1
  fi
  [ "$bad" = 0 ] || return 1

  hdr="$(head -n1 "$CASES_TSV")"
  [ "$hdr" = $'case_id\tmode\texpect_rc\texpect_artifact\tnotes' ] || { echo "[packaging-smoke] STATIC FAIL: packaging-cases.tsv 表头" >&2; bad=1; }
  ids="$(awk -F'\t' 'NR>1 && NF {print $1}' "$CASES_TSV" | LC_ALL=C sort | tr '\n' ' ' | sed 's/ $//')"
  [ "$ids" = "P-01 P-02 P-03 P-04" ] || { echo "[packaging-smoke] STATIC FAIL: case_id 集合='$ids' 期望 '$CASES_OK'" >&2; bad=1; }
  rows="$(awk -F'\t' 'NR>1 && NF {n++} END{print n+0}' "$CASES_TSV")"
  [ "$rows" = "$EXPECT_CASES" ] || { echo "[packaging-smoke] STATIC FAIL: packaging 行数=$rows 期望 $EXPECT_CASES" >&2; bad=1; }
  awk -F'\t' 'NR>1 && NF && NF!=5 {print "[packaging-smoke] STATIC FAIL: 第 " NR " 行列数=" NF " 期望 5"; bad=1} END{exit bad?1:0}' "$CASES_TSV" || bad=1

  [ "$bad" = 0 ]
}

# ---------- 宿主闸 ----------
host_gate() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  if [ "$os" = "Linux" ] && [ "$arch" = "x86_64" ]; then
    return 0
  fi
  if [ "$ALLOW_NONLINUX" = 1 ]; then
    LOCAL_PRECHECK=1
    echo "[packaging-smoke] NOTE: 宿主 $os/$arch 非 Linux x86_64；--allow-nonlinux 仅本地预检，判词为 local-precheck，不作门禁证据" | tee -a "$SUMMARY"
    return 0
  fi
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 host=$os/$arch expected=Linux/x86_64" | tee -a "$SUMMARY"
  return 1
}

# ---------- 公共步骤 ----------
install_tree() { # install_tree <prefix> <evidence_dir> [destdir]
  local prefix="$1" d="$2" destdir="${3:-}"
  local -a args=(--prefix "$prefix" --zorcc "$ZORC_REAL" --runtime "$RUNTIME_DIR")
  if [ -n "$destdir" ]; then
    args=(--destdir "$destdir" "${args[@]}")
  fi
  mkdir -p "$d"
  ( cd "$WORK" && run_capture "$d/install.rc" "$d/install.out" "$d/install.err" 120 \
      env -u ZOR_RUNTIME_DIR bash "$INSTALLER" "${args[@]}" )
  check_rc "$d/install.rc" 0 "安装（prefix=$prefix${destdir:+ destdir=$destdir}）"
}

prepare_cwd() { # prepare_cwd <dir> → hello.zor
  local cwd="$1"
  mkdir -p "$cwd"
  cp "$HELLO_SRC" "$cwd/hello.zor"
  case "$cwd" in
    "$REPO_ROOT"/*) pc_fail_msg "测试 cwd 位于源码树内：$cwd" ;;
  esac
}

# ---------- P-01 ----------
case_p01() {
  CASE_ID=P-01
  local d="$OUT_DIR/cases/P-01" prefix="$WORK/p01-prefix" cwd="$WORK/p01-cwd"
  mkdir -p "$d"
  install_tree "$prefix" "$d"
  [ "$pc_case_ok" = 1 ] || return 0
  [ -x "$prefix/bin/zorcc" ] || pc_fail_msg "安装树缺可执行 bin/zorcc"
  [ -f "$prefix/share/zor/runtime/zor_rt.c" ] || pc_fail_msg "安装树缺 share/zor/runtime/zor_rt.c"
  [ -f "$prefix/share/zor/runtime/zor_rt.h" ] || pc_fail_msg "安装树缺 share/zor/runtime/zor_rt.h"
  prepare_cwd "$cwd"
  cp "$E3001_SRC" "$cwd/e3001.zor"

  ( cd "$cwd" && run_capture "$d/version.rc" "$d/version.out" "$d/version.err" 30 \
      env -u ZOR_RUNTIME_DIR "$prefix/bin/zorcc" --version )
  check_rc "$d/version.rc" 0 "--version"
  check_empty "$d/version.err" "--version stderr"
  printf 'zorcc %s\n' "$EXPECT_VERSION" > "$d/version.expected"
  if ! cmp -s "$d/version.out" "$d/version.expected"; then
    pc_fail_msg "--version stdout='$(tr -d '\n' < "$d/version.out" 2>/dev/null)' 期望 'zorcc $EXPECT_VERSION'"
  fi

  ( cd "$cwd" && run_capture "$d/hello-build.rc" "$d/hello-build.out" "$d/hello-build.err" 120 \
      env -u ZOR_RUNTIME_DIR "$prefix/bin/zorcc" hello.zor -o hello.bin )
  check_rc "$d/hello-build.rc" 0 "hello 构建（源码树外、ZOR_RUNTIME_DIR 未设）"
  check_empty "$d/hello-build.err" "hello 构建 stderr"
  [ -x "$cwd/hello.bin" ] || pc_fail_msg "hello 构建未产出可执行文件"
  if [ -x "$cwd/hello.bin" ]; then
    ( cd "$cwd" && run_capture "$d/hello-run.rc" "$d/hello-run.out" "$d/hello-run.err" 30 \
        env -u ZOR_RUNTIME_DIR ./hello.bin )
    check_rc "$d/hello-run.rc" 0 "hello 运行"
    check_bytes "$d/hello-run.out" "$HELLO_EXPECTED" "hello stdout"
    check_empty "$d/hello-run.err" "hello 运行 stderr"
  fi

  ( cd "$cwd" && run_capture "$d/e3001-build.rc" "$d/e3001-build.out" "$d/e3001-build.err" 120 \
      env -u ZOR_RUNTIME_DIR "$prefix/bin/zorcc" -O0 e3001.zor -o e3001.bin )
  check_rc "$d/e3001-build.rc" 0 "E3001 样本构建 -O0"
  check_empty "$d/e3001-build.err" "E3001 样本构建 stderr"
  [ -x "$cwd/e3001.bin" ] || pc_fail_msg "E3001 样本构建未产出可执行文件"
  if [ -x "$cwd/e3001.bin" ]; then
    ( cd "$cwd" && run_capture "$d/e3001-run.rc" "$d/e3001-run.out" "$d/e3001-run.err" 30 \
        env -u ZOR_RUNTIME_DIR ./e3001.bin )
    check_rc "$d/e3001-run.rc" 101 "E3001 运行"
    check_bytes "$d/e3001-run.out" "$E3001_EXPECTED" "E3001 stdout"
    check_contains "$d/e3001-run.err" "zor: runtime error[E3001]" "E3001 stderr"
  fi
  check_no_tmp_residue "$cwd" "P-01 cwd"
  check_no_tmp_residue "$prefix" "P-01 安装树"
}

# ---------- P-02 ----------
case_p02() {
  CASE_ID=P-02
  local d="$OUT_DIR/cases/P-02" prefix="$WORK/p02-prefix" cwd="$WORK/p02-cwd"
  mkdir -p "$d"
  install_tree "$prefix" "$d"
  [ "$pc_case_ok" = 1 ] || return 0
  rm -f "$prefix/share/zor/runtime/zor_rt.c" "$prefix/share/zor/runtime/zor_rt.h"
  [ ! -e "$prefix/share/zor/runtime/zor_rt.c" ] || pc_fail_msg "helper 删除失败：zor_rt.c"
  prepare_cwd "$cwd"
  cp "$E3001_SRC" "$cwd/e3001.zor"
  # hello 不引用 zor_rt.h，不触发 helper 解析；必须用 -O0 下需要运行期 helper 的样本。
  ( cd "$cwd" && run_capture "$d/build.rc" "$d/build.out" "$d/build.err" 60 \
      env -u ZOR_RUNTIME_DIR "$prefix/bin/zorcc" -O0 e3001.zor -o out.bin )
  check_rc "$d/build.rc" 2 "缺 helper 构建"
  check_zero_compile_artifact "$cwd" "$cwd/out.bin" "缺 helper 构建"
  check_no_tmp_residue "$cwd" "P-02 cwd"
  [ -s "$d/build.err" ] || pc_fail_msg "缺 helper stderr 为空（应指名错误并列出已搜索路径）"
  check_contains "$d/build.err" "zor_rt" "缺 helper stderr"
  check_contains "$d/build.err" "share/zor/runtime" "缺 helper stderr 已搜索路径"
  check_not_link_failure "$d/build.err" "缺 helper 构建"
}

# ---------- P-03 ----------
case_p03() {
  CASE_ID=P-03
  local d="$OUT_DIR/cases/P-03" destdir="$WORK/p03-dest" prefix_arg="/opt/zor-l2d" root
  mkdir -p "$d"
  install_tree "$prefix_arg" "$d" "$destdir"
  [ "$pc_case_ok" = 1 ] || return 0
  root="${destdir}${prefix_arg}"
  [ -d "$root" ] || { pc_fail_msg "DESTDIR 根不存在：$root"; return 0; }

  printf '%s\n' "bin/zorcc" "share/zor/runtime/zor_rt.c" "share/zor/runtime/zor_rt.h" > "$d/files.expected"
  ( cd "$root" && find . -type f -print | sed 's|^\./||' | LC_ALL=C sort ) > "$d/files.actual"
  if ! cmp -s "$d/files.actual" "$d/files.expected"; then
    pc_fail_msg "安装文件集不符：actual=$(tr '\n' ' ' < "$d/files.actual" 2>/dev/null)"
  fi
  local n_files n_links n_special
  n_files="$(find "$root" -type f | wc -l | tr -d ' ')"
  n_links="$(find "$root" -type l | wc -l | tr -d ' ')"
  n_special="$(find "$root" ! -type f ! -type d ! -type l | wc -l | tr -d ' ')"
  [ "$n_files" = 3 ] || pc_fail_msg "文件数=$n_files 期望 3（仅 bin＋share runtime）"
  [ "$n_links" = 0 ] || pc_fail_msg "符号链接数=$n_links 期望 0"
  [ "$n_special" = 0 ] || pc_fail_msg "特殊文件数=$n_special 期望 0"

  printf '%s\n' "bin" "share" "share/zor" "share/zor/runtime" > "$d/dirs.expected"
  ( cd "$root" && find . -mindepth 1 -type d -print | sed 's|^\./||' | LC_ALL=C sort ) > "$d/dirs.actual"
  cmp -s "$d/dirs.actual" "$d/dirs.expected" || pc_fail_msg "安装目录集不符：actual=$(tr '\n' ' ' < "$d/dirs.actual" 2>/dev/null)"

  local m
  m="$(file_mode "$root/bin/zorcc")"
  [ "$m" = "755" ] || pc_fail_msg "bin/zorcc 模式=$m 期望 755"
  m="$(file_mode "$root/share/zor/runtime/zor_rt.c")"
  [ "$m" = "644" ] || pc_fail_msg "zor_rt.c 模式=$m 期望 644"
  m="$(file_mode "$root/share/zor/runtime/zor_rt.h")"
  [ "$m" = "644" ] || pc_fail_msg "zor_rt.h 模式=$m 期望 644"

  ( cd "$root" && find . -type f -print | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r rel; do
      printf '%s  %s\n' "$(sha256_of "$root/$rel")" "$rel"
    done ) > "$OUT_DIR/p03-tree.sha256"
  ( cd "$root" && find . -type f -print | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r rel; do
      printf '%s\t%s\t%s\n' "$(file_mode "$root/$rel")" "$(file_size "$root/$rel")" "$rel"
    done ) > "$OUT_DIR/p03-tree.txt"
  local manifest_rows
  manifest_rows="$(awk 'NF{n++} END{print n+0}' "$OUT_DIR/p03-tree.sha256")"
  [ "$manifest_rows" = 3 ] || pc_fail_msg "树清单行数=$manifest_rows 期望 3"
  ( cd "$root" && { if command -v sha256sum >/dev/null 2>&1; then sha256sum -c --quiet "$OUT_DIR/p03-tree.sha256"; else shasum -a 256 -c "$OUT_DIR/p03-tree.sha256" >/dev/null; fi; } ) \
    > "$d/tree-check.log" 2>&1 || pc_fail_msg "树清单复算失败（见 p03/tree-check.log）"

  local leak=0
  for f in "$root/share/zor/runtime/zor_rt.c" "$root/share/zor/runtime/zor_rt.h"; do
    if grep -qF -- "$REPO_ROOT" "$f" 2>/dev/null; then
      pc_fail_msg "安装树文本泄漏构建机路径：${f#$root/}"
      leak=1
    fi
  done
  [ "$leak" = 0 ] || true
}

# ---------- P-04 ----------
case_p04() {
  CASE_ID=P-04
  local d="$OUT_DIR/cases/P-04" prefix="$WORK/p04-prefix" cwd="$WORK/p04-cwd" bad="$WORK/p04-bad-runtime"
  mkdir -p "$d"
  install_tree "$prefix" "$d"
  [ "$pc_case_ok" = 1 ] || return 0
  prepare_cwd "$cwd"
  cp "$E3001_SRC" "$cwd/e3001.zor"
  mkdir -p "$cwd/runtime" "$bad"
  cp "$RUNTIME_DIR/zor_rt.c" "$RUNTIME_DIR/zor_rt.h" "$cwd/runtime/"
  [ -f "$cwd/runtime/zor_rt.c" ] && [ -f "$cwd/runtime/zor_rt.h" ] || pc_fail_msg "cwd 回退诱饵未就绪"
  # 同 P-02：用 -O0 下需要运行期 helper 的样本，确保走到 ZOR_RUNTIME_DIR 解析。
  ( cd "$cwd" && run_capture "$d/build.rc" "$d/build.out" "$d/build.err" 60 \
      env ZOR_RUNTIME_DIR="$bad" "$prefix/bin/zorcc" -O0 e3001.zor -o out.bin )
  check_rc "$d/build.rc" 2 "非法 ZOR_RUNTIME_DIR 构建"
  check_zero_compile_artifact "$cwd" "$cwd/out.bin" "非法 ZOR_RUNTIME_DIR 构建"
  check_no_tmp_residue "$cwd" "P-04 cwd"
  [ -s "$d/build.err" ] || pc_fail_msg "非法 ZOR_RUNTIME_DIR stderr 为空（应指名错误）"
  check_contains "$d/build.err" "ZOR_RUNTIME_DIR" "非法 ZOR_RUNTIME_DIR stderr"
  check_not_link_failure "$d/build.err" "非法 ZOR_RUNTIME_DIR 构建"
}

run_case() {
  local fn="$1" line
  pc_case_ok=1
  pc_ran=$((pc_ran+1))
  "$fn"
  if [ "$pc_case_ok" = 1 ]; then
    line="[packaging-smoke] $CASE_ID PASS"
    echo "$line" | tee -a "$SUMMARY"
  else
    pc_case_fail=$((pc_case_fail+1))
    line="[packaging-smoke] $CASE_ID FAIL"
    echo "$line" | tee -a "$SUMMARY"
  fi
}

# ---------- 主流程 ----------
if ! derive_expect_version; then
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 static=version" | tee -a "$SUMMARY"
  exit 2
fi
if ! validate_assets; then
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 static=invalid" | tee -a "$SUMMARY"
  exit 2
fi

if [ "$VALIDATE_ONLY" = 1 ]; then
  echo "PASS|packaging-smoke|validate-only static=ok executed=0 version=$EXPECT_VERSION source=$EXPECT_VERSION_SOURCE" | tee -a "$SUMMARY"
  exit 0
fi

if [ "$EXPECT_VERSION_SOURCE" = "override" ]; then
  echo "[packaging-smoke] NOTE: ZOR_EXPECT_VERSION=$EXPECT_VERSION 显式覆写；默认来源 compiler/Cargo.toml" | tee -a "$SUMMARY"
fi

host_gate || exit 2

if ! resolve_zorc; then
  echo "[packaging-smoke] ENV FAIL: 找不到可执行 zorcc（ZORC_BIN='$ZORC'）" | tee -a "$SUMMARY" >&2
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 env=zorcc-missing" | tee -a "$SUMMARY"
  exit 2
fi
if ! command -v "$CC_REAL" >/dev/null 2>&1; then
  echo "[packaging-smoke] ENV FAIL: 找不到 C 编译器 CC='$CC_REAL'" | tee -a "$SUMMARY" >&2
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 env=cc-missing" | tee -a "$SUMMARY"
  exit 2
fi
case "$INSTALLER" in
  /*) ;;
  *) INSTALLER="$(cd "$(dirname "$INSTALLER")" && pwd -P)/$(basename "$INSTALLER")" ;;
esac

WORK="$(mktemp -d "${TMPDIR:-/tmp}/zor-packaging-smoke.work.XXXXXX")" || {
  echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=0 env=workdir" | tee -a "$SUMMARY"
  exit 2
}
cleanup() {
  if [ "${KEEP_WORK:-0}" = "1" ]; then
    echo "[packaging-smoke] KEEP_WORK=1 保留工作目录：$WORK" | tee -a "$SUMMARY"
  else
    rm -rf "$WORK"
  fi
}
trap cleanup EXIT

{
  echo "packaging-smoke evidence"
  echo "time=$(utc_now)"
  echo "host=$(uname -s)/$(uname -m)"
  echo "zorcc=$ZORC_REAL"
  echo "cc=$CC_REAL"
  echo "installer=$INSTALLER"
  echo "expect_version=$EXPECT_VERSION source=$EXPECT_VERSION_SOURCE"
  echo "out_dir=$OUT_DIR"
  echo "work_dir=$WORK"
} >> "$SUMMARY"

run_case case_p01
run_case case_p02
run_case case_p03
run_case case_p04

if [ "$pc_ran" -eq "$EXPECT_CASES" ] && [ "$pc_case_fail" -eq 0 ]; then
  if [ "$LOCAL_PRECHECK" = 1 ]; then
    echo "PASS|packaging-smoke|local-precheck cases=$EXPECT_CASES fail=0 ran=$EXPECT_CASES" | tee -a "$SUMMARY"
  else
    echo "PASS|packaging-smoke|cases=$EXPECT_CASES fail=0 ran=$EXPECT_CASES" | tee -a "$SUMMARY"
  fi
  exit 0
fi
echo "FAIL|packaging-smoke|cases=$EXPECT_CASES fail=$pc_case_fail ran=$pc_ran" | tee -a "$SUMMARY"
exit 1
