#!/usr/bin/env bash
# E1101 逐层实发点编目计数（按 E1101 字符串发点，不按行号）。
# 用法：bash compiler/scripts/e1101_catalog.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

total=0
for f in compiler/src/lexer.rs compiler/src/parser.rs compiler/src/check.rs; do
    n=$(grep -o '"E1101"' "$f" | wc -l | tr -d ' ')
    printf '%s\t%s\n' "$f" "$n"
    total=$((total + n))
done
printf 'total\t%s\n' "$total"
if [ "$total" -ne 30 ]; then
    echo "E1101 发点数漂移：期望 30，实测 $total（L2r 后编目＝30：lexer 3／parser 20／check 7）" >&2
    exit 1
fi
