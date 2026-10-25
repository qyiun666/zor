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
#   - 静态：258 runtime（65+6+9+9+19+18+49+24+27+32）＋194 negative（12+11+14+31+28+24+23+18+17+16）＋17 CLI＝469；new-cases 120（锁定）；
#     L2g 活跃负控排除 N-21～N-25（退休清单；l2f 资产零改、静态仍校验 20 行）；L2h 新增 S11-01～09／N-50～81（N-81 仅执行面退休）；
#     L2i 新增 S12-01～19／N-82～112（N-94 仅执行面退休）；L2j 新增 S13-01～18／N-113～142；schema／路径／重复／expected 缺件（Stage-1 阶段新增 expected 缺件为预期）。
#     L2k 新增 S14-01～31／S15-01～18／N-143～165；Ret=12（N-26/27/31/32/93/95/117～120/125/126）仅执行面退休；schema／路径／重复／expected 缺件（Stage-1 阶段新增 expected 缺件为预期）。
#     L2l 新增 S16-01～22／S17-01～02／N-166～185；Ret=0（无退休）；schema／路径／重复／expected 缺件（Stage-1 阶段新增 expected 缺件为预期）。
#     L2m 新增 S18-01～27／N-186～203；Ret=1（N-183 仅执行面退休，l2l 资产字节零改）；schema／路径／重复／expected 缺件（Stage-1 阶段新增 expected 缺件为预期）。
#     L2n 新增 S19-01～32／N-204～219；Ret=2（N-184／N-200 仅执行面退休，l2l／l2m 资产字节零改）；schema／路径／重复／expected 缺件（Stage-1 阶段新增 expected 缺件为预期）。
#   - 旧资产快照：legacy-snapshot.sha256 逐文件 sha256；l2c-baseline.sha256 锁 68 条未变用例；
#     旧五值／runtime-codes／manifest 计数复核。
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
L2F_RUNTIME_TSV="$STDOUT_DIR/l2f-cases.tsv"
L2F_NEGATIVE_TSV="$STDOUT_DIR/l2f-negative.tsv"
L2G_RUNTIME_TSV="$STDOUT_DIR/l2g-cases.tsv"
L2G_NEGATIVE_TSV="$STDOUT_DIR/l2g-negative.tsv"
L2H_RUNTIME_TSV="$STDOUT_DIR/l2h-cases.tsv"
L2H_NEGATIVE_TSV="$STDOUT_DIR/l2h-negative.tsv"
L2I_RUNTIME_TSV="$STDOUT_DIR/l2i-cases.tsv"
L2I_NEGATIVE_TSV="$STDOUT_DIR/l2i-negative.tsv"
L2J_RUNTIME_TSV="$STDOUT_DIR/l2j-cases.tsv"
L2J_NEGATIVE_TSV="$STDOUT_DIR/l2j-negative.tsv"
L2K_RUNTIME_TSV="$STDOUT_DIR/l2k-cases.tsv"
L2K_NEGATIVE_TSV="$STDOUT_DIR/l2k-negative.tsv"
L2L_RUNTIME_TSV="$STDOUT_DIR/l2l-cases.tsv"
L2L_NEGATIVE_TSV="$STDOUT_DIR/l2l-negative.tsv"
L2M_RUNTIME_TSV="$STDOUT_DIR/l2m-cases.tsv"
L2M_NEGATIVE_TSV="$STDOUT_DIR/l2m-negative.tsv"
L2N_RUNTIME_TSV="$STDOUT_DIR/l2n-cases.tsv"
L2N_NEGATIVE_TSV="$STDOUT_DIR/l2n-negative.tsv"
CLI_TSV="$STDOUT_DIR/cli-cases.tsv"
NEW_TSV="$STDOUT_DIR/new-cases.tsv"
SNAPSHOT="$STDOUT_DIR/legacy-snapshot.sha256"
L2C_BASELINE="$STDOUT_DIR/l2c-baseline.sha256"
NDJSON_CHECK="$STDOUT_DIR/ndjson_check.py"
ZORC="${ZORC_BIN:-zorcc}"
CC_REAL="${CC:-cc}"
OUT_DIR="${OUT_DIR:-}"
STRICT=0
VALIDATE_ONLY=0
ALLOW_NONLINUX=0
EXPECT_RUNTIME_LEGACY=65
EXPECT_RUNTIME_L2F=6
EXPECT_RUNTIME_L2G=9
EXPECT_RUNTIME_L2H=9
EXPECT_RUNTIME_L2I=19
EXPECT_RUNTIME_L2J=18
EXPECT_RUNTIME_L2K=49
EXPECT_RUNTIME_L2L=24
EXPECT_RUNTIME_L2M=27
EXPECT_RUNTIME_L2N=32
EXPECT_RUNTIME=258
EXPECT_NEGATIVE_LEGACY=12
EXPECT_NEGATIVE_L2F=20
EXPECT_NEGATIVE_L2F_ACTIVE=11
EXPECT_NEGATIVE_L2G=14
EXPECT_NEGATIVE_L2H=32
EXPECT_NEGATIVE_L2H_ACTIVE=31
EXPECT_NEGATIVE_L2I=31
EXPECT_NEGATIVE_L2I_ACTIVE=28
EXPECT_NEGATIVE_L2J=30
EXPECT_NEGATIVE_L2J_ACTIVE=24
EXPECT_NEGATIVE_L2K=23
# L2l 因 N-183／N-184 两次转正：静态 20、活跃 18；L2m 因 N-200 转正：静态 18、活跃 17。
EXPECT_NEGATIVE_L2L=20
EXPECT_NEGATIVE_L2L_ACTIVE=18
EXPECT_NEGATIVE_L2M=18
EXPECT_NEGATIVE_L2M_ACTIVE=17
EXPECT_NEGATIVE_L2N=16
EXPECT_NEGATIVE=194
EXPECT_CLI=17
EXPECT_NEW_CASES=120
EXPECT_TOTAL=469
# 退休清单：仅退出活跃负控执行；源文件、l2f/l2h/l2i/l2j/l2k/l2l/l2m-negative.tsv 与各代基线零改。
RETIRED_NEGATIVE_RE='^(N-2[1-5]|N-26|N-27|N-31|N-32|N-81|N-93|N-94|N-95|N-11[7-9]|N-120|N-125|N-126|N-183|N-184|N-200)$'
RUNTIME_ALL=""
NEGATIVE_ALL=""

if [ $((EXPECT_RUNTIME + EXPECT_NEGATIVE + EXPECT_CLI)) -ne "$EXPECT_TOTAL" ]; then
  echo "[stdout-contract] 计数常量不自洽：$EXPECT_RUNTIME+$EXPECT_NEGATIVE+$EXPECT_CLI != $EXPECT_TOTAL" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2F - 9)) -ne "$EXPECT_NEGATIVE_L2F_ACTIVE" ]; then
  echo "[stdout-contract] L2f 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2F-9 != $EXPECT_NEGATIVE_L2F_ACTIVE（N-21～25／N-26／N-27／N-31／N-32 退休）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2H - 1)) -ne "$EXPECT_NEGATIVE_L2H_ACTIVE" ]; then
  echo "[stdout-contract] L2h 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2H-1 != $EXPECT_NEGATIVE_L2H_ACTIVE（N-81 退休）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2I - 3)) -ne "$EXPECT_NEGATIVE_L2I_ACTIVE" ]; then
  echo "[stdout-contract] L2i 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2I-3 != $EXPECT_NEGATIVE_L2I_ACTIVE（N-93／N-94／N-95 退休）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2J - 6)) -ne "$EXPECT_NEGATIVE_L2J_ACTIVE" ]; then
  echo "[stdout-contract] L2j 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2J-6 != $EXPECT_NEGATIVE_L2J_ACTIVE（N-117～120／N-125／N-126 退休）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2L - 2)) -ne "$EXPECT_NEGATIVE_L2L_ACTIVE" ]; then
  echo "[stdout-contract] L2l 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2L-2 != $EXPECT_NEGATIVE_L2L_ACTIVE（N-183／N-184 退休）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_L2M - 1)) -ne "$EXPECT_NEGATIVE_L2M_ACTIVE" ]; then
  echo "[stdout-contract] L2m 活跃负控常量不自洽：$EXPECT_NEGATIVE_L2M-1 != $EXPECT_NEGATIVE_L2M_ACTIVE（N-200 退休）" >&2
  exit 2
fi
if [ $((EXPECT_RUNTIME_LEGACY + EXPECT_RUNTIME_L2F + EXPECT_RUNTIME_L2G + EXPECT_RUNTIME_L2H + EXPECT_RUNTIME_L2I + EXPECT_RUNTIME_L2J + EXPECT_RUNTIME_L2K + EXPECT_RUNTIME_L2L + EXPECT_RUNTIME_L2M + EXPECT_RUNTIME_L2N)) -ne "$EXPECT_RUNTIME" ]; then
  echo "[stdout-contract] 分组合并 runtime 常量不自洽（期望 $EXPECT_RUNTIME）" >&2
  exit 2
fi
if [ $((EXPECT_NEGATIVE_LEGACY + EXPECT_NEGATIVE_L2F_ACTIVE + EXPECT_NEGATIVE_L2G + EXPECT_NEGATIVE_L2H_ACTIVE + EXPECT_NEGATIVE_L2I_ACTIVE + EXPECT_NEGATIVE_L2J_ACTIVE + EXPECT_NEGATIVE_L2K + EXPECT_NEGATIVE_L2L_ACTIVE + EXPECT_NEGATIVE_L2M_ACTIVE + EXPECT_NEGATIVE_L2N)) -ne "$EXPECT_NEGATIVE" ]; then
  echo "[stdout-contract] 分组合并 negative 常量不自洽（期望 $EXPECT_NEGATIVE）" >&2
  exit 2
fi

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

so_fail=(); so_n=0; so_ran=0; neg_ran=0
cli_fail=(); cli_n=0; cli_ran=0
LOCALE_AVAILABLE=unknown
LOCALE_RESULT=not-run
so_fail_msg() { so_fail+=("$1"); so_n=$((so_n+1)); }
cli_fail_msg() { cli_fail+=("$1"); cli_n=$((cli_n+1)); }

# ---------- 静态校验 ----------
validate_assets() {
  local bad=0
  for f in "$RUNTIME_TSV" "$NEGATIVE_TSV" "$L2F_RUNTIME_TSV" "$L2F_NEGATIVE_TSV" "$L2G_RUNTIME_TSV" "$L2G_NEGATIVE_TSV" "$L2H_RUNTIME_TSV" "$L2H_NEGATIVE_TSV" "$L2I_RUNTIME_TSV" "$L2I_NEGATIVE_TSV" "$L2J_RUNTIME_TSV" "$L2J_NEGATIVE_TSV" "$L2K_RUNTIME_TSV" "$L2K_NEGATIVE_TSV" "$L2L_RUNTIME_TSV" "$L2L_NEGATIVE_TSV" "$L2M_RUNTIME_TSV" "$L2M_NEGATIVE_TSV" "$L2N_RUNTIME_TSV" "$L2N_NEGATIVE_TSV" "$CLI_TSV" "$NEW_TSV" "$SNAPSHOT" "$L2C_BASELINE" "$NDJSON_CHECK"; do
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
  hdr="$(head -n1 "$L2F_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2f-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2F_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2f-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2G_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2g-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2G_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2g-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2H_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2h-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2H_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2h-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2I_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2i-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2I_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2i-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2J_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2j-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2J_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2j-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2K_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2k-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2K_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2k-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2L_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2l-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2L_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2l-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2M_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2m-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2M_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2m-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2N_RUNTIME_TSV")"
  [ "$hdr" = $'case_id\tsource_path\tmode\texpect_rc\texpect_stdout\tflags\tnotes' ] || { so_fail_msg "l2n-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$L2N_NEGATIVE_TSV")"
  [ "$hdr" = $'case_id\tsource_path\texpect_rc\texpect_artifact\texpect_stderr_contains\texpect_stderr_not_contains\tnotes' ] || { so_fail_msg "l2n-negative.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$CLI_TSV")"
  [ "$hdr" = $'case_id\tmode\targs\texpect_rc\tnotes' ] || { so_fail_msg "cli-cases.tsv 表头"; bad=1; }
  hdr="$(head -n1 "$NEW_TSV")"
  [ "$hdr" = $'case_id\tface\tkind\tsource\texpected\tnotes' ] || { so_fail_msg "new-cases.tsv 表头"; bad=1; }
  [ "$bad" = 0 ] || return 1

  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_LEGACY" ] || { so_fail_msg "legacy runtime 行数=$rows 期望 $EXPECT_RUNTIME_LEGACY"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2F_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2F" ] || { so_fail_msg "L2f runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2F"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2G_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2G" ] || { so_fail_msg "L2g runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2G"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2H_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2H" ] || { so_fail_msg "L2h runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2H"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2I_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2I" ] || { so_fail_msg "L2i runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2I"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2J_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2J" ] || { so_fail_msg "L2j runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2J"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2K_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2K" ] || { so_fail_msg "L2k runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2K"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2L_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2L" ] || { so_fail_msg "L2l runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2L"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2M_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2M" ] || { so_fail_msg "L2m runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2M"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2N_RUNTIME_TSV")"
  [ "$rows" = "$EXPECT_RUNTIME_L2N" ] || { so_fail_msg "L2n runtime 行数=$rows 期望 $EXPECT_RUNTIME_L2N"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_LEGACY" ] || { so_fail_msg "legacy negative 行数=$rows 期望 $EXPECT_NEGATIVE_LEGACY"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2F_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2F" ] || { so_fail_msg "L2f negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2F（静态资产仍 20 行）"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2G_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2G" ] || { so_fail_msg "L2g negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2G"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2H_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2H" ] || { so_fail_msg "L2h negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2H（静态资产仍 32 行）"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2I_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2I" ] || { so_fail_msg "L2i negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2I（静态资产仍 31 行）"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2J_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2J" ] || { so_fail_msg "L2j negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2J"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2K_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2K" ] || { so_fail_msg "L2k negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2K"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2L_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2L" ] || { so_fail_msg "L2l negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2L（静态资产仍 20 行）"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2M_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2M" ] || { so_fail_msg "L2m negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2M（静态资产仍 18 行）"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$L2N_NEGATIVE_TSV")"
  [ "$rows" = "$EXPECT_NEGATIVE_L2N" ] || { so_fail_msg "L2n negative 行数=$rows 期望 $EXPECT_NEGATIVE_L2N"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$CLI_TSV")"
  [ "$rows" = "$EXPECT_CLI" ] || { so_fail_msg "CLI 行数=$rows 期望 $EXPECT_CLI"; bad=1; }
  rows="$(awk 'NR>1 && NF {n++} END{print n+0}' "$NEW_TSV")"
  [ "$rows" = "$EXPECT_NEW_CASES" ] || { so_fail_msg "new-cases 行数=$rows 期望 $EXPECT_NEW_CASES（锁定 120，不得扩写）"; bad=1; }

  local cid src mode erc exp flags note seen=" "
  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S[1-8]-*) ;; *) so_fail_msg "非法 runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S9-0[1-6]) ;; *) so_fail_msg "非法 L2f runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2F_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S10-0[1-9]) ;; *) so_fail_msg "非法 L2g runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2G_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S11-0[1-9]) ;; *) so_fail_msg "非法 L2h runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2H_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S12-0[1-9]|S12-1[0-9]) ;; *) so_fail_msg "非法 L2i runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2I_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S13-0[1-9]|S13-1[0-8]) ;; *) so_fail_msg "非法 L2j runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2J_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S14-0[1-9]|S14-1[0-9]|S14-2[0-9]|S14-3[01]|S15-0[1-9]|S15-1[0-8]) ;; *) so_fail_msg "非法 L2k runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2K_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S16-0[1-9]|S16-1[0-9]|S16-2[0-2]|S17-0[12]) ;; *) so_fail_msg "非法 L2l runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2L_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S18-0[1-9]|S18-1[0-9]|S18-2[0-7]) ;; *) so_fail_msg "非法 L2m runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2M_RUNTIME_TSV"

  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in S19-0[1-9]|S19-1[0-9]|S19-2[0-9]|S19-3[0-2]) ;; *) so_fail_msg "非法 L2n runtime ID:$cid"; bad=1 ;; esac
    case "$src" in tests/stdout/cases/*) ;; *) so_fail_msg "$cid 源路径不在 tests/stdout/cases"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    case "$exp" in expected/*) ;; *) so_fail_msg "$cid expected 路径非法"; bad=1 ;; esac
    [ -f "$STDOUT_DIR/$exp" ] || { so_fail_msg "$cid expected 缺件:$exp（Stage-2 捕获前预期）"; bad=1; }
    case "$mode" in run|stdout-terminate) ;; *) so_fail_msg "$cid mode 非法:$mode"; bad=1 ;; esac
    case "$erc" in 0|101) ;; *) so_fail_msg "$cid expect_rc 非法:$erc"; bad=1 ;; esac
    case "$flags" in ""|-|-O0) ;; *) so_fail_msg "$cid flags 非法:$flags"; bad=1 ;; esac
  done < "$L2N_RUNTIME_TSV"

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

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-1[6-9]|N-2[0-9]|N-3[0-5]) ;; *) so_fail_msg "非法 L2f negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1201|"box 载荷非整型字面量"|"let 类型标注"|"println 实参不可打印"|E2301) ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2F_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-3[6-9]|N-4[0-9]) ;; *) so_fail_msg "非法 L2g negative ID:$cid"; bad=1 ;; esac
    case "$cid" in *[!A-Za-z0-9-]*) so_fail_msg "$cid ID 非法字符"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1201|"box 载荷非整型字面量"|"let 类型标注"|"println 实参不可打印"|E2301|"额外函数 'f'（首片只支持 main）"|"let 初始化复合值不可打印或形态不支持") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2G_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-5[0-9]|N-6[0-9]|N-7[0-9]|N-8[01]) ;; *) so_fail_msg "非法 L2h negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2H_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-8[2-9]|N-9[0-9]|N-10[0-9]|N-11[0-2]) ;; *) so_fail_msg "非法 L2i negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2I_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-11[3-9]|N-12[0-9]|N-13[0-9]|N-14[0-2]) ;; *) so_fail_msg "非法 L2j negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|E2301|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2J_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-14[3-9]|N-15[0-9]|N-16[0-5]) ;; *) so_fail_msg "非法 L2k negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|E2301|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2K_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-16[6-9]|N-17[0-9]|N-18[0-5]) ;; *) so_fail_msg "非法 L2l negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|E2301|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2L_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-18[6-9]|N-19[0-9]|N-20[0-3]) ;; *) so_fail_msg "非法 L2m negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|E2301|E1601|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2M_NEGATIVE_TSV"

  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$cid" in N-20[4-9]|N-21[0-9]) ;; *) so_fail_msg "非法 L2n negative ID:$cid"; bad=1 ;; esac
    [ -f "$REPO_ROOT/$src" ] || { so_fail_msg "$cid 源缺件:$src"; bad=1; }
    [ "$erc" = "1" ] || { so_fail_msg "$cid expect_rc 非 1"; bad=1; }
    [ "$artifact" = "none" ] || { so_fail_msg "$cid expect_artifact 非 none"; bad=1; }
    case "$contains" in E1101|E1201|E1202|E2301|E1601|"zorcc: codegen:") ;; *) so_fail_msg "$cid 判词不在固定集合:$contains"; bad=1 ;; esac
    [ "$notc" = "zor: runtime error" ] || { so_fail_msg "$cid 禁词非固定值"; bad=1; }
  done < "$L2N_NEGATIVE_TSV"

  local dups
  dups="$(awk -F'\t' 'FNR>1 && NF {print $1}' "$RUNTIME_TSV" "$L2F_RUNTIME_TSV" "$L2G_RUNTIME_TSV" "$L2H_RUNTIME_TSV" "$L2I_RUNTIME_TSV" "$L2J_RUNTIME_TSV" "$L2K_RUNTIME_TSV" "$L2L_RUNTIME_TSV" "$L2M_RUNTIME_TSV" "$L2N_RUNTIME_TSV" "$NEGATIVE_TSV" "$L2F_NEGATIVE_TSV" "$L2G_NEGATIVE_TSV" "$L2H_NEGATIVE_TSV" "$L2I_NEGATIVE_TSV" "$L2J_NEGATIVE_TSV" "$L2K_NEGATIVE_TSV" "$L2L_NEGATIVE_TSV" "$L2M_NEGATIVE_TSV" "$L2N_NEGATIVE_TSV" "$CLI_TSV" | sort | uniq -d)"
  [ -z "$dups" ] || { so_fail_msg "跨 TSV 重复 ID:$(printf '%s' "$dups" | tr '\n' ',')"; bad=1; }

  while IFS=$'\t' read -r cid mode args erc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    case "$seen" in *" $cid "*) so_fail_msg "重复 ID:$cid"; bad=1 ;; esac
    seen="$seen$cid "
    case "$mode" in ndjson-one|ndjson-two|ndjson-escape|ndjson-utf8|ndjson-o|ndjson-check|version|version-short|version-json|o-ok|o-fail|o-missing|usage|usage-flags|help|help-precedence|help-version) ;; *) so_fail_msg "$cid CLI mode 非法:$mode"; bad=1 ;; esac
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
    ( cd "$REPO_ROOT" && sha256sum -c --quiet "$L2C_BASELINE" ) >/dev/null 2>&1 || { so_fail_msg "L2c 未变用例快照不一致"; return 1; }
  else
    ( cd "$REPO_ROOT" && shasum -a 256 -c "$SNAPSHOT" ) >/dev/null 2>&1 || { so_fail_msg "旧资产快照不一致"; return 1; }
    ( cd "$REPO_ROOT" && shasum -a 256 -c "$L2C_BASELINE" ) >/dev/null 2>&1 || { so_fail_msg "L2c 未变用例快照不一致"; return 1; }
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
  [ "$runtime" = 39 ] || { so_fail_msg "runtime-codes=$runtime 期望 39"; bad=1; }
  [ "$width" = 6 ] || { so_fail_msg "width=$width 期望 6"; bad=1; }
  [ "$rcli" = 19 ] || { so_fail_msg "runtime CLI=$rcli 期望 19"; bad=1; }
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

build_merged_tsvs() {
  RUNTIME_ALL="$OUT_DIR/runtime-all.tsv"
  NEGATIVE_ALL="$OUT_DIR/negative-all.tsv"
  { head -n1 "$RUNTIME_TSV"; tail -n +2 "$RUNTIME_TSV"; tail -n +2 "$L2F_RUNTIME_TSV"; tail -n +2 "$L2G_RUNTIME_TSV"; tail -n +2 "$L2H_RUNTIME_TSV"; tail -n +2 "$L2I_RUNTIME_TSV"; tail -n +2 "$L2J_RUNTIME_TSV"; tail -n +2 "$L2K_RUNTIME_TSV"; tail -n +2 "$L2L_RUNTIME_TSV"; tail -n +2 "$L2M_RUNTIME_TSV"; tail -n +2 "$L2N_RUNTIME_TSV"; } > "$RUNTIME_ALL"
  # N-21～N-25（L2g）／N-81（L2h）／N-94（L2i）／L2k Ret=12／L2m N-183／L2n N-184／N-200 仅退出活跃执行；各代 TSV 与源文件零改。
  { head -n1 "$NEGATIVE_TSV"; tail -n +2 "$NEGATIVE_TSV"; tail -n +2 "$L2F_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2G_NEGATIVE_TSV"; tail -n +2 "$L2H_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2I_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2J_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2K_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2L_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2M_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; tail -n +2 "$L2N_NEGATIVE_TSV" | awk -F'\t' -v re="$RETIRED_NEGATIVE_RE" '$1 !~ re'; } > "$NEGATIVE_ALL"
  local rt neg
  rt="$(awk 'NR>1 && NF {n++} END{print n+0}' "$RUNTIME_ALL")"
  neg="$(awk 'NR>1 && NF {n++} END{print n+0}' "$NEGATIVE_ALL")"
  [ "$rt" = "$EXPECT_RUNTIME" ] || so_fail_msg "活跃 runtime 行数=$rt 期望 $EXPECT_RUNTIME（退休/合并异常）"
  [ "$neg" = "$EXPECT_NEGATIVE" ] || so_fail_msg "活跃 negative 行数=$neg 期望 $EXPECT_NEGATIVE（退休/合并异常）"
}

# ---------- runtime ----------
run_runtime() {
  local cid src mode erc exp flags note round work bin rc stdout stderr
  local prev_rc="" prev_sha=""
  while IFS=$'\t' read -r cid src mode erc exp flags note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    so_ran=$((so_ran+1))
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
  done < "$RUNTIME_ALL"
}

# ---------- negative ----------
run_negative() {
  local cid src erc artifact contains notc note round work out rc err
  local prev_rc="" prev_sha=""
  while IFS=$'\t' read -r cid src erc artifact contains notc note; do
    [ "$cid" = "case_id" ] && continue
    [ -n "$cid" ] || continue
    neg_ran=$((neg_ran+1))
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
      if [ "$rc" = "2" ]; then so_fail_msg "$cid r$round rc=2（禁止 cc/CLI 误用）"; ok=0; fi
      if grep -Eq '(^|[^A-Za-z0-9_])error:|undeclared identifier' "$work/stderr"; then
        so_fail_msg "$cid r$round stderr 命中 cc 失败文本"; ok=0
      fi
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
  done < "$NEGATIVE_ALL"
}

# ---------- locale（D-L2c-15，仅浮点 8 条；不改 CI job） ----------
run_locale_check() {
  if [ "$so_n" -ne 0 ]; then LOCALE_RESULT="skipped-stdout-fail"; return; fi
  local locales
  locales="$(locale -a 2>/dev/null || true)"
  if printf '%s\n' "$locales" | grep -Ei '^en_US\.(utf-8|utf8)$' >/dev/null 2>&1; then
    LOCALE_AVAILABLE=yes
  else
    LOCALE_AVAILABLE=no
    LOCALE_RESULT="unavailable-path-analysis"
    echo "[stdout-contract] WARN: en_US.UTF-8 未安装；浮点字节以编译期格式化路径分析为主证据"
    return
  fi
  local cid bin d rc out_c out_u
  LOCALE_RESULT=SAME
  for cid in S3-22 S3-23 S3-24 S3-25 S3-26 S3-27 S3-28 S3-29; do
    bin="$OUT_DIR/runtime/$cid/round1/case.bin"
    d="$OUT_DIR/locale/$cid"; mkdir -p "$d"
    if [ ! -x "$bin" ]; then so_fail_msg "$cid locale 二进制缺失"; LOCALE_RESULT=FAIL; continue; fi
    run_capture "$d/c.rc" "$d/c.out" "$d/c.err" 10 env LC_ALL=C "$bin"
    run_capture "$d/u.rc" "$d/u.out" "$d/u.err" 10 env LC_ALL=en_US.UTF-8 "$bin"
    rc="$(cat "$d/c.rc")"; [ "$rc" = 0 ] || { so_fail_msg "$cid LC_ALL=C rc=$rc"; LOCALE_RESULT=FAIL; }
    rc="$(cat "$d/u.rc")"; [ "$rc" = 0 ] || { so_fail_msg "$cid en_US.UTF-8 rc=$rc"; LOCALE_RESULT=FAIL; }
    cmp -s "$d/c.out" "$d/u.out" || { so_fail_msg "$cid 两 locale stdout 不一致"; LOCALE_RESULT=FAIL; }
  done
  echo "[stdout-contract] locale_available=yes locale_float_cases=8 locale_float_bytes=$LOCALE_RESULT"
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
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$BAD1"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD1"
  if [ "$(cat "$d/rc1")" != 1 ] || [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-01 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" one "$d/err1" "$d/err2" "$BAD1" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-01 NDJSON 校验失败"
  else echo "[cli-contract] PASS C1-01"; fi

  # C1-02 two lines, source order
  d="$W/C1-02"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$BAD2"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD2"
  if [ "$(cat "$d/rc1")" != 1 ] || [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-02 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" two "$d/err1" "$d/err2" "$BAD2" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-02 NDJSON 行序/字段失败"
  else echo "[cli-contract] PASS C1-02"; fi

  # C1-03 escaped path
  d="$W/C1-03"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$ESCAPE"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-03 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" path "$d/err" "$ESCAPE" escape >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-03 JSON 路径转义失败"
  else echo "[cli-contract] PASS C1-03"; fi

  # C1-04 UTF-8 path
  d="$W/C1-04"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$UTF8"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-04 rc 非 1"
  elif ! python3 "$NDJSON_CHECK" path "$d/err" "$UTF8" utf8 >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-04 UTF-8 路径失败"
  else echo "[cli-contract] PASS C1-04"; fi

  # C1-05 with -o: zero artifact
  d="$W/C1-05"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --diag=json "$BAD1" -o "$d/artifact"
  if [ "$(cat "$d/rc")" != 1 ]; then cli_fail_msg "C1-05 rc 非 1"
  elif [ -e "$d/artifact" ]; then cli_fail_msg "C1-05 失败留产物"
  elif ! python3 "$NDJSON_CHECK" schema "$d/err" >"$d/check.log" 2>&1; then
    cli_fail_msg "C1-05 NDJSON schema 失败"
  else echo "[cli-contract] PASS C1-05"; fi

  # C1-06 check-mode success: rc0, stderr 0 B; failure still rc1
  d="$W/C1-06"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" --diag=json "$GOOD"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" --diag=json "$BAD1"
  if [ "$(cat "$d/rc1")" != 0 ] || [ -s "$d/err1" ] || [ -s "$d/out1" ]; then
    cli_fail_msg "C1-06 成功检查态非静默 rc0"
  elif [ "$(cat "$d/rc2")" != 1 ]; then
    cli_fail_msg "C1-06 失败检查态 rc 非 1"
  else echo "[cli-contract] PASS C1-06"; fi

  # C2 version cases
  d="$W/C2-01"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --version
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-01 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-01 stdout/ stderr 不符（期望 zorcc $ver）"
  else echo "[cli-contract] PASS C2-01"; fi

  d="$W/C2-02"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" -V --bogus "$NOFILE"
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-02 版本未短路 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-02 版本短路输出不符"
  else echo "[cli-contract] PASS C2-02"; fi

  d="$W/C2-03"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --version --diag=json
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C2-03 版本未短路 rc 非 0"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C2-03 版本与 JSON 同现输出不符"
  else echo "[cli-contract] PASS C2-03"; fi

  # C3 -o cases
  d="$W/C3-01"; mkdir -p "$d/outdir"
  cli_ran=$((cli_ran+1))
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
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc1" "$d/out1" "$d/err1" 30 "$ZORC" "$BAD1" -o "$d/artifact"
  run_capture "$d/rc2" "$d/out2" "$d/err2" 30 "$ZORC" "$BAD1"
  if [ "$(cat "$d/rc1")" != 1 ] || [ -e "$d/artifact" ]; then
    cli_fail_msg "C3-02 失败 rc/产物不符"
  elif ! cmp -s "$d/err1" "$d/err2"; then
    cli_fail_msg "C3-02 -o 改变文本诊断"
  else echo "[cli-contract] PASS C3-02"; fi

  d="$W/C3-03"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" "$GOOD" -o
  if [ "$(cat "$d/rc")" != 2 ]; then cli_fail_msg "C3-03 -o 缺值 rc 非 2"
  elif ! grep -qF -- "-o" "$d/err"; then cli_fail_msg "C3-03 stderr 未指认 -o"
  else echo "[cli-contract] PASS C3-03"; fi

  # C4 usage
  d="$W/C4-01"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC"
  if [ "$(cat "$d/rc")" != 2 ] || ! grep -qF -- "usage: zorcc" "$d/err"; then
    cli_fail_msg "C4-01 无参 usage 不符"
  else echo "[cli-contract] PASS C4-01"; fi

  d="$W/C4-02"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC"
  local leak=0 tok
  for tok in "-Zoverflow"; do
    grep -qF -- "$tok" "$d/err" && leak=1
  done
  if [ "$leak" = 1 ]; then cli_fail_msg "C4-02 usage 泄露内部位"
  else echo "[cli-contract] PASS C4-02"; fi

  # C5 help
  d="$W/C5-01"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --help
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C5-01 --help rc 非 0"
  elif [ -s "$d/err" ]; then cli_fail_msg "C5-01 --help stderr 非空"
  elif [ ! -f "$STDOUT_DIR/expected/help.stdout" ]; then cli_fail_msg "C5-01 缺 expected/help.stdout"
  elif ! cmp -s "$d/out" "$STDOUT_DIR/expected/help.stdout"; then cli_fail_msg "C5-01 --help 与固定文本不一致"
  elif grep -qF -- "-Zoverflow" "$d/out"; then cli_fail_msg "C5-01 help 泄露 -Zoverflow"
  else echo "[cli-contract] PASS C5-01"; fi

  d="$W/C5-02"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --help --bogus "$NOFILE" -o
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C5-02 help 未优先于非法 flag／-o 缺值"
  elif [ -s "$d/err" ]; then cli_fail_msg "C5-02 help 优先路径 stderr 非空"
  elif [ ! -f "$STDOUT_DIR/expected/help.stdout" ] || ! cmp -s "$d/out" "$STDOUT_DIR/expected/help.stdout"; then
    cli_fail_msg "C5-02 help 优先路径输出非固定文本"
  else echo "[cli-contract] PASS C5-02"; fi

  d="$W/C5-03"; mkdir -p "$d"
  cli_ran=$((cli_ran+1))
  run_capture "$d/rc" "$d/out" "$d/err" 30 "$ZORC" --version --help
  if [ "$(cat "$d/rc")" != 0 ]; then cli_fail_msg "C5-03 版本未优先于 help"
  elif [ "$(cat "$d/out")" != "zorcc $ver" ] || [ -s "$d/err" ]; then
    cli_fail_msg "C5-03 版本优先路径输出不符"
  else echo "[cli-contract] PASS C5-03"; fi
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
    echo "PASS|stdout-contract|validate-only static=ok executed=0"
    echo "PASS|cli-contract|validate-only static=ok executed=0"
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

if [ "$static_bad" = 0 ] && [ "$so_n" -eq 0 ]; then
  build_merged_tsvs
  run_runtime
  run_negative
fi
if [ "$static_bad" = 0 ] && [ "$so_n" -eq 0 ]; then
  run_locale_check
fi
# CLI 独立于 stdout 段：即使 runtime 失败也继续，便于一次看全两段。
if [ "$static_bad" = 0 ] && [ "$cli_n" -eq 0 ]; then
  run_cli
fi
if [ "$so_ran" -ne "$EXPECT_RUNTIME" ]; then
  so_fail_msg "runtime 执行数=$so_ran 期望 $EXPECT_RUNTIME（禁止空集/跳过洗绿）"
fi
if [ "$neg_ran" -ne "$EXPECT_NEGATIVE" ]; then
  so_fail_msg "negative 执行数=$neg_ran 期望 $EXPECT_NEGATIVE（禁止空集/跳过洗绿）"
fi
if [ "$cli_ran" -ne "$EXPECT_CLI" ]; then
  cli_fail_msg "执行数=$cli_ran 期望 $EXPECT_CLI（禁止空集/跳过洗绿）"
fi

if [ "$so_n" -eq 0 ]; then
  echo "PASS|stdout-contract|runtime=$EXPECT_RUNTIME negative=$EXPECT_NEGATIVE fail=0 ran=$so_ran/$neg_ran"
  so_rc=0
else
  for m in "${so_fail[@]}"; do echo "[stdout-contract] FAIL_DETAIL: $m"; done
  echo "FAIL|stdout-contract|${so_fail[*]}"
  so_rc=1
fi
if [ "$cli_n" -eq 0 ]; then
  echo "PASS|cli-contract|cases=$EXPECT_CLI fail=0 ran=$cli_ran"
  cli_rc=0
else
  for m in "${cli_fail[@]}"; do echo "[cli-contract] FAIL_DETAIL: $m"; done
  echo "FAIL|cli-contract|${cli_fail[*]}"
  cli_rc=1
fi
{
  echo "--- summary $now_utc ---"
  echo "host=$(uname -s)/$(uname -m) explicit_allow_nonlinux=$ALLOW_NONLINUX"
  echo "stdout_fail=$so_n cli_fail=$cli_n ran_stdout=$so_ran ran_negative=$neg_ran ran_cli=$cli_ran locale_available=$LOCALE_AVAILABLE locale_float_bytes=$LOCALE_RESULT"
  echo "out=$OUT_DIR"
} | tee "$OUT_DIR/summary.txt"
[ "$so_rc" -eq 0 ] && [ "$cli_rc" -eq 0 ] && exit 0
exit 1
