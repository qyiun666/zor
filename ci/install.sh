#!/usr/bin/env bash
# L2d R-1：packaging-smoke 用安装树构造（CI 判据，不是用户安装承诺）
# canonical 布局：<DESTDIR><prefix>/bin/zorcc
#                <DESTDIR><prefix>/share/zor/runtime/zor_rt.c|h
# 归属：ZOR工具链与开发工程师；判据由 QA tests/packaging/run_packaging_smoke.sh P-01～04 执行。
set -euo pipefail

die() { echo "[install] $*" >&2; exit 2; }

usage() {
  cat <<'USAGE'
用法：bash ci/install.sh --prefix <绝对前缀> [--destdir <目录>] [--zorcc <zorcc>] [--runtime <runtime 目录>]

  --prefix   安装前缀（必须绝对路径）；落 bin/ 与 share/zor/runtime/
  --destdir  DESTDIR 根（可选；GNU 约定，用于打包暂存）
  --zorcc    zorcc 二进制（缺省 $ZORC_BIN 或 compiler/target/release/zorcc）
  --runtime  运行期 helper 目录（缺省 $ZOR_RUNTIME_DIR 或仓库 runtime/）
USAGE
}

PREFIX=""; DESTDIR_ROOT=""; ZORCC=""; RUNTIME_DIR=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix) [ "$#" -ge 2 ] || die "--prefix 缺值"; PREFIX="$2"; shift 2 ;;
    --prefix=*) PREFIX="${1#*=}"; shift ;;
    --destdir) [ "$#" -ge 2 ] || die "--destdir 缺值"; DESTDIR_ROOT="$2"; shift 2 ;;
    --destdir=*) DESTDIR_ROOT="${1#*=}"; shift ;;
    --zorcc) [ "$#" -ge 2 ] || die "--zorcc 缺值"; ZORCC="$2"; shift 2 ;;
    --zorcc=*) ZORCC="${1#*=}"; shift ;;
    --runtime) [ "$#" -ge 2 ] || die "--runtime 缺值"; RUNTIME_DIR="$2"; shift 2 ;;
    --runtime=*) RUNTIME_DIR="${1#*=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) die "未知参数 $1（--help 查看用法）" ;;
  esac
done

[ -n "$PREFIX" ] || die "必须指定 --prefix"
case "$PREFIX" in
  /*) ;;
  *) die "--prefix 必须是绝对路径：'$PREFIX'" ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
[ -n "$ZORCC" ] || ZORCC="${ZORC_BIN:-$REPO_ROOT/compiler/target/release/zorcc}"
[ -n "$RUNTIME_DIR" ] || RUNTIME_DIR="${ZOR_RUNTIME_DIR:-$REPO_ROOT/runtime}"

[ -f "$ZORCC" ] || die "找不到 zorcc：'$ZORCC'（先 cargo build --release 或传 --zorcc）"
[ -f "$RUNTIME_DIR/zor_rt.c" ] || die "缺少运行期 helper：'$RUNTIME_DIR/zor_rt.c'"
[ -f "$RUNTIME_DIR/zor_rt.h" ] || die "缺少运行期 helper：'$RUNTIME_DIR/zor_rt.h'"

ROOT="${DESTDIR_ROOT}${PREFIX}"
install -d "$ROOT/bin" "$ROOT/share/zor/runtime"
install -m 0755 "$ZORCC" "$ROOT/bin/zorcc"
install -m 0644 "$RUNTIME_DIR/zor_rt.c" "$RUNTIME_DIR/zor_rt.h" "$ROOT/share/zor/runtime/"

echo "[install] prefix=$PREFIX destdir=${DESTDIR_ROOT:-<empty>}"
echo "[install] installed: $ROOT/bin/zorcc"
echo "[install] installed: $ROOT/share/zor/runtime/zor_rt.c"
echo "[install] installed: $ROOT/share/zor/runtime/zor_rt.h"
