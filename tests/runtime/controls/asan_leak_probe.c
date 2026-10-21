/* tests/runtime/controls/asan_leak_probe.c — L2a LSAN 泄漏探针（必红）
 * 归属：ZOR质量保障（判据）；CI-01 接线。本文件不属 ZOR 源码面，不参与 tests/specs 计数。
 * 期望：在 Linux＋`detect_leaks=1` 下运行，rc≠0 且 stderr 命中 `ERROR: LeakSanitizer`。
 * 若静默通过＝泄漏检测死，门禁 FAIL（QA v1.3 §6.4 N-LS）。
 */
#include <stdlib.h>

int main(void) {
    volatile int *p = (volatile int *)malloc(64);
    if (p == NULL) {
        return 2;
    }
    *p = 1;
    return 0; /* deliberate leak: no free */
}
