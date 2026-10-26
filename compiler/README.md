# compiler — 参考实现入口（zorcc）

OQ-1 定论=方案 B（评审 3/3 通过）：过渡实现用 Rust 编写，后端走 C 代码生成；0.2.x 以 C11 后端落地首片、后续可替换。本目录即该实现的 Cargo 工程。

## L2q 合并后现状（0.3.16）：R1／R2 前端检查＋C11 后端；typed Box 8 整型宽度、f32／f64 源形态与复合承载、Box<float> 特殊值载荷、Box 常量载荷（struct／定长数组／有限浮点标量）、枚举单载荷与多字段特殊值字面量直接形式、嵌套复合中的枚举值直接特殊值形式、六个命名有限浮点常量（MAX／MIN／EPSILON）、`MIN_POSITIVE`（f64／f32）、十个整型成员（MAX_EXP／MIN_EXP／MANTISSA_DIGITS／DIGITS／RADIX × f64／f32）

- `src/lexer.rs` → `src/parser.rs` → `src/check.rs`（R1 类型检查＋R2 所有权/借用检查，E1xxx／E2xxx 诊断）。
- 诊断格式（T-4 黄金文件基线，变更需 PR+QA 评审）：`zorcc: {E-code} {file}:{line}:{col}: {message}`
- 退出码：0=无诊断；1=有诊断；2=用法/读文件错误。负例在 debug/release 双 flag 下均须非零退出（CI 双模式契约）。
- 已实现诊断码：E1101/E1201/E1202/E1301/E1401/E1501/E1601/E1701/E1702/E1703/E1704（R1 §8 码表）；E1502/E2101/E2102/E2201/E2202/E2301（R2 §8 码表）。
- 已实现（L2h 合并后）：R2 所有权／借用检查（E2101／E2102／E2201／E2202／E2301；返回引用 E1502）与 box／unbox 语法。
- typed Box：8 个整型宽度（`Box<i8>`／`Box<i16>`／`Box<i32>`／`Box<i64>`／`Box<u8>`／`Box<u16>`／`Box<u32>`／`Box<u64>`）的注解构造、`println(*b)`、`unbox`／drop 与局部重赋值；`Box<f32>`／`Box<f64>` 可承载六个特殊值源形态（注解构造、`println(*b)`、移动、局部重赋值、`unbox`／drop）与有限字面量载荷；`Box<struct>`／`Box<[T;N]>` 可承载字面量常量载荷并整体打印（与同值复合打印逐字节同构）；嵌套 Box／含 Box 复合值、变量载荷、函数签名／struct 字段维持 fail-closed。
- 浮点：f32／f64 有限字面量、六个特殊值源形态（`f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`／`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY`）与 `println` 打印（`nan`／`inf`／`-inf`）；tuple／array／struct 常量成员（含既有嵌套与投影，投影仅 `println` 实参）可承载并打印；枚举单载荷变体可承载六命名特殊值并打印（`V(nan)`／`V(inf)`／`V(-inf)`；宽度由变体字段严格匹配）；多字段变体可承载特殊值字面量直接形式（任意字段位置、≥2 字段；与绑定路径逐字节同构）；嵌套复合中的枚举值可直接承载特殊值字面量（任意深度、三种外层别名；与绑定路径逐字节同构）；六个命名有限浮点常量（`f64::MAX`／`MIN`／`EPSILON`、`f32::MAX`／`MIN`／`EPSILON`）与 `f64::MIN_POSITIVE`／`f32::MIN_POSITIVE` 可按类型用于标量／绑定／`Box`／枚举／复合／多实参（A 档）；其他未列名 `::` 成员（`MIN_10_EXP`／`MAX_10_EXP`／`DIG`／`NEG_NAN` 等）仍 E1101；通用 `E::V` 路径语法、`Box<enum>`／含 Box 复合、嵌套 Box、变量载荷、反方向（特殊值／常量位于枚举自身复合载荷内，N-217／218）维持 fail-closed；绑定路径 `V(x)`／`V(x, 1.5)`／`S { e: V(x) }` 等为既有能力；浮点算术／比较不做；非字面量 Box 载荷（变量引用／含 Box 复合值等）、隐式转换、其他 `Type::member` 维持 fail-closed。
- 整型常量成员（L2q 合并后）：十个编译期整型常量——`f64::MAX_EXP`（1024）／`f64::MIN_EXP`（-1021）／`f64::MANTISSA_DIGITS`（53）／`f64::DIGITS`（15）／`f64::RADIX`（2）、`f32::MAX_EXP`（128）／`f32::MIN_EXP`（-125）／`f32::MANTISSA_DIGITS`（24）／`f32::DIGITS`（6）／`f32::RADIX`（2）；按整数字面量语义接入：无标注默认 `i64`，有整型标注时向目标类型收敛，标量输出与同值十进制字面量逐字节一致；可用于标量／绑定／`Box<T>` 标量整型载荷（含负值）／枚举载荷／struct／数组／元组／多实参及上述位置的既有嵌套（A 档）。`MIN_EXP` 值本身为负；既有整型一元负号／算术沿用（不是新增能力）。整型成员→浮点标注：绑定／struct／enum 为 E1202，浮点标量 `Box` 载荷为 E1201（与整数字面量同码）；未列名成员（`MIN_10_EXP`／`MAX_10_EXP`／`DIG`／`NEG_NAN` 等）与成员调用仍 E1101；`Box<struct>`／`Box<[T;N]>` 内含命名整型成员不测试、不宣称（边界归 ⑤）。
- 未实现：R3／R4、stdlib、安装／打包不做。

## 构建与测试

```sh
cargo build          # 产出 target/debug/zorcc
cargo test           # tests/r1_diag.rs：正例零诊断 + 逐码负例
```

## CI 接线

`ci/run_ci.sh` 的 stage_cargo 检测到本目录 `Cargo.toml` 即自动激活（cargo build+test 入门禁）。compile 阶段经 `ZORC_BIN` 指向本二进制（如 `compiler/target/debug/zorcc` 或安装路径）；内部溢出模式覆盖位由 CI 注入，用户文档不展开。

用例目录基准= M-1 定论 `tests/specs/{positive,negative,compare}`。
