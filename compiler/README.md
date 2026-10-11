# compiler — 参考实现入口（zorcc）

OQ-1 定论=方案 B（评审 3/3 通过）：过渡实现用 Rust 编写，后端走 C 代码生成；0.2.x 前可替换。本目录即该实现的 Cargo 工程。

## 0.1.1 现状：R1 最小前端

- `src/lexer.rs` → `src/parser.rs` → `src/check.rs`（R1 类型检查 + E1xxx 诊断）。
- 诊断格式（T-4 黄金文件基线，变更需 PR+QA 评审）：`zorcc: {E-code} {file}:{line}:{col}: {message}`
- 退出码：0=无诊断；1=有诊断；2=用法/读文件错误。负例在 debug/release 双 flag 下均须非零退出（CI 双模式契约）。
- 已实现诊断码：E1101/E1201/E1202/E1301/E1401/E1501/E1601（R1 §8 码表；E1501 覆盖泛型/生命周期/引用位非法/裸指针）。
- 未实现（后续版本）：R2 借用检查（E2xxx）、box/unbox 语法（OI-3 拼写已定，见 specs/r2-ownership.md §5）、C 代码生成。

## 构建与测试

```sh
cargo build          # 产出 target/debug/zorcc
cargo test           # tests/r1_diag.rs：正例零诊断 + 逐码负例
```

## CI 接线

`ci/run_ci.sh` 的 stage_cargo 检测到本目录 `Cargo.toml` 即自动激活（cargo build+test 入门禁）。compile 阶段经 `ZORC_BIN` 指向本二进制（如 `compiler/target/debug/zorcc` 或安装路径）；flags `-Zoverflow=check` / `-O2 -Zoverflow=wrap` 被接受（0.1.1 为 no-op，溢出运行期行为随代码生成后端落地）。

用例目录基准= M-1 定论 `tests/specs/{positive,negative,compare}`。
