#!/usr/bin/env bash
# ZOR 性能基准执行骨架（三项：内存驻留/吞吐/编译耗时）
# 归属：ZOR工具链与开发工程师｜版本：skeleton-v1｜口径见 shared/ZOR-0.1x-性能基准环境-草案-v1.md
# 判据引用：B-1 回退≤5%、B-4 3轮中位数/方差>10% 不可信（QA 门禁 v1 §1.3）
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$REPO_ROOT"
ROUNDS=3; MODE="quick"
for arg in "$@"; do
  case "$arg" in
    --quick) ROUNDS=3 ;;
    --full) ROUNDS=5 ;;
    *) echo "用法: run_bench.sh [--quick|--full]"; exit 2 ;;
  esac
done

OUT="bench/results/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$OUT"
BASELINE="bench/baselines/latest.json"

median() { printf '%s\n' "$@" | sort -n | awk '{a[NR]=$1} END{ if(NR==0){print "nan";exit} mid=int((NR+1)/2); print (NR%2)? a[mid] : (a[mid]+a[mid+1])/2 }'; }
B4_UNTRUSTY=0
strelief() { # 极差/均值近似离散度，正式实现改用相对方差（草案 §5）
  printf '%s\n' "$@" | awk '
    { if (NR==1 || $1<mn) mn=$1; if ($1>mx) mx=$1; s+=$1; n++ }
    END {
      if (n<2) { printf "0.0000"; exit }
      d = (mx-mn)/(s/n);
      printf "%.4f", d
    }'; }

run_probe() { # run_probe <name> <cmd...> : 执行 N 轮，输出 median/spread 到 $OUT/<name>.txt
  local name="$1"; shift
  local -a vals=()
  for i in $(seq 1 "$ROUNDS"); do
    local v
    case "$name" in
      mem_resident) # "$@" 需为可执行样例；RSS 采样用 /usr/bin/time -l 的 maximum resident set size
        v=$(/usr/bin/time -l "$@" 2>&1 | awk '/maximum resident set size/{print $1}') ;;
      compile_time)
        v=$( { /usr/bin/time -p "$@"; } 2>&1 | awk '/^real/{print $2}') ;;
      throughput)
        v=$("$@" 2>/dev/null | awk '{print $1}') ;;  # 样例需打印 ops 数
      *) v="NA" ;;
    esac
    vals+=("${v:-NA}")
  done
  local med spread
  med=$(median "${vals[@]}"); spread=$(strelief "${vals[@]}")
  echo "$name median=$med spread=$spread rounds: ${vals[*]}" | tee "$OUT/$name.txt"
  # B-4：离散度阈值告警（正式判据为中位数±方差归一，>10% 不可信 → 非零退出交由门禁）
  if awk -v s="$spread" 'BEGIN{ if (s+0 > 0.10) exit 0; exit 1 }'; then
    echo "$name: 离散度 ${spread} >0.10，结果不可信，需排查环境后重跑(B-4)" | tee -a "$OUT/$name.txt"
    B4_UNTRUSTY=1
  fi
}

# —— 探针挂载点：hello 级占位样例就绪后启用（PRD §12 解耦约定）——
# 内存驻留：空闲常驻 <20MB 目标；10 万 task 场景待 R3 实现（AC-R3-3）
[ -x bench/samples/mem_idle ] && run_probe mem_resident bench/samples/mem_idle || echo "mem_resident SKIP(样例缺失)" | tee "$OUT/mem_resident.txt"
# 编译耗时：固定语料编译 wall-time；C-4 基线×1.2 对比在门禁侧执行
[ -x bench/samples/compile_corpus ] && run_probe compile_time bench/samples/compile_corpus || echo "compile_time SKIP(样例缺失)" | tee "$OUT/compile_time.txt"
# 吞吐：task/channel 消息 ops/s；对照 Rust 同题 within 10%（B-2，校准前移）
[ -x bench/samples/throughput_ops ] && run_probe throughput bench/samples/throughput_ops || echo "throughput SKIP(样例缺失)" | tee "$OUT/throughput.txt"

echo "[bench] 结果目录: $OUT；基线文件: $BASELINE（不存在则首次运行后由维护者确立为基线）"
[ -f "$BASELINE" ] || echo "[bench] 无基线文件：B-1 回退判定跳过，待校准后回填(D-002)"
exit "$B4_UNTRUSTY"
