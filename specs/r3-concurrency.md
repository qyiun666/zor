# R3 轻量并发（task + channel，规范初稿）

状态：draft｜负责人：核心工程师｜需求来源：PRD §8 R3

## 大纲

1. `spawn task { ... }` → JoinHandle 式令牌（可 join / 丢弃）
2. channel：`chan T`（无界）、`chan(T, N)`（有界，写满阻塞）
3. 发送转移所有权（move into channel）、接收取回所有权——R2 检查跨 task 延伸（与 r2 联合评审，AC-R3-1）
4. 运行时：M:N 绿色线程；单 task 初始栈 ≤8KB（OQ-3 待基准验证）
5. 用户层不暴露线程/锁/原子
6. 边界：全发送端关闭后接收端明确终止信号；task panic 隔离不拖垮主 task

## 测试用例映射（tests/specs/）

- 正例三类：生产者-消费者、扇入扇出、pipeline（AC-R3-2）
- 基准：10 万 task 驻留内存 ≤ Go 同题 70%（AC-R3-3，联动 bench/）
- 负例：向已关闭 channel 发送、借用未转移进 task 等，编译期拦截率 100%（AC-R3-4）
