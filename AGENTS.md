# AGENTS.md

本文件写给进入本仓库协作的 AI agent 和工程协作者。重点不是重复 README，而是帮助你快速知道该改哪里、别改哪里、以及改完至少怎么验证。

## 一句话理解仓库

这是一个 Rust BLE 双端样例：

- `apps/ble/common/` 是共享 BLE 常量 crate
- `apps/ble/peripheral/` 是 ESP32-C6 外设
- `apps/ble/central/` 是桌面中心程序
- `contrib/esp-hal/` 是本地 vendor 子模块

默认把它当成“两端协作 + 一个上游依赖镜像”的仓库，而不是一个可以随便全局重构的普通 workspace。

## 默认修改边界

- 优先修改 `apps/ble/peripheral/`、`apps/ble/central/`、根目录文档、`justfile`
- 如果改 BLE 名称、地址或 UUID，优先先看 `common/`
- 除非任务明确要求，否则不要修改 `contrib/esp-hal/`
- 除非任务明确要求，否则不要把 `llm/` 里的代码搬进主流程
- 文档任务允许直接重写 `README.md`、`CLAUDE.md`、`AGENTS.md`，不需要刻意保持它们内容逐字一致

## 目录含义

- `apps/ble/peripheral/`：嵌入式侧主代码
- `apps/ble/central/`：桌面侧主代码
- `apps/ble/common/`：两端共享的 BLE 协议常量
- `contrib/esp-hal/`：git submodule，提供 `esp-hal` / `esp-radio` / `esp-rtos` 等本地路径依赖
- `llm/`：参考实现、样例、上游源码快照

## 高优先级事实

- 外设广播名：`hello-espcx`
- 外设固定随机地址：`ff:8f:1a:05:e4:ff`
- 中心程序按名字 `hello-espcx` 扫描
- 中心程序读取并订阅 Battery Level 特征值 `0x2A19`
- 外设入口：`apps/ble/peripheral/src/main.rs`
- GATT 服务：`apps/ble/peripheral/src/ble_bas_peripheral.rs`
- 中心程序入口：`apps/ble/central/src/main.rs`

只要你改动广播名、地址、UUID、通知行为，就要默认认为这是跨端改动。

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

## 修改 peripheral 时要记住

- 这是 `no_std + no_main` 程序
- 日志走 `rtt-target`，保持使用 `rprintln!`
- 工具链由 `apps/ble/peripheral/rust-toolchain.toml` 固定为 `nightly`
- 外设目标是 `riscv32imac-unknown-none-elf`
- 当前仓库未提交 `apps/ble/peripheral/.cargo/config.toml`，直接用 Cargo 时需要显式传 `--target riscv32imac-unknown-none-elf`，或优先使用根目录 `just` 命令
- `build.rs` 里有自定义 linker 友好报错逻辑，不要轻易删掉

## 修改 central 时要记住

- 这是普通桌面 Rust + `tokio` 程序
- 主要职责是扫描、连接、发现服务、订阅通知、定期读取
- 当前仓库的 VS Code rust-analyzer 目标默认是 RISC-V，对 `central/` 代码提示不一定完全准确
- 判断 `central/` 是否正常，优先相信 `cargo check` / `cargo run`

## 对 `contrib/esp-hal` 的态度

- 把它当成上游依赖，不要顺手格式化、重命名、批量修复
- 如果必须改，先确保改动是局部且必要的
- 任何会影响上游同步、子模块指针或路径依赖解析的修改，都应该在说明里明确提到

## 文档策略

- `README.md` 给人类开发者，偏 onboarding 和操作说明
- `AGENTS.md` 给 agent / 协作者，偏执行约束和默认流程
- `CLAUDE.md` 给 Claude/Codex 一类编码助手，偏短小、可立即执行的项目提示

重写时允许三者风格不同，但核心事实必须一致。

## 当前基线

截至 2026-03-24，以下检查已在仓库里跑过并通过：

```bash
cargo check -p hello-ble-central
cd apps/ble/peripheral && cargo check --target riscv32imac-unknown-none-elf
```

做完改动后，至少回到这条基线之上。
