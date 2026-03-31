# hello-espcx

[English](README.md) | [简体中文](README.zh-CN.md)

`hello-espcx` 是一个用 Rust 编写的 BLE 双端示例，包含两个可运行侧：

- `apps/ble/peripheral/`：运行在 ESP32-C6 上，提供外设侧 GATT Server
- `apps/ble/central/`：运行在桌面 Windows 上，负责扫描、连接、读写和订阅通知

这个仓库围绕一份共享 BLE 协议合同，加上两个分别从外设侧和中心侧使用它的应用来组织。

## 概览

主要 crate：

- `apps/ble/common/`：共享 BLE 常量、UUID 命名空间、payload 结构和辅助函数
- `apps/ble/peripheral/`：基于 `esp-hal`、`esp-radio`、`esp-rtos`、`trouble-host` 的 ESP32-C6 固件
- `apps/ble/central/`：基于 `btleplus` 和 `tokio` 的桌面中心程序
- `crates/easyble/`：固件使用的外设侧生命周期辅助库
- `crates/btleplus/`：桌面中心程序使用的中心侧 BLE 库

嵌入式依赖通过 `vendor/esp-hal/` git submodule 固定版本。

## 仓库结构

```text
hello-espcx/
|- apps/ble/common/        # 共享 BLE 协议合同
|- apps/ble/peripheral/    # ESP32-C6 BLE 外设固件
|- apps/ble/central/       # 桌面 BLE 中心程序
|- crates/easyble/         # 外设侧生命周期辅助库
|- crates/btleplus/        # 中心侧 BLE 库
|- vendor/esp-hal/         # 固定版本的 esp-rs 栈
|- justfile                # 常用构建 / 烧录 / 测试命令
|- README.md
|- README.zh-CN.md
|- AGENTS.md
`- CLAUDE.md
```

## 当前 BLE 约定

- 外设广播名：`hello-espcx`
- 外设固定随机地址：`FF:8F:1A:05:E4:FF`
- 标准 Battery Service UUID：`0x180F`
- 标准 Battery Level Characteristic UUID：`0x2A19`
- 中心程序按广播名和 Battery Service 发现目标
- 自定义 `echo`、`status`、`bulk` 以及 manufacturer identity payload 都定义在 `apps/ble/common/`

如果你修改广播名、地址、UUID、manufacturer payload 格式或通知行为，请把它视为跨端变更，并同步更新两边应用。

## 环境准备

建议准备以下工具：

- Rust
- `just`
- 可用的 `bash` 可执行文件，用于 `justfile` 中偏 shell 风格的 recipe
- `probe-rs`
- ESP32-C6 开发板
- 桌面蓝牙适配器

克隆仓库后先初始化子模块：

```bash
git submodule update --init --recursive
```

安装所需 Rust 工具链组件：

```bash
just install
```

## 快速开始

### 构建并烧录外设

```bash
just build
just flash
```

调试版本：

```bash
just build-debug
just flash-debug
```

固件使用 `rtt-target` 输出日志，所以 `probe-rs run` 启动后会直接显示 `rprintln!` 输出。

### 构建并运行中心程序

```bash
just build-central
just run-central
```

调试构建：

```bash
just build-central-debug
```

### 一条完整链路

1. 用 `just flash` 烧录并启动 ESP32-C6
2. 用 `just run-central` 启动桌面中心程序

## 验证命令

当前 `just` 入口：

```bash
just check          # host 侧 crate：common + central + btleplus + easyble
just check-all      # host 侧 + peripheral 目标检查
just clippy         # host 侧 clippy 集合
just check-peripheral
```

真实硬件验证：

```bash
just hil-test-live   # 对已运行设备做 HIL
just hil-test        # 构建、烧录、复位后跑 HIL
just hil-stress-live # 3 轮 live 压测
just hil-stress      # 先烧录，再跑 3 轮压测
```

当前 HIL 覆盖：

- 自定义特征值的基础读写
- Battery Level 通知
- 10 KiB `central -> peripheral` bulk 上传及完整性校验
- 10 KiB `peripheral -> central` bulk 通知流及完整性校验

也可以显式覆盖压测参数：

```bash
HELLO_ESPCX_HIL_BYTES=10240
HELLO_ESPCX_HIL_ROUNDS=3
HELLO_ESPCX_HIL_BYTES=1048576 just hil-stress-live
```

## 直接使用 Cargo

如果你更习惯直接用 Cargo：

```bash
# common
cargo check --manifest-path apps/ble/common/Cargo.toml

# easyble
cargo check -p easyble

# central
cargo check --manifest-path apps/ble/central/Cargo.toml
cargo run --manifest-path apps/ble/central/Cargo.toml

# peripheral
cargo check --manifest-path apps/ble/peripheral/Cargo.toml --target riscv32imac-unknown-none-elf
cargo build --manifest-path apps/ble/peripheral/Cargo.toml --target riscv32imac-unknown-none-elf
```

重要说明：

- 不要把仓库根目录的 `cargo check --target riscv32imac-unknown-none-elf` 当成固件验证命令
- 它会尝试把 `central` / `btleplus` 这类 host-side crate 也按裸机目标编译，然后报 `std` 相关错误

## 工具链与架构说明

### `apps/ble/peripheral/`

- 通过 `apps/ble/peripheral/rust-toolchain.toml` 固定使用 `nightly`
- 目标平台是 `riscv32imac-unknown-none-elf`
- 当前没有提交 `apps/ble/peripheral/.cargo/config.toml`
- 直接使用 Cargo 时请显式传 `--target riscv32imac-unknown-none-elf`
- 二进制入口：`apps/ble/peripheral/src/main.rs`
- 产品逻辑集中在 `apps/ble/peripheral/src/lib.rs`
- 固件生命周期主路径是：
  `easyble::gap::init -> easyble::gap::advertising -> easyble::gatt::connected -> easyble::gatt::session`

### `apps/ble/central/`

- 是标准桌面 Rust 程序
- 当前主要围绕 Windows BLE 场景组织
- 二进制入口：`apps/ble/central/src/main.rs`
- 会话与产品逻辑集中在 `apps/ble/central/src/lib.rs`

### `apps/ble/common/`

- 保存共享 BLE 协议合同
- UUID 采用“服务模块 + `service` / `characteristic` 子命名空间”的组织方式
- 修改这里的常量，就等于同时修改两端协议

### `crates/easyble/`

- 是外设侧生命周期辅助库
- 详细文档在 `crates/easyble/docs/`

## IDE 说明

仓库包含 `.vscode/settings.json`，其中 rust-analyzer 默认目标是
`riscv32imac-unknown-none-elf`。

这对嵌入式开发很方便，但编辑 `central/` 时，IDE 诊断可能不如对应目录下直接跑
`cargo check` 准确。出现分歧时，以实际 Cargo 结果为准。

推荐的 VS Code 扩展见 `.vscode/extensions.json`。

## 硬件备注

仓库当前记录的 USB UART 引脚如下：

| 信号 | GPIO |
| --- | --- |
| TX (USB UART) | 20 |
| RX (USB UART) | 19 |

## 当前基线

在当前仓库状态下，以下命令是比较实际的最小回归基线：

```bash
cargo check --manifest-path apps/ble/common/Cargo.toml
cargo check --manifest-path apps/ble/central/Cargo.toml
cargo check -p easyble
cargo check --manifest-path apps/ble/peripheral/Cargo.toml --target riscv32imac-unknown-none-elf
```

如果你修改了运行时行为，仍然建议在真实硬件上补充验证烧录、发现和通知链路。
