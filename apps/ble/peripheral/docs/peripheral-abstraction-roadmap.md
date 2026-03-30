# Peripheral Abstraction Roadmap

**Analysis Date:** 2026-03-30

## Overview

**Purpose:** Define a practical path for extracting reusable peripheral-side
BLE abstractions from `apps/ble/peripheral/` without forcing the firmware into
an over-generalized framework too early.

**Target outcome:**

- keep the current firmware behavior unchanged
- separate peripheral-side `gap` and `gatt` concerns more clearly
- keep product logic such as echo, status, and bulk flows in an app layer
- create a path toward a reusable crate only after the boundaries are proven

## Why This Is Different From `btleplus`

`btleplus` is a central-side library. Its main flow is:

```text
Adapter
  -> discover/find
  -> Peripheral
  -> Connection
  -> Client
```

That shape works because the central:

- discovers remote devices
- chooses one device to connect to
- discovers the remote GATT database at runtime

The peripheral side has different natural responsibilities:

```text
Host/Stack
  -> advertise
  -> accept connection
  -> run GATT server session
  -> execute product tasks
  -> disconnect
  -> advertise again
```

So we can still use the names `gap` and `gatt`, but the object model will not
be a mirror image of `btleplus`.

## Current Structure

**Current state:** The peripheral app has now been split into local `app/`,
`gap/`, `gatt/`, and `services/` modules inside `apps/ble/peripheral/src/`.
The main remaining work is not file splitting, but continuing to reduce product
coupling inside the runtime-facing layers.

Current responsibilities are distributed like this:

- `main.rs`: hardware/bootstrap entry
- `app/server.rs`: product-owned `Server` definition and server construction
- `app/advertising.rs`: product advertisement content and payload selection
- `app/session.rs`: product-specific GATT event matching
- `app/tasks.rs`: product-specific active tasks and bulk behavior
- `gap/advertising.rs`: advertise/accept mechanics using provided advertisement data
- `gap/peripheral_loop.rs`: outer peripheral lifecycle loop
- `gatt/session.rs`: generic connection-scoped GATT event loop driver

The service definitions themselves are already in a decent place under
`src/services/`:

- `battery.rs`
- `device_info.rs`
- `echo.rs`
- `status.rs`
- `bulk.rs`

Those files remain close to a reusable GATT service-definition layer.
What still needs refinement is the boundary between generic runtime mechanics
and product-specific wiring.

## Recommended Layering

Current peripheral-side structure:

```text
apps/ble/peripheral/src/
  main.rs
  app/
    mod.rs
    advertising.rs
    server.rs
    session.rs
    tasks.rs
  gap/
    mod.rs
    advertising.rs
    peripheral_loop.rs
  gatt/
    mod.rs
    session.rs
  services/
    battery.rs
    device_info.rs
    echo.rs
    status.rs
    bulk.rs
```

**Layer responsibilities:**

- `gap`
  - owns random/static address setup
  - builds advertising and scan-response payloads
  - starts advertising
  - accepts incoming connections
  - handles the reconnect-to-advertise loop
- `gatt`
  - owns reusable server-session mechanics, not product service lists
  - defines reusable helpers for server reads, writes, notify, and typed codecs
  - runs the connection-scoped GATT event loop
- `services`
  - holds service and characteristic definitions
  - stays app-owned in the current plan
  - should stay mostly declarative
- `app`
  - owns project-specific behavior
  - decides what to do on writes
  - owns active tasks such as battery tick, echo replay, and bulk streaming

## Peripheral `gap` Responsibilities

These responsibilities are valid peripheral-side `gap` concerns:

- local address policy
- advertising payload construction
- advertisement mode selection
- scannable/connectable policy
- accept connection lifecycle
- restarting advertising after disconnect

In the current codebase, these pieces mostly live in:

- `run(...)`
- `advertise(...)`
- the outer reconnect loop around `advertise(...).await`

This means yes, broadcasting absolutely belongs in peripheral-side `gap`.

## Peripheral `gatt` Responsibilities

These responsibilities are valid peripheral-side `gatt` concerns:

- generic GATT server-session mechanics
- service definitions and characteristic metadata
- dispatching reads and writes from the connection event stream
- typed payload encode/decode helpers
- notification helpers over known characteristics

In the current codebase, these pieces mostly live in:

- `#[gatt_server] struct Server`
- `gatt_events_task(...)`
- characteristic `get` / `set` / `notify` usage
- `postcard` helpers embedded in service and bulk logic

Important boundary:

- a generic library should not hardcode the product's service list
- the product-specific `#[gatt_server] struct Server` may stay app-owned even if
  the reusable GATT session driver moves into a generic layer

## What Should Stay App-Specific

These parts should not be pushed into a generic runtime too early:

- the whole `services/` layer in the current peripheral app
- the exact advertisement identity payload semantics
- battery tick behavior
- echo write-then-notify behavior
- status business rules
- bulk control commands and test-pattern streaming
- reset-on-fatal policy if we later need other recovery modes

These are real product behaviors, not generic peripheral infrastructure.

Service definitions are currently treated as product-owned building blocks, not
as the main target of generic extraction.

## Extraction Strategy

**Recommendation:** Use an app-local staged refactor first, then decide whether
the runtime pieces deserve a shared crate.

Use a staged approach instead of creating a new crate immediately.

### Phase 1: Local Module Split Inside the App

Goal:

- make the boundaries visible without changing behavior

Changes:

- split `ble_bas_peripheral.rs` into `gap`, `gatt`, and `app` modules
- keep everything inside `apps/ble/peripheral`
- keep the same `Server` type and same services
- move only code organization, not semantics
- prefer a shallow first cut: split files first, avoid interface redesign in
  this phase

Expected result:

- easier to see what is actually reusable
- lower risk than designing a shared crate too early

### Phase 2: Define Stable Internal APIs

Goal:

- reduce cross-module knowledge
- continue removing product-specific logic that still remains inside
  phase-1 `gap` and `gatt` modules

Changes:

- introduce small internal types for advertisement config and session context
- move typed serialization helpers into `gatt::codec`
- reduce direct handle-matching logic leaking into unrelated modules
- move product-specific advertisement field selection out of generic
  `gap/advertising.rs` helpers and into app-owned code
- move product-specific handle extraction and event matching out of
  `gatt/session.rs` and into app-owned session/handler code

Expected result:

- we can talk about reusable contracts rather than one file full of helpers

### Phase 3: Make App Behavior Explicit Via Handlers

Goal:

- isolate product logic from transport/runtime logic

Possible shape:

```rust
struct AppHooks { ... }

impl AppHooks {
    async fn on_gatt_event(...);
    async fn run_active_tasks(...);
}
```

Expected result:

- the runtime owns the session loop
- the app owns product decisions

### Phase 4: Evaluate a Reusable Crate

Goal:

- decide whether we have enough proven generic behavior to justify a shared
  peripheral runtime crate

Only do this if:

- a second peripheral app appears
- or the app-local modules have stayed stable long enough
- or we want tests/documentation around the runtime as a standalone unit

Possible crate names:

- `crates/bleperiph`
- `crates/trouble-peripheral-plus`
- `crates/ble-server-runtime`

The exact name should wait until the abstractions are proven.

## Proposed Target Call Path

A good end-state call path could look like:

```text
main.rs
  -> app::run()
       -> gap::build_stack()
       -> gatt::build_server()
       -> gap::run_peripheral_loop()
            -> gap::advertise_once()
            -> gatt::run_session()
                 -> app::on_event(...)
                 -> app::run_active_tasks(...)
```

This preserves the same conceptual separation that `btleplus` has, but with a
peripheral-native flow instead of a central-native flow.

## Refactor Progress

Completed moves:

- move advertisement payload building into `gap/advertising.rs`
- move outer advertise/accept/retry loop into `gap/peripheral_loop.rs`
- move `Server` into `app/server.rs`
- replace `gatt_events_task(...)` with a reusable `gatt::session::run_session(...)`
- move `custom_task(...)` and `run_bulk_stream(...)` into `app/tasks.rs`
- move product advertisement selection into `app/advertising.rs`
- move product-specific GATT event matching into `app/session.rs`
- introduce `app/runtime.rs` as the app-facing runtime bundle
- reduce `gap/peripheral_loop.rs` to a single app-facing runtime entry point
- make `gap/advertising.rs` return a plain BLE `Connection`
- move GATT server binding into `app/runtime.rs`
- make `gap/advertising.rs` consume a generic advertisement byte view rather
  than the app-owned advertisement struct

Remaining likely moves:

- introduce small config/context types for runtime wiring
- decide whether `app/runtime.rs` should stay a simple aggregation point or be
  split further in a later phase
- decide whether `gatt::codec` is needed after the next round of API cleanup

**Things to postpone:**

- creating traits for every service
- genericizing service definitions such as `BulkService` or `DeviceInfoService`
- moving anything into a new shared crate
- designing a public API before the internal split feels natural

**Phase 1 scope guard:**

- keep changes inside `apps/ble/peripheral/` where practical
- prefer not to modify `crates/`, `apps/ble/common/`, or other workspace areas
  during the first extraction pass
- use local module movement to prove boundaries before broader repo changes

**Phase 1 limitation to keep in mind:**

- after the first split, some files under `gap/` and `gatt/` may still contain
  product-specific logic
- this is acceptable for Phase 1 because the goal is structural separation, not
  final abstraction purity
- follow-up phases should keep pushing service-specific and advertisement-policy
  logic down into `app/`

## Roadmap

### Milestone A: Clarify Boundaries

**Status:** largely complete

- create `apps/ble/peripheral/docs/`
- document the target layering and responsibilities
- split the current file structure without behavior changes

Exit criteria:

- `ble_bas_peripheral.rs` is no longer the single owner of all runtime logic
- each module has a single clear reason to change

### Milestone B: Clean Internal Contracts

**Status:** in progress

- add small internal config/context types
- decide whether additional config/context types are still needed beyond
  `app::runtime::AppRuntime`
- decide whether `gatt::codec` is needed after the current split
- continue reducing product wiring duplication inside app-owned modules when it
  materially improves clarity

Exit criteria:

- app logic no longer directly owns generic transport details
- generic runtime code does not know product semantics

### Milestone C: App Hooks and Reusable Session Flow

- introduce explicit app hooks for event handling and background tasks
- reduce direct coupling between session loop and service-specific behavior
- parameterize product-specific standard-service values where needed, especially
  `DeviceInfoService`

Exit criteria:

- one runtime session loop can host multiple app behaviors
- adding a new service feels additive instead of invasive

### Milestone D: Decide on Shared Crate Extraction

- review whether the local abstractions are stable
- either extract a crate or intentionally keep the code app-local

Exit criteria:

- the decision is based on proven reuse, not aesthetic symmetry

## Non-Goals

- changing the over-the-air protocol
- changing UUIDs, names, or manufacturer payload format
- changing the no-std or ESP32-C6 platform assumptions
- extracting `services/` into a generic library in the current phase plan
- rewriting the service definitions into a heavily trait-driven framework
- mirroring `btleplus` API names one-for-one just for symmetry

## Known Abstraction Gaps

These are currently acceptable, but they are also the clearest targets for the
next rounds of cleanup.

- `gatt/session.rs` is already close to a reusable session runtime.
- `gap/advertising.rs` is now close to a generic advertise+accept helper, but
  its current API still assumes the project's fixed `DefaultPacketPool`.
- `app/runtime.rs` is now the main product wiring hub; if future cleanup is
  needed, this is the most likely place to split further.
- `services/` remains intentionally app-owned, so service-level cleanup is a
  product concern rather than a generic-runtime concern.

## Open Questions

These are the places where I would want your input before a larger refactor.

### 1. Do we want app-local modules first or a new crate immediately?

My recommendation:

- app-local modules first

Reason:

- we only have one peripheral app today
- the generic boundary is visible but not yet proven stable

### 2. How generic should advertising be?

Options:

- minimal: generic builder for adv/scAN response, app still chooses fields
- medium: reusable product-identity advertisement helper
- aggressive: full reusable advertisement policy layer

My recommendation:

- minimal to medium

### 3. Should `#[gatt_server] struct Server` stay app-owned or move into generic `gatt`?

Tension:

- moving it into `gatt` improves layering
- but the concrete list of services is still product-specific

My current lean:

- put the server assembly in `gatt`, but keep the concrete service selection
  app-owned

### 4. Do we want handler traits early?

Options:

- no, keep free functions first
- yes, introduce traits for event hooks now

My recommendation:

- no traits in phase 1
- consider hooks or traits only after the module split settles

### 5. Is bulk considered reusable infrastructure or app behavior?

My recommendation:

- keep it app behavior for now

Reason:

- the current control protocol and test-pattern semantics are specific to this
  project

### 6. Should fatal reset stay baked into the runtime?

This may be right for the current firmware, but if we later add richer
recovery behavior or test harnesses, we may want a pluggable failure policy.

My recommendation:

- keep reset behavior as-is during the first split
- isolate it behind one helper so the policy can change later

## Proposed Discussion Order

If we continue this design discussion, the best order is:

1. confirm app-local-first vs new-crate-first
2. confirm whether `Server` assembly should remain product-owned
3. decide whether bulk stays fully app-side
4. decide how much advertisement logic should be generalized

Once those four are settled, the first refactor plan becomes straightforward.

## Discussion Log

### Topic 1: App-Local Modules First vs New Crate First

**Status:** decided

**Recommendation:** Start with app-local modules first.

**Why:**

- we only have one peripheral app today
- the boundary is visible, but the reusable API is not stable yet
- an early crate extraction would force public API decisions too soon
- local refactor is cheaper to reverse if the split feels wrong

**What this means if accepted:**

- phase 1 and phase 2 stay inside `apps/ble/peripheral/src/`
- no new crate is added yet
- shared-crate evaluation moves to milestone D

**Decision:** accepted

**Notes:** We will prove the boundary inside the app first, then revisit crate
extraction only after the internal `gap/gatt/app` split has stabilized.

### Topic 2: Ownership of `#[gatt_server] struct Server`

**Status:** decided

**Current question:** should a generic layer own `#[gatt_server] struct Server`,
or should it only own the runtime/session mechanics around a product-owned
server type?

**Updated lean:** a generic library should not own the concrete product service
list; it should own the session/runtime mechanics around an app-owned server.

**Why this changed:**

- if the generic layer hardcodes `BatteryService`, `EchoService`, `BulkService`,
  it stops being genuinely generic
- adding a new product service would force edits to the generic library
- the `#[gatt_server]` macro produces a concrete compile-time server type, which
  fits app ownership more naturally than library ownership

**Decision:** accepted

**Notes:**

- `#[gatt_server] struct Server` and the service list stay app-owned
- the reusable layer owns GATT session/runtime mechanics around that server
- adding a new product service should primarily change app code, not the
  generic runtime layer

### Topic 3: Bulk as App Behavior vs Runtime Primitive

**Status:** decided

**Current lean:** keep bulk fully app-side for now.

**Decision:** accepted

**Notes:**

- the generic layer may eventually offer thin helpers for chunked notify/write
  patterns
- `BulkService`, `BulkControlCommand`, `BulkStats`, and stream semantics remain
  app-owned
- adding or changing bulk protocol behavior should not require generic runtime
  changes

### Topic 4: Advertising Generalization Level

**Status:** decided

**Current lean:** start with a minimal reusable advertising builder, not a full
policy framework.

**Decision:** accepted

**Notes:**

- the generic layer should help build advertisement and scan-response payloads
- product code still decides which AD structures to include
- manufacturer identity semantics, product flags, and advertisement policy stay
  app-owned
- we are explicitly not building a high-level product advertisement framework in
  phase 1

### Topic 5: Phase 1 Refactor Depth

**Status:** decided

**Options considered:**

- A: split files only, keep interfaces and call flow as stable as possible
- B: split files and also introduce small internal context/config types
- C: split files and introduce app hooks immediately

**Decision:** accepted option A

**Notes:**

- phase 1 is a structural extraction, not an interface redesign
- we should preserve current function signatures and call paths where practical
- interface cleanup and context types move to phase 2
- hook-style app/runtime boundaries move to phase 3

### Topic 6: Phase 1 Extraction Order

**Status:** decided

**Decision:** start with `gap`, then `gatt`, then `app`.

**Planned order:**

1. extract `gap/advertising.rs` and `gap/peripheral_loop.rs`
2. extract `gatt/session.rs`
3. extract `app/tasks.rs` and bulk-related app behavior

**Why this order:**

- `advertise(...)` and the outer reconnect loop have the clearest boundary
- GAP extraction depends least on product-specific server internals
- once GAP is separated, the remaining `gatt` and `app` boundary becomes easier
  to see
- this minimizes the risk of over-coupled moves in the first refactor pass

### Topic 7: `Server` Location and Phase 1 Change Scope

**Status:** decided

**Decision:**

- place `Server` in `apps/ble/peripheral/src/app/server.rs`
- keep Phase 1 changes scoped to `apps/ble/peripheral/`

**Notes:**

- this matches the accepted rule that `Server` and service selection are
  app-owned
- it keeps the first refactor local to the peripheral app instead of spreading
  across the repository
- if a later shared crate emerges, the extraction should start from proven local
  modules rather than from premature cross-crate moves

### Topic 8: Acceptable Product Coupling After Phase 1

**Status:** decided

**Decision:** accept temporary product coupling inside Phase 1 `gap/` and
`gatt/` modules, then reduce it in later phases.

**Notes:**

- Phase 1 is allowed to leave concrete advertisement field selection inside
  `gap/advertising.rs`
- Phase 1 is allowed to leave concrete service-handle extraction and event
  matching inside `gatt/session.rs`
- this does not mean those files are already generic; it only means the code
  has been structurally separated
- Phase 2 and Phase 3 should continue pushing product-specific logic into
  app-owned modules until the runtime layers are genuinely reusable

### Topic 9: Current Highest-Value Follow-Up Optimizations

**Status:** decided

**Decision:** treat the following as the next high-value cleanup targets:

- keep `gatt/session.rs` stable as the generic session driver
- evaluate whether `app/runtime.rs` should remain the long-term aggregation
  boundary
- only add more abstraction if it clearly reduces complexity rather than moving
  it around

**Notes:**

- `gatt/session.rs` already looks close to a reusable runtime driver
- `gap/peripheral_loop.rs` and `gap/advertising.rs` have already crossed the
  biggest cleanup thresholds from the earlier review
- `services/` stays app-owned in the current roadmap
- service value choices such as `DeviceInfoService` contents are owned by the
  product/application layer rather than the generic runtime layer

### Topic 10: `services/` Ownership

**Status:** decided

**Decision:** keep the whole `services/` layer app-owned and out of scope for
the current generic extraction effort.

**Notes:**

- runtime extraction currently targets `gap` lifecycle and `gatt` session
  mechanics
- `services/` may remain exactly where it is under `apps/ble/peripheral/src/`
- service values and service composition are product decisions
- we should not spend current-phase effort trying to make service definitions a
  generic library surface
