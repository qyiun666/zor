# compiler — 参考实现入口（zorcc）

OQ-1 定论=方案 B（评审 3/3 通过）：过渡实现用 Rust 编写，后端走 C 代码生成；0.2.x 以 C11 后端落地首片、后续可替换。本目录即该实现的 Cargo 工程。

## L2i 合并后现状（0.3.8）：R1／R2 前端检查＋C11 后端；typed Box 8 整型宽度、f32／f64 源形态与复合承载／打印

- `src/lexer.rs` → `src/parser.rs` → `src/check.rs`（R1 类型检查＋R2 所有权/借用检查，E1xxx／E2xxx 诊断）。
- 诊断格式（T-4 黄金文件基线，变更需 PR+QA 评审）：`zorcc: {E-code} {file}:{line}:{col}: {message}`
- 退出码：0=无诊断；1=有诊断；2=用法/读文件错误。负例在 debug/release 双 flag 下均须非零退出（CI 双模式契约）。
- 已实现诊断码：E1101/E1201/E1202/E1301/E1401/E1501/E1601/E1701/E1702/E1703/E1704（R1 §8 码表）；E1502/E2101/E2102/E2201/E2202/E2301（R2 §8 码表）。
- 已实现（L2h 合并后）：R2 所有权／借用检查（E2101／E2102／E2201／E2202／E2301；返回引用 E1502）与 box／unbox 语法。
- typed Box：8 个整型宽度（`Box<i8>`／`Box<i16>`／`Box<i32>`／`Box<i64>`／`Box<u8>`／`Box<u16>`／`Box<u32>`／`Box<u64>`）的注解构造、`println(*b)`、`unbox`／drop 与局部重赋值；其余 Box 形态维持 fail-closed。
- 浮点：f32／f64 有限字面量、六个特殊值源形态（`f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`／`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY`）与 `println` 打印（`nan`／`inf`／`-inf`）；tuple／array／struct 常量成员（含既有嵌套与投影，投影仅 `println` 实参）可承载并打印；浮点算术／比较不做；`Box<f32>`／`Box<f64>`、隐式转换、其他 `Type::member` 维持 fail-closed。
- 未实现：R3／R4、stdlib、安装／打包不做。

## 构建与测试

```sh
cargo build          # 产出 target/debug/zorcc
cargo test           # tests/r1_diag.rs：正例零诊断 + 逐码负例
```

## CI 接线

`ci/run_ci.sh` 的 stage_cargo 检测到本目录 `Cargo.toml` 即自动激活（cargo build+test 入门禁）。compile 阶段经 `ZORC_BIN` 指向本二进制（如 `compiler/target/debug/zorcc` 或安装路径）；内部溢出模式覆盖位由 CI 注入，用户文档不展开。

用例目录基准= M-1 定论 `tests/specs/{positive,negative,compare}`。
