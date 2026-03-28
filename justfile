CHIP := "esp32c6"

default := "show"

# === Default ===

# Show all available recipes
show:
    @just --list

# Install tools and toolchain
install:
    rustup toolchain install nightly
    rustup target add riscv32imac-unknown-none-elf --toolchain nightly
    rustup component add rust-src --toolchain nightly
    cargo install cargo-binstall
    cargo binstall probe-rs-tools -y

# === Peripheral (requires riscv target) ===

# Kill any probe-rs processes that may be holding the device
kill-probe:
    @taskkill //F //IM probe-rs.exe 2>/dev/null || true

[working-directory: "apps/ble/peripheral"]
build:
    cargo build --release

[working-directory: "apps/ble/peripheral"]
build-debug:
    cargo build

[working-directory: "apps/ble/peripheral"]
check-peripheral:
    cargo check

# Burn: download only (no run)
[working-directory: "apps/ble/peripheral"]
burn: build kill-probe
    probe-rs download --chip {{CHIP}} target/riscv32imac-unknown-none-elf/release/hello-ble-peripheral

# Flash: download and run
[working-directory: "apps/ble/peripheral"]
flash: build kill-probe
    probe-rs run --chip {{CHIP}} target/riscv32imac-unknown-none-elf/release/hello-ble-peripheral

# Debug versions
[working-directory: "apps/ble/peripheral"]
burn-debug: build-debug kill-probe
    probe-rs download --chip {{CHIP}} target/riscv32imac-unknown-none-elf/debug/hello-ble-peripheral

[working-directory: "apps/ble/peripheral"]
flash-debug: build-debug kill-probe
    probe-rs run --chip {{CHIP}} target/riscv32imac-unknown-none-elf/debug/hello-ble-peripheral

[working-directory: "apps/ble/peripheral"]
clippy-peripheral:
    cargo clippy --no-deps -- -D warnings

# === Central ===

[working-directory: "apps/ble/central"]
build-central:
    cargo build --release

[working-directory: "apps/ble/central"]
build-central-debug:
    cargo build

[working-directory: "apps/ble/central"]
check-central:
    cargo check

clippy-central:
    cargo clippy -p hello-ble-central --all-targets --no-deps -- -D warnings

[working-directory: "apps/ble/central"]
run-central:
    cargo run

# === btleplus ===

[working-directory: "crates/btleplus"]
check-btleplus:
    cargo check

[working-directory: "crates/btleplus"]
clippy-btleplus:
    cargo clippy -- -D warnings

# === Hardware ===

# List connected debug probes
list-devices:
    probe-rs list

# List supported ESP chips
list-chips:
    probe-rs chip list | grep -i esp

# === Tests ===

# Check common workspace (common + central)
check:
    just check-central

# Check common + peripheral (requires riscv target)
check-all:
    just check-central
    just check-peripheral

clippy:
    just clippy-central
    just clippy-btleplus

hil-test-live:
    cargo test -p hello-ble-central --test hil_real esp32c6_end_to_end_hil -- --ignored --nocapture --test-threads=1

hil-stress-live:
    HELLO_ESPCX_HIL_ROUNDS=3 cargo test -p hello-ble-central --test hil_real esp32c6_bulk_stress_hil -- --ignored --nocapture --test-threads=1

hil-stress-live-5m:
    HELLO_ESPCX_HIL_BYTES=5242880 HELLO_ESPCX_HIL_ROUNDS=1 cargo test -p hello-ble-central --test hil_real esp32c6_bulk_stress_hil -- --ignored --nocapture --test-threads=1

[working-directory: "apps/ble/peripheral"]
hil-flash-debug:
    cargo build
    probe-rs download --chip {{CHIP}} --verify ../../../target/riscv32imac-unknown-none-elf/debug/hello-ble-peripheral
    probe-rs reset --chip {{CHIP}}
    sleep 2

hil-test:
    just hil-flash-debug
    just hil-test-live

hil-stress:
    just hil-flash-debug
    just hil-stress-live

