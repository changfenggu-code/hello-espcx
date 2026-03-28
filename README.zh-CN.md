# hello-espcx

[English](README.md) | [简体中文](README.zh-CN.md)

`hello-espcx` 是一个用 Rust 编写的精简但完整的 BLE 双端示例：

- `apps/ble/peripheral/` 运行在 ESP32-C6 上，提供 Battery Service GATT Server
- `apps/ble/central/` 运行在桌面端，负责扫描、连接、订阅通知并读取特征值

当前项目主要围绕 ESP32-C6 固件开发和 Windows 桌面 BLE 中心程序来组织。

## 项目简介

仓库由两个可执行 crate 组成：

- `apps/ble/common/`：两端共享的 BLE 常量
- `apps/ble/peripheral/`：嵌入式 BLE 外设，基于 `esp-hal`、`esp-radio`、`esp-rtos` 和 `trouble-host`
- `apps/ble/central/`：桌面 BLE 中心程序，基于 `btleplus` 和 `tokio`

`contrib/esp-hal/` 通过 git submodule 方式引入本地路径依赖，便于项目稳定地绑定到特定版本的嵌入式栈。

## 仓库结构

```text
hello_espcx/
|- apps/ble/common/     # 共享 BLE 常量
|- apps/ble/peripheral/ # ESP32-C6 BLE 外设
|- apps/ble/central/    # 桌面 BLE 中心程序
|- crates/btleplus/   # central 使用的跨平台 BLE 客户端库
|- contrib/esp-hal/     # 作为本地依赖使用的 esp-rs 子模块
|- llm/                 # 参考代码和上游样例
|- justfile             # 常用构建与烧录命令
|- README.md
|- README.zh-CN.md
|- AGENTS.md
`- CLAUDE.md
```

## 当前 BLE 约定

- 外设广播名：`hello-espcx`
- 外设随机地址：`FF:8F:1A:05:E4:FF`
- Battery Level 特征值 UUID：`0x2A19`
- 外设会周期性发送通知，并打印 RSSI
- 中心程序会先按设备名发现目标，再订阅和读取特征值

如果你修改广播名、特征值 UUID 或通知流程，建议把它视为跨端变更，并同步检查 `apps/ble/peripheral/` 与 `apps/ble/central/`。

## 环境准备

建议准备以下工具：

- Rust
- `just`
- 可用的 `bash`
- `probe-rs`
- ESP32-C6 开发板
- 桌面端蓝牙适配器

克隆仓库后先初始化子模块：

```bash
git submodule update --init --recursive
```

安装项目需要的 Rust 工具链组件：

```bash
just install
just check
just clippy
just hil-test-live
just hil-test
```

注意：根目录 `justfile` 明确指定 `bash` 作为 shell。在 Windows 上，请确保系统可通过 Git Bash、MSYS2、WSL 或等效环境提供 `bash`。

## 快速开始

### 构建外设

```bash
just build
```

调试构建：

```bash
just build-debug
```

### 烧录外设并查看 RTT 日志

```bash
just flash
```

调试版本烧录：

```bash
just flash-debug
```

外设使用 `rtt-target` 输出日志，因此通过 `probe-rs run` 启动后可以直接看到 `rprintln!` 输出。

### 构建并运行中心程序

```bash
just build-central
just run-central
```

调试构建：

```bash
just build-central-debug
```

## 验证命令

如果你想同时检查桌面端和嵌入式端，又不想碰根 workspace 的混合目标限制，可以直接使用：

```bash
just check
just clippy
```

如果要做真实硬件闭环验证，可以使用：

```bash
just hil-test-live   # 对已经在运行的设备做联通测试
just hil-test        # 构建、下载、复位后再执行 HIL 测试
just hil-stress-live # 运行 3 轮真实硬件压测
just hil-stress      # 先烧录，再跑 3 轮真实硬件压测
```

当前 HIL 测试覆盖：

- 自定义特征值的基础读写
- Battery Level 通知
- `central -> peripheral` 方向按 128 字节分块的 10 KiB 上传，并做校验和验证
- `peripheral -> central` 方向按 128 字节分块的 10 KiB 通知流，并做校验和验证

压测模式会额外执行多轮传输，并输出每轮吞吐。也可以通过环境变量覆盖总字节数和轮数：

```bash
HELLO_ESPCX_HIL_BYTES=10240
HELLO_ESPCX_HIL_ROUNDS=3
```

如果你仍然想跑更大的数据量，比如 1 MiB，可以显式覆盖：

```bash
HELLO_ESPCX_HIL_BYTES=1048576 just hil-stress-live
```

## 直接使用 Cargo

如果你更习惯直接使用 Cargo，可以分别在对应目录下执行：

```bash
# 外设
cd apps/ble/peripheral
cargo check --target riscv32imac-unknown-none-elf
cargo build --target riscv32imac-unknown-none-elf

# 中心程序
cd ../central
cargo check
cargo run
```

## 工具链与平台说明

### `apps/ble/peripheral/`

- 通过 `apps/ble/peripheral/rust-toolchain.toml` 固定使用 `nightly`
- 目标平台为 `riscv32imac-unknown-none-elf`
- 当前仓库没有提交 `apps/ble/peripheral/.cargo/config.toml`
- 直接使用 Cargo 时请显式传 `--target riscv32imac-unknown-none-elf`，或者优先使用根目录 `just` 命令
- 入口文件：`apps/ble/peripheral/src/main.rs`
- GATT Server 定义：`apps/ble/peripheral/src/ble_bas_peripheral.rs`

### `apps/ble/central/`

- 是标准的桌面 Rust 程序
- 当前开发流程主要围绕 Windows BLE 场景组织
- 入口文件：`apps/ble/central/src/main.rs`

## IDE 说明

仓库提供了 `.vscode/settings.json`，其中把 rust-analyzer 的默认目标设为 `riscv32imac-unknown-none-elf`。

这对嵌入式开发很方便，但编辑 `central/` 时，IDE 诊断可能没有直接在 `central/` 目录下执行 `cargo check` 那么准确。出现分歧时，以实际 Cargo 结果为准。

推荐的 VS Code 扩展见 `.vscode/extensions.json`。

## 硬件备注

仓库当前记录的 USB UART 引脚如下：

| 信号 | GPIO |
| --- | --- |
| TX (USB UART) | 20 |
| RX (USB UART) | 19 |

## 已验证的最小基线

在当前仓库状态下，以下命令可以通过：

```bash
cargo check -p hello-ble-central
cd apps/ble/peripheral && cargo check --target riscv32imac-unknown-none-elf
```

如果你修改了运行时行为，仍然建议在真实硬件上补充验证烧录、BLE 发现和通知链路。
