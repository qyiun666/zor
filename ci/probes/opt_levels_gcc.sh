#!/usr/bin/env bash
# L2e 工具链探针：Linux GCC 六档 -O*（不建 opt-matrix job，一次性证据）
# 覆盖：六档 hello 构建/运行/argv 映射；overflow 矩阵 O0/O1/Og checked、O2/O3/Os wrap。
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ZORC_BIN="${ZORC_BIN:-$REPO_ROOT/compiler/target/release/zorcc}"
[ -x "$ZORC_BIN" ] || { echo "PROBE FAIL: zorcc 不可执行：$ZORC_BIN"; exit 2; }
command -v gcc >/dev/null 2>&1 || { echo "PROBE FAIL: 缺少 gcc"; exit 2; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
printf 'Hello, ZOR\n' > "$WORK/hello.expected"

CC_WRAP="$WORK/cc-probe"
cat > "$CC_WRAP" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "${ZOR_ARGV_LOG:?}"
exec gcc "$@"
SH
chmod +x "$CC_WRAP"

fail=0
echo "gcc_version=$(gcc --version | head -1)"
echo "zorcc=$ZORC_BIN"
echo "zorcc_version=$("$ZORC_BIN" --version)"

for lvl in O0 O1 O2 O3 Os Og; do
  : > "$WORK/argv.log"
  CC="$CC_WRAP" ZOR_ARGV_LOG="$WORK/argv.log" \
    "$ZORC_BIN" "-$lvl" "$REPO_ROOT/examples/hello.zor" -o "$WORK/hello-$lvl" \
    >"$WORK/build-$lvl.out" 2>"$WORK/build-$lvl.err"
  build_rc=$?
  run_rc=skip; out_cmp=skip; argv=no
  if [ "$build_rc" -eq 0 ]; then
    "$WORK/hello-$lvl" >"$WORK/run-$lvl.out" 2>"$WORK/run-$lvl.err"; run_rc=$?
    cmp -s "$WORK/run-$lvl.out" "$WORK/hello.expected" && out_cmp=SAME || out_cmp=DIFF
  fi
  grep -Eq -- "(^| )-$lvl( |$)" "$WORK/argv.log" && argv=yes
  echo "LEVEL -$lvl build_rc=$build_rc run_rc=$run_rc out=$out_cmp argv=$argv"
  [ "$build_rc" -eq 0 ] && [ "$run_rc" -eq 0 ] && [ "$out_cmp" = SAME ] && [ "$argv" = yes ] || fail=1
done

overflow_case="$REPO_ROOT/tests/runtime/cases/l2b/e3001-add-o0.zor"
for lvl in O0 O1 Og; do
  "$ZORC_BIN" "-$lvl" "$overflow_case" -o "$WORK/e3001-$lvl" >/dev/null 2>"$WORK/e3001-$lvl.err"; br=$?
  if [ "$br" -eq 0 ]; then "$WORK/e3001-$lvl" >/dev/null 2>&1; rr=$?; else rr=skip; fi
  echo "OVERFLOW -$lvl build_rc=$br run_rc=$rr expect_run=101"
  [ "$br" -eq 0 ] && [ "$rr" -eq 101 ] || fail=1
done
for lvl in O2 O3 Os; do
  "$ZORC_BIN" "-$lvl" "$overflow_case" -o "$WORK/e3001-$lvl" >/dev/null 2>"$WORK/e3001-$lvl.err"; br=$?
  if [ "$br" -eq 0 ]; then "$WORK/e3001-$lvl" >/dev/null 2>&1; rr=$?; else rr=skip; fi
  echo "OVERFLOW -$lvl build_rc=$br run_rc=$rr expect_run=0"
  [ "$br" -eq 0 ] && [ "$rr" -eq 0 ] || fail=1
done

echo "PROBE fail=$fail"
exit "$fail"
