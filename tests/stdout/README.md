# tests/stdout — L2i stdout／CLI 契约门禁（QA）

归属：ZOR质量保障。入口由 `ci/run_ci.sh --stdout-contract` 直调；CI 不维护第二套矩阵。L2f Stage-1 已纳入 S9-01～06 与 N-16～35；L2g Stage-1 新增 S10-01～09 与 N-36～49；L2h Stage-1 新增 S11-01～09 与 N-50～81；L2i Stage-1 新增 S12-01～19 与 N-82～112。S11／S12 expected 在核心实现 head 两轮捕获后由 Stage-2 回填，**Stage-1 阶段 `--validate-only` 会因 S12 expected 缺件而失败，这是预期**。

## 计数（L2i 活跃 232＝228 stdout＋4 packaging；L2h 历史 183／247 冻结）

| 段 | 条数 | 矩阵 |
|---|---:|---|
| stdout runtime | **108** | `stdout-cases.tsv`（65；S1 6／S2 4／S3 29／S4 5／S5 11／S6 4／S7 3／S8 3）＋`l2f-cases.tsv`（6；S9-01～06）＋`l2g-cases.tsv`（9；S10-01～09）＋`l2h-cases.tsv`（9；S11-01～09）＋`l2i-cases.tsv`（19；S12-01～19） |
| stdout 编译负控 | **103** | `negative.tsv`（12；N-01～03、N-07～15）＋`l2f-negative.tsv` **活跃 15**（20 行 − N-21～25 退休）＋`l2g-negative.tsv`（14；N-36～49）＋`l2h-negative.tsv` **活跃 31**（32 行 − N-81 退休）＋`l2i-negative.tsv`（31；N-82～112） |
| CLI | **17** | `cli-cases.tsv`（C1 6／C2 3／C3 3／C4 2（C4-02 显式变更）／C5 3 help） |
| stdout 合计 | **228** | 108＋103＋17；`new-cases.tsv` 仍 **120**（锁定，不扩写） |
| packaging | **4** | `tests/packaging/packaging-cases.tsv`（独立 job） |

- **L2h 历史冻结**：活跃 183＝179（89／73／17）＋4；全量 247；`l2h-baseline` 247／327；L2g 历史 142＝138（80／41／17）＋4；全量 206；`l2g-baseline` 206／275；L2f 124＝120（71／32／17）＋4；`l2f-baseline` 188／246；`l2e-baseline` 162／209；`l2c-baseline` 68／105；legacy 226；runtime-codes 39／6／19；旧值全部零漂移。
- **L2g 退休清单**：N-21（i8）／N-22（i16）／N-23（i32）／N-24（u16）／N-25（u64），均为 `Box<T> = box 1`；仅退出活跃负控执行。源文件、`l2f-negative.tsv`（20 行）、`l2f-baseline.tsv`／`.sha256` **零改**；静态校验仍按 20 行；不得静默删例、不得重签 l2f。先例 L2c→L2e N-04～N-06。
- **L2g 负控固定分布**：E1201 **8**（N-36～40、N-43、N-44、N-49）／codegen **6**（N-41、N-42、N-45～N-48）／E2301 新增 **0**（读后 unbox 由既有 N-34 覆盖）；l2f 负控分布不变（E1201 11／codegen 8／E2301 1）。
- **L2h 负控固定分布**：E1201 **6**（N-63、N-64、N-65、N-67、N-70、N-71）／E1202 **2**（N-68、N-69）／E1101 **9**（N-72～78、N-79、N-80）／codegen **15**（N-50～62、N-66、N-81）。其中 N-65（特殊值绑定后一元负号）、N-75（NaN 载荷）、N-81（复合内特殊值）的最终 token 在实现 head 的 Stage-1 捕获后固定；**同 32 条、同计数，不删例、不放宽**。N-67（浮点 unbox 于非 Box）与 N-78（println 上下文非六形式 member）按 0.3.6 实测固定为 E1201／E1101。
- **L2i 负控固定分布**：E1201 **7**（N-93、N-100～102、N-106～108）／E1202 **3**（N-103～105）／E1101 **4**（N-96、N-109～111）／codegen **17**（N-82～92、N-94、N-95、N-97～99、N-112）。N-81 仅退出活跃执行（L2h 资产 33 行／247／327 字节零改）；N-105 的 E1202 为整型字面量→浮点数组（隐式转换），array 内“特殊值→整型”沿用既有 E1201（不改 check、不新增 E 码）。
- **L2i 复合正例**：S12-01～19 覆盖 tuple／array／struct 直接成员、混合宽度、既有嵌套、投影（仅 `println` 实参）、struct 定义序、多实参／重复读、绑定别名（S12-19）；投影 `let y = p.x` 仍 fail-closed（N-112）。
- **L2h 六特殊值形式**：`f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`／`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY`，逐形式 S11-01～06 追溯；`::` 仅此六形式，其他 `Type::member`／裸 `nan`／`inf`／`f64::NAN()` 仍 fail-closed。
- **五宽已支持**，不再作负控；仍 fail-closed 的是 `Box<f32>`／`Box<f64>`／`Box<bool>`／`Box<byte>`／`Box<unit>`（N-26～N-30），嵌套／复合（N-15／N-31／N-32／N-33／N-48），`println(b)`（N-07／N-14）。
- **S5-04 N/A**：1 元组无源形态，不建 case、不计数；不得写“1 元组已支持”。
- N-04／05／06 退役转正例，归档 `retired/l2c/`；不得继续 fail-closed。
- L2c 基线：`l2c-baseline.tsv` 锁 68 条未变用例；`l2c-baseline.sha256` 105 行。
- L2h Stage-2：`l2h-baseline` 设计 **247 条／327 files（设计值；files 以捕获 unique 为唯一值）**；`l2e`／`l2f`／`l2g` 零改；`l2h-baseline.tsv` 只含活跃矩阵（206＋9＋32＝247）。
- D-L2c-12：L2h 起 NaN／±Inf 有端到端源形态（S11-01～06）；`backend::float_fmt` 单测与 S3-27 `1e-5` 继续保留，不替代端到端证据。
- N-09／N-10 为 `&T`／`&mut T` 条件负控；`Handle<T>`／fn 无语法，记 N/A。

## 入口

```bash
bash tests/stdout/run_stdout_contract.sh --strict
```

- env：`ZORC_BIN`（zorcc 路径）、`CC`（C 编译器，CI 为 clang-18）、`OUT_DIR`（证据目录）。
- 输出：恰一行 `PASS|stdout-contract|…` 或 `FAIL|stdout-contract|…`，以及 `PASS|cli-contract|…` 或 `FAIL|cli-contract|…`；任一段 FAIL → rc=1。显式 FAIL 优先。
- 宿主：默认只允许 Linux x86_64；`--allow-nonlinux` 仅供本地预检，CI 不传。
- `--validate-only`：只做静态校验（矩阵／路径／expected／旧资产快照），不需 zorcc。Stage-1 因 S12 expected 缺件而红；Stage-2 后转绿。

## 判据

- runtime：两轮构建＋运行；stdout 逐字节 `cmp`；`run` 模式 stderr 0 B；`stdout-terminate` 模式只判 stdout 与 rc=101（E30xx 文案归 runtime-codes）。
- negative：两轮 rc=1、零产物、stderr 含固定唯一 token；禁止 cc rc=2／`error:`／`undeclared identifier`；不新增 E 码。
- CLI：NDJSON 六键固定序 `code/severity/message/file/line/col`、行序与文本模式一致、无诊断 stderr 0 B；`--version`／`-V` 短路；`-o` 零残留；usage 不泄露 `-Zoverflow`；`--help` 输出固定文本（C5）。
- 旧资产：`legacy-snapshot.sha256`（226 文件）逐文件 sha256；旧五值 41／53／53／3／94、manifest 48、runtime-codes 39＋6＋19 计数复核。
- L2g 退休：`l2f-negative.tsv` 静态仍 20 行；活跃执行排除 N-21～N-25；L2i 起同时排除 N-81（`l2h-negative.tsv` 静态仍 32 行）；当前活跃 `negative=103`（12＋15＋14＋31＋31）；`new-cases.tsv` 仍 120；l2f／l2g／l2h 资产零改。

## 资产说明

- `expected/<case>.stdout`：逐字节期望；S6 只含终止前 stdout；S9-01～06、S10-01～09、S11-01～09 expected 已冻结；S12-01～19 expected 在 Stage-2 捕获，**Stage-1 不预写**。
- `cases/S4-05.zor`：源文件内含原始 NUL 字节（`\0` 转义不是 NUL），按二进制资产处理。
- S6-03／S6-04 使用 `-O0` 触发 E3001；其余 runtime 用默认 `-O2`。
- `S5-09` 源写 `Red()`，输出 `Red`；`S5-10` `Circle(3)`；`S7` 为 `[u8;N]` 数组形态，与 S4 `[byte;N]` 原始字节严格区分。
- S9 三型：`Box<i64>`／`Box<u8>`／`Box<u32>` 注解构造；S9-01 为 `box 0`／`i64::MAX`，无 `i64::MIN`；六档为执行维度，不增 TSV 行。
- S10 五宽：`Box<i8>`／`Box<i16>`／`Box<i32>`／`Box<u16>`／`Box<u64>` 注解构造；逐宽 0／max、五宽混排、重复读＋unbox、重赋值、默认 `box 1` 仍 `Box<i64>`；六档为执行维度，不增 TSV 行。
- S11 六形式：`f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`／`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY` 逐条；S11-07 多实参 f32／f64／整型混排；S11-08 Copy 重复读并守卫裸 `nan` 仍为标识符；S11-09 守卫零／`-0` 双宽、有限非字面量 `-x` rc=0、有限浮点复合 `(1.5, 1)` rc=0。
- L2i 复合：S12-01～19 覆盖 tuple／array／struct 常量成员、混合宽度、既有嵌套、投影（仅 `println` 实参）、struct 定义序、多实参／重复读、绑定别名（S12-19）；N-82～112 覆盖算术／比较／条件、Box 浮点、枚举载荷、非字面量／越界下标、E1201／E1202、NaN 载荷、其他 `::`、裸 `nan`、投影 let fail-closed。S12 expected 在 Stage-2 捕获，Stage-1 不预写。
- N-50～62：浮点算术／比较／条件 fail-closed；N-63～65：特殊值一元负号（`-f64::NAN`／`-f32::INFINITY` E1201，绑定后 `-x` rc=1）；N-66：`box` 浮点载荷 codegen；N-67：浮点 `unbox` 于非 Box E1201；N-68／69：隐式整型↔浮点 E1202；N-70／71：跨宽特殊值 E1201；N-72～77、N-79／80：非六形式 `Type::member`／后缀／裸标识符 E1101；N-78：`println` 上下文中的非六形式 member E1101；N-81：复合内特殊值 fail-closed（codegen）。
- N-36～49：五宽越界、负数／非字面量载荷、非 Box 算术零扩容、非 i64 Box 参数／返回／字段、复合打印、重赋值越界；不得新增 E 码。
- **Stage-1.1 判词订正（L2g）**：N-45／N-46 为 `额外函数 'f'（首片只支持 main）`；N-47 为 `let 初始化复合值不可打印或形态不支持`；其余 11 条同 Stage-1（N-41／N-42 `box 载荷非整型字面量`；N-48 `println 实参不可打印`；E1201 8 条）。仅判词订正，9／14 与活跃 L2g 138 不变。

- **Stage-1 v1.1 订正（N-97）**：别名绑定特殊值作 array 元素在实现后为正例（与 S12-19 同义），原“非恒定数组元素”负控前提失效；N-97 同 ID 改为“含特殊值数组的非字面量下标”（`let a: [f64;2] = [f64::NAN, 1.5]; let i = 0; println(a[i]);`），head `6bacb3a` 实测 codegen rc=1、零产物；31 条与 token 分布 E1201 7／E1202 3／E1101 4／codegen 17 不变。