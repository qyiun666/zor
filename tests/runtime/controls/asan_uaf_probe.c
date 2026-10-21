/* tests/runtime/controls/asan_uaf_probe.c — L2a ASAN 活性探针（必红）
 * 归属：ZOR质量保障（判据）；CI-01 接线。本文件不属 ZOR 源码面，不参与 tests/specs 计数。
 * 期望：以 ci/asan/cc-asan 同旗标构建后运行，rc≠0 且 stderr 命中 `ERROR: AddressSanitizer`。
 * 若静默通过＝ASAN 探测器死，门禁 FAIL（QA v1.3 §6.4 N-5）。
 */
#include <stdlib.h>

int main(void) {
    volatile int *p = (volatile int *)malloc(sizeof(int));
    if (p == NULL) {
        return 2;
    }
    *p = 7;
    free((void *)p);
    *p = 9; /* deliberate heap-use-after-free */
    return 0;
}
