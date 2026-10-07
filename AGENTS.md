# FLOW 8 PC Controller — main branch instructions

## Scope

This branch contains the active Rust application. The former C++/Qt implementation is kept on the separate `c++` branch. Do not reintroduce C++, Qt, CMake, or the old source tree here. Keep the product name `FLOW 8 PC Controller`.

Do not execute Git operations unless the user explicitly requests them. Preserve user data and hardware captures.

## Architecture

- `flow8-model`: transport- and GUI-independent product state, evidence, parameter metadata.
- `flow8-protocol`: FLOW packet codecs, framing, checksums, typed RX and fragment reassembly. No GUI or BLE dependency.
- `flow8-core`: authoritative Store, semantic commands, pending/confirmed reconciliation, deterministic Simulator.
- `flow8-ble`: Tokio session, transport boundary, handshake and GUI event channels.
- `flow8-directhci`: thin FLOW profile adapter over the external `directhci-ble` SDK.
- `flow8-gui`: egui rendering and semantic intents only; no raw packet construction.
- `flow8-cli` and `flow8-hardware-bringup`: offline and explicit hardware tooling.

FLOW 8's confirmed device state wins over pending local edits. Atomic RX, composite RX, simulator updates and GUI rendering use one Store. No GUI-local mirror of device truth.

## Evidence

Priority for device behavior: real FLOW 8 capture > documented official-app evidence > explicitly labelled inference. Never fabricate packet bytes, BLE UUIDs, parameter ranges, handshake results, or hardware verification. The old UI can be consulted on the `c++` branch for visual reference, not protocol truth. Keep local research notes independent and untracked unless the user specifically requests changes to them.

## BLE and DirectHCI

Linux/generic BLE uses btleplug. Windows FLOW 8 uses DirectHCI by default. The external `directhcid` owns controller takeover, Raw HCI and recovery; FLOW 8 uses only the Rust SDK and does not manage drivers or spawn the daemon. The FLOW characteristic has no CCCD; use passive `listen()`, not standard subscription, for the production Windows path. Do not hard-code the observed ATT handle or MTU.

Only a valid complete `0x38` mixer state applied to Store may mark the session Ready. The verified handshake shape is RX `0x35`, TX `0x39`, RX `0x36`, TX `0x37`, RX fragmented `0x38`. Keep protocol serialization in `flow8-protocol`.

## Build and validation

Source may live on a VMware Shared Folder, but Cargo targets must be on a platform-local filesystem:

```bash
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo build --workspace
cargo run -p flow8-gui
```

On Windows use `$env:CARGO_TARGET_DIR="$env:LOCALAPPDATA\flow8-rust-target"`. The independent DirectHCI SDK is pinned to a Git revision; a sibling checkout is not required and the controller runtime remains external. Hardware success cannot be claimed from compilation or offline tests; report exactly which Windows FLOW 8 milestones were observed.

Keep changes focused and verify proportionately. Do not run large test suites when the user explicitly asks for only compile checks.
