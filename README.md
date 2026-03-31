# hello-espcx

[English](README.md) | [简体中文](README.zh-CN.md)

`hello-espcx` is a Rust BLE end-to-end example with two runnable sides:

- `apps/ble/peripheral/` runs on ESP32-C6 and exposes a peripheral-side GATT server
- `apps/ble/central/` runs on desktop Windows and scans, connects, reads, writes, and subscribes

The repository is centered on one shared BLE contract plus two app crates that
exercise it from both sides.

## Overview

Main crates:

- `apps/ble/common/`: shared BLE constants, UUID namespaces, payload structs, and helpers
- `apps/ble/peripheral/`: ESP32-C6 firmware built on `esp-hal`, `esp-radio`, `esp-rtos`, `trouble-host`
- `apps/ble/central/`: desktop central application built on `btleplus` and `tokio`
- `crates/easyble/`: stage-oriented peripheral helper library used by the firmware
- `crates/btleplus/`: central-side BLE library used by the desktop app

Embedded dependencies are pinned through the `vendor/esp-hal/` git submodule.

## Repository Layout

```text
hello-espcx/
|- apps/ble/common/        # Shared BLE contract
|- apps/ble/peripheral/    # ESP32-C6 BLE peripheral firmware
|- apps/ble/central/       # Desktop BLE central application
|- crates/easyble/         # Peripheral-side lifecycle helpers
|- crates/btleplus/        # Central-side BLE library
|- vendor/esp-hal/         # Pinned esp-rs stack via git submodule
|- justfile                # Common build / flash / test commands
|- README.md
|- README.zh-CN.md
|- AGENTS.md
`- CLAUDE.md
```

## Current BLE Contract

- Peripheral advertised name: `hello-espcx`
- Peripheral fixed random address: `FF:8F:1A:05:E4:FF`
- Standard Battery Service UUID: `0x180F`
- Standard Battery Level Characteristic UUID: `0x2A19`
- The central discovers the target by advertised name and Battery Service
- Custom `echo`, `status`, `bulk`, and manufacturer identity payloads are shared in `apps/ble/common/`

If you change the advertised name, address, UUIDs, manufacturer payload format,
or notification behavior, treat that as a cross-side change and update both
apps together.

## Prerequisites

Recommended tools:

- Rust
- `just`
- a working `bash` executable for shell-oriented recipes in `justfile`
- `probe-rs`
- an ESP32-C6 development board
- a Bluetooth adapter on the desktop host

Initialize submodules after cloning:

```bash
git submodule update --init --recursive
```

Install the required Rust toolchain pieces:

```bash
just install
```

## Quick Start

### Build and flash the peripheral

```bash
just build
just flash
```

Debug build / flash:

```bash
just build-debug
just flash-debug
```

The firmware uses `rtt-target`, so `probe-rs run` will stream `rprintln!`
output after launch.

### Build and run the central

```bash
just build-central
just run-central
```

Debug build:

```bash
just build-central-debug
```

### Typical end-to-end flow

1. Flash the ESP32-C6 firmware with `just flash`
2. Start the desktop central with `just run-central`

## Verification Commands

Current `just` entry points:

```bash
just check          # host-side crates: common + central + btleplus + easyble
just check-all      # host-side crates + peripheral target check
just clippy         # host-side clippy set
just check-peripheral
```

Real hardware verification:

```bash
just hil-test-live   # run HIL against an already running board
just hil-test        # build, flash, reset, then run HIL
just hil-stress-live # 3-round live stress test
just hil-stress      # flash first, then run the 3-round stress test
```

The HIL tests currently cover:

- basic read/write on custom characteristics
- Battery Level notifications
- a 10 KiB central-to-peripheral bulk upload with integrity verification
- a 10 KiB peripheral-to-central bulk notification stream with integrity verification

You can override stress parameters explicitly:

```bash
HELLO_ESPCX_HIL_BYTES=10240
HELLO_ESPCX_HIL_ROUNDS=3
HELLO_ESPCX_HIL_BYTES=1048576 just hil-stress-live
```

## Cargo-First Workflow

If you prefer direct Cargo commands:

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

Important note:

- do not use root-level `cargo check --target riscv32imac-unknown-none-elf` as a substitute for firmware validation
- that tries to compile host-side crates like `central` / `btleplus` for the bare-metal target and will report `std`-related errors

## Toolchain and Architecture Notes

### `apps/ble/peripheral/`

- uses `nightly` via `apps/ble/peripheral/rust-toolchain.toml`
- targets `riscv32imac-unknown-none-elf`
- does not check in `apps/ble/peripheral/.cargo/config.toml`
- when using Cargo directly, pass `--target riscv32imac-unknown-none-elf`
- binary entry point: `apps/ble/peripheral/src/main.rs`
- product logic lives in `apps/ble/peripheral/src/lib.rs`
- the firmware lifecycle loop is app-owned and built around:
  `easyble::gap::init -> easyble::gap::advertising -> easyble::gatt::connected -> easyble::gatt::session`

### `apps/ble/central/`

- is a standard desktop Rust application
- is currently organized around a Windows BLE workflow
- binary entry point: `apps/ble/central/src/main.rs`
- session and product logic live in `apps/ble/central/src/lib.rs`

### `apps/ble/common/`

- holds the shared BLE contract
- UUIDs are organized by service module plus nested `service` / `characteristic` namespaces
- if you change these constants, you are changing both sides at once

### `crates/easyble/`

- is a peripheral-side lifecycle helper library
- detailed docs live under `crates/easyble/docs/`

## IDE Notes

The repository includes `.vscode/settings.json` with a default rust-analyzer
target of `riscv32imac-unknown-none-elf`.

That is convenient for embedded development, but diagnostics for `central/`
may be less reliable inside the editor. When in doubt, trust `cargo check`
from the relevant crate directory or `--manifest-path` command.

Recommended VS Code extensions are listed in `.vscode/extensions.json`.

## Hardware Notes

The repository currently documents the following USB UART pins:

| Signal | GPIO |
| --- | --- |
| TX (USB UART) | 20 |
| RX (USB UART) | 19 |

## Verified Baseline

At the current repository state, the following commands are the practical
baseline to return to:

```bash
cargo check --manifest-path apps/ble/common/Cargo.toml
cargo check --manifest-path apps/ble/central/Cargo.toml
cargo check -p easyble
cargo check --manifest-path apps/ble/peripheral/Cargo.toml --target riscv32imac-unknown-none-elf
```

Hardware flashing, discovery, and live notification flow should still be
validated on real devices when you change runtime behavior.
