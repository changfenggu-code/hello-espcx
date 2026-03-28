# hello-espcx

[English](README.md) | [简体中文](README.zh-CN.md)

`hello-espcx` is a compact end-to-end BLE example built with Rust:

- `apps/ble/peripheral/` runs on an ESP32-C6 and exposes a Battery Service GATT server
- `apps/ble/central/` runs on a desktop machine and scans, connects, subscribes, and reads characteristic values

The current workflow is centered on ESP32-C6 firmware development plus a Windows BLE central application.

## Overview

The project is split into two executable crates:

- `apps/ble/common/`: shared BLE constants used by both applications
- `apps/ble/peripheral/`: embedded BLE peripheral based on `esp-hal`, `esp-radio`, `esp-rtos`, and `trouble-host`
- `apps/ble/central/`: desktop BLE central based on `btleplus` and `tokio`

Local path dependencies under `contrib/esp-hal/` are vendored through a git submodule, so the repository can be developed against a pinned embedded stack revision.

## Repository Layout

```text
hello_espcx/
|- apps/ble/common/     # Shared BLE constants
|- apps/ble/peripheral/ # ESP32-C6 BLE peripheral
|- apps/ble/central/    # Desktop BLE central application
|- crates/btleplus/   # Cross-platform BLE client library used by central
|- contrib/esp-hal/     # esp-rs submodule used as local dependencies
|- llm/                 # Reference code and upstream examples
|- justfile             # Common build and flash commands
|- README.md
|- README.zh-CN.md
|- AGENTS.md
`- CLAUDE.md
```

## Current BLE Contract

- Peripheral device name: `hello-espcx`
- Peripheral random address: `FF:8F:1A:05:E4:FF`
- Battery Level Characteristic UUID: `0x2A19`
- The peripheral periodically sends notifications and logs RSSI
- The central discovers the peripheral by name before subscribing and reading

If you change the advertised name, characteristic UUIDs, or notification flow, treat that as a cross-device change and update both sides together.

## Prerequisites

Recommended tools:

- Rust
- `just`
- a working `bash` executable
- `probe-rs`
- an ESP32-C6 development board
- a Bluetooth adapter on the desktop host

Initialize submodules after cloning:

```bash
git submodule update --init --recursive
```

Install the required Rust toolchain components:

```bash
just install
just check
just clippy
just hil-test-live
just hil-test
```

Note: the root `justfile` explicitly uses `bash` as its shell. On Windows, make sure `bash` is available through Git Bash, MSYS2, WSL, or an equivalent environment.

## Quick Start

### Build the peripheral

```bash
just build
```

Debug build:

```bash
just build-debug
```

### Flash the peripheral and view RTT logs

```bash
just flash
```

Debug build and flash:

```bash
just flash-debug
```

The peripheral uses `rtt-target`, so `probe-rs run` will stream `rprintln!` output directly after launch.

### Build and run the central application

```bash
just build-central
just run-central
```

Debug build:

```bash
just build-central-debug
```

## Verification Commands

Use the project-level helper commands when you want to validate both sides without relying on a mixed-target workspace check:

```bash
just check
just clippy
```

For real hardware-in-the-loop verification:

```bash
just hil-test-live   # test against an already running device
just hil-test        # build, download, reset, then run the HIL test
just hil-stress-live # run a 3-round real-hardware stress test
just hil-stress      # flash first, then run the 3-round stress test
```

The HIL test currently verifies:

- basic read/write on custom characteristics
- Battery Level notifications
- a 10 KiB central-to-peripheral bulk upload in 128-byte chunks with checksum validation
- a 10 KiB peripheral-to-central bulk notification stream in 128-byte chunks with checksum validation

The stress test additionally runs multiple rounds and prints per-round throughput. You can also override the bulk size and round count with:

```bash
HELLO_ESPCX_HIL_BYTES=10240
HELLO_ESPCX_HIL_ROUNDS=3
```

If you still want a larger run, for example 1 MiB, override it explicitly:

```bash
HELLO_ESPCX_HIL_BYTES=1048576 just hil-stress-live
```

## Cargo-First Workflow

If you prefer using Cargo directly:

```bash
# Peripheral
cd apps/ble/peripheral
cargo check --target riscv32imac-unknown-none-elf
cargo build --target riscv32imac-unknown-none-elf

# Central
cd ../central
cargo check
cargo run
```

## Toolchain and Platform Notes

### `apps/ble/peripheral/`

- uses the `nightly` toolchain via `apps/ble/peripheral/rust-toolchain.toml`
- targets `riscv32imac-unknown-none-elf`
- does not currently check in `apps/ble/peripheral/.cargo/config.toml`
- when running Cargo directly, pass `--target riscv32imac-unknown-none-elf` explicitly, or use the root `just` commands
- entry point: `apps/ble/peripheral/src/main.rs`
- GATT server definition: `apps/ble/peripheral/src/ble_bas_peripheral.rs`

### `apps/ble/central/`

- is a standard desktop Rust application
- is currently organized around a Windows BLE workflow
- entry point: `apps/ble/central/src/main.rs`

## IDE Notes

The repository includes `.vscode/settings.json` with a default rust-analyzer target of `riscv32imac-unknown-none-elf`.

That is convenient for embedded development, but diagnostics for `central/` may be less reliable inside the editor. When in doubt, trust `cargo check` from the `central/` directory.

Recommended VS Code extensions are listed in `.vscode/extensions.json`.

## Hardware Notes

The repository currently documents the following USB UART pins:

| Signal | GPIO |
| --- | --- |
| TX (USB UART) | 20 |
| RX (USB UART) | 19 |

## Verified Baseline

At the current repository state, the following commands pass:

```bash
cargo check -p hello-ble-central
cd apps/ble/peripheral && cargo check --target riscv32imac-unknown-none-elf
```

Hardware flashing, BLE discovery, and live notification flow should still be validated on real devices when you change runtime behavior.
