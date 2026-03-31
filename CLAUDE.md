# hello-espcx

这是一个 BLE 双端 Rust 项目：

- `apps/ble/common/`：共享 BLE 常量
- `apps/ble/peripheral/`：ESP32-C6 外设，`esp-hal` + `esp-radio` + `trouble-host`
- `apps/ble/central/`：桌面中心程序，`btleplus`

## 进入仓库后先知道这些

- 主工作目录通常只在 `apps/ble/peripheral/`、`apps/ble/central/`、根目录文档
- 改 BLE 名称、地址、UUID 时先看 `common/`
- `vendor/esp-hal/` 是 esp-hal 子模块，默认不要改
- `llm/` 是参考代码，不是主交付路径
- 根目录 `justfile` 依赖 `bash`

## 代码变更后必做

**每次修改代码后，必须执行完整的测试验证，确保变更正确：**

1. 静态检查：`cargo check` / `just check`
2. 代码风格：`cargo clippy` / `just clippy`
3. 单元测试：`cargo test`
4. 集成测试：ESP32 侧运行 `just hil-test`，Windows 侧运行 `just hil-test-live`
5. 回归验证：`just build` + `just build-central` 确认两端均能正常编译

测试必须是**真的**（实际执行，不是 mock 伪造的 pass）。如果测试本身有问题或缺失，先修复测试再声称功能正确。跨端变更（广播名、UUID、连接流程）必须两端同时验证。

## 常用命令

```bash
git submodule update --init --recursive
just install
just check
just clippy
just hil-test-live
just hil-test

just build
just flash

just build-central
just run-central
```

## 最小验证

```bash
just check
```

## 关键协议约定

- 广播名：`hello-espcx`
- 外设地址：`ff:8f:1a:05:e4:ff`
- Central 按设备名扫描
- Battery Level Characteristic：`0x2A19`

如果改广播名、UUID、通知逻辑或连接流程，要默认同时检查两端。

## 代码提示

- `apps/ble/peripheral/src/main.rs` 是外设入口
- `apps/ble/peripheral/src/lib.rs` 是 crate root，包含所有 GATT 服务定义和生命周期辅助函数
- `apps/ble/central/src/main.rs` 是中心程序入口
- 外设日志使用 `rprintln!`
- `apps/ble/peripheral/` 使用 `nightly`
- 直接进入外设目录运行 Cargo 时，显式传 `--target riscv32imac-unknown-none-elf`

## Codebase Map（`.planning/codebase/`）

项目已通过 `/gsd:map-codebase` 生成完整的 codebase 文档，作为代码库全景参考：

| 文档 | 行数 | 用途 |
|------|------|------|
| `ARCHITECTURE.md` | 456 | 软件架构、状态管理、并发模型、错误处理 |
| `STRUCTURE.md` | 396 | 目录结构、文件用途、模块组织 |
| `CONVENTIONS.md` | 359 | 编码规范、GATT 宏用法、序列化格式 |
| `STACK.md` | 227 | 完整依赖栈、工具链、构建工具 |
| `TESTING.md` | 167 | HIL 测试策略、运行方式、测试覆盖 |
| `INTEGRATIONS.md` | 158 | 组件集成、依赖图、Cargo workspace |
| `CONCERNS.md` | 150 | 技术债、安全考量、扩展性、运维风险 |

## IDE 备注

`.vscode/settings.json` 把 rust-analyzer 目标固定成了 RISC-V，这对外设侧是对的，但看 `central/` 时以 `cargo check` 结果为准。
