# FLOW 8 PC Controller

This branch is the Rust implementation of FLOW 8 PC Controller. The former C++/Qt implementation is retained separately on the `c++` branch; its source and CMake build are not part of this branch.

```text
egui → semantic command → Flow8Store/queue → protocol encoder → BLE transport → FLOW 8
egui ← confirmed Store ← typed parser/reassembly ← raw BLE notifications ← FLOW 8
```

The device is authoritative: a device notification updates confirmed state even when it differs from a pending UI value. Simulator state is explicitly `SYNTHETIC`. Protocol evidence and real-hardware results remain separate in [the protocol record](docs/protocol.md) and [hardware ledger](docs/hardware-validation.md).

## Development

Rust 1.95+ and Cargo are required. Keep Cargo artifacts on a local filesystem when the source tree is on a VMware Shared Folder:

```bash
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo build --workspace
cargo run -p flow8-gui
```

The GUI starts in Simulator mode and does not need a Bluetooth adapter. Linux BLE uses btleplug/BlueZ. The Windows FLOW 8 path uses the external DirectHCI runtime through its Rust SDK; it does not need Qt or a C++ toolchain.

For Windows PowerShell, keep build artifacts off the shared folder:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
$env:FLOW8_CLIENT_UUID = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
$env:RUST_LOG = "flow8_ble=debug,flow8_directhci=debug"
cargo run -p flow8-gui
```

Use the paired 16-byte client UUID from your own FLOW 8 session; do not copy a captured response packet. During development, `directhcid` must already be available as an external service/process. FLOW 8 does not start it as a child or manage Windows drivers.

The workspace currently depends on the independent DirectHCI repository at `../DirectHCI` (the path is relative to this repository root). It links the `directhci-ble` SDK, not the daemon. A fresh checkout requires that sibling repository until the SDK has a versioned distribution. CI jobs require the `DIRECTHCI_REPOSITORY` repository variable to check out that sibling.

## Workspace

| Package | Role |
|---|---|
| `flow8-model` | Mixer model, evidence levels, parameter specifications |
| `flow8-protocol` | Packet framing, 31 target TX commands, typed RX, FIX8, `0x38` reassembly |
| `flow8-core` | Store, pending/confirmed reconciliation, semantic queue, Simulator |
| `flow8-directhci` | FLOW-specific device/UUID selection and passive-listen adapter over the generic SDK |
| `flow8-ble` | Platform transport boundary, session/handshake coordination, Tokio GUI channels |
| `flow8-gui` | egui desktop app; renders Store state and emits semantic intent |
| `flow8-cli` | Offline Gain/Route encoding utility |
| `flow8-hardware-bringup` | Explicit hardware diagnostics and safe manual controls |

`reference/flow-8-midi` is separate Rust research material, not a workspace package or authority for FLOW 8 BLE behavior. `tests/fixtures` preserves protocol/evidence data. Local hardware captures belong under ignored `captures/`.

## Windows BLE behavior

The physical FLOW 8 transport characteristic reports `WRITE | NOTIFY` but has no CCCD. The device sends notifications without standard subscription. On Windows, the production adapter uses DirectHCI's passive `listen()`, then forwards raw bytes unchanged to the FLOW protocol session. It does not call `subscribe()` or write a synthetic descriptor.

The verified handshake shape is `RX 0x35 → TX 0x39 → RX 0x36 → TX 0x37 → RX 0x38`. The production codec generates responses; the transport does not construct packets. `0x38` fragments are header-driven, not tied to an MTU or fixed fragment count. Ready requires a complete parsed mixer state and atomic Store application. The Rust-only `windows_native.rs` implementation is retained as an opt-in experimental backend, not the default.

The GUI's Scan and Connect use the same DirectHCI backend on Windows. Scan releases its temporary SDK session before Connect creates the long-lived one; duplicate Connect requests do not replace an active session. A DirectHCI session closes its listener and BLE connection on explicit disconnect, leaving controller ownership/recovery to `directhcid`.

## Product model

The Mixer presents seven conventional inputs into five destinations: MAIN, MON1, MON2, FX1 and FX2. MAIN/MON1/MON2 are independent Mix Buses; FX1/FX2 are engines/destinations. USB 1/2 and USB 3/4 are separate audio endpoints, not MON buses or extra rows of the 7×5 matrix. Mixer, Input Detail, MAIN/MON, FX, Routing, Stage, Snapshots and Settings render the same Store. Parameters whose real ranges or propagation are unknown remain marked by evidence rather than invented.

See [architecture](docs/architecture.md), [BLE](docs/ble.md), [state model](docs/state-model.md), [hardware validation](docs/hardware-validation.md), [USB MIDI](docs/midi.md), and the immutable [APK reverse-engineering record](docs/reverse-engineering.md). For the historical C++/Qt UI and implementation, use the separate `c++` branch.
