# Changelog

本项目的所有重要变更记录在此文件。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循项目 0.x 顺序递进规则；0.2.x 为首个可执行闭环与运行链路起步阶段（见 README「看进度·版本节奏」）。

## [0.3.7] - 2026-10-23

内部验证里程碑，非可用语言版本：本版为 L2h「f32／f64 特殊值源形态与打印」——在 0.3.6 的 8 个整型宽度 typed Box 与三基线之上，新增六个特殊值源形态 `f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`／`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY` 及 `println` 的 `nan`／`inf`／`-inf` 字节输出，并把 `l2h-baseline` 接入 `stdout-contract`，形成 `l2e`／`l2f`／`l2g`／`l2h` 四基线 fail-closed 门禁。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：新增仅为六个特殊值源形态与打印；有限 `f32`／`f64` 字面量与打印（含 `-0`／`0`／`inf`／`-inf` 既有源）为 0.3.6 既有能力、L2h 零改。六形式按宽度严格：`let x: f32 = f64::NAN;` 及反向 → E1201；整型标注 → E1202；不新增 E 码。`println` 输出小写 `nan`／`inf`／`-inf`，负零 `-0`、正零 `0`；有限值沿用最短往返与既有十进制／科学记数法阈值；支持多实参与重复读取；编译期字节输出，无运行期浮点库依赖。特殊值一元负号 → E1201（`-f64::NAN`、`let x = f64::NAN; -x`），有限非字面量 `-x` 保持 0.3.6 行为 rc=0；复合（tuple／array／struct）内特殊值与经绑定别名的特殊值 fail-closed；`Box<f32>`／`Box<f64>`／`box <float>`／浮点 `unbox`、浮点算术／比较／条件、隐式数值转换均维持 fail-closed；`::` 仅六形式，其他 `Type::member`、裸 `nan`／`inf`、`f64::NAN()` → E1101；`1e999`／`-1e999`／`1e-999` 既有行为保持；NaN 载荷／signaling N/A。计数：L2h 活跃 stdout **179**（runtime 89＋负控 73＋CLI 17）＋packaging **4**＝**183**；活跃全量 **247**；`l2h-baseline` **247** 条（sha256 清单 327 行）；0.3.6 历史 **142**／全量 **206** 冻结不漂移；`l2e-baseline` **162** 条（209 行）、`l2f-baseline` **188** 条（246 行）、`l2g-baseline` **206** 条（275 行）、`l2c-baseline` **68** 条（105 行）、`legacy-snapshot` **226** 条、runtime-codes **39＋6＋19**、`new-cases.tsv` **120** 均零改。① 本切片按上；② 跨平台证据 N/A（Linux x86_64 唯一终证，macOS 仅辅助）；③ 安装／包管理器不做；④ 文档债 doc-only（不进本账本）。

### Added

- **六个特殊值源形态**：`f64::NAN`／`f64::INFINITY`／`f64::NEG_INFINITY`、`f32::NAN`／`f32::INFINITY`／`f32::NEG_INFINITY`；`Ex::FloatSpecial(width,kind)`，`p_atom` 识别双冒号＋精确成员，不改 lexer、不新增关键字；宽度严格 E1201／E1202。
- **特殊值打印**：`println` 输出 `nan`／`inf`／`-inf`；复用既有编译期 `float_fmt` 字节路径，无运行期浮点库、无 locale 依赖；支持多实参、重复读取、`-0`／`0`。
- **`l2h-baseline` 第四基线**：`ci/run_ci.sh` 的 `stdout-contract` 阶段在 `l2e`／`l2f`／`l2g` 之后校验 `l2h-baseline`（247／327／327），判词含 `fail=0`；任一 mismatch 即 FAIL，不 SKIP、不重跑洗绿。

### Changed

- **计数与资产**：活跃 stdout 由 **138**（80／41／17）扩至 **179**（89／73／17）＋packaging 4＝**183**；活跃全量 **206 → 247**；`l2h-baseline` 新建 247 条（sha256 清单 327 行）；`l2e` 162／209、`l2f` 188／246、`l2g` 206／275、`l2c` 68／105、legacy 226、runtime-codes 39／6／19、`new-cases.tsv` 120 零改；0.3.6 历史 142／206 冻结不漂移。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.6` 落至 `0.3.7`；`compiler/Cargo.toml:5` description 版本串同步；`compiler/Cargo.lock:3` 的 `version = 4` 零触；`CHANGELOG.md` 本节顶部纯插入；账本合并后 `--version`／`-V` 输出 `zorcc 0.3.7`（以账本实测为准）。
- **CI**：四基线并存；不新建 job、不改 workflow YAML、不改 ruleset；`stdout-contract`／`packaging-smoke` 维持 required；artifact 沿用现有日志上传。

### 门禁与回归读数

- GATE-L2h-01：**PASS**——main `8f8546aa668a0f461cbc93fec80f59984ccd027a`；run `39993064651`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract`／`packaging-smoke` 五 job 全 **success**。
- 判词：`PASS|l2e-baseline|cases=162 files=209 unique=209 fail=0`；`PASS|l2f-baseline|cases=188 files=246 unique=246 fail=0`；`PASS|l2g-baseline|cases=206 files=275 unique=275 fail=0`；`PASS|l2h-baseline|cases=247 files=327 unique=327 fail=0`；`PASS|stdout-contract|runtime=89 negative=73 fail=0 ran=89/73`；`PASS|cli-contract|cases=17 fail=0 ran=17`；`PASS|packaging-smoke|cases=4 fail=0 ran=4`；runtime-codes `runtime_rows=39 width_rows=6 cli_rows=19 fail=0`。
- 读数：四基线判词全 PASS；asan-linux **success**（`PASS|asan|exit=0`，`R=7` ASAN／LSAN 零报告）；S11-01～09 **9/9**、N-50～81 **32/32**；`l2h-baseline` **327/327**、`l2g-baseline` **275/275**、`l2f-baseline` **246/246**、`l2e-baseline` **209/209**、`l2c-baseline` **105/105**、legacy **226/226**；全 job `SKIP|`／`FAIL|` 0 行。
- 证据：GATE 报告 `shared/ZOR-L2h-GATE-01-20261023/REPORT.md`（sha256 `cad734c472322d3a3d887945997f938c3f87e721f352910bef91b852f68c9483`）；证据包 `SHA256SUMS`（21 文件）`344bae1b4a460ade6cc89308af48ead91c076d9f090dd9a72090131b0ef71fc2`；PR #126 **65 files +1430／−44**；Stage-2 九条 expected 与产品会签 `b3deef91…6f25` 逐字节一致。

### Known issues（跟踪中，非本版拦截）

- **特殊值覆盖**：仅六个精确形式；`f64::MAX`／`MIN`／`EPSILON` 等扩展常量、其他 `Type::member`、裸 `nan`／`inf`、`f64::NAN()` 维持 E1101；不新增关键字。
- **浮点运算**：算术／比较／条件／类型转换不做；`Box<f32>`／`Box<f64>`、`box <float>`、浮点 `unbox`、复合内特殊值与绑定别名特殊值维持 fail-closed。
- **负号口径**：特殊值一元负号 E1201；有限非字面量 `-x` 保持 rc=0；`1e999`／`-1e999`／`1e-999` 保持。
- **复合**：特殊值进入 tuple／array／struct 或经绑定别名 fail-closed；有限浮点复合保持 0.3.6。
- **E1101 目录**：新增单一 `err_float_member` 发点，parser 19→20、总数 29→30，仍为同一 E1101，不新增码位。
- **NaN 载荷／signaling**：N/A，不提供载荷／符号语法。
- **平台**：Linux x86_64 为唯一终证；macOS 仅辅助，不作判据；Windows／其他发行版不在本版。
- **PA-L2h**：PA-L2h-01～07 待 0.3.7 账本合并后执行。
- **性能**：`-O*` 只承诺接受与 C 编译器映射，不出性能数字、不作达标承诺。
- 零 tag／Release；不构成安装、包管理器、跨平台或生产环境可用性承诺。

## [0.3.6] - 2026-10-23

内部验证里程碑，非可用语言版本：本版为 L2g「typed Box 八宽扩容与第三基线」——在 0.3.5 的三型之上，把 typed Box 扩至 **8 个整型宽度**（新增 `Box<i8>`／`Box<i16>`／`Box<i32>`／`Box<u16>`／`Box<u64>`）的注解构造、`println(*b)`、`unbox`／drop 与局部重赋值；同时把 `l2g-baseline` 接入 `stdout-contract`，形成 `l2e`／`l2f`／`l2g` 三基线 fail-closed 门禁。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：typed Box 覆盖 8 个整型宽度 `i8`／`i16`／`i32`／`i64`／`u8`／`u16`／`u32`／`u64`；构造为 `let b: Box<T> = box <非负整数字面量>;`，载荷仅非负整数字面量；范围 i8 0～127、i16 0～32767、i32 0～2147483647、i64 0～9223372036854775807、u8 0～255、u16 0～65535、u32 0～4294967295、u64 0～18446744073709551615；越界 E1201、rc=1、零产物；默认 `box 1` 仍为 `Box<i64>`；负数载荷与 `box -1` 维持 fail-closed。`println(*b)` 对 8 宽按十进制输出，支持单实参、多实参、同调用多个 Box 解引用与重复读取；不消费 Box、作用域结束只 drop 一次，`unbox` 语义不变；六档 `-O*` 输出一致；`let v = *b` 仅核心单测，不作 stdout 验收面。仍 fail-closed：`Box<f32>`／`Box<f64>`／`Box<bool>`／`Box<byte>`／`Box<unit>`；`Box<Box<T>>`／`Box<struct>`／`Box<[T;N]>`／含 Box 的复合值；`println(b)`；`println((*b,1))`；非 Box 的 i8／i16／i32／u16／u64 算术与比较；非 i64 Box 的函数参数／返回值／struct 字段。不新增 E 码。计数：L2g 活跃 stdout **138**（runtime 80＋负控 41＋CLI 17）＋packaging **4**＝**142**；活跃全量 **206**；`l2g-baseline` **206** 条（sha256 清单 275 行）；L2f 历史 **124**／全量 **188**（stdout 120＝71／32／17＋4）冻结不漂移；`l2e-baseline` **162** 条（209 行）、`l2f-baseline` **188** 条（246 行）、`l2c-baseline` **68** 条（105 行）、`legacy-snapshot` **226** 条、runtime-codes **39＋6＋19**、`new-cases.tsv` **120** 均零改。① 浮点／NaN 结转（0.3.5 的 f64／f32 字面量与打印保持原样，无 NaN 源形态、无浮点算术）；③ 跨平台证据 N/A（Linux x86_64 唯一终证，macOS 仅辅助）；④ 安装／包管理器不做。

### Added

- **typed Box 八宽**：`Box<i8>`／`Box<i16>`／`Box<i32>`／`Box<u16>`／`Box<u64>` 注解构造按声明宽度推断载荷类型（独立 `BoxIntTy`，不改算术 `IntTy`）；C 后端按 `int8_t*`／`int16_t*`／`int32_t*`／`uint16_t*`／`uint64_t*` 分配与读取，常量用对应 `INT*_C`／`UINT*_C`。
- **五新宽打印**：i8／i16／i32 → `%d`＋`(int)`；u16 → `%u`＋`(unsigned)`；u64 → `%llu`＋`(unsigned long long)`；支持单实参、多实参、同调用多 Box、重复读取；不消费、drop 一次；`unbox` 后不可读（既有 E2301）。
- **`l2g-baseline` 第三基线**：`ci/run_ci.sh` 的 `stdout-contract` 阶段在 `l2e`／`l2f` 之后校验 `l2g-baseline`（206／275／275），判词含 `fail=0`；任一 mismatch 即 FAIL，不 SKIP、不重跑洗绿。

### Changed

- **计数与资产**：活跃 stdout 由 **120**（71／32／17）扩至 **138**（80／41／17）＋packaging 4＝**142**；活跃全量 **188 → 206**；`l2g-baseline` 新建 206 条（sha256 清单 275 行，含 `new-cases.tsv` 与两个 packaging expected）；`l2e` 162／209、`l2f` 188／246、`l2c` 68／105、legacy 226、runtime-codes 39／6／19、`new-cases.tsv` 120 零改；L2f 历史 124／188 冻结不漂移。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.5` 落至 `0.3.6`；`compiler/Cargo.toml:5` description 版本串同步；`compiler/Cargo.lock:3` 的 `version = 4` 零触；`CHANGELOG.md` 本节顶部纯插入；账本合并后 `--version`／`-V` 输出 `zorcc 0.3.6`。
- **CI**：三基线并存；不新建 job、不改 workflow YAML、不改 ruleset；`stdout-contract`／`packaging-smoke` 维持 required；artifact 沿用现有日志上传。

### 门禁与回归读数

- GATE-L2g-01：**PASS**——main `c9c4d77e6443e258a606b680188059f1755dfa32`；run `39915038458`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract`／`packaging-smoke` 五 job 全 **success**。
- 判词：`PASS|l2e-baseline|cases=162 files=209 unique=209 fail=0`；`PASS|l2f-baseline|cases=188 files=246 unique=246 fail=0`；`PASS|l2g-baseline|cases=206 files=275 unique=275 fail=0`；`PASS|stdout-contract|runtime=80 negative=41 fail=0 ran=80/41`；`PASS|cli-contract|cases=17 fail=0 ran=17`；`PASS|packaging-smoke|cases=4 fail=0 ran=4`；runtime-codes `runtime_rows=39 width_rows=6 cli_rows=19 fail=0`。
- 读数：baseline 七段 **7/7**；asan-linux 八段 **8/8**（`R=7` ASAN／LSAN 零报告）；`cargo test` **292/0**；`l2g-baseline` **275/275**、`l2f-baseline` **246/246**、`l2e-baseline` **209/209**、`l2c-baseline` **105/105**、legacy **226/226**；`compiler/src` E30xx 字面 **0**；L2f 三型 C 快照 **5/5 逐字节 SAME**；全 job `SKIP|`／`FAIL|` 0 行。
- 证据：GATE 报告 `shared/ZOR-L2g-GATE-01-20261023/REPORT.md`（sha256 `eebb71c5660904831a6c9658faecbc8d98b1864edca930373846eee9fb1cfd6d`）；证据包 `SHA256SUMS`（23 文件）`d4a38c247ee402e7c1411acea1aa6a90c677f6cd02b095eb8d8174820989d19d`；Stage-2 expected 九条与产品会签逐字节一致（`expected/S10-01..09`）。

### Known issues（跟踪中，非本版拦截）

- **typed Box 覆盖**：仅 8 个整型宽度；`f32`／`f64`／`bool`／`byte`／`unit`、嵌套与复合 Box 维持 fail-closed，另票跟踪。
- **载荷形态**：仅非负整数字面量；负数载荷与 `box -1` fail-closed；不放开有符号负字面量、算术表达式、标识符与函数返回值。
- **零扩容**：新增宽度只开 Box 路径；非 Box 的 i8／i16／i32／u16／u64 算术／比较、非 i64 Box 参数／返回／struct 字段、复合 `println((*b,1))` 均维持 rc=1。
- **`let v = *b`**：仅核心单测覆盖，不列入 stdout 新验收面、不计入 142／206，不作为用户能力表述。
- **浮点／NaN**：① 结转；0.3.5 的 f64／f32 字面量与打印（含 inf／-inf／-0）保持原样；无 NaN 源形态、无浮点算术／比较，不列入本版判据。
- **平台**：Linux x86_64 为唯一终证；macOS 仅辅助，不作判据；Windows／其他发行版不在本版。
- **PA-L2g**：PA-L2g-01～07 待 0.3.6 账本合并后执行。
- **性能**：`-O*` 只承诺接受与 C 编译器映射，不出性能数字、不作达标承诺。
- 零 tag／Release；不构成安装、包管理器、跨平台或生产环境可用性承诺。

## [0.3.5] - 2026-10-23

内部验证里程碑，非可用语言版本：本版为 L2f「typed Box 三型与双基线门禁」——在 0.3.4 的六档 `-O*` 与 `println(*b)` 修复之上，把 `println(*b)` 的载荷覆盖扩至 `Box<i64>`／`Box<u8>`／`Box<u32>` 三型整数注解构造，打印、重复读取、`unbox` 与 drop 语义保持；同时把 `l2e-baseline` 与 `l2f-baseline` 双基线接入 `ci/run_ci.sh` 的 `stdout-contract` 阶段 fail-closed 校验。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：typed Box 仅三型 `Box<i64>`／`Box<u8>`／`Box<u32>`，构造为 `let b: Box<T> = box <非负整数字面量>;`，载荷仅非负整数字面量；`Box<i64>` 范围 0～`i64::MAX`（min=0），`i64::MIN` 不可构造（N/A），`box -1` 维持 fail-closed；默认 `box 1` 仍为 `Box<i64>`。`println(*b)` 对三型按十进制输出，支持单实参、多实参（含同调用多个 Box 解引用）与重复读取，不消费 Box、作用域结束只 drop 一次，`unbox` 语义不变；六档 `-O*` 输出一致。`println(b)`、`Box<Box<T>>`、`Box<struct>`、`Box<[T;N]>`、含 Box 的复合值、其余 9 个原始类型与 `unit` 的 Box 维持 rc=1 fail-closed，不新增 E 码。计数 **124**＝stdout **120**（runtime 71＋负控 32＋CLI 17）＋packaging **4**；全量 **188**；runtime-codes **39＋6＋19**（64 行）；`l2f-baseline` **188** 条（sha256 清单 246 行）；`l2e-baseline` **162** 条（sha256 清单 209 行）零改；`l2c-baseline` **68** 条（sha256 清单 105 行）、`legacy-snapshot` **226** 零改。NaN 端到端与跨平台证据均 N/A；Linux x86_64 为唯一终证，macOS 仅辅助。

### Added

- **typed Box 三型**：`Box<i64>`／`Box<u8>`／`Box<u32>` 注解构造按声明宽度推断载荷类型（check 期望下推约 22 行，仅宽度推断，不改所有权／类型规则）；IR 解引用泛化，C 后端按 `int64_t*`／`uint8_t*`／`uint32_t*` 与对应十进制格式输出。
- **`println(*b)` 三型覆盖**：单实参、多实参、同调用多个 Box 解引用（`println(*a, *b, *c)`）与重复读取；不消费 Box、不 move／unbox／drop 载荷，作用域结束只 drop 一次；`unbox` 后不得再读（既有 E2301）。
- **双基线门禁**：`ci/run_ci.sh` 的 `stdout-contract` 阶段校验 `l2e-baseline`（162／209／209）与 `l2f-baseline`（188／246／246），判词含 `fail=0`；任一 mismatch 即 FAIL，不 SKIP、不重跑洗绿。

### Changed

- **计数与资产**：stdout 由 **94**（65＋12＋17）扩至 **120**（71＋32＋17）＋packaging 4＝**124**（0.3.4 总 98 → 124）；全量 **162 → 188**；`l2f-baseline` 新建 188 条（sha256 清单 246 行）；`l2e-baseline` 162 条（209 行）、`l2c-baseline` 68 条（105 行）、`legacy-snapshot` 226 条零改；runtime-codes 维持 39＋6＋19（64 行）。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.4` 落至 `0.3.5`；`compiler/Cargo.toml:5` description 版本串同步；`compiler/Cargo.lock:3` 的 `version = 4` 零触；`CHANGELOG.md` 本节顶部纯插入；账本合并后 `--version`／`-V` 输出 `zorcc 0.3.5`。
- **CI**：不新建 job、不改 workflow YAML、不改 ruleset；`stdout-contract`／`packaging-smoke` 维持 required；artifact 沿用现有日志上传。

### 门禁与回归读数

- GATE-L2f-01：**PASS**——main `c2923d2a59de56e7f6e992efce075002801d3805`；run `39902624395`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract`／`packaging-smoke` 五 job 全 **success**。
- 判词：`PASS|l2e-baseline|cases=162 files=209 unique=209 fail=0`；`PASS|l2f-baseline|cases=188 files=246 unique=246 fail=0`；`PASS|stdout-contract|runtime=71 negative=32 fail=0 ran=71/32`；`PASS|cli-contract|cases=17 fail=0 ran=17`；`PASS|packaging-smoke|cases=4 fail=0 ran=4`；runtime-codes `runtime_rows=39 width_rows=6 cli_rows=19 fail=0`。
- 读数：baseline 七段 **7/7**；asan-linux 八段 **8/8**（`R=7` ASAN／LSAN 零报告）；`cargo test` **281/0**；`l2f-baseline` **246/246**、`l2e-baseline` **209/209**、`l2c-baseline` **105/105**、`legacy-snapshot` **226/226**；`compiler/src` E30xx 字面 **0**；`--help` 706 B 与 0.3.4 逐字节 SAME；`Box<i64>` C 快照与 0.3.4 逐字节 SAME；全 job `SKIP|` 0 行。
- 证据：GATE 报告 `shared/ZOR-L2f-GATE-01-20261023/REPORT.md`（sha256 `6f5f8f12cf5da12070312fe562dc96c6d3417a9a9eec85e95ee9799aeb55f285`）；证据包 `SHA256SUMS`（16 文件）`3632abbff39dbb9f1d802972490f10d07d96573bec06c229261ff4060c1743f2`；Stage-2 expected 六条与产品会签逐字节一致（`evidence/CAPTURE.md` `e9431055368058cdefba22636e6ec63210df77dbd0bedf93c958a9c4fced9d66`）。

### Known issues（跟踪中，非本版拦截）

- **typed Box 覆盖**：仅 `i64`／`u8`／`u32` 三型；其余 9 个原始类型、`unit`、嵌套与复合 Box 维持 fail-closed，另票跟踪。
- **载荷形态**：仅非负整数字面量；`i64::MIN` 不可构造（N/A），`box -1` fail-closed；不放开有符号负字面量、算术表达式、标识符与函数返回值。
- **`let v = *b`**：仅核心单测覆盖，不列入 stdout 新验收面、不计入 124／188，不作为用户能力表述。
- **NaN**：端到端源形态 N/A；格式化位模式单测保持，不列入本版判据。
- **平台**：Linux x86_64 为唯一终证；macOS 仅辅助，不作判据；Windows／其他发行版不在本版。
- **PA-L2f**：PA-L2f-01～07 待 0.3.5 账本合并后执行。
- **性能**：`-O*` 只承诺接受与 C 编译器映射，不出性能数字、不作达标承诺。
- 零 tag／Release；不构成安装、包管理器、跨平台或生产环境可用性承诺。

## [0.3.4] - 2026-10-23

内部验证里程碑，非可用语言版本：本版为 L2e「用户可见优化档、`--help` 与 `println(*b)` 修复」——在 0.3.3 的 stdout／CLI 与复合输出之上，开闸 `-O0`／`-O1`／`-O2`／`-O3`／`-Os`／`-Og` 六档并映射 C 编译器同名旗标，新增固定 `--help`，修复 `println(*b)` 的 `Box<i64>` 单 Box 解引用路径；安装树 `packaging-smoke` 仍为 advisory，待账本合并后连续两次 main 全绿再切 required。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：`-O*` 六档全接受，缺省 `-O2`，重复按最后一次生效；每档真实传给 C 编译器同名旗标，不静默降级。溢出语义：`-O0`／`-O1`／`-Og` 为 checked（E3001、rc=101），`-O2`／`-O3`／`-Os` 为 wrap；E3002 除零在所有档位 rc=101。非法值 `-O4`／`-O9`／`-Ofast`／`-O2x`／裸 `-O` rc=2、零产物。`--help` 仅长旗标，任意位置短路，stdout 706 B 固定文本、rc=0、stderr 0 B、零产物；`--version`／`-V` 优先于 help，help 优先于非法 flag／`-o` 缺值；不提供 `-h`。`println(*b)` 仅修 `Box<i64>`，单／多实参、重复读取不消费；其余 Box 载荷与 `println(b)` 维持 rc=1 fail-closed；typed Box 泛型化另票，不是本版能力。内部溢出覆盖位仍仅由 CI 注入，不作为用户开关、不出现在 `--help`。计数 **98**＝stdout **94**（runtime 65＋负控 12＋CLI 17）＋packaging **4**；runtime-codes **39＋6＋19**（64 行）；全量 **162**；`l2e-baseline` **162** 条（sha256 清单 209 行）、`l2c-baseline` **68** 条（sha256 清单 105 行）、`legacy-snapshot` **226**（恰 5 条变更、221 条零改）。

### Added

- **用户可见 `-O*` 六档**：接受 `-O0`／`-O1`／`-O2`／`-O3`／`-Os`／`-Og`；缺省 `-O2`；同一 argv 重复按最后一次生效；各档真实映射 C 编译器同名旗标，`-g` 正交；无 `-o` 检查态接受档位但不产生产物。非法 `-O4`／`-O9`／`-Ofast`／`-O2x`／裸 `-O` rc=2、零产物。
- **`--help`**：argv 任意位置命中即短路；stdout 输出 706 B 固定帮助文本（15 行、LF、无 BOM／尾随空格）；rc=0、stderr 0 B、不读源、不依赖 runtime helper、零产物；`--version`／`-V` 优先于 help；help 优先于非法 flag／`-o` 缺值；`-h` 不提供，仍 rc=2。
- **`println(*b)` 修复**：`Box<i64>` 单 Box 解引用实参按载荷输出（`println(*b)`、`println(*b, 1)`）；不消费 Box，重复读取正确，作用域结束只 drop 一次；`unbox` 语义不变。

### Changed

- **优化与溢出语义**：`-O0`／`-O1`／`-Og` checked（E3001、rc=101）；`-O2`／`-O3`／`-Os` wrap；E3002 全档 rc=101；0.3.3 的 `-O0` checked／`-O2` wrap 零回退。
- **计数与资产**：stdout 由 **86**（62＋10＋14）扩至 **94**（65＋12＋17）＋packaging 4＝**98**（0.3.3 总 90 → 98）；runtime-codes 由 29＋6＋8 扩至 **39＋6＋19**（64 行）；全量 **162**；`l2e-baseline` 162 条（sha256 清单 209 行）、`l2c-baseline` 68 条（sha256 清单 105 行）、`legacy-snapshot` 226 条（恰 5 条变更、221 条零改）；`C4-02` 显式变更为无参 usage 断言（help 由 C5-01～03 覆盖），旧→新哈希在 QA 资产说明留痕。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.3` 落至 `0.3.4`；`compiler/Cargo.toml:5` description 版本串同步；`compiler/Cargo.lock:3` 的 `version = 4` 零触；`CHANGELOG.md` 本节顶部纯插入；账本合并后 `--version`／`-V` 输出 `zorcc 0.3.4`。
- **`packaging-smoke`**：本版合并时仍 advisory；账本合并后连续两次 main 全绿（GATE run `39860926956`＋账本 run）转 required，只改 ruleset `25633324`，切换后以 ruleset 为准；仅基础设施未执行可记录回退，代码红必须修复。

### 门禁与回归读数

- GATE-L2e-01：**PASS**——main `bdf45c626073ec79bc13d0cc1b902817bb7bca92`；run `39860926956`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract`／`packaging-smoke` 五 job 全 **success**。
- 三段判词：`PASS|stdout-contract|runtime=65 negative=12 fail=0 ran=65/12`；`PASS|cli-contract|cases=17 fail=0 ran=17`；`PASS|packaging-smoke|cases=4 fail=0 ran=4`；runtime-codes `runtime_rows=39 width_rows=6 cli_rows=19 fail=0`。
- 读数：baseline 七段 **7/7**；asan-linux 八段 **8/8**（`R=7` ASAN／LSAN 零报告）；runtime-codes **39＋6＋19**（64 行）；`l2e-baseline` **209/209**、`l2c-baseline` **105/105**、`legacy-snapshot` **226/226**；`compiler/src` E30xx 字面 **0**；help 706 B 与产品会签逐字节 SAME；GCC 六档探针 build／run rc=0、输出 SAME、argv 同名旗标，overflow 矩阵符合本版口径。
- 证据：GATE 报告 `shared/ZOR-L2e-GATE-01-20261023/reports/GATE-L2e-01-报告-v1.md`（sha256 `1453be4b433c183ca61161808d2c154707afc57429d43da1fc5a2b849a22536d`）；证据包 `SHA256SUMS`（32 文件）`a838b3de240a436255f82c6cdf362102f09fa99295a3d00ff28e8072aaf41586`；产品 expected 会签 `shared/ZOR-L2e-Expected会签-产品-v1.md`（`efdb7f2eb981df039235e0fd41c77c3d05473018181fcb40b896f5b77b279af2`）。
- 零 tag／零 Release；`[0.3.3]` 及更早历史节零改。

### Known issues（跟踪中，非本版拦截）

- **NaN**：无端到端源形态；格式化函数位模式单测覆盖，不列入本版判据。
- **平台**：stdout／CLI／packaging 终证仅 Linux x86_64；macOS 仅辅助，不作判据；Windows／其他发行版不在本版。
- **`println(*b)` 范围**：仅 `Box<i64>`；其余 Box 载荷与 `println(b)` 维持 rc=1 fail-closed；typed Box 泛型化不在本版，另票跟踪。
- **`l2e-baseline` 接入**：尚未接入 `ci/run_ci.sh`，由 GATE／QA 以 `shasum -c` 手工核验；本版通过。
- **`packaging-smoke`**：仍为 advisory；账本＋两连绿后再切 ruleset，切换前不作为 required。
- **PA-L2e**：PA-L2e-01～08 待 0.3.4 账本合并后执行。
- **性能**：`-O*` 只承诺接受与 cc 映射，不出性能数字、不作达标承诺。
- 零 tag／Release；不构成安装、包管理器、跨平台或生产环境可用性承诺。

## [0.3.3] - 2026-10-22

内部验证里程碑，非可用语言版本：本版为 L2d「复合类型输出与安装树 smoke」——在 0.3.2 的 stdout／CLI 契约之上，实现可打印复合值（tuple 0／≥2 元、array、struct、enum、嵌套）、`[u8;N]` 数组形态与只读投影 `p.x`／`t[0]`／`a[1]`，并提供 Linux x86_64 安装树 smoke。1 元组源形态 N/A；`-O*` 维持关闭并转 L2e。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：可打印性按类型级判定；含引用／`Box`／`Handle`／函数类型的复合值整体 rc=1 fail-closed（`zorcc: codegen:`、零产物），不宣称所有类型均可打印。G-D-1 实现面 **6／6** 仅在 stdout 契约 v1.2 口径下成立（含 v1.2.1 Q-2～Q-4 补丁）：多实参／终饰／12 原始类型／`[byte;N]`／运行期终止出口（L2c）＋复合 tuple（0／≥2 元）／array／struct／enum／嵌套（L2d）。计数 **90**＝stdout **86**（runtime 62＋负控 10＋CLI 14）＋packaging **4**；S5 11＋S7 3；N-04～06 退役归档、N-11～13 新增；旧五值 41／53／53／3／94、码表 `23/23/19/1/1/2/19/19/14`、manifest 48／7、runtime-codes 29＋6＋8、baseline 7/7、asan 8/8 零改。14 条新增 expected 与产品会签逐字节 SAME。

### Added

- **复合类型输出**：tuple（0 元 `()`；≥2 元 `(`＋元素以 `, ` 连接＋`)`）、array（`[`＋元素以 `, ` 连接＋`]`，0 长 `[]`）、struct（`类型名 { 字段: 值, ... }`，0 字段 `类型名 {}`，字段按声明顺序）、enum（无载荷／0 载荷 `变体名`；≥1 载荷 `变体名(...)`；不输出 enum 前缀）与嵌套递归；无尾逗号。1 元组源形态 N/A。可打印性按类型级判定；`&T`／`&mut T`／`Box<T>`／`Handle<T>`／fn 及含其成员的复合值整体 fail-closed；一次 `println` 原子输出。
- **`[u8;N]` 数组形态**：`[u8;N]` 按十进制数组输出（`[1, 2]`、`[]`），与 `[byte;N]` 原始字节严格区分；`[byte;N]`／`[u8;N]` 字面量元素仅编译期常量，非常量元素 rc=1 fail-closed。
- **只读投影**：`p.x`／`t[0]`／`a[1]` 可作为 `println` 实参，按成员类型递归格式化；下标须为整数字面量，非字面量下标 fail-closed；投影不移动／drop／改写值。
- **R-1 安装树 smoke**：canonical `prefix/bin/zorcc`＋`prefix/share/zor/runtime/{zor_rt.c,h}`；resolver 新增 `exe/../share/zor/runtime` 候选；安装树外置 exe 不命中构建机绝对路径；解析失败列出已搜索候选，rc=2、零产物。Linux x86_64 安装树 smoke 通过。
- **`packaging-smoke` 门禁**：新增独立 `packaging-smoke` job（`ubuntu-24.04` x86_64、timeout 20、artifact 90 天、fail-closed、advisory）；P-01～P-04 4/4 真执行；required 仍仅 `stdout-contract`，ruleset 未改。

### Changed

- **计数与负控**：stdout 由 72 扩至 **90**（runtime 62／负控 10／CLI 14）＋packaging 4；N-04～06（tuple／struct／enum 旧 fail-closed）退役归档，N-11～13 新增；S5 11／S7 3；旧五值、码表九值、manifest 48／7、runtime-codes 29＋6＋8、baseline 7/7、asan 8/8 零改。
- **`stdout-contract`**：更新为 62／10／14 全真执行；两段 `ran=` 守门；两轮 SAME；Linux x86_64 终证。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.2` 落至 `0.3.3`；`compiler/Cargo.toml:5` description 版本串同步；`Cargo.lock:3` 的 `version = 4` 零触；`CHANGELOG.md` 本节顶部纯插入；`--version`／`-V` 输出 `zorcc 0.3.3`。

### 门禁与回归读数

- GATE-L2d-01：**PASS**——main `fbab0752f42e8b0a550152787516fa7aeacf92de`；run `39811902198`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract`／`packaging-smoke` 五 job 全 **success**。
- 三段判词：`PASS|stdout-contract|runtime=62 negative=10 fail=0 ran=62/10`；`PASS|cli-contract|cases=14 fail=0 ran=14`；`PASS|packaging-smoke|cases=4 fail=0 ran=4`。
- 读数：baseline strict 七段 **7/7**；asan-linux 八段 **8/8**（`R=7` ASAN／LSAN 零报告）；runtime-codes **29＋6＋8**（runtime 58/58、width 12/12、CLI 8/8、`fail=0`）；`cargo test` **261/0**；码表九值 `23/23/19/1/1/2/19/19/14`；旧五值 **41／53／53／3／94**；manifest **48＝41＋7**；legacy-snapshot **226**；`compiler/src` E30xx 字面 **0**；14 条 expected 与产品会签 `l2d-expected.sha256`（`c5ea65994b47d4448c937d044979636d90d6687ef2ae187a7c75deca38c50cef`）**14/14 逐字节 SAME**。
- 证据：GATE 报告 `shared/ZOR-L2d-GATE-01-20261022/reports/GATE-L2d-01-报告-v1.md`（sha256 `d7686f223be1b9803ee97f0d9045e0afd4df61a601b6b546034cd0d0121111b6`）；证据包 `SHA256SUMS`（1126 文件）`2475689f56a396945298578b46a92c4e97c127210d5591d25de9990a8dc23560`。
- 零 tag／零 Release；`[0.3.2]`／`[0.3.1]`／`[0.3.0]` 历史节零改。

### Known issues（跟踪中，非本版拦截）

- **1 元组**：源形态 N/A（`(x,)` 折叠为 `x`，`(T,)` 折叠为 `T`），S5-04 未建；不新增语法。
- **`-O*`／G-D-2**：0.3.2 所记 `-O*`／G-D-2 归 L2d 为当时计划，0.3.3 起以 L2e 为准；`-O*` 维持关闭（`-O0`／`-O2`／缺省；`-Og`／`-O1`／`-O3`／`-Os` rc=2）。
- **`println(*b)`**：独立 `Box` 解引用实参在 main 0.3.2 已复现的既有缺陷（C 后端未声明变量，rc=2），不属 L2d 修复面，另票跟踪；复合内 `(*b, 2)` 已由新路径处理。
- **R-1**：仅 Linux x86_64 安装树 smoke 通过；不构成安装或包管理器／跨平台／生产环境可用性承诺。
- **`packaging-smoke`**：仍为 advisory job；终证仅 Linux x86_64，未提升为 required，ruleset 未改。
- **ASAN 范围**：asan-linux 八段 8/8；`R=7` 条 build-run 结论不外推至 62 条 runtime，未逐条 ASAN。
- **Q-3／Q-4 负控**：非常量元素、非字面量下标由类型／check 面与单测覆盖，无独立 CLI 负例；不改变 90 计数。
- **NaN**：无端到端源形态（D-L2c-12）；格式化函数位模式单测覆盖，不列入 L2d 判据。
- **跨平台**：stdout／CLI／packaging 终证仅 Linux x86_64；macOS 仅辅助，不作判据；Windows／其他发行版不在本版。
- 零 tag／Release；不构成对外 API 稳定承诺。

## [0.3.2] - 2026-10-22

内部验证里程碑，非可用语言版本：本版为 L2c「stdout 与结构化 CLI 契约」——在 0.3.1 的运行期诊断契约之上，定稿 stdout 契约 v1，落多实参分隔、终饰、12 原始类型、`[byte;N]` 与运行期终止前 stdout 原子输出，新增 `--diag=json`，冻结 `--version`／`-V`，并以独立 Linux x86_64 `stdout-contract` 门禁覆盖 stdout／CLI 字节级判据。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成安装承诺或生产环境可用性承诺。

本版口径：stdout 契约 v1 六面**定稿**；实现面 **5／6**——多实参分隔符、终饰、12 原始类型、`[byte;N]`、运行期终止前 stdout；**复合类型输出实现归 L2d**，L2c 对复合 `println`、`print`、引用／`Box`／`Handle`／函数类型及含其成员的复合类型按 rc=1 fail-closed（`zorcc: codegen:`、零产物），不宣称六面实现全部闭合，也不宣称所有类型均可打印。`--diag=json` 以 NDJSON 写 stderr，六键固定序 `code`／`severity`／`message`／`file`／`line`／`col`；`--version`／`-V` 保持任意位置短路；用户可见 `-O*` 维持关闭（仅 `-O0`／`-O2`／缺省；`-Og`／`-O1`／`-O3`／`-Os` rc=2）。本版不新增 AC 编号、不新增 E 码；计数 41／53／53／3／94 与码表 `23/23/19/1/1/2/19/19/14` 不变；L2a manifest 48＝41 `compile-only`＋7 `build-run` 零改。

### Added

- **stdout 契约 v1／五面实现**：`println(a1, ..., aN)`（N ≥ 1）按参数顺序输出，相邻参数之间恰一个 ASCII 空格 `0x20`，无首／尾多余空格；整次调用恰一枚 LF `0x0A`，不追加 CR 或第二个 LF；`println("")` 为 `\n`，`println("", "")` 为 ` \n`；单参 `println("Hello, ZOR")` 保持 `Hello, ZOR\n`。12 原始类型 `i8／i16／i32／i64／u8／u16／u32／u64／f32／f64／bool／byte` 按契约格式化：整型十进制、无前导零／无 `+`；`bool` 小写；`byte` 十进制 0～255；`f32／f64` 最短往返十进制，`f64` 阈值 `-4 ≤ e < 16`、`f32` `-4 ≤ e < 7`，科学记数形 `d[.ddd]e±XX`，`-0`／`nan`／`inf`／`-inf` 小写；`unit` 输出 `()`。`[byte;N]` 原样输出 N 字节，不加引号／不转义／不截断，N=0 输出零字节，可含 NUL／LF。一次 `println` 的参数全部求值成功后才写 stdout；任一参数求值触发运行期终止时，该调用输出零字节，此前已完成调用保留。
- **`--diag=json`**：编译期诊断以 NDJSON 写 stderr，每行一个对象，UTF-8、LF 结尾、无 BOM／多余空白；键恰为 `code`／`severity`／`message`／`file`／`line`／`col`，顺序固定，不新增键；`severity` 恒 `"error"`，`line`／`col` 为 1-based，`file` 为入参路径原样，`message` 与文本模式同一句。无诊断时 stderr 0 字节；rc 0／1／2 与文本模式一致；`-o` 失败零产物；CLI 误用仍走文本 `zorcc: CLI: …`；运行期 E30xx 仍走 `zor:` 文本，不输出 JSON。文本模式逐字节不变。
- **`--version`／`-V` 冻结**：argv 任意位置命中即短路；stdout 恰 `zorcc <version>\n`、rc=0、stderr 0 B；不读源文件、不依赖 helper、不产生产物；与 `--diag=json`／`-o`／未知 flag 同现时版本优先。
- **独立 `stdout-contract` 门禁**：CI 新增独立 `stdout-contract` job（`ubuntu-24.04` x86_64、timeout 20、artifact 90 天、`if: always()`、`if-no-files-found: error`、无 `continue-on-error`）；直调 `tests/stdout/run_stdout_contract.sh --strict`，stdout／CLI 两段 summary；72＝48 runtime＋10 负控＋14 CLI 全部真执行并以 ran 计数守门；两轮 rc／stdout／stderr SAME；Linux 双 locale（`LC_ALL=C` 与 `en_US.UTF-8`）8 条浮点字节 SAME（宿主可用时；不可用记路径分析，不判 FAIL）。artifact 仅证据文本，不含 `.zor`／`.bin` 夹具。
- **CO-8／G-D-3 终审**：R1 §3 定稿 `,` 为唯一规范字段分隔符形态；`;` 仅为 0.1.x 兼容容忍形态，保留解析但不出现在新样例、文档或输出示例中；移除评估归 1.0 前，不在本版改解析器；错误文案仍为 `expected ';' or ',' after field, found <token>`。旧 golden／negative 逐字节零改。
- **E1101 编目／G-D-4**：产出逐层实发点编目，lexer 3＋parser 19＋check 7＝**29** 发点；编目＝实测，不新增触发点、不新增 E 码；`spawn` 仍按未识别标识符／未知函数报 E1101、rc=1，不新增专属诊断。
- **W-2 文档清理**：`compiler/README.md` 历史旗标名删除，改为「内部溢出模式覆盖位由 CI 注入，用户文档不展开」；用户文档／usage 不含内部溢出模式覆盖位旗标名。

### Changed

- **stdout 输出面**：由单字符串 `println` 扩展为多实参／12 原始类型／`[byte;N]`；0.2.0／0.2.1 的 `-o` 构建、无 `-o` 检查态、rc 0／1／2、零残留、零隐式产物零回退；`println("Hello, ZOR")` 等既有输出零回退。
- **CLI 面**：新增 `--diag=json`；`--version`／`-V` 仅加断言与文档，不改既有短路行为；用户可见 `-O*` 维持关闭（`-O0` checked、`-O2`／缺省 wrap、`-g` 正交；`-Og`／`-O1`／`-O3`／`-Os` rc=2、零产物）；G-D-2 的 `-O*` 归独立 L2d 票，不在本版宣称整体闭合。
- **计数与码表**：positive 41／negative 53／golden 53／compare 3／`tests/specs` `.zor` 94 不变；code-table 九值 `23/23/19/1/1/2/19/19/14` 不变（`DECLRT=2`、`EMIT=19`、`GOLDEN=19`）；L2a manifest 48＝41 `compile-only`＋7 `build-run` 零改；`runtime-codes` 29＋6＋8 零改；stdout 期望落 `tests/stdout/expected/`，不进 `tests/specs/golden`。
- **版本锚**：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.3.1` 直锚 `0.3.2`（`Cargo.lock:3` 的 `version = 4` 零触）；`CHANGELOG.md` 本节顶部纯插入；`zorcc --version`／`-V` 实测 `zorcc 0.3.2`。

### 门禁与回归读数

- QA GATE-01（L2c）结论：**PASS**——`shared/ZOR-L2c-GATE-01-v2-20261022/ZOR-L2c-GATE-01-v2-QA-v1.md`（sha256 `d46ea764ac18b7c64f59cdabc2f0e1e8adbb532f6fea37534e1fb2ce9d29f606`）；main push run：`39766618919`／`0c0a87f`；`baseline`／`asan-linux`／`runtime-codes`／`stdout-contract` 四 job 结论：**success（4/4）**。
- `stdout-contract`：runtime 48／负控 10／CLI 14，**72 全部真执行**；两段 `ran=`：`PASS|stdout-contract|runtime=48 negative=10 fail=0 ran=48/10`、`PASS|cli-contract|cases=14 fail=0 ran=14`；`stdout_fail=0 cli_fail=0`；两轮 SAME；Linux x86_64、`locale_float_bytes=SAME`；artifact：`stdout-contract-logs-39766618919-1`（104 文件，无 `.zor`／`.bin`）。
- 旧门禁：baseline strict 七段 **7/7**；asan-linux 八段 **8/8**（`R=7` ASAN／LSAN 零报告）；runtime-codes **29＋6＋8**（runtime 58/58、width 12/12、CLI 8/8、`fail=0`、`run_rc=101` 34 行）；`cargo test`：**238/0**。
- 计数与码表：**41／53／53／3／94**；九值 `23/23/19/1/1/2/19/19/14`（A1–A8b1／HDR 违规 0）；manifest **48／7**；`r2:9` 零触；`compiler/src` E30xx 字面 0。
- 零 tag／零 Release；无 `[0.3.3]`；`[0.3.1]`／`[0.3.0]` 历史节零改。

### Known issues（跟踪中，非本版拦截）

- **复合类型输出**：契约在册，实现归 **L2d**；L2c 对复合 `println` rc=1 fail-closed，不宣称复合可打印。`[u8;N]` 按数组形态归 L2d；L2c 构建态对 `[u8;N]` 标注 rc=1 fail-closed。
- **`print`**：不在 stdout 契约、本版不实现；按 rc=1 fail-closed（`zorcc: codegen:`、零产物），不得以 `print` 替代 `println` 宣传。
- **NaN**：L2c 无浮点算术／NaN 源形态；NaN 输出契约保留，由格式化函数单测（位模式）覆盖，无端到端样本（D-L2c-12）。
- **检查态既有语义**：无 `-o` 时仅跑前端；不可 codegen 形态返回 0，构建态才 fail-closed。若需检查态也 fail-closed，另立 PRD 条款。
- **R-1 打包路径**：安装树／`packaging-smoke` 未纳入本版；L2c 不宣称具备安装能力，归 L2d 打包切片。
- **G-D-2／用户可见 `-O*`**：`-Og`／`-O1`／`-O3`／`-Os` 维持 rc=2；G-D-2 整体不在本版闭合，归独立 L2d 票。
- **stdout job required**：独立 job 已接入并以 Linux x86_64 终证；required 状态随本版账本切换。
- **跨平台**：stdout／CLI 字节级终证仅 Linux x86_64；macOS 仅辅助，不作 stdout 判据；Windows／其他发行版不在本版。
- **运行期终止**：E30xx 终止不保证 drop／Box 释放；L2b 的 ASAN／LSAN 7 条 `build-run` 结论不外推。
- 内存／性能：未出数，只写「目标」，不作达标承诺。
- 零 tag／Release；不构成对外 API 稳定承诺。

## [0.3.1] - 2026-10-22

内部验证里程碑，非可用语言版本：本版为 L2b「运行期错误与溢出契约」——在 0.3.0 的运行期安全地基之上，实发 E3001／E3002 运行期诊断（rc=101、`zor:` 前缀、源行号），落整数算术 checked／wrap 双档与除零一致语义，并新增独立 Linux `runtime-codes` 门禁。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。

本版口径：运行期诊断**仅**在算术真正执行且越界／除零时发出，编译期 rc 0／1／2 不变；E3001／E3002 不进入编译期 EMIT／golden，运行期期望串落 `tests/runtime/`。溢出为 checked 时（`-O0` 或内部模式覆盖位）发 E3001，wrap 时（`-O2`／缺省）按模 2^N 回绕；除零两档均 E3002。内部溢出模式覆盖位仅用于 CI 取证，不对用户暴露，未进入 `--help`、README 或用户文档。本版 `codegen` 面**仅** i64／u8／u32；其余在册整型宽度（i8／i16／i32／u16／u64／byte）参与算术时按 rc=1 fail-closed，不实发检查，**不构成覆盖全部整型宽度的承诺**。本版不新增 AC 编号；E3001／E3002 已在 R1 §8b 登记（`DECLRT=2`）并由本版起实发；计数 41／53／53／3／94 与码表 `23/23/19/1/1/2/19/19/14` 不变；`r2:9` 零触；L2a manifest 48＝41 `compile-only`＋7 `build-run` 零改。

### Added

- **E3001／E3002 运行期诊断**：新增 `runtime/zor_rt.c`／`runtime/zor_rt.h`，由 `zor_rt_fail(code, file, line, col)` 统一出口：先 `fflush(stdout)`，向 stderr 写**恰一行**＋一枚 LF（形如 `zor: runtime error[E3001] <file>:<line>:<col>: integer overflow`；E3002 同形，消息为 `division by zero`），再 `fflush(stderr)`，最后 `exit(101)`；不使用 `abort`／`_exit`／`exit(1)`。E30xx 字符串只存在于 runtime，生成 C 只传数值码位；stdout 不被污染。源位置由 IR 携带 line／col，生成 C 以 `#line`＋`__FILE__` 还原；路径按 `zorcc` 入参原样（相对／绝对不改写），列取操作符 token 列。
- **checked／wrap 溢出模式**：checked 用 `__builtin_*_overflow`（i64／u8／u32，含有符号与无符号的溢出／下溢），wrap 用无符号算术＋`memcpy` 回读；`+／-／*` 溢出在 checked 发 E3001、wrap 按模 2^N；`/ 0`、`% 0` 两档均 E3002；`MIN / -1` checked E3001、wrap `MIN`；`MIN % -1` 恒 `0`；`MIN / 0` 先判除数为 0 → E3002；`-g` 与溢出模式正交。常量表达式在 checked 档保留运行期检查，不提前转为编译期错误；wrap 可模 2^N 折叠。
- **`runtime-codes` 独立门禁**：CI 新增独立 `runtime-codes` job（`ubuntu-24.04`、`bash ci/run_ci.sh --runtime-codes`、timeout 20、strict 常驻、artifact `runtime-codes-logs-${run_id}-${attempt}` 90 天、`if: always()`、`if-no-files-found: error`）；样本面＝29 条 runtime＋6 条宽度 fail-closed＋8 条 CLI 负控，只使用 i64／u8／u32 与已接受档位（`-O0`／`-O2`／缺省／`-g`）。rc=101 只在**无 sanitizer 的普通构建**上判（LSAN 会覆盖 101）；ASAN／LSAN 退出码在 `asan-linux` 段单列；终止样本固定 `detect_leaks=0`；L2a 的 baseline 7/7、asan-linux 8/8 与 manifest 48／7 零改。
- **规范登记转实发**：R1 §8b 标题改为「运行期诊断码位登记（L2b 起实发）」，正文补历史句；`ci/run_ci.sh` HDR 四项（新标题正／旧标题负／历史句／A8b1 归口）与本版**同一提交**收紧；`DECLRT=2`、`EMIT=19`、`GOLDEN=19`、码表九值不变；E30xx 不进入编译期 EMIT／golden。

### Changed

- **退出码分层**：编译期成功 0、源码诊断 1、CLI／环境误用 2 均不变；运行期 E3001／E3002 统一 101；sanitizer 工具退出码（ASAN／LSAN）单独分类，不并入 E30xx，也不作为 ZOR 运行期码。
- **溢出语义**：无符号与有符号**同规则**——checked 档下 `+／-／*` 溢出或下溢发 E3001，不因无符号而静默回绕；wrap 档才按模 2^N。`f32／f64` 不走 E3001／E3002（IEEE754 直通）。
- **用户 CLI 不变**：仍只接受 `-O0`（checked）／`-O2`／缺省（wrap）／`-g`；`-Og`／`-O1`／`-O3`／`-Os` 继续 rc=2、零产物；合法重复参数 last-wins（`-o` 仍为唯一重复即 rc=2 的参数）；`--output` 不作别名。用户可见 `-O*` 档位契约与白名单扩展归 L2c，不在本版。
- **文档与门禁**：本节为 0.3.1 新增用户文档行，不含内部溢出模式覆盖位的旗标名；`F-RC-14` 以本节插入为转绿前置；历史 `[0.3.0]` 节不回改。

### 门禁与回归读数

- QA GATE-01（L2b）结论：**PASS**——`shared/ZOR-L2b-GATE-01-20261022/ZOR-L2b-GATE-01终报告-QA-v1.md`（sha256 `5aa0597a14abaf4f6ece8b5bb1f83545c3b7141f0baea014acf539a0aac649ed`；证据包 49 文件 `900a1ae981ad7c6371ac1400c55ccefa099df57ead94cd96a661003585582c5e`）。main push run：`39756401504`（head `0b00eb2`），baseline／asan-linux／runtime-codes 三 job success；`runtime-codes` job：runtime 58/58（29×2）、width 12/12（6×2）、CLI 8/8、`fail=0`、两轮 summary SAME、artifact `runtime-codes-logs-39756401504-1`；baseline strict 七段：7/7（artifact `ci-logs-39756401504-1`）；asan-linux 八段：8/8（artifact `asan-logs-39756401504-1`）；`cargo test`：191/0。
- 计数与码表：positive 41／negative 53／golden 53／compare 3／`tests/specs` `.zor` 94（已复核）；code-table `23/23/19/1/1/2/19/19/14`（`DECLRT=2`、`EMIT=19`、`GOLDEN=19`）；L2a manifest 48／7 零改；`r2:9` 零触。
- 退出码取证：Linux `ubuntu-24.04` x86_64 实测 `exit(101)`（main run `39756401504`，`run_rc=101` 34 行；artifact `runtime-codes-logs-39756401504-1`）；ASAN UAF rc、LSAN rc 单列于 `asan-logs-39756401504-1`；GATE 报告 sha256 `5aa0597a14abaf4f6ece8b5bb1f83545c3b7141f0baea014acf539a0aac649ed`。macOS 不作 rc=101 终证。
- 零 tag／零 Release；无 `[0.3.2]`；0.3.0 历史节零改。

### Known issues（跟踪中，非本版拦截）

- **整型宽度覆盖**：本版 `codegen` 仅 i64／u8／u32；i8／i16／i32／u16／u64／byte 参与算术时 rc=1 fail-closed（含 `zorcc: codegen:`、零产物）。其余宽度的 codegen 与门禁样本归后续切片；本版**不宣称**覆盖全部整型宽度。
- **用户可见优化档**：`-O*` 档位契约、`-Og`／`-O1`／`-O3`／`-Os` 白名单扩展归 L2c G-D-2；本版不承诺用户可选择 checked／wrap。用户可见缺省（`-O2`）语义为 wrap，而非 checked。
- **内部溢出模式覆盖位**：仅内部／CI；不对外暴露，未进入 `--help`、README 或用户文档；非法值 rc=2、零产物。不作为用户契约。
- **运行期终止清理**：E30xx 终止**不保证** drop／Box 释放；终止样本不纳入 ASAN／LSAN 泄漏零报告；L2a 的 7 条 `build-run` 零报告结论不外推到运行期终止样本。
- **诊断面**：E30xx 只在运行期；编译期不报溢出／除零；不新增操作符、类型名或调用栈字段，用户据 E 码＋源位置定位。
- **stdout 六数／CO-8／G-D-3／结构化 CLI**：L2c；`-o` 仍为唯一对外构建入口；不在本版。
- **容器／close**：L3；**FFI／回调**：R4；**G-B-3 基准劣化门禁**：L4（`bench/` 仅骨架）；均不在本版判绿。
- **跨平台**：本版新增 `runtime-codes` 的 Linux 宿主（`ubuntu-24.04`）；macOS 仅辅助，Windows 与其他发行版不在本版；不构成功能矩阵承诺。
- 内存／性能：未出数，只写「目标」，不作达标承诺。
- 零 tag／Release；不构成可安装／生产可用／对外 API 稳定承诺。

## [0.3.0] - 2026-10-22

内部验证里程碑，非可用语言版本：本版为 L2a「运行期安全地基」——在 0.2.1 的 hello 闭环之上，落 typed IR／type table、E2203／E2204 借用硬闸、G-E-2 的 move／overwrite／Box 载荷状态机、drop flags 与 Box 运行期释放，以及 Linux ASAN／LSAN 门禁。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。

本版口径：ASAN／LSAN 零报告**仅限** `tests/runtime/manifest.tsv` 的 7 条 `build-run`（hello＋6 条 Box／drop）；41 枚既有正例只承担**编译期零回归**，不进入 ASAN 运行集。不把 41 枚正例写成 ASAN 零报告范围；不将 manifest 7 条的结论外推为语言级结论。本版不新增 AC 编号；新增 E2203／E2204 两枚 E 码（R2 §8）；`r2:9` 零触；既有 45 枚 golden／45 枚 negative 零改（新增 8／8 另计）。G-E-2 只做 move／overwrite／Box 载荷部分闭合；容器／close 归 L3，FFI／回调归 R4。

### Added

- **typed IR／type table**：新增 `compiler/src/type_table.rs`，记录表达式／let／赋值目标类型与函数签名（键＝位置＋节点类别）；check 写入、lower 只读，`compile_typed` 返回 `Compilation{prog,types,diags}`；`compile`／`compile_to_diags` 兼容保留；lower 缺表、非 byte 数组或与字面量不符即 fail-closed。既有 41 正例编译期零回归；不新增用户可见 flag／输出。
- **E2203／E2204 借用硬闸**：E2203＝借用活跃期间写／移／unbox owner 槽位；E2204＝`&mut` 活跃期间直接读取 owner 槽位；R2 §2 增 owner 槽位直接访问句，R2 §8 增两行。新增 8 枚负例 `r2-n22…r2-n29`＋8 枚 golden：E2203 6 枚（p4／p5／p7／p9／p9b／p11）、E2204 2 枚（p10／p10b）；p12／p3／p10d 放行；绑定级保守口径，字段不相交的精确放行不在本版承诺。E2203 落件（含负例＋golden 同批）先于 drop／Box 代码生成。
- **G-E-2 状态机（部分）**：move／overwrite／Box 载荷的槽位状态机（`Live／Moved／Unboxed`）与 IR 事件；覆盖＝先移源、再 drop 旧活值；自赋值不 drop；移动／unbox 后重初始化不 drop；作用域按声明逆序只 drop `Live`。容器／close／FFI／回调不在本版。
- **drop flags／Box 运行期**：`BindBox`／`BindBoxAssign`＝`malloc`＋存值（覆盖先按 `drop_dst` 释放旧值）；`Unbox`／`ScopeExit`＝按运行时 `live` flag `free`，作用域逆序；`MoveAssign`／`SelfAssign` 不释放并转移 flag；OOM 显式报错；A1 守卫删净，7 条 `build-run` 转绿。
- **Linux ASAN／LSAN 门禁**：CI 新增 `asan-linux` job（`ubuntu-24.04`＋clang `18.1.3`、strict 八段、artifact 90 天、fail-closed F-1～F-15）；`tests/runtime/manifest.tsv` 48 行＝41 `compile-only`＋7 `build-run`（`cases=48`、`run_cases=7`）；7 条 `build-run` 带 `-g` 零 ASAN／LSAN 报告，UAF／泄漏／去 `-g` 三探针必红／必 FAIL，零 allowlist。macOS 仅作辅助，不作零报告证据。

### Changed

- 构建路径由 `compile()` 切到 `compile_typed()`，lower 消费 type table；0.2.0／0.2.1 的 `-o`、无 `-o` 检查态、rc 0／1／2、零残留、零隐式产物零回退。
- 计数与码表：positive 41 不变；negative 45→53、golden 45→53、`tests/specs` `.zor` 86→94；code-table 九值 `21/21/17/1/1/2/17/17/12`→`23/23/19/1/1/2/19/19/14`（A6=0）；`compare` 3 不变；`cargo test` 119→166（不删测）。M1：CI 资产计数含 M1 两枚，AC 用例面不含；报告不得把 41／45 写成 AC 覆盖数。
- CI：Linux `--strict --asan` 为八段；macOS baseline 保持七段；`asan-linux` 在 CORE-04 前预期红，转绿后由 GATE-01 复核；不在 macOS 上判 LSAN。

### 门禁与回归读数

- QA GATE-01 PASS @ main `60025b8`；main push run `39734639811`：baseline＋asan-linux 双 success；`cargo test` 166/0；双 locale 七段 strict 7/7；Linux `--strict --asan` 八段 8/8；计数 **41／53／53／3／94**；compile 82/0；unit 正例 41/0、负例 53/漏检 0；golden 53/53/差异 0；code-table `23/23/19/1/1/2/19/19/14`（A1–A8b1／HDR 违规 0）；compare 3/3；run-smoke PASS。
- manifest 48＝41 `compile-only`＋7 `build-run`；7 条 `build-run` 全 PASS（build/run rc=0、stderr 0 B、`debug_info=present`、`san=none`、stdout sha 匹配）；UAF／LEAK／NOG 三探针有牙；两轮一致；零 allowlist；artifact `asan-logs-39734639811-1` 90 天。
- 既有 45 枚 golden／45 枚 negative 零改（新增 8／8 另计）；`r2:9` 零触；零 tag／零 Release；无 `[0.3.1]`。

### Known issues（跟踪中，非本版拦截）

- **E30xx／`-Zoverflow`**：L2b；本版不实发 E3001／E3002，`-Zoverflow` 仅内部／CI 位，不构成用户可见契约；溢出模式按有效优化档（`-O0`／`-Og` checked，其余 wrap，缺省 `-O2` wrap，`-g` 正交）留待 L2b。
- **stdout 六数／CO-8／G-D-3／结构化 CLI**：L2c；本版未实现多实参分隔符、12 原始类型、`[byte;N]`、复合类型、运行期终止出口、`-O*` 契约、`--diag=json` 或版本短路；`-o` 仍为唯一对外构建入口。
- **G-B-3 基准劣化门禁**：L4；`bench/` 仅骨架，本版不判绿。
- **G-E-2 容器／close**：L3；**FFI／回调**：R4；不宣称 G-E-2 全闭。
- **跨平台**：只新增 Linux ASAN 验收宿主；macOS 仅辅助，Windows／其他发行版不在本版；不构成功能矩阵承诺。
- **字段不相交借用精度**：L2d／L3；绑定级保守拒绝不作为 L2a FAIL。
- **运行期残余**：OOM 路径不清理已分配 owner；emit 信任静态 liveness；ScopeExit 诊断锚点取最后语句位置；列入后续硬化。
- 内存／性能：未出数，只写「目标」，不作达标承诺。
- 零 tag／Release；不构成可安装／生产可用／对外 API 稳定承诺。

## [0.2.1] - 2026-10-21

内部验证里程碑，非可用语言版本：本版为 0.2.0 之后的**维护性补丁，零行为变更**——仅修复 `ci/run_ci.sh` 五处 shell 变量后接全角标点（`，`／`（`／`：`）在 UTF-8 locale 下被并入变量名的解析问题；不改门禁判定、rc 语义、七段结构、`compiler/src/**`、`tests/specs` 或运行期行为。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。零新增 AC 编号、零新增 E 码、零修改 `r2:9`、零新增 golden／negative；0.2.0 的首个可执行闭环与 0.1.x 编译期语义均不变。

### Fixed
- `ci/run_ci.sh` 五处 shell 花括号（仅在对应失败／告警分支触发；成功路径零行为变化）：L120 `$n_neg，`→`${n_neg}，`；L301 `$f（`→`${f}（`；L327 `$rc（`→`${rc}（`；L343 `$n_unann（`→`${n_unann}（`；L567 `$n_skip：`→`${n_skip}：`。全角标点造成变量名边界歧义，在 `set -u` 下报 unbound；加花括号后按字面边界解析，其余行零改。
- 0.2.0 闭合记录维护项计数勘正（六→五）：集成报告曾记「6 处」；按 main `0d514e0` 实读为 **5 处**——L360 已是 `${missing}（`（0.2.0 locale 修复已加括号），L553 无此形态；闭合记录已就地勘正，本版按 5 处执行。
- 版本与记录：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.2.0` 直锚 `0.2.1`（`Cargo.lock:3` 的 `version = 4` 零触）；`CHANGELOG.md` 本节顶部纯插入；`zorcc --version`／`-V` 实测 `zorcc 0.2.1`。
- 门禁与回归读数（QA 终门禁 @`981620f18a4c3bc211c55d398365d0363e50cbf3`，取刻 `2026-10-21T09:37Z–09:39Z`）：双 locale（`en_US.UTF-8`／`zh_CN.UTF-8`）strict 七段=7/7 PASS、rc=0（run `20261021T093813Z-57294`／`20261021T093819Z-58280`）；compile=`ok=82 fail=0`；unit=正例 41/0、负例 45/漏检 0；golden=`45/45/45`；code-table=九值 `21/21/17/1/1/2/17/17/12`；rust-compare=3/3；run-smoke=PASS（构建 rc=0、`Mach-O 64-bit executable arm64`；运行 rc=0、stdout 11 B sha256 `2d9c3d17…baca`、stderr 0 B；N-1～N-5h 全 PASS、`TMP_CLEAN`×3、0 行 FAIL）；`cargo test`=119/0；0.1.x 计数=41／45／45／3／86（五段日志与 0.2.0 main run `20261021T033348Z-17790` 逐字节 SAME）；版本锚 `zorcc 0.2.1`=`--version`／`-V` 一致（`Cargo.toml:3`／`Cargo.lock:7` 0.2.1、`Cargo.lock:3` 仍 `version = 4`）；零 tag、零 0.2.1 Release；无 `[0.2.2]`。

### Known issues（跟踪中，非本版拦截）
- 本版为维护性补丁：不新增语言能力、不改变 0.2.0 首片子集与用户 CLI 契约；0.2.0 的残余风险（G-E-1b／G-E-1c、ASAN／LSAN、stdout 六数、E30xx、R3／R4、stdlib／zb／LSP、跨平台、基准与内存／性能）不在本版判绿，归 L2–L4；内存／性能出数前只写「目标」。
- 0.2.1 仅以 CI 基线宿主（macos-14 arm64）验收；其他平台归后续切片。
- 0.1.x 计数不变式 41／45／45／3／86、`tests/specs` 零 diff、`r2:9` 零触；不新增 AC 编号；零 tag、零 0.2.1 Release。

## [0.2.0] - 2026-10-21

内部验证里程碑，非可用语言版本：`zorcc` 首次具备**可执行闭环**，但仅覆盖 0.2.0 首片最小子集（`main`＋字符串字面量 `let`＋`println` 单实参），不承诺安装、stdlib、性能／内存、跨平台或对外 API 稳定。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。本版性质＝**首个可执行闭环（对账锚 G-A-0，非 AC 编号）＋G-E-1 前置**，非语言能力全面扩面：零新增 AC 编号、零新增 E 码、未改动 `tests/specs` 的 golden／negative 集合与计数、零修改 `r2:9`；0.1.x 源码判定与 `tests/specs` 计数不变；新增面为 C11 后端、`-o` 构建、运行期门禁与运行期控制件。

### Added
- **原生 codegen 首片**：新增 `compiler/src/backend/**`（typed IR、subset 闸、lower、C11 emitter、系统 `cc` 调用）与 `compile() -> (Prog, Vec<Diag>)`；`compile_to_diags` 改为薄包装。首片子集＝`main`（unit 返回）＋字符串字面量 `let`＋`println` 单实参＋正常退出；子集外 rc=1、`zorcc: codegen:` 诊断、无产物、不新增 E 码。
- **`-o` 构建模式**：`zorcc <input.zor> -o <output>` 构建宿主原生可执行；无 `-o` 保持 0.1.x 检查模式（rc 按诊断、零新增文件）；`-o` 缺值／重复／空串／不可写／父目录缺失／目标为目录／与源同路径 rc=2、零产物、既有目标不变、不建父目录、无 `.zorcc-tmp-*` 残留；`cc` 缺失／失败 rc=2；源码诊断 rc=1，不以 rc=2 吞诊断。0.2.0 用户 CLI 只承诺 `-o`；不新增 `--check`，`--output` 不作别名，`-O*`／`-g` 仅内部／CI。
- **`hello.zor` 首个可执行闭环**：`zorcc examples/hello.zor -o <tmp>/hello` 产出宿主原生可执行（macOS arm64 Mach-O），运行 stdout 恰为 `Hello, ZOR\n`（11 B）、stderr 0 B、rc=0；连续两次构建／换临时目录结果一致；产物只落临时目录／用户 `-o`，不提交仓库。
- **运行期门禁雏形**：`ci/run_ci.sh` 新增 `run-smoke` 段（位于 `rust-compare` 后、可选 `bench` 前）；正例构建＋运行＋stdout／stderr／rc 比对；每次运行五类负控（期望串错一位／前端 `spawn` E1101／后端 fail-closed 最小 struct rc=1、`zorcc: codegen:`、无产物／无 `-o` 零新增文件／`-o` 误用与失败原子性 rc=2、sentinel 不变、无 `.zorcc-tmp-*`）；strict 缺 `cc`／`file` 判 FAIL、不 SKIP；同路径负控用临时副本；D1 措辞降日志。
- **G-E-1 前置（规范面）**：`specs/r2-ownership.md` §1／§3／§10 纯插入 3 行——非 Copy owner 槽位重赋值采「条件式立即 drop」（RHS 求值→槽位仍持活值则 drop 旧值→写入新值；自赋值／`x=g(x)`／已移出／已 unbox／部分移出子值不 drop；Copy 不适用）；`r2:9` 零触、不触发 J-7、不新增 E 码／AC／golden／negative；0.2.0 只落规范文本，运行期 drop 代码归 L2，不宣称 drop 已实现或运行期内存安全。
- **测试与版本**：前端诊断 76 项单测零改；按 D-020-9 更新 2 条 CLI `-o` 测试（`form1_output_flag_rejected_at_any_position`、`c2_cli_diagnostics_allocate_no_language_codes`），逐条列旧／新断言；总数 ≥78、不删测；新增 codegen smoke 与构建路径测试；版本锚 `compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 0.1.27→0.2.0（`Cargo.lock:3` 的 `version = 4` 零触）；`CHANGELOG.md` 顶部纯插入；`zorcc --version`／`-V` 实测 `zorcc 0.2.0`。
- 门禁与回归读数（QA 终门禁 @`70e81907311523aba29e5e39147eeaeba1fcbf68`，取刻 `2026-10-21T03:23Z–03:25Z`（构建 03:23:04Z；runs 03:23:08Z／03:23:16Z／03:23:19Z））：三 locale（`en_US.UTF-8`／`C`／`zh_CN.UTF-8`）strict 七段=7/7 PASS、rc=0（run `20261021T032308Z-5220`／`20261021T032316Z-6737`／`20261021T032319Z-7617`；五段日志跨 locale 逐字节 SAME）；cargo=build rc=0（`--release --offline`）；compile=`ok=82 fail=0`；unit=正例 41/0、负例 45/漏检 0；golden=`45/45/45`、差异 0；code-table=九值 `21/21/17/1/1/2/17/17/12` PASS；rust-compare=3/3（`error[E0106]`=3；D1 仅日志）；run-smoke=PASS（构建 rc=0、`Mach-O 64-bit executable arm64`；运行 rc=0、stdout 11 B sha256 `2d9c3d17…baca`、stderr 0 B；换目录复算一致；N-1～N-5 全 PASS、`TMP_CLEAN`×3、日志 0 行 FAIL）；`cargo test`=119/0；0.1.x 计数=41／45／45／3／86（`tests/specs` 零 diff、`r2:9` sha `72f7f391…` 未变）；版本锚 `zorcc 0.2.0`=`--version`／`-V` 一致（`Cargo.toml:3`／`Cargo.lock:7` 0.2.0、`Cargo.lock:3` 仍 `version = 4`）；F1（head B）=docs-only 4 文件 `+11/−11` 与改文 v3 逐字一致、代码／CI／specs／tests 零触；零 tag、零 0.2.0 Release；无 `[0.2.1]`。

### Changed
- `-o` 由 0.1.x 的「未实现／一律拒判」转为构建入口；无 `-o` 检查模式与既有六段行为保持不变；2 条 CLI 测试按 D-020-9 更新；`--output` 仍拒判、`-o` 缺值仍 rc=2。
- `rust-compare` D1 处置：判据统一为 `rc≠0` 且 stderr 含 `error[E0106]`；`missing lifetime specifier` 降为日志、不计红；0.1.x 历史件与已出报告不回改；0.2.0 CI 按 D-020-7 执行。

### Fixed
- `run-smoke` locale 脚本修复：9 处 `$var` 加花括号，避免全角括号在 UTF-8 locale 被并入变量名（`set -u` unbound）；六段函数体零改。
- CI 负控加固：无 `-o` 改查源目录零新增；`-o` 误用每例独立子目录并断言零产物；成功构建后补查 `.zorcc-tmp-*` 零残留；README 补 CI↔W5 编号映射。

### Known issues（跟踪中，非本版拦截）
- 0.2.0 只覆盖首片最小子集；stdlib／zb／LSP／泛型／trait／模式匹配／async／R3／R4 运行期／FFI／完整 stdout 契约／完整 CLI 均不做；`i64` 算术／`if`／`while`／多实参 `println` 为可选加速项，不作为 0.2.0 出口。
- ASAN／LSAN、溢出、基准、内存／性能均不在 0.2.0；macOS 无 LSAN，泄漏面归 Linux 或 BLOCKED（L2）；出数前内存／性能只写「目标」，不构成达标承诺。
- G-E-1 只落规范文本；AC-R2-3（ASAN 零报告）归 L2，0.2.0 不宣称 drop 已实现、不宣称运行期内存安全。
- G-E-1b：活跃借用期间直接写／移／unbox 现态 rc=0；drop 进入运行期后这些形态会 UAF；E2203 仅为 L2 候选码位，L2 开窗须先入 PRD／R2 §8 并经 QA 会签，4–6 负例＋golden 同批；未落件前不对含活跃借用的程序生成 drop／Box 代码。0.2.0 首片**可构建子集**不涉 Box／借用／重赋值／drop；无 `-o` 检查模式仍覆盖既有正例集。
- G-E-1c：`&mut` 活跃期间直接读现态 rc=0；不造成 UAF，归 L2 R2 §2 另案，不并入 G-E-1、不阻塞 0.2.0。
- 跨平台：0.2.0 仅以 CI 基线宿主（macos-14 arm64）验收；Linux／其他平台归后续切片。
- `run-smoke` 五类负控是门禁有牙的前提；任一类未 FAIL 即按缺陷处理，不得以正例通过代替。
- 0.1.x 计数不变式 41／45／45／3／86、golden／negative 零 diff、`r2:9` 零触；不新增 AC 编号；本版为内部验证里程碑，零 tag、零 release。

## [0.1.27] - 2026-10-20

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成／无产物／无运行链路）；本节不含任何运行期判据。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。本版性质＝**册实不符勘正批，非语言能力扩面**：零新增语言判定、零新增错误码、零新增 AC、零新增负例、零 golden 重捕、零运行期判据；`compiler/src/**` 零字节改动；`.zor` 源码面接受与拦截宽度与 0.1.26 逐字同宽。

### Fixed
- `compiler/README.md` 状态段勘正：L5 标题由「0.1.1 现状：R1 最小前端」改为「0.1.x 现状：R1＋R2 前端」（删版本括注，避免 bump 后过期）；L7 链路补 R2 所有权/借用检查（E1xxx／E2xxx 诊断）；L11 由「未实现 R2 借用检查／box/unbox」改为「已实现 R2 所有权/借用检查与 box/unbox；未实现 C 代码生成（0.2.x）」。
- `compiler/README.md` L10 已实现诊断码随实补齐（核心逐码确认，实发 17 枚）：`- 已实现诊断码：E1101/E1201/E1202/E1301/E1401/E1501/E1601/E1701/E1702/E1703/E1704（R1 §8 码表）；E1502/E2101/E2102/E2201/E2202/E2301（R2 §8 码表）。`
- README L10 码面口径：实发 17 枚；E2302 为 R2 §8 保留码位（0.1.x 不实发）、E2401 为 E1502 别名（不独立实发），二者均不列入 README「已实现诊断码」。
- `specs/r1-type-system.md` §6 追加控制流语句条款：0.1.x `if`／`while` 为语句、非表达式，不产出值；条件均须为 `bool`；`else`／`else if` 合法，`if` 无 `else` 合法；表达式化 `if` 归 0.2.x 候选；语句形正例引 §9 `r1-23`／`r1-24`／`r1-25`。条款逐字见 `specs/r1-type-system.md` §6。
- `specs/r1-type-system.md` §9 负例范围句由 `r1-n01…r1-n16` 改为 `r1-n01…r1-n23`，与 §8 表及目录实存一致。
- `examples/hello.zor` L3 注释勘正：由「原型编译器就绪前不承诺可编译」改为「本件为前端可编译的最小示例（`zorcc examples/hello.zor` rc=0）；0.1.x 无代码生成与运行链路，不产出可执行体」。
- 版本身份锚：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.26` 直锚 `0.1.27`（lock 由 cargo 同步、非手写；`Cargo.lock:3` 的 `version = 4` 零触碰）；`CHANGELOG.md` 本节顶部纯插入；`zorcc --version`／`-V` 实测 `zorcc 0.1.27`。
- 门禁与回归读数（QA 终门禁 @`cecfc9d5ea8f7e1eec80d306aa8c4f0bffbf8c3f`，取刻 2026-10-20T15:44:43Z（C）／2026-10-20T15:44:47Z（UTF-8））：positive `.zor` 41、negative 45、golden 45（逐字节零改）、compare 3 对、`tests/specs` `.zor` 86；双 locale（`LC_ALL=C`／`en_US.UTF-8`）strict 六段 PASS、rc=0；compile `ok=82 fail=0`；unit 正例 41／失败 0、负例 45／漏检 0；golden `45/45/45`、差异 0；code-table `21/21/17/1/1/2/17/17/12`、双 locale 逐字节 SAME；rust-compare `3/3`（annotated rc=0；unannotated rc=1、E0106=1）；`cargo test` 78（0/7/31/40/0）；三项阴性对照（缺 rustc／未去标注／仅 2 对）均按预期 FAIL；`zorcc --version`＝`zorcc 0.1.27`、`zorcc examples/hello.zor` rc=0（实际二进制）。

### Known issues（跟踪中，非本版拦截）
- 运行期／ASAN 验证不在本版范围；AC-R2-4 为编译期／规范面判据，运行期面仍挂 0.2.x。
- `rust-compare` 除 `error[E0106]` 外还要求 `missing lifetime specifier` 文案；rustc 文案若变更可能假红（rustc 版本以 QA 终门禁日志 `rustc -vV` 为准）。D1 登记，不在本版扩批。
- 0.1.27 为 0.1.x 册实不符勘正批；0.2.x 开窗与运行链路仍按路线图另行推进。

## [0.1.26] - 2026-10-20

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成／无产物／无运行链路）；本节不含任何运行期判据。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。本版性质＝**AC-R2-4 对外证据面落钉与规范同步，非语言能力扩面**：零新增语言判定、零新增错误码、零新增 AC、零新增负例、零 golden 重捕；`compiler/src/**` 零字节改动，`.zor` 源码面接受与拦截宽度与 0.1.25 逐字同宽。

### Added
- AC-R2-4 Rust 同题零标注对照落钉（0.1.26 项 7）：新建 `tests/specs/compare/`（`.gitkeep`＋映射表 `README.md`＋T-a／T-b／T-c 各一对 `*-annotated.rs`／`*-unannotated.rs`，共 3 对 6 件）。T-a＝二输入择一返回（Rust 侧 `pick` 返回引用 → ZOR 侧 `r2-ref-01-two-input-select-owned.zor` 改返回拥有值）；T-b＝struct 持引用字段（→ `r2-ref-02-struct-hold-value.zor` 改持值）；T-c＝容器／数组元素持引用（→ `r2-ref-03-array-hold-value.zor` 改持值，引用形由既有 E1501 负例承载）。三枚 ZOR 正例零标注、rc=0；三题与三枚控制流正例载荷限 Copy 集（`i64`），**本集不覆盖非 Copy 绑定重赋值形态**。
- CI `ci/run_ci.sh` 新增 `rust-compare` 两段判：`*-annotated.rs` 可编（`rustc --edition=2021 --crate-type lib --emit=metadata`，rc=0）；`*-unannotated.rs` 为同题最小去标注变体，必 rc≠0 且 stderr 同时含 `error[E0106]` 与 `missing lifetime specifier`。反空转三项＝具名生命周期在位、差异仅去标注、rc≠0 与 E0106 同时命中；strict 下 ≥3 对且按 basename 成对，缺 rustc 即 FAIL（fail-closed），非 strict SKIP。日志记录 `rustc -vV`、edition 与逐对 rc／E0106 命中数。
- 三枚控制流正例随项 7 并入：`r1-23-if-statement-no-else.zor`／`r1-24-if-statement-else.zor`／`r1-25-while-statement.zor`；0.1.x 口径 `if`／`while` 为语句、非表达式（表达式化 `if` 归 0.2.x 候选）；`specs/r1-type-system.md` §9 随实同步为 `r1-01…r1-25`，条款正文归 0.1.27 R1 §6 勘正。
- 规范同步：`specs/r2-ownership.md` §9 正例行点名 `r2-ref-01…r2-ref-03` 实存件，AC-R2-4 行点名 9 个实存对照文件名（3 ZOR＋6 Rust），`r2-ref-*` 通配集非空；AC-R2-4 在册结论句与「引用 AC-R2-4 结果必须同引 R2 §7 不可表达清单，不得单引」同引纪律逐字在位（全文见 R2 §9）。R2 §8 码表 8 行、码集不变。
- 回归读数（QA 终门禁 @`b8f10ff`）：positive `.zor` 41、negative 45、golden 45（逐字节零改）、compare 3 对、`tests/specs` `.zor` 86；双 locale（`LC_ALL=C`／`en_US.UTF-8`）strict 六段全 PASS、rc=0；compile `ok=82 fail=0`；unit 正例 41／失败 0、负例 45／漏检 0；code-table `21/21/17/1/1/2/17/17/12`、双 locale 逐字节 SAME；rust-compare 3/3（annotated rc=0；unannotated rc=1、E0106=1）；`cargo test` 78（0/7/31/40/0）；三项阴性对照（缺 rustc／未去标注／仅 2 对）均按预期 FAIL。
- 版本身份锚：并入新 main（merge commit `8a6440f`，未改写历史）后，`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.25` 直锚 `0.1.26`（lock 由 cargo 同步、非手写；`Cargo.lock:3` 的 `version = 4` 零触碰）；`CHANGELOG.md` 本节顶部纯插入；`zorcc --version`／`-V` 实测 `zorcc 0.1.26`。

### Known issues（跟踪中，非本版拦截）
- 运行期／ASAN 验证不在本版范围：AC-R2-4 为编译期／规范面判据，运行期面仍挂 0.2.x。
- `rust-compare` 除 `error[E0106]` 外还要求 `missing lifetime specifier` 文案；rustc 文案若变更可能假红。当前基线 `rustc 1.99.0 (b940084d7 2026-09-28)`，host `aarch64-apple-darwin`；后续建议固定工具链或改为只判码。
- `cargo test` 78（较 0.1.25 的 72 增加 6 枚同源单测）；不作为门禁通过条件。
- 表达式化 `if` 归 0.2.x；本版 `if`／`while` 仅语句形。
- 0.1.27 手册侧三处（H-a／H-b／H-c）与仓库侧四处批次未开工，待本版定稿后接单。

## [0.1.25] - 2026-10-20

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成／无产物／无运行链路）；本节不含任何运行期判据。按 D-043 发布方式，日常版本以本账本条目＋main 推送为准；本版零新增 tag、零新增 release，不构成任何可安装或可发布承诺。本版性质＝**规范裁定面与治理面的闭合，非语言能力扩面**：零新增语言判定、零新增错误码、零新增 AC、零新增 golden／负例，`compiler/src/**` 零字节改动，`.zor` 源码面接受与拦截宽度与 0.1.24 逐字同宽。

### Added
- 0.1.25 项 8b 联审闭合（PR #87）：四条矛盾面按「编译期可判定／运行期待验证」二分，六面裁定＝**三计入（①全、②条文面、④(i)）／三挂 0.2.x（③全、②行为面、④(ii)＋阻塞）**。产品侧签字 PASS（条件生效，2026-10-20T14:07:48Z）；核心侧 C-1/C-3 逐条同意、A/B 会签 PASS；QA 可复算复核与联审签字 PASS（2026-10-20T14:10:24Z，附条件）；**三席条件已满足；版本闭合待 bump 复取后由 PMO 宣布**。F1（A 句未入仓）／F2（B 句未入仓）／F4（核心签字缺）三项阻断随落仓与签字清除。零新增 AC、零新增错误码；不改 `r2:9` 公理。运行期面（③全、②行为面、④(ii)＋阻塞）挂 0.2.x 首项，0.1.x 不落承诺句。
- R2 规范面纯追加（`specs/r2-ownership.md` §10 段尾 L127–L131；`git diff --numstat`＝`5 0`，文件 126→131 行）：A 句（发送／接收归属）＝0.1.x 不向 R1 §5 移动／复制表补 `chan T`／`JoinHandle` 行；`chan T` 发送按 R2 §1 在册移动点「移入 channel（R3）」处理：发送即移动、接收即取回 owner（引 `specs/r3-concurrency.md` L9）；`JoinHandle` 在 0.1.x 不进入 R1 §5 表。B 句（多接收端条文面）＝0.1.x 只按 `r2:9` 单 owner 公理记录所有权（任一时刻恰一个 owner）；多接收端并发／竞争取回的运行语义在 0.1.x 未定义，挂 0.2.x 首项；R3 成文前不得以「扇入扇出」正例（AC-R3-2）宣称多接收端语义，本句不使 `r3:16` 的「扇入扇出」任务形态失效。标题与 A/B 各自带 C-3 字面句「本节不定义具体拼写」；`chan`／`JoinHandle` 的具体拼写、类型签名与语法形态归 0.2.x 语法冻结窗（OI-5-g）。`r2:9`、§7 清单、§8 码表（8 行）与 AC 集零改。
- 门禁与复取读数（QA 复取 @`385ad39`）：静态复取 22/22 PASS；双 locale（`LC_ALL=C`／`en_US.UTF-8`）strict 五段全 PASS、rc=0；golden 45/45/45、差异 0；code-table `21/21/17/1/1/2/17/17/12`，双 locale 日志逐字节 SAME（sha256 `f8d321e2…`）；正例 35／负例 45、漏检 0。bump 后新 head 由 QA 按裁定 A 再做一次终门禁，复取版本锚与 `zorcc --version`。

### Changed
- 版本身份锚随本版 bump：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.24` 直锚 `0.1.25`（lock 由 cargo 同步、非手写；`Cargo.lock:3` 的 `version = 4` 系清单格式行、零触碰）；`CHANGELOG.md` 本节顶部纯插入。按项目管理 2026-10-20T14:22Z 裁定 A，0.1.25 为路线图 v13 §二分配的独立版本位，不得以 0.1.24 身份合入后再跳到 0.1.26；bump 后 `zorcc --version` 以 QA 终门禁复取值为准（期望 `zorcc 0.1.25`）。

### Known issues（跟踪中，非本版拦截）
- 运行期面（③全、②行为面（多接收端并发／竞争取回）、④(ii)＋阻塞）在 0.1.x 未定义、未实发，挂 0.2.x 首项；R3 成文前不得以「扇入扇出」正例宣称多接收端语义。
- `chan`／`JoinHandle` 的具体拼写、类型签名与语法形态、`chan(T,N)` 形态归 0.2.x 语法冻结窗（OI-5-g）；spawn 捕获列表归 OI-2。
- 本版无运行链路，无 ASAN／行为验证；`E3001`／`E3002` 仍为规范登记、未实发。

## [0.1.24] - 2026-10-17

### Added
- 规范面 空白补录（0.1.24 项 8a・票面 v1→v2・裁口位 主裁⑱／⑲ 定形）＝`specs/r2-ownership.md` §10 段尾 追加 OI-5 一条线 八枚 条目（add 8／del 0・追加首行号 119＝既有 118 行 逐字节 零改写）：本线 唯一定义源＝**owner 失活点**（非 Copy 值 之 owner 槽位 终结 之刻），凡书「失活」必带档别（借用／owner），借用 档 活跃性 保持 §2 L24 原位原文＝本窗 零吸入 零改写；四处 现身（channel 槽位／重赋值 旧值／C→ZOR 回调 入值／`Box` 堆内存）归 同一条线，不逐场景 追加规则。本版性质＝**规范 登记面 之 补录**，判定面 与 接受面 零变动：零新增 错误码（§8 行集 仍 8 枚＝[98…105]）、零新增 AC、零实现（`compiler/src/**` 零字节改动）、零 golden 变更（枚 45／objectsize 和 7066 之 集合恒等三判 预期值 未动）。「编译期可判定／运行期待验证」之 二分 与 联审签字 归 0.1.25 项 8b，`chan(T,N)` 归 0.2.x 语法冻结，spawn 捕获列表 归 OI-2＝本窗 皆 不成文。本版仍为**内部验证里程碑、非可用语言版本**，`zorcc` 仍为纯编译前端（零代码生成／零产物／零运行链路），`E3001`／`E3002` 仍为规范登记、未实发，`E1701`～`E1704` 在「码表对外页」未启动期间只作仓内登记，不构成任何可安装或可发布承诺，零新增 tag／零新增 release。
- 版本身份锚（名册 恰 4 枚・主裁⑱ 定形）＝`compiler/Cargo.toml:3` 与 `compiler/Cargo.lock:7` 由 `0.1.23` 直锚 `0.1.24`（各 add 1／del 1；`Cargo.lock:3` 之 `version = 4` 系清单格式行、零触碰＝主裁⑲-四 禁面(八)），`CHANGELOG.md` 本节 顶部 一节 纯插入（add 6／del 0），合入体树 之前形 blob 逐枚＝r2 `c6c1d0e5d628ac066a66f2e64c59a557e1d6f215`／CHANGELOG `0ece1a82d6238d3db592bba47ea9f5783efb40bf`／Cargo.toml `ca3ce66761ce9203b468bedef3b65d59e8df5c00`／Cargo.lock `230d863e5298e71aec672ce613c5f817ec0a9740`（后形 blob 与 numstat 之实测 由 支主 实现拍 落笔件 与 判据位 复取拍 并书）。判据面 三枚 之 `zorcc --version`＝`zorcc 0.1.24` 依 主裁㉕ 唯 归 管理席 合入前 全闸面（丙档・位内 build 实取）＝本窗 支主 零 `cargo`、不取 亦 不代取；base 双档 随标（主裁㉒-三）＝身份档 `8c927a78e12254fe4adeb048ed1d00d2f0b4ab2d`／内容档 `44dea20d5d3ac2aecad0162b6417ad84e19d08ff`。

## [0.1.23] - 2026-10-17

### Added
- 新增语言判定=跨类定义名冲突（0.1.23 项 7，票面 v1 格 1 择乙制「禁止同命名域」）。本版性质=**语言判定面扩一枚禁令**，与 0.1.22 同属接受面收窄、`.zor` 语法面零扩：同一模块命名域内同名而**异类**（`fn`／`struct`／`enum` 三类之间）的具名定义自本版起不得并存，后定义者必须报 `E1704 跨类定义名冲突`，报点＝该后定义者的定义关键字位（与 `E1701` 同位制，0.1.21 甲案随标沿用；struct 名位无独立 span，报点不落名 token）。唯一实现落点=`compiler/src/check.rs` 的 `check_def_names()` 内追加跨域交一支路（12 行纯插入、del 0）＝取上支同名同类所留 `seen`（每类首现枚）按 span 定序，对每一枚后定义者报一枚、先现枚不报；两码不叠报实测＝同名同类重复形仍只落 `E1701`（探针 p4），一名具三形时后两枚各报一枚（p3）。函数上行原注「跨类同名…本窗不拦」自本面作废＝逐字留档不回改（本窗 del 上界一枚已整枚归 §6 该句之整句替换，该注改写候后窗另拍呈批）。配对件＝`tests/specs/negative/r1-n23-cross-kind-same-name.zor`（10 行）＋`tests/specs/golden/r1-n23-cross-kind-same-name.txt`（1 行／213 B，由 `ci/capture_golden.sh` 同式以本支 release 二进制实测捕获产出、非手写；捕获实测 rc=1）。本版仍为**内部验证里程碑、非可用语言版本**，`zorcc` 仍为纯编译前端（零代码生成／零产物／零运行链路），本节不含任何运行期判据，`E3001`／`E3002` 仍为**规范登记、未实发**，`E1701`～`E1704` 在「码表对外页」未启动期间只作**仓内登记**，本版不构成任何可安装或可发布承诺，零新增 tag／零新增 release；退出码语义零扩面：新失败＝语言诊断，独占 rc=1，CLI 误用 rc=2 面未触。
- 规范面同步与门禁回归读数（本拍现取，两标＝面别＋式别）：`specs/r1-type-system.md` §6 的 0.1.20 跨类未定注记整句替换为**跨类同名句**（del 1／add 1，句内随标歧义缓解半句「上句『三类各成一域』只述 `E1701` 之辖界、不述跨域许可」），§8 最小码表其后新增 `E1704` 一行（定性＝实发，判据归口列非空，承载件＝上述 golden 一枚；§8 数据行自此 11 枚），§6 定义名重复句、既有 §8 表行描述串、`specs/r2-ownership.md`、`ci/**` 与 `ci/capture_golden.sh` 皆零字节改动（D2 成律辖界＝既有表行描述串，本行为新增行）。码表不变式 @base（BASE_next 面 `c6e3e74b833878fbbb451af3c9c0c2e89c59a06f`）`声明行=20 去重=20 DECLEMIT=16 DECLRES=1 DECLALIAS=1 DECLRT=2 EMIT=16 GOLDEN=16 散文码位=11` → @head `21/21/17/1/1/2/17/17/12`，六枚 +1 依次为声明行、去重、DECLEMIT、EMIT、GOLDEN、散文码位（末者来源＝§6 该句内那枚码位字面，A6 违规=0），A1–A7＋A8b1＋HDR＋定性闭集 @head 全部违规=0。`ZORC_BIN=<本支 release 二进制> bash ci/run_ci.sh --strict` 双 locale（`LC_ALL=C`／`en_US.UTF-8`）各 rc=0、五段全 PASS，两段 `code-table.log` 逐字节 SAME（845 B／`f8d321e2bc316c6e9b8cbc53be53c4ca5a1a42713f690924a64f33f7ff4e2706`；@base 侧另枚 845 B／`09cb83eab1dd4f6358479521f6a812a1905857e8aa06b2e2c6a052de21b7728a`＝前形与后形并立不互替）；golden 面读 `负例=45 比对=45 一致=45 差异=0 golden缺失=0 正例stderr非空=0 正例退出非零=0 golden基线空件=0`；既有 44 枚基线件身零写（`git diff --stat -- tests/specs/golden` 空集）并以全 45 枚重捕至仓外副本逐字节对平＝差异 0 枚。零回归之静态件证＝扫 81 枚 `.zor`（正例 35／负例 45／`examples` 1）之三类取名面，跨类同名命中恰 1 枚＝本枚新负例，其余 80 枚零命中⇒新支路于既有面无从发火。`cargo test` passed 之和 **72 → 72**（分段 0／7／28／37／0，failed 0、rc=0；本窗名册未含 `compiler/tests`，本判定行为覆盖＝配对件一枚＋仓外四枚探针，未新增 Rust 单测＝如实记，非降向）。`numstat` 对平票面 v1 §五 上界＝**add 31／del 1／合计 32**（界内 49／1／50，未触先报后动；逐枚＝check.rs 12／r1 增 2 删 1／负例 10／golden 1／本账本节 6）。版本身份锚未动＝`compiler/Cargo.toml:3` 仍 `0.1.22`（票面名册恰 5 路径未含该件，升版候裁、本席不越名册）。
- 版本身份锚（本拍补入链＝裁口位「0.1.23 随窗 bump・必做」甲格之执行形；上行末段「版本身份锚未动＝`compiler/Cargo.toml:3` 仍 `0.1.22`…升版候裁」为前形、逐字留档不回改＝纯插入形）：版本身份源恰两处点位由 `0.1.22` 直锚 `0.1.23`＝`compiler/Cargo.toml:3` 与 `compiler/Cargo.lock:7`（lock 由 cargo `--offline` 同步、非手写；`compiler/Cargo.lock:3` 的 `version = 4` 系清单格式行、非版本身份源、零触碰），实测 `zorcc --version` 后形＝`zorcc 0.1.23`；名册自此 恰 5 路径 → 恰 7 路径（＋`compiler/Cargo.toml` ＋`compiler/Cargo.lock`）、del 上界 1→3、合计上界 50→52、add 上界 49 不变。

## [0.1.22] - 2026-10-17

### Added
- 新增语言判定=程序入口缺失与入口签名形不合（0.1.22 项 6，范围冻结件 v2 §三）。本版性质=**语言判定面扩两枚禁令**，与 0.1.21 的「报点形修复、判定面零变动」不同：`.zor` 源码面的接受面未扩、拦截面自本版起对「编译单元内 `main` 恰一枚及其返回类型形」收窄；本版仍为**内部验证里程碑、非可用语言版本**，`zorcc` 仍为纯编译前端（零代码生成／零产物／零运行链路），本节不含任何运行期判据，`E3001`／`E3002` 仍为**规范登记、未实发**，新码 `E1702`／`E1703` 与 `E1701` 同属「码表对外页」未启动期间的**仓内登记**，本版不构成任何可安装或可发布承诺，零新增 tag／零新增 release。唯一实现落点=`compiler/src/check.rs`：新增 `check_entry_presence()`（零枚 `main` → 「程序入口缺失」，报点＝该文件 `1:1`，由 `run()` 先于定义名域调用）与 `check_entry_signature()`（显式非 `unit` 返回类型 → 「入口签名形不合」，报点＝该返回类型位，由 `run()` 后于唯一性检查调用且多枚时**只判首现那枚**）；`compiler/src/parser.rs` 零触（入口集合自 `self.fns` 登记面可取，实测无需 parser 扩面）。唯一性不另设码，沿 0.1.20 的 `E1701`；零枚时不叠报签名形；**带参数形态（`fn main(x: T)`）本版未作判定**，实测 rc=0 与 0.1.21 同（既非允许亦非禁止，见 §6 入口契约句）。退出码语义零扩面：新失败=语言诊断，独占 rc=1；CLI 误用 rc=2 面未触。配对件＝`tests/specs/negative/r1-n21-no-main.zor` 与 `tests/specs/negative/r1-n22-main-ret-int.zor`（各配同目录 `golden` 基线一枚，基线一律由 `ci/capture_golden.sh` 实测捕获产出、非手写）。
- 规范面同步（规范句先于码落）：`specs/r1-type-system.md` §6「函数」末新增**入口契约句**（三句式＝存在性 → 唯一性 → 签名形，为在册句「名字作用域=模块级唯一」与 §6 定义名重复句 `main` 半句的判据扩写；纯语义文，句内不落任何码位字面），§8 最小码表其后新增 `E1702`／`E1703` 两行（定性=实发，判据归口列各指 §6 该句的对应半句、非空，承载件＝上述两枚 golden）。随窗并销一处在册待销缺口＝`compiler/src/check.rs` 的 `check_def_names()` 文注原句「`main` 的存在性与签名形态归 0.1.22 项 6，本处不判」改为实态句（该两判自 0.1.22 起已由 `check_entry_presence()`／`check_entry_signature()` 实发，本函数只收枚数超限那一枚）＝注称不判而实判属件身与行为不合之假面，该件因此得 del 1／add 1 一对＝本窗 `check.rs` del 侧唯一非零项。本版该文件 `numstat`＝3 增 0 删（§6 一枚 bullet ＋ §8 两行，皆纯插入）；§6 其后的**跨类同名未定注记逐字未动**（在册行与 BASE 面 `cmp` rc=0）；`specs/r2-ownership.md` 零字节。
- 门禁与回归读数（本拍现取，同一 `--strict` 段面；`ZORC_BIN=<本支 release 二进制>`，双 locale `LC_ALL=C`／`en_US.UTF-8`）：@base（BASE 检出 `153669f5187a5cedfb77ec64476b911df4c96b84` 件身、BASE 二进制 `0b1105ce310c6bdef451533fe3b0f75d2d1b3a6b21c3df1c8bbb6430b83d33b8`）两 locale 各 rc=0、五段全 PASS，码表计数 `声明行=18 去重=18 DECLEMIT=14 DECLRES=1 DECLALIAS=1 DECLRT=2 EMIT=14 GOLDEN=14 散文码位=11`，两段 `code-table.log` 逐字节 SAME（845 B／`c0d5fe13537817686d1db7d0ea4b686f6963d38d60c4611c2a24fbb0b19f03df`）；@head 甲（两判与 §6/§8 文面在位、负例仍 42 枚面）两 locale 各 rc=0、五段全 PASS，计数 `20/20/16/1/1/2/16/14/11`——+2 者＝声明行／去重／DECLEMIT／EMIT，**GOLDEN 仍 14**（配对基线尚未捕获）、**散文码位仍 11**（§6 三句式不落码位字面的实测兑现），两段 `code-table.log` 逐字节 SAME（845 B／`0d5196df4c888ddfd7d6e7c9f8cf3d550660d6385f9ccbd93e86dc32af0ee80e`）；@head 乙（两枚新负例源件在位、44 枚面）两 locale 各 rc=1，仅 golden 段 FAIL 且读 `负例=44 比对=42 一致=42 差异=0 golden缺失=2`，其余四段 PASS、code-table 读数与甲同——该面＝棒次③ 捕获拍的入场信号，非本版终态，捕获后同式复跑之终值（GOLDEN=16、负例=44／比对=44／一致=44／差异=0／缺失=0）随该拍入本节。零回归：既有 76 枚 `.zor` × 三形（no-flags／`-Zoverflow=check`／`-O2 -Zoverflow=wrap`）＝234 项检查与 BASE 二进制逐字节同值且 rc 同＝**差异 0 枚**；全 78 枚 × 三形＝240 项检查仅 6 枚差异，逐枚点名＝两枚新负例（OLD rc=0 → NEW rc=1）；既有 42 枚 golden 之内容与命名零触（`git diff --stat -- tests/specs/golden` 空集、名集与 HEAD 面 `cmp` rc=0）；`.zor` 面 `正例数=35 正例失败=0`、`负例漏检(双模式合计)=0`；`cargo test` passed 之和 **72 → 72**（分段 0／7／28／37／0，failed 0、rc=0；本窗名册未含 `compiler/tests`，两枚新判定的行为覆盖＝两对配对件＋仓外九枚探针，未新增 Rust 单测＝如实记，非降向）。版本身份锚：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.21` 直锚 `0.1.22`（lock 由 cargo 同步、非手写；`Cargo.lock:3` 的 `version = 4` 未触），`zorcc --version` 实测 `zorcc 0.1.22`（12 B 去尾／13 B 含尾）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行（0.1.21→0.1.22 顺序直锚，无欠账叙事）。

## [0.1.21] - 2026-10-16

### Fixed
- 修复 `struct` 定义名重复的报点形（四形统一取定义关键字位）：`compiler/src/parser.rs` 的 `parse_struct` 补齐与 `parse_fn`／`parse_enum` 同形的一行取位，`StructDecl.span` 不再硬写 `(0, 0)`；`compiler/src/check.rs` 的 `struct` 支路随之简化，「首字段位代报」与零字段形态落 `0:0` 的兜底支路同删。`E1701` 短名与定性不变，诊断文案除坐标外逐字不动。语言接受面零变动：本版不新增语言判定、不新增错误码、不扩 `.zor` 语法与类型宽度，`main` 的存在性与签名形态仍不在判据内（归 0.1.22 项 6）；`zorcc` 仍为纯编译前端。 规范面随标一句（无新增承诺）：`specs/r1-type-system.md` §6 定义名重复句的报点子句改为「四形皆报该定义的关键字位」，同时移除「`struct` 报其首字段位…随 parser 扩面另窗收」那枚残差表述；§8 码表行的文案与归口列不动。
- 测试面与回归读数：补三对配对件 `tests/specs/negative/r1-n18-dup-struct.zor`／`r1-n19-dup-enum.zor`／`r1-n20-two-main.zor`（各配同目录 `golden` 基线一枚，基线一律由 `ci/capture_golden.sh` 实测捕获产出、非手写），使 `E1701` 的四形各得一枚回归位；本版唯一预期漂移＝golden 枚数 39→42，`EMIT==GOLDEN` 仍＝14（三对新件同码，去重侧不动），既有 39 件基线在报点形改动后逐字节零变。窗内次序律＝`span` 修复必先于基线捕获（golden 文案内嵌报点坐标）。

## [0.1.20] - 2026-10-16

### Added
- 新增语言判定=定义名重复（0.1.20 项 5a）。本版性质=**语言判定面扩一枚禁令**，与 0.1.17／0.1.18／0.1.19 三版的「门禁与规范面收口、语言宽度零变动」不同：`.zor` 源码面的接受面未扩、拦截面自本版起对「同名同类定义」收窄；本版仍为**内部验证里程碑、非可用语言版本**，`zorcc` 仍为纯编译前端（零代码生成／零产物／零运行链路），本节不含任何运行期判据，`E3001`／`E3002` 仍为**规范登记、未实发**，`E1701` 在「码表对外页」未启动期间只作**仓内登记**，本版不构成任何可安装或可发布承诺。`compiler/src/check.rs` 新增 `check_def_names()`（唯一实现落点，由 `run()` 步 0 先于登记调用），同一模块命名域内**同名且同类**的具名定义（`fn`／`struct`／`enum` 三类各成一域）第二次及其后的每一枚报 `E1701 定义名重复`，位置报在该重复定义处；多枚 `fn main` 由 fn 域收（名为 `main` 的函数至多一枚，`main` 的存在性与签名形态不在本句辖界）。四形行为本窗全部实装，配对输出基线只落重复 `fn` 一枚：`tests/specs/negative/r1-n17-duplicate-def-name.zor`＋`tests/specs/golden/r1-n17-duplicate-def-name.txt`（基线由 `ci/capture_golden.sh` 以本支 release 二进制实测捕获产出，非手写；捕获脚本零改动）。**跨类同名（`struct Foo`＋`enum Foo`）本版未作判定**，实测与 0.1.19 同为 rc=0（既非允许亦非禁止，见 `specs/r1-type-system.md` §6 跨类注记句）。退出码语义零扩面：新失败=语言诊断，独占 rc=1；CLI 误用 rc=2 面未触。
- 规范面同步（规范句先于码落）：`specs/r1-type-system.md` §6 新增**定义名重复句**（在册句「名字作用域=模块级唯一」的实发判据扩写，含 `main` 至多一枚半句）与其后的跨类未定注记（说明形，不设立任何码位），§8 码表其后新增 `E1701 定义名重复` 一行（定性=实发，判据归口列指向 §6 该句、非空）。码表不变式实测值（本席双拍现取，同一 `--strict` 段面）：@base（合入点 `432630795a42e2580688c4debdd5e966a8c17065` 件身）`声明行=17 去重=17 DECLEMIT=13 DECLRES=1 DECLALIAS=1 DECLRT=2 EMIT=13 GOLDEN=13 散文码位=10`；@head `18/18/14/1/1/2/14/14/11`——五枚 +1 依次为声明行、去重、DECLEMIT、EMIT、GOLDEN，`散文码位` +1 的来源是 §6 禁令句内那枚码位字面（A6 违规=0），DECLRES／DECLALIAS／DECLRT 零变；A1–A7＋A8b1＋HDR＋定性闭集 @head 全部违规=0。`ci/run_ci.sh` 与 `ci/capture_golden.sh` **零字节改动**（`stage_code_table` 计数为动态集合、`stage_golden` 负例面为 glob 自动发现，故 +1 码 +1 基线在现有形制下自动成立），三枚在册等值锚 `cmp -s`＠L146／`-ne`＠L145／`exit 2`＠L11,L19 未位移。版本身份锚与回归读数：`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.19` 直锚 `0.1.20`（lock 由 cargo 同步、非手写；全库带引号的版本字面量恰此 2 枚），`zorcc --version` 实测 `zorcc 0.1.20`。`cargo test` passed 之和 **72 → 72**（分段 0／7／28／37／0，failed 0、rc=0；本窗名册未含 `compiler/tests`，故新增判定的行为覆盖由新增负例对＋仓外四形探针承担，未新增 Rust 单测＝如实记，非降向）。`ZORC_BIN=<本支 release 二进制> bash ci/run_ci.sh --strict` @head 双 locale（`LC_ALL=C`／`en_US.UTF-8`）各 rc=0、五段全 PASS，两段 `code-table.log` 逐字节 SAME；golden 面 @head 负例=39／比对=39／一致=39／差异=0／缺失=0，正例=35／失败=0／stderr 非空=0，既有 38 枚输出基线重捕后逐字节零变（`git diff --stat -- tests/specs/golden` 空集）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行（0.1.19→0.1.20 顺序直锚，无欠账叙事）。

## [0.1.19] - 2026-10-16

### Added
- 门禁脚本在**非 `--strict` 且存在被跳过段**时于终端输出打出一行显眼告警（`[ci][警告] … 绿≠全绿`）：跳过面不再静默计入「全绿」印象。告警行仅随 stdout 输出，不进任何日志件；退出码语义完全不变（语言诊断独占 rc=1、CLI 误用 rc=2），summary 段数与判定结果不受影响。

## [0.1.18] - 2026-10-15

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成／无产物／无运行链路）；本节不含任何运行期判据——本版新登记的两枚运行期号位为**规范登记、未实发**，不构成运行期判据，内存与速度基准在本版仍是**未验证目标**。"每日一版"节奏第十八轮（序数依据=本账本在册各节首段连续计数，0.1.16 第十六轮与 0.1.17 第十七轮为在档前证，无仓库面第二证、不另造）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版环 1=PR A（项 2 诊断码表不变式；动件面五路径=`specs/r1-type-system.md`／`specs/r2-ownership.md`／`ci/run_ci.sh`／`ci/README.md`＋本账本条目，另并入版本身份锚两处=`compiler/Cargo.toml`／`compiler/Cargo.lock`；`compiler/tests/` 按选型裁定未加，不造规范第二载体）。**版本身份锚随本环并入**（`compiler/Cargo.toml:3`／`compiler/Cargo.lock:7` 由 `0.1.17` 直锚 `0.1.18`）。此处不设「另环执行」是有依据的：本节若先写「锚未闭」、bump 再拆到后续环，后续环就必须回头删改本节里那一行——而彼时 `[0.1.18]` 已随本环合入 main 成为在册历史节，破的是本窗最硬的账本纪律「逐版纯插入不回改」。趁本节尚未入册一次写完，不构成回改。本版性质=**规范面与门禁面的完整性收口，非语言能力扩面**：未新增检查器、未新增语言诊断码位实发面、未新写语言诊断文案、`.zor` 源码面接受与拦截宽度与 0.1.17 逐字同宽（本席 @base 与 @head 双侧现测：正例／负例／输出基线=**35／38／38**，对 0.1.17 在册值 +0/+0/+0；既有 38 枚输出基线逐字节零变，式别=工作树内容，复算=`git diff --name-only main -- tests/` 空集；负例数=基线数不变式维持）。

### Added
- 诊断码表不变式首次获得机器落点（0.1.18 环 1，PR A）：在此之前，"§8 声明的码位"与"编译器实际会发的码位"两件事只靠人工读册对平——本册 0.1.13 起在册的那枚缺口（`E1502` 有实发、有输出基线，却在两节码表里没有条目；而 `E2401` 一边被写成 `E1502` 的别名、一边自己占一条目）正是这种人工对平漏掉的东西。本版把它变成**每次门禁都跑一遍的判定**：`ci/run_ci.sh` 新增独立段 `code-table`，判定项为 A1 声明实发⊆实发集／A2 实发集⊆声明并集／A3 别名所指必为实发码／A4 运行期码位与实发集及输出基线互斥／A5 同码不得两行／A6 册内散文提到的任何码位都必须有表行／A7 标为保留或运行期的码位不得出现在实发集／A8b1 每行必须有非空判据归口／HDR 运行期节表头诚实句必须在位／定性列必落在闭集四值（实发／保留／别名→<码>／运行期）。单一实现处＝`ci/run_ci.sh` 的 `stage_code_table`（按选型裁定不另建单测载体，免规范文本被抄成第二份期望表）。口径源＝两节各自的取径注记。**本版实测值随节呈**：@base 声明集 14 枚、实发集 13 枚、输出基线集 13 枚（与实发集双向空差＝同集），`实发−声明＝{E1502}`、`声明−实发＝{E2302, E2401}`；@head 表行 17（编译期 15＋运行期 2）、`DECLEMIT=13 / DECLRES=1 / DECLALIAS=1 / DECLRT=2`、散文面命中 10 枚、A1–A7＋A8b1＋HDR＋闭集全部违规=0。**留证形（不拆拍、两枚 commit 同 PR）**：commit1 只升形制不补 `E1502` 行 ⇒ 该段 `A2` 与 `A6` 各判红一枚、集合均恰＝`{E1502}`、`summary.txt` 落 `FAIL|code-table|exit=1|…`、`overall` 非零；commit2 补行 ⇒ 终 head 五段全 PASS、`overall rc=0`。红落在合入体之外的中间 commit 上（合入闸的绿只绑终 head），不变式因此**被证伪过一次**而不是"从不判红的装饰"。
- `specs/r1-type-system.md`：§8 最小码表由单行列表升为**每码一行四列表**（7 行；`码位`列格内首枚 `E[0-9]{4}` 为码位、其后至格尾为描述串，描述串逐字搬运自升格前旧句，由一次性搬运闸 A8a 核对 7/7 通过）＋新增取径注记一枚（声明取径源＝**表结构**、非全角 `｜`（U+FF5C）字符约定，两者不得混用；并钉死"节区间只界定从哪一节取、区间内仍须按码位列首枚取、禁对区间正文全文 grep"——本席现核旧区间正文全量 grep 得 8 枚、其中 `E2202` 出处为 §8 表后的可达性注记行而非 r1 条目，若按全量口径即同码两收、A5 当场误红）。新增 **§8b 运行期诊断码位登记**（`E3001 整型算术溢出`／`E3002 除零`，定性=运行期；描述串取 §7 在册逐字短名，本席现核 `整型算术溢出` 与 `除零` 在 §7 各命中 1、在 §8 各命中 0；一次性闸 A8b2 两支锚齐成立）；节标题不含「编译期」故自成 `DECLRT` 与编译期声明集互斥。§7 节末新增一条独立 bullet 作交叉引用；**`:67` 溢出句与 `:68` 除零句逐字不动**（本席现核该两句现值与 base 同串），两句原文的"终止"侧措辞按实并存两种（`:67` 运行时终止错误／`:68` 运行期终止错误，各 1），故本版不取"终止"侧串入表，以免登记一枚不可对合的措辞。**§8 表题行、E1101 覆盖面消歧段、E1501 释义扩写表与其后各注记行零触碰。**
- `specs/r2-ownership.md`：§8 升同一形制（7 行）＋同一口径的取径注记（并明写定性列取闭集四值、不靠解析中文括注取得）＋**补 `E1502 返回引用` 一行**（定性=实发；描述串取在册逐字串 `返回引用`——本席现核该串在 r2 单件三处在册：§4「禁止返回引用」句、§7 清单条、§8 旧 E2401 条目括注；`E1502` 与 `E2401` 两行同串不是缺陷，恰是别名关系的语义自证）。承载件按**三址**登：输出基线件＋主发点 `compiler/src/parser.rs:281`（本席现核其文案与 `tests/specs/golden/r2-n07-return-ref.txt` 首行逐字对合）＋兜底点 `compiler/src/check.rs:71`（文案次形，少「禁引用逃逸」四字——此残差八断言无一条覆盖，随实记入承载件列而非另立判据）。`E2302`＝保留、`E2401`＝别名→`E1502` 两行只做"注记→填列"的定性承接、不顺手改判；**§7 In1 表行、§8 旧 E2401 括注、`tests/specs/negative/r2-n07-return-ref.zor:2` 与该 golden 首行构成的四方逐字对合链全程零触碰。**
- `ci/README.md`：新增**段名清单**（按"枚段名、不写段数计数句"的断本形制——计数句本身会随版本腐烂，仓库内唯一写"四段"的地方恰是不可改的历史节）；同时明写 `summary.txt` 每行形如 `状态|段名|exit=N|log=路径`（段名为第二列、非首列，本席现核 `record()` 体字面 `$2|$1|…` 而 `record <stage> <status>` ⇒ 状态在前），并写明历史「四段全 PASS」句绑 `[0.1.17]` 节名锚、**不得读作现态**。

### Fixed
- 版本身份锚：版本身份源恰两处点位——`compiler/Cargo.toml:3` 与 `compiler/Cargo.lock:7`，两处字面量由 `0.1.17` 直锚 `0.1.18`；`compiler/Cargo.lock:3` 的 `version = 4` 系清单格式行、非版本身份源、零触碰（lock 由 cargo 同步，非手写）。单一版本源不变：`zorcc --version`／`-V` 与 debug／release 双模式四路出口串实测均为 `zorcc 0.1.18`（各 13 B、rc=0，本席 @head 自建双模式二进制实跑留数，不以读码代替）。全库带引号的版本字面量恰此 2 枚（复算＝`grep -rn '"0\.1\.1[0-9]"' --include=Cargo.toml --include=Cargo.lock` 剔 `target/`），其余 `0.1.18` 串属规格注记与账本记述用途、非第二身份源。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.17→0.1.18 顺序直锚，无欠账叙事（PR#79，0.1.18 环 1；**第十五次例行口径**，序数依据=0.1.17 在册「第十四次例行口径」为唯一在档前证，无仓库面第二证、不另造）。本版与历版唯一不同处：bump 不独立成环、随项 2 同 PR 落地，依据与后果见本节首段。
- 门禁脚本自身的两处在册锚：本席 @head 现核 `ci/run_ci.sh` 的 **L1–L166 与 main 逐字节相同**（两枚 hunk 均纯增＝`@@ -166,0 +167,90 @@`／`@@ -171,0 +262 @@`；工具链位会审按此值复核同串，并指出本席首报 L1–L165 属保守值——超集成立，出清为 L1–L166）（新函数按"定义末追加"落在 `stage_bench` 之后），故三枚在册锚零位移——`exit 2` 仍恰 2 枚（`:11` 仓库根守卫／`:19` 未知参数，两处原文逐字未动）、`-ne` 仍恰 1 枚（`:142`，`grep -c -- '-ne'`＝1）、`exit "$overall"` 仍为件内末式。新段调用行为**独立一行裸调用**，插在 `stage_golden` 之后、`[ "$WITH_BENCH" = 1 ] && stage_bench` 之前（机制依据：`record()` 写的 `summary` 数组在 tee 块处即展开完毕，晚于该块则归档件无该行；且 `record` 尾式在 status=PASS 时短路使函数返回 1，取 `$?` 或接 `&&`／`||` 会把 PASS 读成 FAIL）。件内新增判定不含任何以 rc=1 为通过条件的形制；语言诊断独占 rc=1、CLI 用法错误固定 rc=2 的 0.1.17 口径未变。
- 单测面零扩如实记：`cargo test` passed 之和 **72 → 72**（本席 @head 分段实跑 0／7／28／37／0，failed 0、rc=0）。本版未新增单测——非降向；"只上向"在册句按"不得下降"读解，等值合规，此处明写以免被后席读成欠账。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效；`E3001`／`E3002` 的实发与用例面归此路径，0.2.x 前不得对外表述"已实现溢出／除零保护"）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。
- 本版新添一项残差（如实记，不另立判据）：A8a 搬运闸与 A8b2 新行出处闸须对 **base 侧旧文**核对，属升格窗一次性执行、不入常驻段面（A8b 的常驻半边 A8b1 已入段）；后席若再动表形，同一式须对当次 base 重跑一次。

## [0.1.17] - 2026-10-15

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成／无产物／无运行链路）；本节不含任何运行期判据——内存与速度基准在本版仍是**未验证目标**，其可测性随 0.2.x 运行链路落地后才成立。"每日一版"节奏第十七轮（序数依据=本账本在册各节首段连续计数，0.1.15 第十五轮与 0.1.16 第十六轮为在档前证，无仓库面第二证、不另造）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版=三个 PR 载荷（环 1 版本身份锚 PR#76／环 2 对未实现能力显式拒判 PR#77／环 3 本账本条目），环数=载荷数=3。本版主题=固定载荷位例行续执行·版本身份锚（环 1）+ 四个假绿面一次收口：对未实现能力显式拒判，不再静默接受（环 2）+ 本账本条目（环 3）。**本版性质=门禁完整性收口，不是语言能力扩面**：未新增检查器、未新增错误码、未新写语言诊断文案，`.zor` 源码面的接受与拦截宽度与 0.1.16 逐字同宽（本席 @main 现测：正例／负例／输出基线=**35／38／38**，对 0.1.16 在册值 +0/+0/+0，既有 38 枚输出基线逐字节零变，负例数=基线数不变式维持）。本版另有一处治理面随实记入（合入形态判据的适用条件首次成文），与产品面无关、见下。

### Added
- 命令行对未实现能力显式拒判，不再静默接受（0.1.17 环 2，PR#77，合入体 `9ad2f72`）：本版把四形「看着通过、其实什么都没做」的静默通过改成当场指名退出。**改前实态**（源=DOC-3 群共享权威坐标 `conv_01m4nmakww4e28bee4bx801pdb/shared/ZOR-0.1.17-范围冻结-PM裁定-v1.md` §〇-2 表 L31–L34，**权威件、非各席私有副本**；该件本席现取＝mtime `2026-10-15T19:46:23+0800`／116 行／12854 B／整件 sha256 `88a2cb162674fbdff25901ad214e221b…`，系 C-1a／C-1b 两段裁定回写并于 19:46 就地修正归因后的版；本窗该件整件 sha 先后换过三串（落笔时刻 18:58→19:36:09→19:46:23，值 `c512448e…`→`31e812f8…`→`88a2cb16…`），而 §〇-2 表 L31–L34 与 §三 C-1a（L73）／C-1b（L77–L78）三处行号锚**一律未位移**——本席逐处复核，故行号可续引、整件 sha 须以合入前最后一次现取为准；四形改前值系产品经理 @0.1.16 二进制现跑留数，本席引作改前基线、非自测——改前形在本版已不可复现，复现需回到 `4441c8b` 侧）：`zorcc a.zor -o out.bin` **rc=0 且无任何产物**、`zorcc a.zor <未知 flag>` **rc=0**、`zorcc b1.zor b2.zor` **rc=0 且第二枚从未被检查过**（该文件含非法类型名仍一路通过）、`zorcc -o out.bin a.zor` 非零退出但**错指**——报的是"读不到 `out.bin`"，把选项的值当成了源文件，真正的原因（`-o` 未实现）没被说出口。**改后真值（本席 @main 本机现测，release 二进制 `compiler/target/release/zorcc`；取 rc 一律去管道直取、argv 逐枚分列、显式 bash 运行）**：`-o` 前置／后置两形各 rc=2、stderr 120 B，同一指名句 `zorcc: CLI: '-o' 未实现：0.1.x 无代码生成路径，不产出任何文件（显式拒判，非接受并忽略）`；长写法 `--output=<path>` rc=2、127 B；未知选项 rc=2、96 B（句形 `zorcc: CLI: 未知选项 '<flag>'：不在本窗白名单内，不予静默接受`）；多位置参数 rc=2、230 B 且**在句内点名被丢弃的那一枚**（`只接受一个源文件，收到 2 枚：… （第二枚起显式拒判，静默丢弃会漏检未检查的源文件）`）；缺源文件形 rc=2、stderr 恰 32 B，在册 usage 句 `usage: zorcc [flags] <file.zor>` **一字未改**。无产物向自证：上述三形跑完后目标路径下 `out.bin` 不存在。承载件唯一=`compiler/src/main.rs`（白名单判定 L14、`-o`／`--output=` 指名句 L38／L43、未知选项句 L50、多出参句 L56），+161/−23。
- 白名单承重墙维持原样（0.1.17 环 2 判据句 C-1／C-1a）：`-O0`｜`-O2`｜`-Zoverflow=check`｜`-Zoverflow=wrap`｜`-g` 五枚，加 `--version`／`-V` 共为 C-1a 所定**固定六枚**，**仍为接受并忽略、退出码不变**，本席逐枚现测 rc=0 且 stderr 0 B，组合形 `-O2 -Zoverflow=wrap` 亦 rc=0；`--version`／`-V` 短路先于文件处理（0.1.2 在册语义原样保留），stdout 恰为 `zorcc 0.1.17`、rc=0。承重墙之所以是承重墙：门禁的双模式调用形全部落在白名单内（`ci/run_ci.sh:30` debug=`-Zoverflow=check`、`:31` release=`-O2 -Zoverflow=wrap`），拒判面在门禁路径上不可达，故本版不影响 73 组用例中的任何一枚。**准入判据自此成文（同件 §三 C-1a，L73；已回写范围唯一准件正文，非帖面）：白名单成员固定六枚，资格＝「有在册调用方」或「有已登记的占位语义」，二者皆无一律拒——"与白名单内某枚同族"不构成准入理由**；据此 `-O1`／`-Os` 维持拒判，本席现测各 rc=2、stderr 79 B 指名句。**一处必须按字面读的口径**：`-Zoverflow=check`／`-Zoverflow=wrap`／`-g` 的「接受」是**接受并忽略**，其运行期溢出行为与 ASAN 对等语义本版不实现、不生效、不承诺（归 0.2.x 运行链路落地后的 T-5 行为用例档）；「接受」不等于「生效」，本账本面只按前者记述。全量优化级别集合（`-O0/-O1/-O2/-O3/-Os/-Og`）的统一定形归 0.2.x CLI 面契约件（源=DOC-3 群共享权威坐标同目录件 `ZOR-PRD-§3判据改文与0.2x移交界面对账-v1.2.md` §二 **G-D-2 行 L71**，**权威件、非私有副本**；本席现读＝92 行／9381 B／整件 sha256 `a700da63f2877bbb…`，系「短路契约归口＋C-1a／C-1b 双向接缝」补写后的版），0.1.x 内不逐枚加白——逐枚加白正是"接受并忽略"复活的通道。
- 退出码分段与 CLI 误用面**不新增对外契约**（0.1.17 环 2 判据句 C-2）：命令行误用一律 rc=2，**绝不占用 rc=1**。理由为在册契约现读：`ci/run_ci.sh:142` 以 `[ "$rc" -ne 1 ]` 判负例 `EXIT_MISMATCH`、`ci/capture_golden.sh:14` 对 `rc≠1` 直接 ABORT——**rc=1 是语言诊断专用位**；若 CLI 用法错发 rc=1，负例门禁会把"命令行写错"读成"语言拦下"，那是比本版要收口的四个假绿更隐蔽的第五个假绿。回归向本席现测：含非法类型名件仍 rc=1 并发语言诊断（首行 `zorcc: E1201 …类型不匹配…`），本席 8 枚 CLI 误用形现测无一发 rc=1；判据位 41 枚用例表内 21 枚 CLI 句同样无一发 rc=1、无一含 `E[0-9]{4}` 码位串（源=DOC-3 票面 `ZOR-0.1.17-P2-判据位票面-v1.md`，本席引作辅证、非自算），R1 §8／R2 §8 码表零触碰。更要紧的一条定性：`compiler/README.md:9`（仓库在册件）早就写明「退出码：0=无诊断；1=有诊断；2=用法/读文件错误」，**所以本版不是新约定一套退出码，而是补上一处 0.1.x 一直声明却没有实现的用法错误面**——账本面按「对外契约零新增」记述，不写成能力扩面。
- 质量资产面同源 CLI 单测补齐 7 枚（0.1.17 环 2）：覆盖白名单接受集一枚、四形各一枚、长写法与缺值、非白名单须拒、CLI 诊断句不侵占语言码位、单文件正形回归。`cargo test` passed 之和 **66 → 72**（本席 @main 本机重跑分段=0／7／28／37／0，bin target 1→7，failed 0、rc=0），只上向；历史判定表与负例断言面零改动。**回归门**：`ZORC_BIN=<本支 release 二进制> bash ci/run_ci.sh --strict` 本席独立重跑 rc=0、四段全 PASS（cargo／compile／unit-test／golden，证据目录 `ci/logs/20261015T112602Z-64779`）；`git diff --name-only 4441c8b 9ad2f72 -- tests/` **空集**、逐件 blob id 升序清单 `cmp` 全等（38 行）⇒ 既有 **38 枚输出基线逐字节零变**；全库计数正／负／基线维持 35／38／38。
- 一处取证口径的教训随实入册（取证面，非产品缺陷）：同一句「38 枚基线零变」，本窗三席各呈一串聚合值且**互不相等**——执行席 `98ba28cf…`、判据位 `3f0a4fce…` 与 `93ffd86b…`、本席另算一枚 `d5c18f2384aa1437…`（算式=按文件名升序对 38 件各取 `sha256sum` 的摘要列首串、无分隔拼接后再取 sha256；式别=**blob 内容聚合，非 git tree**；复算路径=在 `tests/specs/golden/` 下跑 `ls -1 *.txt | LC_ALL=C sort | xargs sha256sum | awk '{print $1}' | tr -d '\n' | sha256sum`，**人人可复算、非本席单点**）。值不同而结论一致，病灶只是**没标算式与式别**。⇒ 本窗起账本面记则：**凡呈 digest／tree／聚合值必随标算式与式别；未标算式的聚合值只作辅证、不作判据，主判据一律取逐字节比对**。本版判据位的主判据即 `cmp` 逐字节 38/38 ＋ `git diff` 空集，聚合值列在票面内仅作辅证。**同条通则本窗触发第二次，且形态不同**：范围唯一准件 19:36 追加 C-1b、19:46 又**就地**修正段内归因，整件 sha 连换两串（`31e812f8…`→`88a2cb16…`）而三处行号锚（§〇-2 L31–L34／§三 C-1a L73／C-1b L77–L78）一律未位移。⇒ 记则：**凡引他册坐标（路径／行数／字节／sha／行号），在合入、出票、呈证据三类动作前任一时刻各重取一次现值，转述坐标永不算在册事实**；能就地修正就不追加行——追加同时打断整件 sha 与行号锚，就地修正只打断整件 sha。本席本节首稿引 `c512448e…` 当时为真、数分钟后即为旧值，正坐此条，非任何席位之过。
- 合入形态判据的适用条件首次成文（治理面随实记入，产品代码零改动）：核收判据 `tree(merge)==tree(head)` **仅当被合入 head 已包含当刻 main 时成立**；顺序合多支时必然不等。本窗现测恰为两组对照，全部本席 @main 侧 `git rev-parse <rev>^{tree}` 现取（式别＝**git tree 对象 id**；可达性＝**四枚对象本席侧均可取**，非仅呈方单点可取）：#76 该判据 TRUE（`tree(b3fa8e1)`＝`tree(14380e8)`＝`4be98f47da23f36b…`），#77 FALSE（`tree(9ad2f72)`＝`249034550926255d…` vs `tree(0b24155)`＝`25b978358fd91aeb…`），原因是 #77 的 base 为 `4441c8b`、而合入时 main 已含 #76——**这不是差项，是该判据的适用前提**。链可核对性自此双条校验：① first-parent 逐位对平（本席现测 `9ad2f72 → b3fa8e1 → 4441c8b`）；② **载荷并集三段可复算式**（每条即 `git diff --name-only <票 head> <main>`，任一票 head 所在支均可跑、不依赖任何合成对象）：`14380e88 → 9ad2f72` 恰＝`{compiler/src/main.rs}`，`0b241553 → 9ad2f72` 恰＝`{compiler/Cargo.lock, compiler/Cargo.toml}`，`4441c8b → 9ad2f72` 恰＝上两件之和＋`main.rs`（合计 3 件、+163/−25）⇒ **main 内容体恰等于两支载荷并集、零第三方内容**。判据位所呈合成等效树 `36e4584…` 系其独立 clone 内 `read-tree` 产物、**他席侧不可达**（本席 `git cat-file` 报 unknown revision），故该式只作加强版、呈值须随标可达性——这也是本版把「凡呈对象串必随标**算式＋式别＋复算路径（含可达性）**」记入账本的实况依据。限定句首次成文已入决策日志 D-063 ⑦，勘正批文句随 0.1.27 批 H-a 落册；本席当窗记入账本，是为防下一窗把这个"不等"误报成差项——误报差项最终落到账本就是册实不符。

### Fixed
- 版本身份锚（0.1.17 环 1，PR#76，合入体 `b3fa8e1`）：版本身份源仅两处点位：`compiler/Cargo.toml` L3 与 `compiler/Cargo.lock` L7，两处字面量由 `0.1.16` 直锚 `0.1.17`，字符级差异恰 2 处；`compiler/Cargo.lock` L3 的 `version = 4` 系清单格式行、非版本身份源、零触碰。单一版本源不变：`zorcc --version`／`-V` 出口串本席现测为 `zorcc 0.1.17`、rc=0。判据句 C-1 要求的**三处同号**＝`Cargo.toml:3` ＋ 命令行出口串 ＋ **本账本 `[0.1.17]` 节标题**：前两处由环 1 落地并经判据位 PASS 票，第三处在本节之前物理上不存在，故验证时点按已裁口径后移至本节合入之后——**判据本身一字未动，只移验证时点**（裁口＝项目管理席本窗判定；同形态先例＝0.1.13 达批窗的「双实体三处同号同日同锚」终核，本来就在标签与发布页实体落地之后才做）。**本节合入即本版第三点位补齐**。全库带引号的点位版本字面量恰此 2 枚，其余 `0.1.17` 串属记述用途、非第二身份源。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.16→0.1.17 顺序直锚，无欠账叙事（第十四次例行口径，序数依据=0.1.16 在册「第十三次例行口径」为唯一在档前证，无仓库面第二证、不另造）。
- 实体面零操作如实记：本版无标签实体、无发布页、无下载物（0.5.x Preview 前不承诺可安装、不提供下载物）。本席 @main 现取：标签面维持恰 **12 行**（口径=`git ls-remote --tags origin` 行数＝6 枚带注标签各 2 行）、本版新增 **0 枚**、既有标签面全程未触碰、最新仍 `0.1.13`；发布页维持 **5 条**、最新仍 `0.1.13`。本窗达批节点未触发（上一攒批节点已于 0.1.13 结算，计数自该节点后重新起算）。**本版收口值以本节合入后的 main 为准**：`9ad2f72` 系环 1／环 2 的合入点，非本版 SHA_FINAL（依据=在册判据「末位 PR 载荷＝本版 SHA_FINAL」+ D-043 链锚）。
- 判据位票面齐备（治理事实随实入册，非本版产物）：环 1＝`ZOR-0.1.17-P1-判据位票面-v1.md`（5600 B／sha256 `ebd00d8b13360d33…8aae`，结论 PASS）；环 2＝`ZOR-0.1.17-P2-判据位票面-v1.md`（22400 B／sha256 `aca2a13a66708109…e505`，结论 PASS，C-4 四项否证条件全未触发）。两件均在 DOC-3 群共享权威坐标 `conv_01m4nmakww4e28bee4bx801pdb/shared/`（本席 @该坐标现取复算，算式=整件 `sha256sum`，两串与帖面呈值全等）；票首所绑 head 整值与合入前复核值逐字同串（`14380e88d56d7b89…`／`0b241553b8d9bd66…`），未发生 force-push 或追加 commit，票未失效。本席记此条为满足「账本面结论须可追溯到判据位实跑」，票面本身不进本账本正文。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：`-o` 的**真实产物落点**（本版只做了显式拒判，出参依赖代码生成，归 0.2.x CLI 面契约件 G-D-2）；`-Zoverflow=check/wrap` 的运行期溢出行为与 T-5 行为用例、ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；AC-R2-4 `compare/` 槽位（M2 排期）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结；CLI 面其余能力（多文件输入、`--emit` 类输出控制、`--help`、JSON 诊断面）本版不接、归 0.2.x。
- `--version`／`-V` 短路的优先级高于拒判面：**已由「实测如此」升为「册上如此」，成文已入册**。归因按载体现文＝请示＝项目管理席（seq103 §四-1）、裁定与作者＝产品经理、候选句与实测锚＝判据位（seq101 §四-1 O-1 ＋ seq113 附节）；成文载体＝DOC-3 群共享权威坐标 `conv_01m4nmakww4e28bee4bx801pdb/shared/ZOR-0.1.17-范围冻结-PM裁定-v1.md` §三 **C-1b（L77）＋防腐边界（L78）**，最终契约归口＝同目录 `ZOR-PRD-§3判据改文与0.2x移交界面对账-v1.2.md` **G-D-2（L71）**。两件均**权威件、非私有副本**，行数／字节／整件 sha256／mtime 一律取本席合入前最后一次现值（冻结件＝mtime `2026-10-15T19:46:23+0800`／116 行／12854 B／`88a2cb162674fbdff25901ad214e221b…`；PRD 追加件＝mtime `2026-10-15T19:36:49+0800`／92 行／9381 B／`a700da63f2877bbb88df744b488937b7…`；**转述坐标永不算在册事实**，他席所呈同值亦不作为本席断言之据）。在册语义定性＝短路先于文件处理与拒判面，属 **0.1.2 起在册语义**（`main.rs` 单测锚定），出口仅 stdout 版本串、零产物、不含"已编译"暗示，**故不属假绿族**；**防腐边界同段在册**＝短路优先级不得被用作绕过拒判的探测手段，一旦出现以 `--version` 组合探 CLI 面的脚本用法即归 G-D-2 定形、**豁免不永久化**。本席 @main `9ad2f72` 现测四形（`--version -o <path>`／`--version <good> <bad>`／`<good> --version`，另加 `-V -o <path>` 变体）＝**rc=0、stdout 恰 `zorcc 0.1.17`（13 B）、stderr 0 B、目标路径无产物**，与 C-1b 实测锚逐值同串。
- 册实项维持 0.1.27 勘正批在册、**本窗零触碰**（本席只登记、不代改；共三处异源）：① `compiler/README.md:11`（仓库在册件）仍写「未实现（后续版本）：R2 借用检查（E2xxx）」，与在册实态矛盾（E2101／E2102／E2201／E2202／E2301 自 0.1.1 起已实装并跑在门禁双模式下；本席 @main 现读确认，管理席亦已数次现读）。② **H-a 面**＝手册权威件 `conv_01m4nmakww4e28bee4bx801pdb/shared/ZOR-0.1x-团队手册-可读副本-v1.md`（**DOC-3 权威件、非各席私有副本**；本席现读＝63 行／8081 B／整件 sha256 `a5391076cf72e3b4…`）**L50**＝「由项目管理双条件复核并执行 **squash** 合入」句，与 main 实态矛盾：本席 @main `9ad2f72` 现取 `git log --first-parent` **前 20 条 20/20 全为 `Merge pull request` 形态**、PR#58–#77 连续无断。③ **H-b 面＝同件四处改文面**：**L10**（SOP 绑定号行现写 1.1.0，而群绑定 SOP pin 现读 `zor-team-lifecycle@1.2.0`）＋ **L4／L5／L63**（登记面三处同款断言＝底稿 pin `1.0.0`／「项目管理 2026-10-12 经平台通道登记 skill 包 1.0.0→1.1.0」过去时断言／表列「已登记为 skill 1.1.0」），与群 skill pin 现读 `zor-team-handbook@1.0.0`（digest `35aabc7f…84e7`）不一致；究竟「登记未落」还是「pin 未随登」本席判**核验不可达**（无按群只读入口），不得滑写成"确认未登记"，随 0.1.27 报平台侧。**出处自纠留痕**：本节首稿在②③两处误引私有克隆 `/Users/zt_mac/zt_hd/projects/ZOR/zor-team-handbook-1.1.0.md`（68 行／8520 B／整件 sha256 `1eae13355ac6fd6f…`，带 YAML 前导、行号恒差 +5，其 L55／L15 即权威件 L50／L10）——账本面是永久正账，**私有区副本不作判据依据**，已改标权威件全路径＋行号＋载体性质。三条均按 SOP 1.2.0 §5「册实一致」登记为待勘正项、非本版行动项；本席不在环 3 顺手改任何非账本载体（本窗载荷唯一=`CHANGELOG.md` 一件）。

## [0.1.16] - 2026-10-15

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十六轮（序数依据=本账本在册各节首段连续计数，0.1.14 第十四轮与 0.1.15 第十五轮为在档前证，无仓库面第二证、不另造）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版=三个 PR 载荷（环 1 版本身份锚／环 2 质量资产面同源单测补齐／环 3 本账本条目），环数=载荷数=3。本版主题=固定载荷位例行续执行·版本身份锚（环 1）+ 上一版定成验收标准的四种数组写法补上编译器自带自检（同源单测一枚＋段标续号，环 2）+ 本账本条目（环 3）。

### Added
- 质量资产面同源单测补齐（0.1.16 环 2，PR#73）：上一版把那四种写法定成了可复跑的验收标准；这一版给它们再加了一道机器自检——编译器自带的测试现在会逐条核对这四种写法是不是都还在同一句说明书原文下被拦下，四种里少拦一种、自检当场就失败（只是补自检，没有新增拦截规则、没有新文案，也不开放任何新的语言能力承诺）。命令行版本号按"每版必对齐"固定规则例行锚定（0.1.15→0.1.16）；变更日志新条目。承载件唯一=`compiler/tests/r2_diag.rs`，件内改动恰两处：①在 `neg_r2_n20_array_lit_ref_element`（@base 件内 L188–L192）之后、判定表补充边界段标之前新增恰一枚 `#[test] fn neg_r2_n21_array_lit_ref_element_forms()`（@head 件内 L194–L199、函数名行 L195）；②段标行（现件 L87）改写为两立措辞——逐字源=本版范围冻结裁定件 §五 S2（件内 L174），本席由件内行程序直读入笔、未自加语义。**同源**定义（沿用 `neg_r2_n20` 在册实态）＝单测内联源码串与 `tests/specs/negative/r2-n21-array-lit-ref-element-forms.zor` 自 `fn main()` 起至件尾**逐字节同文**（该取段本席实测 143 B，sha256 值随交付帖呈、不入账本面）。**断言形态**＝`codes(src)` 全序恰等 `["E1501","E1501","E1501","E1501","E1201"]`，逐值对平在册 golden `tests/specs/golden/r2-n21-array-lit-ref-element-forms.txt` 的五行（E1501 四行坐标 8:13／9:14／10:13／11:13 ＋ E1201 一行 11:18）；不只断首诊、不排序后比对、不以 `contains` 代替全序（三禁字面在新枚块内命中各 0）；`codes` helper 沿用件内现存 L5 定义、未重名自定义。实测锚：`fn neg_r2_n*` **20→21**（01–21 连号、缺号 0）、件内 `#[test]` **36→37**、同源单测计数 **65→66**（口径=`cargo test` 各 target `test result: ok.` 之 passed 之和，rc=0、failed 0，本席 @head 自建全新 target 实跑分段=0/1/28/37/0，新枚 `neg_r2_n21_array_lit_ref_element_forms ... ok` 在列）。`tests/specs/**` **零改动**（逐 blob added 0／removed 0／modified 0）：单测与输出基线**并存**、非替代关系。规格面零动为按实结论：本窗未新增在册输出基线承载件、`#8` 在册文案「数组元素不得为引用类型」在 golden 面维持 **2 件／5 次**、同一取点 E1501 码位维持 **15 次** → 裁定件 §五 S3 的唯一触发判＝**不触发**，S1-r 随版续列条款本版不启动、规格面零动非省略。全库计数（本席 @head 现算，口径=目录清单枚数剔 `.gitkeep`）：正例 **35** / 负例 **38** / golden **38**——对 0.1.15 在册值 35/38/38 为 **+0/+0/+0**，测试资产面维持 **73 组用例 + 38 份输出基线**，负例数=golden 数不变式维持；`r2-n*` 目录面维持 **21 枚连续 01–21**（取径口径=`git ls-tree -r` 后按 `^r2-n` 前缀去重计数）；历史 golden 零字节漂移维持（本席对 0.1.15 尾点逐文件比对：新增 0 件、修改 0 件、删除 0 件）。

### Fixed
- 版本身份锚：版本身份源仅两处点位：`compiler/Cargo.toml` L3 与 `compiler/Cargo.lock` L7，两处字面量由 `0.1.15` 直锚 `0.1.16`，字符级差异恰 2 处；`compiler/Cargo.lock` L3 的 `version = 4` 系清单格式行、非版本身份源、零触碰。单一版本源不变：`zorcc --version`／`-V` 与 debug／release 双模式四路出口串实测均为 `zorcc 0.1.16`（执行席自建双模式二进制实跑留数，不以读码代替）。全库带引号的点位版本字面量恰此 2 枚，其余 `0.1.16` 串属规格注记与用例注释的记述用途、非第二身份源。本窗达批节点未触发，实体面零操作。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.15→0.1.16 顺序直锚，无欠账叙事（PR#72，0.1.16 环 1；第十三次例行口径，序数依据=0.1.15 在册「第十二次例行口径」为唯一在档前证，无仓库面第二证、不另造）。
- 实体面零操作如实记：本版无标签实体、无发布页、无下载物（0.5.x Preview 前不承诺可安装、不提供下载物）；标签面维持恰 12 行、本版新增 0 枚，既有标签面全程未触碰；发布页维持 5 条、最新仍 `0.1.13`；本窗达批节点未触发（上一攒批节点已于 0.1.13 结算，计数自该节点后重新起算）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.15] - 2026-10-15

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十五轮（序数依据=本账本在册各节首段连续计数，0.1.13 第十三轮与 0.1.14 第十四轮为在档前证，无仓库面第二证、不另造）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版=四个 PR 载荷（P1 版本身份锚／P2 附带实发面入册／P3 规格面随实同步／P4 本账本条目），环数=载荷数=4。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ 数组字面量元素持引用的四种推导写法首次同件钉定进验收基线（先实测后入册，P2）+ 规格面实发数与归属注记随实同步、§9 负例范围行至 r2-n21（P3）。

### Added
- 附带实发面入册（0.1.15 P2，PR#69）：说明书早已写明数组元素不得是引用类型，上一版让这一条对推导写法真的生效；本版把这批推导写法——单元素、嵌套、可变引用元素、与值混合四种——逐一站上了可复跑的验收基线：此前只有人工实测记录，现在每一形的拦截结果都进入机器比对的输出基线，改动一处写法就会被验收当场拦下。本批未新增检查器、未新增错误码、未新写提示文案，也未开放任何语言能力的对外承诺（本版本仍为内部验证里程碑，非可用语言版本）；非首位元素的引用仍由既有的元素一致性判定拦下。实体证据=先实测后入册，三方同数标准验收门本轮运行于拦下侧并全程成立（执行者实跑留数、本席以同一输入件做第三方复跑、判据位独立复跑、核收位第四向自证——四席逐形数一致后才钉定，本席与执行位两向另达 stderr 全串文件字节级同值；缺口登记分支未触发）。用例入册：`tests/specs/negative/r2-n21-array-lit-ref-element-forms.zor`（四形同件钉：`[&x]`／`[[&x]]`／`[&mut x, &mut y]`／`[&x, x]`）＋ golden 一份同批入册（`tests/specs/golden/r2-n21-array-lit-ref-element-forms.txt`，内容=实发诊断五行逐字：E1501 四行坐标 8:13／9:14／10:13／11:13 ＋ E1201 一行 11:18；debug/release 双模式均编译期失败、退出码非零、两模式 stderr 逐行同值）。与值混合形 `[&x, x]` 的非首位元素一句按实：该形第二元素不发 E1501，由既有的元素一致性判定路径发 E1201 拦下（两态均非零退出、禁则不破、无静默逃逸）。本批未新增检查器、未新增错误码、未新写提示文案：错误文案逐字沿用在册串（错误文案基线=门禁 T-4）。历史 golden 零字节漂移维持（本席复算口径：`tests/specs/golden` 面 37 份→38 份，相对 0.1.14 尾点新增 1 份、修改 0 份、漂移 0 份；`tests/specs/` 面逐文件差异=新增 2 件（一负例＋一 golden）、修改 0 件、删除 0 件）。全库计数（主干实测复算，正例按目录独立计数面口径注沿用）：正例 **35** / 负例 **38** / golden **38**——对 0.1.14 在册值 35/37/37 为 **+0/+1/+1**（本件一枚负例对五行诊断入一份 golden），正例面零扩，负例数=golden 数不变式维持；`r2-n*` 目录面 **21 枚连续 01–21**（取径口径=`ls tests/specs/negative` 后按 `^r2-n` 前缀计数）。
- `specs/r1-type-system.md`：规格面随实同步（0.1.15 P3，PR#70）——E1501 释义表表后注区在上一版注记行之后**新增四行**，逐字源=本版范围冻结裁定件件内行、本席未自加语义：①实发数二维注记（在册输出基线内「数组元素不得为引用类型」文案命中按版本并列三点＝0.1.13 及以前 0 件／0 次、0.1.14 1 件／1 次、0.1.15 2 件／5 次；同一取点按 E1501 码位口径＝10 次→11 次→15 次，两口径并列记述、不混用）；②承接注记（上一窗「本窗不另增用例」句系 0.1.14 窗内事实，按台账行不回改通则原样留存，自本版起该四形已有在册承载件）；③归属维度注记（「纯标注无初值位仍只发 #4」中的「只」限 E1501 码位维度——无初值的标注绑定实测另发既有 E1601 在前、本码在后，函数参数标注位实测仅 1 行、无伴发）；④可达性注记（内联双可变引用元素形态实测 E2202 不发、由 E1501 一行兜住且退出码非零，属诊断归属残差而非静默逃逸；是否扩至逐元素判定与是否补发该码另窗勘定）。四行全为记述：本面未新增检查器、未新增／注销错误码，§8 码表数值、L91／L92 单元格与 §7 覆盖勘定注记表数值零触碰（本席 @合入体复算＝以 `|` 起始的码表行 10 条逐条同值）。
- `specs/r2-ownership.md` §9：负例范围行末位范围 token 至 `r2-n01…r2-n21`（同件 PR#70；该行唯一改动=末位范围 token 一处、字符级差异恰 1 处实证；AC-R2-2 下限句、F-1/F-1b 修订注与 n17/n18/n19 三段子描述逐字节原样，行内另两处历史注不变；n21 段描述按核收判定未自加，本窗不另立第二处改动）。Ex4/KL1 行原因原文不动、维持留册（触发条件=R3/R4 规则成文与运行链路落地；本版无相应动作，非本版行动项、不构成缺口叙事）。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.14` → `0.1.15`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 与 `-V` 输出现与本版账本节标题同为 0.1.15 字面量——本席 @现 main 以自建双模式二进制实测四路出口串均为 `zorcc 0.1.15`；版本身份源仅 `Cargo.toml`／`Cargo.lock` 两处，其余 0.1.15 字面量属规格注记与用例注释的记述用途、非第二身份源）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.14→0.1.15 顺序直锚，无欠账叙事（PR#68，0.1.15 P1；第十二次例行口径，序数依据=0.1.14 在册「第十一次」为唯一在档前证，无仓库面第二证、不另造）。
- 实体面零操作如实记：本版无标签实体、无发布页、无下载物（0.5.x Preview 前不承诺可安装、不提供下载物）；标签面维持恰 12 行、本版新增 0 枚，既有标签面全程未触碰；本窗达批节点未触发（上一攒批节点已于 0.1.13 结算，计数自该节点后重新起算）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.14] - 2026-10-14

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十四轮（序数依据=本账本在册各节首段连续计数，0.1.12 第十二轮与 0.1.13 第十三轮为在档前证，无仓库面第二证、不另造）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版=四个 PR 载荷（P1 版本身份锚／P2 缺口面收口／P3 规格面随实同步／P4 本账本条目），环数=载荷数=4。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ 0.1.13 所登记实体缺口面 A 收口：数组字面量推导形态持引用首次获得实发点（三方同数标准验收门先实测后入册、本版该门运行于拦下侧并全程成立，P2）+ 规格面可达性注记随实同步与 §9 负例范围行至 r2-n20（P3）。

### Added
- 缺口面 A 收口（0.1.14 P2，PR#65）：说明书早已写明数组元素不得是引用类型，本版让这一条对推导写法也真的生效：此前会静默放过的 `[&x, &y]` 现在编译期被拦下并给出说明书原句。实体证据=先实测后入册，三方同数标准验收门本轮运行于拦下侧并全程成立（执行者实跑留数、判据位与核收位各自独立复跑、裁决位同批复核，逐形数四席一致后才钉定，缺口登记分支未触发）。本批不新增检查器、不新增码位、不新写文案：改动面=既有数组元素判定的一处可达性补齐，错误文案逐字复用 R1 §3 释义表在册串（E1501 同号；该串自 0.1.13 及以前即已在册，而本版前该码实发数=0；错误文案基线=门禁 T-4）。覆盖面句与本窗 `specs/r1-type-system.md` 可达性注记同句同源、不另立第二版本：现本码实发面＝数组字面量元素求解这一路的三类形态（推导字面量／带字面量初值的标注 let／嵌套字面量），纯标注无初值位仍只发 #4。非首元素持引用的数组字面量（`let a = [x, &y];`）不发本码，由既有的元素一致性判定路径发 E1201 拦下（两态均非零退出、禁则不破、无静默逃逸）。用例入册：`tests/specs/negative/r2-n20-array-lit-ref-element.zor`（推导绑定形态，debug/release 双模式均编译期失败、退出码非零）＋ golden 一份同批入册（`tests/specs/golden/r2-n20-array-lit-ref-element.txt`，内容=实发诊断一行逐字，坐标 `:8:13`）＋ 同源单测一枚追加在册宿主（`compiler/tests/r2_diag.rs` 段标 R2 负例 19→20 条；全量跑全绿——本席于本版基线复算 cargo test 合计 65 通过 0 失败）。历史 golden 零字节漂移维持（本席复算：本版 `tests/specs/` 面相对 0.1.13 尾点逐文件差异=新增 2 件（一负例＋一 golden）、修改 0 件、删除 0 件）。全库计数（主干实测复算，正例按目录独立计数面口径注沿用）：正例 35 / 负例 37 / golden 37——对 0.1.13 在册值 35/36/36 为 +0/+1/+1（本件一枚负例对一行诊断入一份 golden），正例面零扩（不作用例扩充表述），负例数=golden 数不变式维持；`r2-n*` 目录面 20 枚。
- `specs/r1-type-system.md`：规格面可达性注记随实同步（0.1.14 P3，PR#66）——E1501 释义表表后注区新增「可达性注记」一行，按本版实际合入实态记 #8 自 0.1.14 起首次获得实发点（0.1.13 及以前本码实发数=0：源点实存但结构不可达＝0.1.13 所登记实体缺口面 A 的原态；容器位与标注位由 #4「引用类型不得出现在容器位置（0.1.x 禁逃逸）」拦下，数组字面量的推导形态静默通过），覆盖面与本码文案同上句同源口径。本注记不新增/不注销任何错误码，§8 码表数值、本表各单元格文字与 §7 覆盖勘定注记表零触碰（In2 行与统计行「吻合 11 / 部分吻合 0 / 未落钉 2」维持 0.1.11 在档值，本席于本版基线核对）。本注记首落句曾把静态读码的出口可达性误写为实发历史，经判据位与裁决位双席同批复跑实测报差、同日就地追加第三枚提交勘正（不改写历史），勘正前后净改动面同值 +3/−1；本窗据此固化一条判据：凡历史句类表述须先 @base 与 @上一版尾点侧同批复跑对平后才落笔，@head 单侧测不出历史错位。
- `specs/r2-ownership.md` §9：负例范围行末位范围 token 至 `r2-n01…r2-n20`（同件 PR#66；该行唯一改动=末位范围 token 一处、字符级差异仅 2 位实证；AC-R2-2 下限句、F-1/F-1b 修订注与 n17/n18/n19 三段子描述逐字节原样，行内另两处历史注不变；n20 段描述按核收判定未自加，本窗不另立第二处改动）。Ex4/KL1 行原因原文不动、维持留册（触发条件=R3/R4 规则成文与运行链路落地；本版无相应动作，非本版行动项、不构成缺口叙事，亦不作后续版本预告）。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.13` → `0.1.14`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 与 `-V` 输出现与本版账本节标题同为 0.1.14 字面量——本席于本版基线双模式实测出口串均为 `zorcc 0.1.14`；全库字面量检索现态如实记：版本身份源仅 `Cargo.toml`／`Cargo.lock` 两处，另两处 0.1.14 字面量属规格注记与用例注释的记述用途、非第二身份源）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.13→0.1.14 顺序直锚，无欠账叙事（PR#64，0.1.14 P1；第十一次例行口径，序数依据=0.1.13 在册「第十次」为唯一在档前证，无仓库面第二证、不另造）。
- 实体面零操作如实记：本版无标签实体、无发布页、无下载物（0.5.x Preview 前不承诺可安装、不提供下载物）；标签面维持恰 12 行、本版新增 0 枚，既有标签面全程未触碰；本窗达批节点未触发（上一攒批节点已于 0.1.13 结算，计数自该节点后重新起算）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。


## [0.1.13] - 2026-10-14

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十三轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版执行终态=两个 PR 载荷（首环版本身份锚+末位本账本条目）：范围冻结时点预期四环，因 P2 容器路径深层形态验证实跑落于缺口侧、钉定批分支未触发，P3 经裁定明判不成立（零动作、不派）——差项按实注记、不补空环、不改写开工时点预期口径。

### Added
- 容器路径深层形态验证批（缺口侧如实记述）：说明书承诺与机器实际行为存在一处已登记不符——引用入数组的推导写法暂未拦下，已入已知缺口台账，修复面列下一版本窗首项候补。实体证据=先实测后入册全程在档：三方独立实跑（执行者留数、两路独立复跑）加裁定侧第四次复核，文件字节与行为数四方对平、零反差——数组字面量推导写法（`let a = [&x, &y];`，被推导元素为引用）实跑 debug/release 双模式均静默通过（退出码零、零诊断）；在册「数组元素不得为引用类型」与「引用类型不得出现在容器位置」（无后缀兜底形态）两条实发文案于当前语法可达面内均不可发，容器标注面既有拦截全部落在早前版本已配有实跑对照基线的 parser 容器位形态（带「（0.1.x 禁逃逸）」后缀）。本批不新增检查器、不改行为、不补测试面、不猜改、不改编译器：终验批未触发、用例册与基线计数维持正 35 / 负 36 / golden 36（不变式 负例数=golden 数）；缺口台账承载于群共享登记文书（非仓库册面），修复面=下一版本窗首项在册候补并挂时限裁定标记（达 D2 前不得处于「已知未裁定」状态）——本句为记述口径，不引用台账内部编号入正文。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.12` → `0.1.13`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 与 `-V` 输出现与本版账本节标题同为 0.1.13 字面量——同号闭环位以本节合入为锚，改动面无第二处版本字面量经全库锚定检索实证）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.12→0.1.13 顺序直锚，无欠账叙事（PR#62，0.1.13 P1；第十次例行口径，依据=本版范围冻结裁定卷面与开工派单在档，序数无仓库面第二证、不另造）。
- 规格面零动作如实记（0.1.13 P3=不成立、零派，经裁定明判）：缺口分支零新增下 §9 负例范围行现文与目录实态逐字节相符、无动册点，范围行不动即如实；§7 勘定表零动册（统计面 吻合 11 / 部分吻合 0 / 未落钉 2 维持在册值）、§8 码表与 E1501 释义表零触碰——滞旧表述面归属登记侧，释义表可达性注记与否归下一版本窗随行为面一并裁定，达批窗不动册以防扩面。本条为事实记述，改动面=零（无册面变更）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。


## [0.1.12] - 2026-10-14

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十二轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ 引用入 Box 载荷内子位非法写法首次配有实跑对照基线（同件双行钉定、先实测后入册，P2）+ 规格文字同步件a 单件（r2 §9 范围行至 r2-n19，§7 勘定表零动册，P3）。

### Added
- `tests/specs/negative/`：引用入 Box 载荷内子位非法写法首次配有实跑对照基线（0.1.12 P2，PR#59）——`r2-n19-ref-in-box-payload` 同件双行钉：`fn f(b: Box<&i64>) { }` 与 `fn g(b: Box<&mut i64>) { }` 两子形态入同一用例文件（0.1.11 r2-n17/n18 同件钉先例同构）。本批不新增检查器、不改行为：`Box<&T>`/`Box<&mut T>` 为 R1 §3 容器禁则的既有检查器命中形态，钉定用例口径、不构成能力开放表述。判据=三方同数实跑门在档（按号引用：核心 seq1676 先行留数/文档 seq1680/QA seq1682 各自独立复跑；& 形探针文件字节 sha256 与 0.1.11 在档 p3 三方定案值跨窗全等=base 复跑互证零漂移，&mut 形为本窗新采、两方独立重建全等）：三路×两子形态×双模式=12 跑全拦下零反差（debug/release 均编译期失败、退出码非零，E1501 #4 容器位文案、坐标 1:13），先实测后入册、非纸面推定，缺口登记分支未触发。golden 一份同批入册（`tests/specs/golden/r2-n19-ref-in-box-payload.txt`，内容=实发诊断两行逐字 `:5:13`/`:6:13`，坐标随新文件名+头部注释行布局重建，错误文案基线=门禁 T-4）；同源单测一枚追加在册宿主（`compiler/tests/r2_diag.rs` `neg_r2_n19_ref_in_box_payload`，全量跑全绿，段标 R2 负例 18→19 条）。历史 35 份 golden 零字节漂移维持（36 比对全一致含存量）。全库计数（主干实测复算，正例按目录独立计数面口径注沿用）：正例 35 / 负例 36 / golden 36——基线 35/35/35 +1 负 +1 golden（本件同件双行钉=一枚负例对两份诊断行入一份 golden），正例面零扩，负例数=golden 数不变式维持。
- `specs/r2-ownership.md` §9：规格文字同步件a 单件（0.1.12 P3，PR#60；本版无件b）——§9 负例范围行至 `r2-n01…r2-n19`（仅末位范围 token 改动；AC-R2-2 下限句、F-1/F-1b 修订注与原注逐字节原样、零虚构扩展）+ n19 逐条描述补句（引用入 Box 载荷内子位：`Box<&T>`/`Box<&mut T>` 两子形态同件双行钉、golden 实发 E1501 #4 容器位文案两行口径，措辞对目录实态逐字节核对）+ 钉定史注（三方同数实跑门按 seq1676/1680/1682 号引用，码表面零触碰；0.1.11「后两条」原注逐字保留、n19 归属单列不混记）。改动面=单行三点一面（+1/−1，全文件仅 L98 变化实证）。§7 覆盖勘定注记表零动册：Box 内子位不在 In2 条目标题名域，In2 行与统计行「吻合 11 / 部分吻合 0 / 未落钉 2」维持 0.1.11 在档值=不变式核验点；§8 码表、E1501 释义表、清单条目语义与禁令/承诺文字全零触碰。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.11` → `0.1.12`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 与 `-V` 输出现与本版账本节标题同为 0.1.12 字面量，改动面无第二处版本字面量经全库锚定检索实证；第九次例行口径依据=0.1.12 P1 票面在档「单一版本源惯例……现为第九次例行沿用」与派单明判，群内派发链为序数唯一在档依据）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.11→0.1.12 顺序直锚，无欠账叙事（PR#58，0.1.12 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。


## [0.1.11] - 2026-10-14

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十一轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ In2 组合面引用入容器两类非法写法首次配有实跑对照基线用例（两枚同批钉定、先实测后入册，P2）+ 规格文字同步（r2 §9 范围行至 r2-n18 / §7 In2 行与统计行按实际合入集，同票单 PR，P3）。

### Added
- `tests/specs/negative/`：In2 组合面（引用入容器）两类非法写法首次配有实跑对照基线用例，两枚同批钉定、零前移（0.1.11 P2，PR#55）——`r2-n17-ref-in-enum-variant`（引用入 enum 变体载荷：`enum Wrap { Hold(&i64) }` 与 `&mut` 子形态 `enum WrapM { HoldM(&mut i64) }` 同件钉）、`r2-n18-ref-in-array-element`（引用入数组元素标注位：`[&T; N]` 即 `fn take(a: [&i64; 2])` 与 `[&mut T; N]` 子形态同件钉）。二者均为 R1 §3 容器禁则的既有检查器命中形态——码表面零触碰，本批为钉定用例口径、不构成能力变更或新检查器上线表述。判据=三方同数实跑门在档（执行者核心工程师探针留数、QA 与文档各自独立复跑、对齐与确认闭环；探针 p1a/p1b/p2a/p2b 哈希三方同值）：实跑两枚 debug/release 双模式均编译期失败、退出码非零，诊断=E1501 容器位文案；先实测后入册，非纸面推定。golden 两份同批入册（`tests/specs/golden/r2-n17-ref-in-enum-variant.txt` / `r2-n18-ref-in-array-element.txt`，内容为实发 E1501 文案逐字，错误文案基线=门禁 T-4）；同源单测两枚同批入册（`compiler/tests/r2_diag.rs` `neg_r2_n17_ref_in_enum_variant` / `neg_r2_n18_ref_in_array_element`，全量跑全绿）。历史 33 份 golden 零字节漂移维持。全库计数（主干实测复算，正例按目录独立计数面口径注沿用）：正例 35 / 负例 35 / golden 35——基线 35/33/33 +2 负 +2 golden（本批），正例面零扩，负例数=golden 数不变式维持。
- `specs/r2-ownership.md` §9 与 §7：规格文字同步（0.1.11 P3，PR#56，同票单 PR）——件a：§9 负例范围行至 `r2-n01…r2-n18`（仅末位范围 token 改动；AC 下限句与原修订注原样保留、零虚构扩展）并补两条用例描述（r2-n17/r2-n18 各一条，含 `&`/`&mut` 两子形态同件钉与 golden 实发 E1501 容器位文案口径），钉定史按在档链记「0.1.11 P2 先实测后入册（三方同数实跑门在档；码表面零触碰）」；件b：§7 覆盖勘定注记 In2 行状态改写为吻合（三面全钉：struct 面 0.1.5/0.1.6 在册史 + enum 变体载荷/数组元素标注位 0.1.11 已钉定，用例号 r2-n17/n18），统计行同步为吻合 11 / 部分吻合 0 / 未落钉 2（13 条目全覆盖）。表述为钉定用例口径、不构成能力开放句（0.1.9 P3 判例承接）；Ex4/KL1 行原因原文不动，§8 码表、清单条目语义与禁令/承诺文字全零触碰。本版缺口面=零：P2 批为拦下形态、按钉定分支执行，In2 面已闭合，无缺口登记项；余下未落钉候选（Ex4/KL1）维持 0.1.9 在册候补口径、非本版行动项，不构成缺口叙事。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.10` → `0.1.11`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 与 `-V` 输出现与本版账本节标题同为 0.1.11 字面量，改动面无第二处版本字面量经 QA 全库检索实证）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.10→0.1.11 顺序直锚，无欠账叙事（PR#54，0.1.11 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。


## [0.1.10] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第十轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ 0.1.9 覆盖勘定组合面候补三正例首批准正（件级全钉零前移，P2）+ 规格文字同步两件（r1 §9 范围行对目录实态 / r2 §7 勘定注记按实际合入集，同票单 PR，P3）。

### Added
- `tests/specs/positive/`：0.1.9 覆盖勘定注记在册候补三枚同批转正钉定、零前移（0.1.10 P2，PR#51）——`r2-10-enum-box-self-cycle-indirection`（enum 变体载荷经 `Box` 间址的自环形态：递归环仍在声明层由 E1401 断侧，`Cons(Box<List>)` 形态合法通过）、`r2-11-enum-box-mutual-indirection`（两 enum 经 `Box` 间址互递形态合法通过）、`r2-12-box-return-expr-tail`（函数返回新分配 `Box<T>` 的表达式尾形态：`fn make() -> Box<i64> { box 42 }`，本语言无 `return` 关键字）。判据=执行者、核心工程师、QA 三方会签实跑结论在票面在档：双模式 exit=0 零诊断；同源单测三枚同批入册（`compiler/tests/r2_diag.rs` 全量跑全绿）。golden 面不扩（三枚均为通过型正例，无 golden 义务），历史 33 份零字节漂移维持。全库计数（主干实测复算，正例按目录独立计数面口径注沿用）：正例 35 / 负例 33 / golden 33——基线 32/33/33 +3 正（本批），负例/golden 面零扩，负例数=golden 数不变式维持。
- `specs/r1-type-system.md` §9 与 `specs/r2-ownership.md` §7：规格文字同步两件（0.1.10 P3，PR#52，同票单 PR）——件a：§9 正例/负例两范围行对目录实态刷新（正例范围至 `r1-01…r1-22`、负例范围至 `r1-n01…r1-n16`，各自仅末位范围 token 改动；描述文字、F-1c 修订注与「每条对应 §8 一个错误码」句原样保留、零虚构扩展；§9 标题行 AC 下限句零触碰），0.1.9 勘定尾注所记同型漂移（负例范围行滞于目录实态）随本版出清。件b：§7 覆盖勘定注记表按 P2 实际合入集同步——Ex1（enum×Box 组合面）与 Ex5（返回新分配 `Box<T>`）两状态行改写为「0.1.10 已钉定，用例号 r2-10/11/12」口径，统计行同步为吻合 10/部分吻合 1/未落钉 2（13 条目全覆盖）；表述为钉定用例口径、不构成能力开放句（0.1.9 P3 判例承接）；In2/Ex4/KL1 行、§8 码表、清单条目语义与禁令/承诺文字全零触碰。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.9` → `0.1.10`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 输出现与本版账本节标题同为 0.1.10 字面量，改动面无第二处版本字面量经 QA 全库检索实证）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.9→0.1.10 顺序直锚，无欠账叙事（PR#50，0.1.10 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.9] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第九轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=固定载荷位例行续执行·版本身份锚（P1）+ R2 可表达清单覆盖勘定三件首次成批入册（件a 索引对位/件b 归口修正/件c 勘定注记表，同票单 PR；P2）。

### Added
- `specs/r2-ownership.md` §7：覆盖勘定注记表入册（0.1.9 P2 件c，PR#48）——本节全部 13 条目（可表达 5+不可表达 7+已知限制 1）逐条按「条目↔§8 诊断码↔§9 用例映射↔`tests/specs/` 目录实存」四向核对，结论统计=吻合 8/部分吻合 2/未落钉 3。裁定两处点名项如实记「未落钉+原因+触发条件」：文档级正例 `r2-leak-note` 目录无文件（运行链路未落地、无基准/ASAN 报告面可挂；M1 起运行链路落地后呈现，不预排）；§9 正例行「管道式所有权交接（R3 交叉预备）」目录无对应拼写（R3 规则未成文，成文线后随正例钉定）。勘定另按同一口径如实记两处：可表达面「返回新分配 `Box<T>`」无专项钉定正例（表达式尾形态实跑零诊断，探针不入仓库；0.1.10 候补随正例钉定）；enum 变体载荷经 `Box` 间址的组合面（自环与两 enum 互递形态）实跑零诊断放行、无钉定正例（0.1.10 候补，钉定随会签与 QA 实跑数同批）。注记表为纯新增子节，本件零改动任何清单条目语义与测试面。
- 勘定顺带发现的同型漂移按「只记证据、零扩本件载荷」入注记尾注，本版不改相关行字节：`specs/r1-type-system.md` §9 负例范围行文句「r1-n01…r1-n11」滞于目录实态（`negative/` 目录 r1 列实至 r1-n16）；不可表达条目 2（引用入容器类型）的 enum 变体/数组元素两个容器面禁则文字在册（R1 §3 引用类型行）而目录无专项负例——两处均 0.1.10 候补随条目勘定。

### Fixed
- `specs/r2-ownership.md` §2：判定表活跃性行指向不可表达清单的索引两端对位修正（0.1.9 P2 件a，PR#48）——「§7 第 8 条」单 token 修正为「第 7 条」：§7 不可表达实列条目 1–7、块尾失活即第 7 条，§9 范围行「清单第 7 条」本已在册，本次为 §2 侧滞后补齐，三处今同号。
- `specs/r1-type-system.md` §3：enum 递归禁止句尾分句归口修正（0.1.9 P2 件b，PR#48）——原尾句「堆递归需 Box 亦未开放，列入不可表达清单 R2 §7」与 R2 §5（`Box<T>` 内建具名句柄）、§7 可表达首行、r2-06-box-recursive-list 钉定正例的在册实态矛盾（R2 §7 不可表达 7 条目中并无该条目），随本件修正移除；禁令句作用域限定为变体载荷类型不得经不经 `Box` 间址的具名类型链（直接或间接）引用该 enum 自身，报 E1401 与钉定负例 r1-n05/n06 及 §8 码表短名行判据同号。`Box` 间址组合面的表述口径=本句不作「已开放」亦不作「禁止」表述，归 §7 覆盖勘定注记（件c）如实记。改写前后语义经同源实跑对表：直接递归与两 enum 具名互引报 E1401、`Box` 间址自环与两 enum 互递零诊断放行、钉定正例 r2-06 原形态照常通过——执行者、核心工程师、QA 三方实跑结论在票面在档且全件吻合，前移条款确认未触发。E1401 码位、r1-n05/n06 负例+golden、§8 码表语义行本版零触碰；改写句承诺集前后相等（不新增承诺、不放宽既有禁令，新增文字全部为在册事实转写）。
- 版本身份锚：`compiler/Cargo.toml` `0.1.8` → `0.1.9`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 输出现与本版账本节标题同为 0.1.9 字面量，改动面无第二处版本字面量经 QA 全库检索实证）。本版延续「凡发版，链内含 bump 至当版号」固定载荷位例行执行——0.1.8→0.1.9 顺序直锚，无欠账叙事（PR#47，0.1.9 P1）。全库计数（主干实测复算）：正例 32 / 负例 33 / golden 33——P2 勘定件测试面零扩，计数与 0.1.8 账本在册值一致，负例数=golden 数不变式维持。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.8] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第八轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=固定载荷位第三次例行执行（P1）+ E1101 覆盖面消歧条款入册（P2）+ §3 第三在册承诺调用形态行为首钉（P3）。

### Added
- `specs/r1-type-system.md` §8：E1101 覆盖面消歧条款入册——0.1.5 P4 释义消歧先例的轻量段落格式（非全量编目表），三件入册：①E1101 为跨 lexer/parser/check 三层共享的标识符/解析失败码，码表短名"未知类型名"仅为速记不实（消歧不注销，逐层实发点全量编目留 0.2.x 语法冻结窗口，本版零扩面）；②check 层调用点口径——函数与变体均未命中的调用报 E1101，该文案模板与本条款句、§3 名字解析顺序句的在册承诺句、check.rs 实发点源字面之字节级三方互核结论由 QA 票面在档（零引载承接形态）；③本条款零新增/零注销任何错误码声明句（PR#44，0.1.8 P2）。
- `tests/specs/negative/`：行为钉住负例 `r1-n16-unknown-fn-call-with-args`（未声明函数带实参调用，实跑单诊断首诊 E1101——§3 名字解析顺序句"函数与变体均未命中"第三在册承诺首钉，0.1.7 §五滚动候选承接；spec↔用例双向对源，"实跑不符则整件前移"条款经 QA 双模式独立实跑复核确认未触发，stderr 与实捕 golden 双模式字节一致）。golden 为 `ci/capture_golden.sh` 实捕产物（禁手写，QA 全量重捕 modified=0 共 33 份字节稳定），历史 32 份 golden 零字节漂移维持；本环不扩面（载荷未列同源单测面，r1_diag 零触碰）。全库计数：正例 32 / 负例 33 / golden 33（基线 32/32/32 +1 负 +1 golden，负例数=golden 数不变式维持；PR#45，0.1.8 P3）。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.7` → `0.1.8`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 输出现与本版账本节标题同为 0.1.8 字面量，改动面无第二处版本字面量经 QA 全库 grep 实证）。本版为「凡发版，链内含 bump 至当版号」固定载荷位第三次例行执行——0.1.7→0.1.8 顺序直锚，无欠账叙事（PR#43，0.1.8 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.7] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第七轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=「凡发版，链内含 bump 至当版号」固定载荷位口径首轮例行执行（P1）+ 三条在册条款的剩余行为面实跑钉住进门禁面（P2）。

### Added
- `tests/specs/negative/`：行为钉住负例两枚——`r1-n14-enum-nullary-with-args`（无参变体构造带实参，实跑单诊断首诊 E1601 载荷不符，把 0.1.5 入册 §3"实参个数与空载荷不符"承诺首次钉进门禁面）与 `r1-n15-shadow-println-arity`（用户自定义同名 `println` 后实参个数不符，实跑单诊断首诊 E1601 调用 arity 模板——§6 内置归口条款的"同名让位按普通函数签名检查"面首钉，0.1.6 仅钉零实参报错面）。两件的提示文案按零引载口径不落字面入节：spec 入册行=check.rs 源码字面量=实捕 golden 的字节级三方互核结论由 QA 票面在档。`tests/specs/positive/`：行为钉住正例 `r1-22-print-zero-arg`（`print()` 零实参合法 + `print(1)` 带参基线——§6"`print` 零实参合法"在册承诺此前全部正例仅覆盖带参形态，本版补正例面）。golden 两份均为 `ci/capture_golden.sh` 实捕产物（禁手写，QA 全量重捕 modified=0 字节级复现），历史 30 份 golden 零字节漂移维持；同源单测三枚同批（r1_diag 25→28 全绿）。全库计数：正例 32 / 负例 32 / golden 32——如实定性：裁定原文计数行「负 31 / golden 31」系裁量面算术笔误（与本行载荷 +2 负/+2 golden 自不闭合），被执行面按 §二"逐字=文件字节为准"纪律当场拦下候裁、产品经理就地勘误为 32/32/32（裁定 v1.1，seq1035 裁定原帖在档），本账本以勘误后口径落账（PR#41，0.1.7 P2；0.1.5 规范条款→0.1.6 两钉→本版余面三钉的覆盖推进）。

### Fixed
- 版本身份锚：`compiler/Cargo.toml` `0.1.6` → `0.1.7`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 输出现与本版账本节标题同为 0.1.7 字面量，改动面全库无第二处版本字面量经 QA 票面全库 grep 实证）。口径入册：本版为「凡发版，链内含 bump 至当版号」固定载荷位的新规首轮例行执行——0.1.6→0.1.7 顺序直锚，无欠账叙事（0.1.6 节的清偿性直锚先例为该口径入据之链史；PR#40，0.1.7 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.6] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第六轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。本版主题=版本身份欠账清偿（P1）+ 0.1.5 两入册条款行为钉住进门禁面（P2）。

### Added
- `tests/specs/negative/`：行为钉住负例两枚——`r1-n12-println-zero-arg`（裸 `println()`，实跑单诊断首诊 E1601，实捕文案与 0.1.5 入册 §6 内置归口行一致；该文案的 spec 入册行=check.rs 源码字面量=实捕 golden 字节级三方互核由 QA 票面在档）与 `r1-n13-enum-bare-variant`（裸变体名 `let a = Red;`，实跑单诊断首诊 E1101，与 0.1.5 入册 §3"构造拼写仅限 `Variant()`"在册承诺相符——裁定"实跑不符则整体前移、不猜行为改承诺"条款经 QA 独立实跑复核确认未触发）。同源单测两枚同批（r1_diag 23→25 全绿，单诊断首诊码固化）；golden 两份均为 `ci/capture_golden.sh` 实捕产物（禁手写，QA 全量重捕 modified=0 字节级复现），历史 28 份 golden 零字节漂移。全库计数：正例 31 / 负例 30，golden 基线 30（PR#38，0.1.6 P2；上两版规范条款→本版行为钉住的闭环）。

### Fixed
- 版本身份欠账清偿：`compiler/Cargo.toml` `0.1.4` → `0.1.6`（`Cargo.lock` zorcc 条目为 generated 产物同批联动；单一版本源，`zorcc --version` 输出现与本版账本节标题同为 0.1.6 字面量）。在册事实如实留痕：0.1.4 发布时版本串曾清偿至 `0.1.4`（见 [0.1.4] 节 Fixed 行），而 0.1.5 发布链未含 bump——该版发布时 crate 版本串仍滞后为 `0.1.4`（以本账本 [0.1.5] 节无 bump 条目为源）；本版自 `0.1.4` 直锚当版 `0.1.6`，跳过 `0.1.5` 系清偿性直锚而非漏账（PR#37，0.1.6 P1）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。

## [0.1.5] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第五轮；本版主题=0.1.3 期滚动"文档勘定项"四项收口（E1501 释义扩写、`println` 内置归口、r1-09 无参变体构造形态、Copy 批次增量口径消歧），全部为 `specs/r1-type-system.md` 规范条款入册，零测试面/golden/编译器改动（正例 31 / 负例 28 / golden 28 计数不变）；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准，本节合入即本版链锚。

### Added
- `specs/r1-type-system.md` §8：E1501 释义扩写表入册——与 compiler 实发点集合一一对应的 10 行表（parser 7 + check 3），提示文案逐字如实转写，附两注记（#4/#9 近同文案分属 parser/check 两实发点、`&mut x` 表达式借用与参数位 `&T/&mut T` 合法不触发本码）；纯释义说明，不新增/不注销任何错误码，码表短名"引用类型不合规出现位置"速记的实际覆盖面（引用、参数化与指针三类语法禁则）同步言明（PR#32，0.1.5 P1，勘定项之一闭合）。
- `specs/r1-type-system.md` §6：内置 I/O 调用 `println`/`print` 实施归口条款——用户未定义同名时按内建特判（特判条件与 check 层源码逐字对源）、自定义同名让位按普通函数签名检查、各实参逐个类型检查、不提供格式串/格式化承诺、调用表达式类型 `unit`、`println` 零实参报 E1601（文案逐字"至少一参"语义入册）而 `print` 零实参合法（PR#33，0.1.5 P2，勘定项之二闭合）。
- `specs/r1-type-system.md` §3：enum 无参变体构造形态条款——声明形态裸 `A` 与 `A()` 均合法（载荷括号可选）、构造拼写仅限 `Variant()`（裸变体名报 E1101，文案逐字）、名字解析顺序函数优先（同名函数与变体并存时调用点按函数签名检查、变体让位，均未命中报 E1101 逐字文案）、实参个数与空载荷不符报 E1601（模板归一转写非逐字）；与 AC-R1-2 正例 `r1-09-enum-nullary` 对齐，零行为改动纯插入（PR#34，0.1.5 P3，勘定项之三闭合）。
- `specs/r1-type-system.md` §5：Copy 判定"批次增量口径"消歧澄清条款——言明该口径为用例覆盖推进口径（测试面记账规则，非语言语义），不蕴含任何运行期/编译期"批次"语义、不得引申为"聚合赋值的复制/移动批次语义"类未登记承诺，各行判定语义唯一由 §5 表列与既有结构规则（含无 trait 标注条款）决定；语言语义面零新增，回写②原文逐字引载入条款（字节级可互核），出处考证段（D-029+《M1 判据模板·用例清单》PR#9 回写②）入 PR 描述留痕（PR#35，0.1.5 P4，PM 裁定 (a) 口径，勘定项之四闭合）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结。
- 0.1.3 期登记"文档勘定项"四项已随本版全部收口（上列 Added 四条），不再滚存。

## [0.1.4] - 2026-10-13

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏第四轮；按 D-043 发布方式新规，日常版本以本账本条目+main 推送为准。

### Added
- `tests/specs/positive/`：行为钉住正例 `r1-21-struct-mixed-separators`——CO-8 struct 字段分隔符过渡口径落规范后的首条用例，钉住解析器现行为 `eat(Semi) || eat(Comma)`（混合 `,`/`;` 分隔合法），与 `r1_diag` 同源单测 `pos_struct_mixed_separators` 同批合入（PR#29）。全库计数：正例 31 / 负例 28，golden 基线 28 份零差异。

### Changed
- `specs/r1-type-system.md` struct 条款：字段分隔符过渡口径入册——`,` 为规范形态、`;` 为 0.1.x 兼容容忍形态（两分隔符均缺且未到 `}`/EOF 报 E1101，提示 `expected ';' or ',' after field, found <token>`），分隔符终审归 0.2.x 语法冻结（CO-8 溯源注记）；§其余条款零改动（PR#28，口径本体入册）。
- `README.md` 状态行防漂移：过期断言「0.1.1 已发布」改为指向型口径（0.1.x 逐日推进、最新发布见 Releases/CHANGELOG），不硬编码单版本号；诚实声明原句「内部验证里程碑，非可用语言版本」逐字保留（PR#30）。

### Fixed
- crate 版本串清偿：`compiler/Cargo.toml` `0.1.2` → `0.1.4`（清 0.1.3 链版本串滞后账），`Cargo.lock` 同批联动；`zorcc --version` 输出现与账本版本一致（PR#27）。

### Known issues（跟踪中，非本版拦截）
- 承接在册滚项：AC-R2-4 `compare/` 槽位（M2 排期）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；结构化 CLI 解析（`--version/-V` 短路语义 0.2.x CLI 面扩展时再议）；R3/R4 真交叉负例各 1 条（待规范成文）；CO-8 分隔符终审归 0.2.x 语法冻结（本版仅过渡口径，PR#28/#29 已钉行为）。

## [0.1.3] - 2026-10-12

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。"每日一版"节奏当日第三次发布（0.1.1/0.1.2/0.1.3 同日均含真实增量）。

### Added
- `tests/specs/positive/`：R2 语义正例 `r2-08-field-auto-borrow-read-write`、`r2-09-full-lifecycle-construct-borrow-release` 两项补位（追溯矩阵行3/行10 闭环），与 `r2_diag` 同源单测（+12 行）同批合入（PR#25）。全库计数：正例 30 / 负例 28，golden 基线 28 份零差异（strict 双模式复验 ok=60）。

### Changed
- `specs/r1-type-system.md`：标题行抹"（初稿 draft-0.1）"字样，状态行由 draft-0.1 切换为「0.1.x 实施基线（原 draft-0.1；0.1.0 tag 前置项 V-2 已随实施转正，正/负例与 golden 门禁在 main）」；§正文条款零改动（PR#22）。
- `specs/README.md`：表格状态列改为对 spec 实际状态行的镜像转写——r1 行逐字镜像已切换状态行、r2 行按裁定口径「0.1.1 冻结基线（V-2 初稿，随 0.1.1 发布链定版）」、r3/r4 行「大纲（M2 前成文）」不动；只镜像不新增冻结宣告（PR#24）。

### Fixed
- `CHANGELOG.md` 节号勘正：`R2 §7.4` → `R2 §7` 全文 2 处（0.1.1 Known issues 行 + 0.1.2 承接行）。勘误依据：`specs/r2-ownership.md` 节结构 §1–§10 无 §7.4，循环引用已知限制实证在 §7；仅节号指向、零语义改动（PR#23）。

### Known issues（跟踪中，非本版拦截）
- 承接上版在册项：R3/R4 真交叉负例各 1 条（待规范成文）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；循环引用 leak 不属编译期门禁面（R2 §7）；文档勘定项（spec R1 §8 E1501 释义扩写、r1-09 构造形态条款、`println` 内置归口、Copy 判定批次增量口径）；`--version/-V` 预扫描短路语义 0.2.x CLI 面扩展时再议结构化解析。

## [0.1.2] - 2026-10-12

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。本版为"每日一版"节奏首个版本（0.1.1/0.1.2 同日发布，均含真实增量）。

### Added
- `zorcc --version` / `-V` 独立版本串：预扫描命中输出 `zorcc {CARGO_PKG_VERSION}`、exit 0；版本源统一为 Cargo.toml（crate 版本 0.1.0 → 0.1.2，清 0.1.1 链遗留欠账），usage 兜底与 CLI 三态契约（exit 0/1/2）回归不破（PR#20）。
- `tests/specs/positive/`：R2 语义正例首批 7 项 `r2-01..r2-07`（box 创建读回释放 / move 链传参返回 / 借用读写参数 / 块尾借用失效 / 部分移动恢复 / box 递归链表 / 绑定间赋值移动），CO-5 豁免转正、随批进门禁面（QA 门禁记录，n_pos 21→28）（PR#19）。全库计数：正例 28 / 负例 28，golden 基线 28 份零差异。

### Changed
- `specs/r2-ownership.md` 标题行清理：抹去"（初稿 draft-0.1）"残留字样，与状态行"0.1.1 冻结基线"口径一致；§正文条款零改动（PR#18，D-037③ 登记项闭合）。

### Known issues（跟踪中，非本版拦截）
- 承接上版在册项：R3/R4 真交叉负例各 1 条（待规范成文）；T-5 行为用例与 ASAN 对等校验（判据预置+不可执行留痕，运行链路落地后首版自动生效）；循环引用 leak 不属编译期门禁面（R2 §7）；文档勘定项（spec R1 §8 E1501 释义扩写、r1-09 构造形态条款、`println` 内置归口、Copy 判定批次增量口径）。
- `--version/-V` 预扫描为短路语义（args 任意位置命中即先于文件处理），0.2.x CLI 面扩展时再议结构化解析（QA PR#20 非阻塞备注在册）。

## [0.1.1] - 2026-10-12

内部验证里程碑，非可用语言版本：`zorcc` 仍为纯编译前端（无代码生成/运行路径）。

### Added
- `compiler/`：`zorcc` R1 最小前端（lexer/parser/type check），R1 诊断码 E1101–E1601 全量实装（PR#5）。
- `zorcc` R2 所有权/借用/移动检查：E2101（移动后使用）、E2102（部分移动后整体使用）、E2201（独占借用期间再借用）、E2202（共享/独占共存冲突）、E2301（unbox 后再用/double free 类）、E1502（返回引用，定码；E2401 保留码位）（PR#10）。
- `tests/specs/positive/`：AC-R1-2 正例 20 项 `r1-01..r1-20`（slug 命名）（PR#9）。
- `tests/specs/negative/`：AC-R1-2 负例 11 项 `r1-n01..n11`（PR#7，slug 回填 PR#11）；AC-R2-2 所有权/借用负例 16 项 `r2-n01..n16`（PR#10）。
- `tests/specs/golden/`：T-4 黄金基线 28 份（负例 stderr 全文、双模式零差异）；`ci/run_ci.sh` 新增 `stage_golden`（strict 下 golden 缺失或零比对=FAIL）；`ci/capture_golden.sh` 捕获脚本（PR#12）。
- CI：GitHub Actions 外部通道发布并默认 `strict` 模式（CIQ-2，PR#4/#8）；`ZORC_BIN` 注入（PR#6）。

### Changed
- 规范 `specs/r2-ownership.md` §5 堆分配语法定稿（OI-3 关闭；拼写载文于 PR#2/D-020 团队自决，落码实装于 PR#10）：`box expr` / `*expr` / `b.field` 自动借用 / `unbox b`。
- CI push/PR 默认运行模式切换为 `strict`：空套件、负例/正例零计数、golden 缺失均为 FAIL（PR#8）。
- r1 负例文件命名对齐测试契约 `r1-n<序号>-<slug>.zor`，全库命名契约一致（PR#11）。

### Fixed
- parser：条件表达式上下文（`if`/`while` 块头）误按 record 结构体花括号糖解析导致吞块（PR#10）。
- CI runner：`stage_cargo` 日志相对路径在 `cd compiler` 子 shell 下失效导致主干门禁误红；`$REPO_ROOT` 绝对路径化修复（PR#6）。
- `run_ci.sh` spec 目录 glob 与 runner 目录契约不一致（M-1）、`stage_compile` 双模式空循环旁路（S-1）、n_pos 对称守卫（PR#4）。

### Known issues（跟踪中，非本版拦截）
- R3/R4 真交叉负例各 1 条待对应规范成文后补入（判据模板 §2 登记）。
- T-5 行为用例（运行期溢出 checked/wrap、除零）与 ASAN 对等校验：0.1.1 按门禁 v2 §6 修订行（D-033 会签）作"判据预置+不可执行留痕（文档级核对）"，可执行义务自运行链路落地后首版自动生效；0.1.2/0.1.3 仅为排期注记，不作期限。
- 循环引用导致的内存 leak 不属编译期门禁面（R2 §7 已知限制）。
- 文档项：spec R1 §8 E1501 释义扩写、r1-09 构造形态条款、`println` 内置归口、Copy 判定批次增量口径。

## [0.1.0] - 2026-10-11

内部验证里程碑，非可用语言版本（annotated tag 锚点 `16347de`）。

### Added
- 仓库骨架：README/LICENSE（MIT）/`specs`/`tests`/`bench`/`examples`/`compiler` 目录契约（PR#2 成文；glob 与 runner 不一致的 M-1 缺陷修复见 0.1.1 Fixed）。
- 规范草案：R1 类型系统、R2 所有权 draft-0.1 成文（PR#2）；吸收 QA V-4 裁定 F-1/F-1b/F-1c/F-2 勘误——R2 循环引用移出编译期负例清单、负例补足、MG/RG/HG 门禁层级命名更正（PR#3，tag 锚点即此 head）。
- CI 基线骨架：git 直连编译 + 单元测试流水线（PR#1）。

## 更早
- 0.1.x 需求包（PRD）与质量门禁标准（MG/RG/HG）在内部协作空间维护，不随仓库发布。
