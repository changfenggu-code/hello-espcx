# easyble

Stage-oriented BLE peripheral helper library built on top of `trouble-host`.

## Overview

`easyble` exposes peripheral-side BLE concepts in two layers:

- `gap`: stack initialization, stack driving, advertising
- `gatt`: attribute-server binding, per-connection session driving

Recommended lifecycle path:

```text
gap::init
  -> gap::advertising
  -> gatt::connected
  -> gatt::session
  -> app-defined disconnected handling
  -> gap::advertising ...
```

This crate intentionally does not own the outer lifecycle loop. The app keeps
control of reconnect policy, fatal-error policy, product tasks, and session-end
handling.

## Quick Start

```rust
use embassy_futures::join::join;

let easyble::gap::InitializedStack {
    mut peripheral,
    runner,
} = easyble::gap::init::<_, 1, 2>(
    controller,
    easyble::gap::InitConfig {
        address: PERIPHERAL_ADDRESS,
    },
);

let advertisement = build_advertisement()?;
let server = build_server()?;

join(
    async {
        easyble::gap::run_stack(runner).await?;
        Ok::<(), _>(())
    },
    async {
        loop {
            let conn =
                easyble::gap::advertising(&mut peripheral, advertisement.as_view()).await?;
            let gatt = easyble::gatt::connected(conn, server)?;
            easyble::gatt::session(&gatt, |event| {
                // handle one GATT event
            })
            .await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), _>(())
    },
)
.await;
```

## Call Path

Recommended app-owned lifecycle flow:

```text
main.rs
  -> build_advertisement
  -> build_server
  -> gap::init
  -> gap::run_stack
  -> gap::advertising
  -> gatt::connected
  -> gatt::session
  -> app::custom_task
  -> disconnected handling in app
  -> gap::advertising ...
```

## One-line Memory Map

- `gap/init.rs`: build the host stack
- `gap/advertising.rs`: advertise once and accept one connection
- `gatt/connected.rs`: bind a raw connection to an `AttributeServer`
- `gatt/session.rs`: drive the passive GATT event loop
- `gap/mod.rs`: expose GAP-stage helpers
- `gatt/mod.rs`: expose GATT-stage helpers
- `lib.rs`: expose the crate structure

## Public API

Module-oriented surface:

```rust
pub mod gap;
pub mod gatt;
```

Use module paths instead of root-level re-exports:

```rust
easyble::gap::InitConfig
easyble::gap::InitializedStack
easyble::gap::AdvertisementData
easyble::gap::advertising(...)
easyble::gatt::connected(...)
easyble::gatt::session(...)
```

## GAP

### `InitConfig`

Initialization-time host configuration.

```rust
pub struct InitConfig {
    pub address: [u8; 6],
}
```

Current scope:

- fixed/random BLE address setup
- host resource allocation
- stack construction

### `InitializedStack`

Result of the init stage.

```rust
pub struct InitializedStack<C: Controller + 'static> {
    pub peripheral: Peripheral<'static, C, DefaultPacketPool>,
    pub runner: Runner<'static, C, DefaultPacketPool>,
}
```

The app typically splits these two pieces:

- `runner`: sent to `gap::run_stack`
- `peripheral`: reused across repeated `gap::advertising` calls

### `init`

Build the BLE host stack for the peripheral side.

```rust
pub fn init<C, const CONN: usize, const L2CAP: usize>(
    controller: C,
    config: InitConfig,
) -> InitializedStack<C>
where
    C: Controller + 'static
```

Important note:

- `HostResources` and `Stack` are leaked with `Box::leak`
- this is deliberate, so the stack and bound server can satisfy required
  lifetimes in embedded firmware

### `run_stack`

Drive the underlying `trouble-host` runner task.

```rust
pub async fn run_stack<C: Controller + 'static>(
    runner: Runner<'static, C, DefaultPacketPool>,
) -> Result<(), BleHostError<C::Error>>
```

Recommended usage:

- run in parallel with the app-owned lifecycle loop
- treat failure as fatal unless the product has a recovery policy

### `AdvertisementData`

Owned advertisement payload storage.

```rust
pub struct AdvertisementData {
    pub adv_data: [u8; 31],
    pub adv_len: usize,
    pub scan_data: [u8; 31],
    pub scan_len: usize,
}
```

Helper method:

```rust
pub fn as_view(&self) -> AdvertisementView<'_>
```

### `AdvertisementView`

Borrowed advertisement payload view used for one advertising attempt.

```rust
pub struct AdvertisementView<'a> {
    pub adv_data: &'a [u8],
    pub scan_data: &'a [u8],
}
```

### `advertising`

Run one advertising phase and wait for one incoming connection.

```rust
pub async fn advertising<'stack, C: Controller>(
    peripheral: &mut Peripheral<'stack, C, DefaultPacketPool>,
    data: AdvertisementView<'_>,
) -> Result<Connection<'stack, DefaultPacketPool>, BleHostError<C::Error>>
```

Behavior:

- starts connectable advertising
- waits for `accept()`
- returns one raw BLE `Connection`
- does not bind GATT automatically

## GATT

### `connected`

Bind an accepted raw BLE connection to an `AttributeServer`.

```rust
pub fn connected<
    'stack,
    'server,
    'values,
    M: RawMutex,
    const ATT_MAX: usize,
    const CCCD_MAX: usize,
    const CONN_MAX: usize,
>(
    conn: Connection<'stack, DefaultPacketPool>,
    server: &'server AttributeServer<'values, M, DefaultPacketPool, ATT_MAX, CCCD_MAX, CONN_MAX>,
) -> Result<GattConnection<'stack, 'server, DefaultPacketPool>, Error>
```

Boundary:

- this is the transition from GAP lifecycle to GATT lifecycle
- after this point, the app works with `GattConnection`

### `session`

Drive the passive GATT event loop for one connected session.

```rust
pub async fn session<P, F>(
    conn: &GattConnection<'_, '_, P>,
    on_event: F,
) -> Result<(), Error>
where
    P: PacketPool,
    F: for<'stack, 'server> FnMut(&GattEvent<'stack, 'server, P>)
```

Behavior:

- waits for GATT events from the connection
- dispatches each event through `on_event`
- automatically accepts and sends the GATT response
- exits when the connection disconnects

Important boundary:

- `session(...)` only drives passive GATT events
- active product tasks stay app-owned and should run in parallel

## Design Boundaries

What `easyble` should own:

- stack setup
- one advertising attempt
- raw-connection to GATT binding
- passive GATT event-loop mechanics

What `easyble` should not own:

- product advertisement semantics
- service definitions
- product-specific GATT event handling
- active tasks such as battery push / echo replay / bulk streaming
- outer reconnect loop
- disconnected policy

## Example With `hello-ble-peripheral`

```rust
let advertisement = build_advertisement()?;
let server = build_server()?;

let easyble::gap::InitializedStack {
    mut peripheral,
    runner,
} = easyble::gap::init::<_, 1, 2>(
    controller,
    easyble::gap::InitConfig {
        address: PERIPHERAL_ADDRESS,
    },
);

join(
    async {
        easyble::gap::run_stack(runner).await?;
        Ok::<(), _>(())
    },
    async {
        loop {
            let conn =
                easyble::gap::advertising(&mut peripheral, advertisement.as_view()).await?;
            let gatt_conn = easyble::gatt::connected(conn, server)?;

            join(
                run_product_session(&gatt_conn, server),
                custom_task(&gatt_conn, server),
            )
            .await;
        }
    },
)
.await;
```

## Notes

1. `easyble` is peripheral-side oriented; it is not a central/client library.
2. The crate follows lifecycle flow, not central-style object discovery flow.
3. The outer reconnect loop intentionally stays in the app layer.
4. `disconnected` handling is currently app-owned rather than a library phase.
5. The current API is intentionally narrow and optimized for embedded firmware integration.
