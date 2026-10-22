#ifndef ZOR_RT_H
#define ZOR_RT_H

#include <stddef.h>
#include <stdint.h>

/* L2b 运行期终止入口：写一行诊断后 exit(101)。 */
_Noreturn void zor_rt_fail(int code, const char *file, size_t line, size_t col);

/* 有符号 i64 除法／取模：除零先终止；MIN/-1 按调用方模式处理。 */
int64_t zor_rt_div_i64_checked(int64_t a, int64_t b, const char *file, size_t line, size_t col);
int64_t zor_rt_div_i64_wrap(int64_t a, int64_t b, const char *file, size_t line, size_t col);
int64_t zor_rt_rem_i64(int64_t a, int64_t b, const char *file, size_t line, size_t col);

/* 无符号 u8／u32 除法／取模：除零先终止；无 MIN 特例。 */
uint8_t zor_rt_div_u8(uint8_t a, uint8_t b, const char *file, size_t line, size_t col);
uint8_t zor_rt_rem_u8(uint8_t a, uint8_t b, const char *file, size_t line, size_t col);
uint32_t zor_rt_div_u32(uint32_t a, uint32_t b, const char *file, size_t line, size_t col);
uint32_t zor_rt_rem_u32(uint32_t a, uint32_t b, const char *file, size_t line, size_t col);

#endif
