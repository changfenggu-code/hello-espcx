# Device Selection Roadmap

This roadmap covers the next evolution of `btleplus` for product-style BLE
workflows where one scan may discover many matching devices and the caller must
choose which one to connect to.

## Goal

Support this end-to-end flow cleanly:

1. Discover multiple matching peripherals.
2. Apply reusable selection strategies.
3. Choose one target peripheral.
4. Connect to it.
5. Enter GATT and operate normally.

Target usage:

```rust
let adapter = Adapter::default().await?;
let peripherals = adapter.discover(filter, timeout).await?;

let chosen = Selector::new()
    .prefer_connectable()
    .prefer_strongest_signal()
    .select(&peripherals)?;

let connection = chosen.connect().await?;
let gatt = connection.into_gatt().await?;
```

## Design Principles

- Keep GAP responsibilities in GAP.
- Keep GATT responsibilities in GATT.
- Treat `Peripheral` as one discovered device, not a collection.
- Put reusable strategy tools in the library.
- Keep final product policy in the application layer.

## Phase 1: Multi-device discovery

Objective:
Add a scan API that returns multiple peripherals instead of only the first
match.

Proposed API:

```rust
pub async fn discover(
    &self,
    filter: ScanFilter,
    timeout: Duration,
) -> Result<Vec<Peripheral>, BtleplusError>
```

Notes:

- Keep `Adapter::find(...)` as the "first matching device" convenience API.
- Implement deduplication by `PeripheralProperties.id`.
- Reuse the current scan/filter logic instead of creating a second path.

Files likely involved:

- `crates/btleplus/src/gap/adapter.rs`
- `crates/btleplus/docs/btleplus.md`
- `crates/btleplus/docs/btleplus.zh-CN.md`

## Phase 2: Richer peripheral metadata

Objective:
Expose enough pre-connection data so callers can make meaningful selection
decisions, while staying aligned with what the current peripheral actually
advertises.

### What the current peripheral already provides

Today the firmware advertises:

- complete local name (`hello-espcx`)
- battery service UUID (`0x180F`)
- standard BLE flags

And the scan path already gives us:

- platform device id
- RSSI
- connectable flag

So the library already has a solid first-pass base for selection.

### Current fields to keep

- `id`
- `local_name`
- `advertised_services`
- `rssi`
- `is_connectable`

### Phase 2A: Add address now

Recommendation:

- add `address`

Proposed shape:

```rust
pub address: Option<String>
```

Why this is the first priority:

- it matches BLE semantics more closely than the platform-specific `id`
- it is immediately useful for product/device lists
- it does not require changing the peripheral firmware first

Important distinction:

- `id` = platform-specific device identifier
- `address` = BLE address-like identity used in scanning/selection flows

### Phase 2B: Plan for manufacturer data

Recommendation:

- prepare to add `manufacturer_data`
- do not block Phase 2A on it

Rationale:

- this is the best place for device identity metadata in product workflows
- it is more suitable than `service_data` for selecting one device among many
- it requires peripheral advertising changes, so it should follow after the
  current metadata cleanup

Proposed first-pass shape:

```rust
pub manufacturer_data: Option<Vec<u8>>
```

Recommended payload direction for future firmware work:

- `version`
- `product_id`
- `unit_id`
- `flags`

Where:

- `version` identifies the layout version of the manufacturer payload
- `product_id` distinguishes product/board family
- `unit_id` distinguishes one physical device from another
- `flags` carries compact device-state bits

### Why not lead with service data

`service_data` is still valuable, but it is better suited to service-level
state or capability summaries than device identity.

For the current product goal, selecting one device from many, the preferred
priority is:

1. `address`
2. `manufacturer_data`
3. `service_data`

### Resulting Phase 2 recommendation

Do now:

- add `address`

Prepare next:

- add `manufacturer_data` after deciding the peripheral-side payload format

Defer for later:

- `service_data`
- product-specific parsing of advertisement payloads

Files likely involved:

- `crates/btleplus/src/gap/peripheral.rs`
- internal scan mapping in `crates/btleplus/src/gap/adapter.rs`
- later, peripheral advertising code in `apps/ble/peripheral`

## Phase 3: Selector builder

Objective:
Provide reusable selection logic in the library without hard-coding one product
policy.

Proposed module:

- `crates/btleplus/src/gap/selection.rs`

Proposed API sketch:

```rust
pub struct Selector { ... }

impl Selector {
    pub fn new() -> Self;
    pub fn prefer_connectable(self) -> Self;
    pub fn prefer_strongest_signal(self) -> Self;
    pub fn prefer_id(self, id: impl Into<String>) -> Self;
    pub fn prefer_local_name(self, name: impl Into<String>) -> Self;
    pub fn prefer_manufacturer_data<F>(self, f: F) -> Self;
    pub fn filter<F>(self, f: F) -> Self;
    pub fn select<'a>(&self, peripherals: &'a [Peripheral]) -> Result<&'a Peripheral, BtleplusError>;
}
```

First-pass strategy set:

- `prefer_connectable`
- `prefer_strongest_signal`
- `prefer_id`
- `prefer_local_name`
- generic `filter(...)`

## Phase 4: Selection semantics

Objective:
Make rule behavior explicit and predictable.

Semantics to standardize:

- `filter_*`: remove non-matching candidates entirely
- `prefer_*`: sort matching candidates earlier but keep fallbacks
- optional future `require_*`: fail if no candidate matches

Recommendation:

- First version ships with `filter_*` and `prefer_*` only
- Add `require_*` only if real product flows need it

## Phase 5: Center integration

Objective:
Move `hello-ble-central` from "connect the first match" to "discover, choose,
then connect".

Default first strategy:

1. discover all matching peripherals
2. prefer connectable devices
3. prefer strongest RSSI
4. connect the selected peripheral

Likely changes:

- `apps/ble/central/src/lib.rs`
- `apps/ble/central/docs/hello-ble-central.md`

## Phase 6: Error model

Objective:
Differentiate scan failures from selection failures.

Potential new errors:

- `NoMatchingPeripheral`
- `SelectionFailed(String)`
- optional `MultipleCandidates`

Recommendation:

- Add at least one explicit selection error in `BtleplusError`
- avoid over-modeling until the first selector version is in use

## Phase 7: Tests

Objective:
Make discovery and selection behavior stable before product logic depends on it.

Priority tests:

- `discover()` returns multiple matches
- deduplication by device id
- selector prefers strongest RSSI correctly
- selector prefers connectable devices correctly
- selector filters by predicate correctly
- empty result behavior
- no-match selection behavior

Files likely involved:

- new tests in `crates/btleplus`
- possibly helper-only tests around selector ranking

## Phase 8: Documentation

Objective:
Document both the simple and product-oriented flows.

Examples to add:

- connect the first matching device with `find(...)`
- discover many devices and select one with `Selector`
- guidance on what belongs in library policy vs app policy

Files likely involved:

- `crates/btleplus/docs/btleplus.md`
- `crates/btleplus/docs/btleplus.zh-CN.md`

## Recommended execution order

1. Add `Adapter::discover(...)`
2. Expand `PeripheralProperties` where needed
3. Add `Selector` builder
4. Integrate default selection strategy in central
5. Add tests
6. Update docs

## Out of scope for first iteration

- UI-driven manual selection
- persistent "last connected device" storage in the library
- continuous scan streams as the primary API
- complex multi-stage policy DSL

These can be added later once the basic discovery-plus-selection workflow is
stable.
