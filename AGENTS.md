# AGENTS.md

本文件写给进入本仓库协作的 AI agent 和工程协作者。重点不是重复 README，而是帮助你快速知道该改哪里、别改哪里、以及改完至少怎么验证。

## 一句话理解仓库

这是一个 Rust BLE 双端样例仓库：

- `apps/ble/common/`：两端共享的 BLE 常量、UUID、广播载荷定义
- `apps/ble/peripheral/`：ESP32-C6 外设固件
- `apps/ble/central/`：桌面中心程序
- `crates/btleplus/`：Windows BLE 中心侧通用库
- `contrib/esp-hal/`：本地 vendor / 上游依赖镜像

默认把它当成“两端协作 + 一个库层 + 一个上游依赖镜像”的仓库，而不是一个可以随便全局重构的普通 workspace。

## 默认修改边界

- 优先修改 `apps/ble/peripheral/`、`apps/ble/central/`、`crates/btleplus/`、根目录文档、`justfile`
- 如果改 BLE 名称、地址、UUID、广告载荷格式，优先先看 `apps/ble/common/`
- 除非任务明确要求，否则不要修改 `contrib/esp-hal/`
- 除非任务明确要求，否则不要把 `llm/` 或其他参考目录里的代码直接搬进主流程
- 文档任务允许直接重写 `README.md`、`README.zh-CN.md`、`CLAUDE.md`、`AGENTS.md`

## 目录含义

- `apps/ble/common/`：两端共享协议定义
- `apps/ble/peripheral/`：嵌入式侧主代码
- `apps/ble/central/`：桌面侧主代码和实机 HIL 测试
- `crates/btleplus/`：BLE 中心侧通用抽象库
- `contrib/esp-hal/`：git submodule / 本地路径依赖

### `crates/btleplus/` 当前结构

- `src/gap/adapter.rs`：扫描、发现、连接入口
- `src/gap/filter.rs`：扫描期硬过滤 `ScanFilter`
- `src/gap/selection.rs`：发现后排序/选择 `Selector`
- `src/gap/display.rs`：`Peripheral` 和 peripheral 集合的格式化辅助
- `src/gap/peripheral.rs`：`Peripheral` / `PeripheralProperties` / `ManufacturerData`
- `src/gap/connection.rs`：GAP 连接生命周期
- `src/gatt/`：GATT client / database 逻辑
- `docs/`：库文档和 roadmap

### `apps/ble/central/` 当前结构

- `src/lib.rs`：产品侧 BLE 会话封装、扫描策略、候选设备组装
- `src/main.rs`：程序入口
- `src/tests.rs`：中心程序内部业务逻辑单测
- `tests/hil_real.rs`：真实硬件 HIL 测试
- `docs/hello-ble-central.md`：中心程序结构和使用说明

## 高优先级事实

- 外设广播名：`hello-espcx`
- 外设固定随机地址：`ff:8f:1a:05:e4:ff`
- 中心程序按名字 `hello-espcx` 扫描
- 中心程序读取并订阅 Battery Level 特征值 `0x2A19`
- 外设入口：`apps/ble/peripheral/src/main.rs`
- GATT 服务：`apps/ble/peripheral/src/ble_bas_peripheral.rs`
- 中心程序入口：`apps/ble/central/src/main.rs`

只要你改动广播名、地址、UUID、通知行为、manufacturer payload 结构，就要默认认为这是跨端改动。

## 当前推荐调用路径

在 `btleplus` / `hello-ble-central` 里，默认推荐显式三段式流程：

1. `ScanFilter`：扫描期硬过滤
2. `Selector`：发现后排序 / 选择
3. `Peripheral::connect()`：连接

推荐代码形状：

```rust
let filter = ScanFilter::default()
    .with_name_pattern("hello-espcx")
    .with_service_uuid(Uuid::from_u16(0x180F));

let selector = Selector::default()
    .prefer_connectable()
    .prefer_strongest_signal();

let peripherals = adapter.discover(filter, timeout).await?;
let peripheral = peripherals.select_with(&selector)?;
let connection = peripheral.connect().await?;
```

默认把 `peripherals.select_with(&selector)` / `peripherals.rank_with(&selector)` 当成推荐写法；`selector.select(&peripherals)` / `selector.rank(&peripherals)` 仍然保留，但不再是文档主路径。

## 工作方式

优先用仓库已有命令，不要自己重新发明脚本。

```bash
just --list
just check
just clippy
just hil-test-live
just hil-test
just build
just flash
just build-central
just run-central
```

注意：`justfile` 使用 `bash` 作为 shell。在 Windows 上运行失败时，先检查是否安装了可用的 Bash。

## 验证优先级

最小验证：

```bash
just check
```

如果改的是 `btleplus` 或 central 侧纯桌面逻辑，优先补：

```bash
cargo test -p btleplus
cargo test -p hello-ble-central --lib
cargo check -p hello-ble-central
```

真实设备可用时，优先补一轮：

```bash
just hil-test-live
```

如果你需要从固件下载开始验证整条链路，使用：

```bash
just hil-test
```

更贴近实际的验证：

```bash
just build
just build-central
just run-central
just flash
```

如果没有板子或蓝牙硬件，明确写出“未做实机验证”，不要假装已经验证成功。

## 测试放置规则

按当前仓库约定，测试分三层：

### 1. 底层：单元测试，数量最多

放源文件旁边，测私有逻辑、纯函数、小范围行为。

当前例子：

- `crates/btleplus/src/gap/filter/tests.rs`
- `crates/btleplus/src/gap/selection/tests.rs`
- `crates/btleplus/src/gap/adapter/tests.rs`
- `apps/ble/central/src/tests.rs`

适合放这里的测试：

- 规则判定
- 排序逻辑
- payload 解析
- 小范围 helper 行为

### 2. 中层：集成测试 / 接口测试，数量适中

放 crate 或 app 根目录下的 `tests/`，只通过公开接口验证模块组合，不依赖真实硬件。

当前仓库这一层还偏少；如果后面要补：

- `crates/btleplus/tests/...`
- `apps/ble/central/tests/...`

适合放这里的测试：

- 公开 API 组合
- 跨模块协作
- 从调用方视角验证 `discover -> select -> connect` 之前的逻辑拼装

### 3. 顶层：HIL / E2E / 手工验收，数量最少

放 `tests/`，文件名明确标识为实机测试。

当前例子：

- `apps/ble/central/tests/hil_real.rs`

这层运行慢、依赖真实硬件、维护成本高，但最接近用户真实路径。

### 测试添加建议

- 先补单元测试，再补集成测试，最后再考虑 HIL 扩容
- 不要为了测试 `scan_for_targets` 直接引入脆弱的系统蓝牙 mock
- 优先抽可测试的纯逻辑 helper，再在模块旁写单测
- 只有当某段组合逻辑已经跨模块、且单测不足以覆盖时，再上 `tests/` 目录的集成测试

## 修改 peripheral 时要记住

- 这是 `no_std + no_main` 程序
- 日志走 `rtt-target`，保持使用 `rprintln!`
- 工具链由 `apps/ble/peripheral/rust-toolchain.toml` 固定为 `nightly`
- 外设目标是 `riscv32imac-unknown-none-elf`
- 当前仓库未提交 `apps/ble/peripheral/.cargo/config.toml`，直接用 Cargo 时需要显式传 `--target riscv32imac-unknown-none-elf`，或优先使用根目录 `just`
- `build.rs` 里有自定义 linker 友好报错逻辑，不要轻易删掉

## 修改 central 时要记住

- 这是普通桌面 Rust + `tokio` 程序
- 主要职责是扫描、连接、发现服务、订阅通知、定期读取
- 当前 central 已经使用 `discover + Selector + connect` 路径，不要轻易退回 “find first match” 风格
- 当前 central 的 manufacturer payload 判定逻辑在 `apps/ble/central/src/lib.rs`
- 当前仓库的 VS Code rust-analyzer 目标默认是 RISC-V，对 `central/` 代码提示不一定完全准确
- 判断 `central/` 是否正常，优先相信 `cargo check` / `cargo test` / `cargo run`

## 修改 `btleplus` 时要记住

- `ScanFilter` 是扫描期硬过滤
- `Selector` 是发现后排序/选择
- `PeripheralDisplayExt` / `PeripheralSelectionExt` 是集合视角的扩展 trait
- 自定义结构序列化默认统一用 `postcard`
- 标准 BLE 特征值不要为了统一而硬包一层 `postcard`，优先按规范直接解析
- 如果调整 API 风格，优先统一文档主路径，不要轻易删除底层入口
- 修改 `gap` 逻辑时，优先补对应模块旁边的单元测试

## 对 `contrib/esp-hal` 的态度

- 把它当成上游依赖，不要顺手格式化、重命名、批量修复
- 如果必须改，先确保改动是局部且必要的
- 任何会影响上游同步、子模块指针或路径依赖解析的修改，都应该在说明里明确提到

## 文档策略

- `README.md` / `README.zh-CN.md`：给人类开发者，偏 onboarding 和操作说明
- `AGENTS.md`：给 agent / 协作者，偏执行约束、目录和测试规则
- `CLAUDE.md`：给 Claude/Codex 一类编码助手，偏短小、可立即执行的项目提示
- `crates/btleplus/docs/device-selection-roadmap.md`：记录 `btleplus` 设备选择能力的演进和当前进度

重写时允许风格不同，但核心事实必须一致。

## 当前基线

截至 2026-03-28，以下检查已在仓库里跑通：

```bash
cargo test -p btleplus
cargo test -p hello-ble-central --lib
cargo check -p hello-ble-central
cd apps/ble/peripheral && cargo check --target riscv32imac-unknown-none-elf
```

做完改动后，至少回到这条基线之上。
