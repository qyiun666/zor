#include "zor_rt.h"

#include <stdio.h>
#include <stdlib.h>

_Noreturn void zor_rt_fail(int code, const char *file, size_t line, size_t col) {
    fflush(stdout);
    switch (code) {
    case 3001:
        fprintf(stderr, "zor: runtime error[E3001] %s:%zu:%zu: integer overflow\n",
                file, line, col);
        break;
    case 3002:
        fprintf(stderr, "zor: runtime error[E3002] %s:%zu:%zu: division by zero\n",
                file, line, col);
        break;
    default:
        fprintf(stderr, "zor: runtime error: %s:%zu:%zu: runtime error\n",
                file, line, col);
        break;
    }
    fflush(stderr);
    exit(101);
}

int64_t zor_rt_div_i64_checked(int64_t a, int64_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    if (a == INT64_MIN && b == -1) {
        zor_rt_fail(3001, file, line, col);
    }
    return a / b;
}

int64_t zor_rt_div_i64_wrap(int64_t a, int64_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    if (a == INT64_MIN && b == -1) {
        return INT64_MIN;
    }
    return a / b;
}

int64_t zor_rt_rem_i64(int64_t a, int64_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    if (a == INT64_MIN && b == -1) {
        return 0;
    }
    return a % b;
}

uint8_t zor_rt_div_u8(uint8_t a, uint8_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    return (uint8_t)(a / b);
}

uint8_t zor_rt_rem_u8(uint8_t a, uint8_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    return (uint8_t)(a % b);
}

uint32_t zor_rt_div_u32(uint32_t a, uint32_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    return a / b;
}

uint32_t zor_rt_rem_u32(uint32_t a, uint32_t b, const char *file, size_t line, size_t col) {
    if (b == 0) {
        zor_rt_fail(3002, file, line, col);
    }
    return a % b;
}
