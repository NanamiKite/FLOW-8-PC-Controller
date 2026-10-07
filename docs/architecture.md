# Architecture

The main branch is the Rust application. The older C++/Qt implementation is on the separate `c++` branch.

```text
egui GUI ── semantic intent ──▶ Flow8Store / command queue
                                  │
                                  ▼
                           FLOW session / codec
                                  │ raw characteristic values
                                  ▼
                  transport ──────┬────── Linux: btleplug / BlueZ
                                 └────── Windows: flow8-directhci
                                          → directhci-ble SDK → directhcid

egui GUI ◀── confirmed Store ◀── typed RX / reassembly ◀── transport RX
```

The experimental `windows_native.rs` GATT backend remains in source but is not the Windows production default. DirectHCI is a separate runtime: its daemon owns controller acquisition, driver transitions, Raw HCI and recovery. FLOW 8 PC Controller only consumes the SDK and never owns a WinUSB or HCI handle.

## Responsibility boundaries

| Layer | Owns | Must not own |
|---|---|---|
| `flow8-gui` | Rendering, localization, user intent | BLE packets or a second copy of confirmed device truth |
| `flow8-model` / `flow8-core` | Product state, semantic commands, pending/confirmed reconciliation | Platform GATT operations |
| `flow8-protocol` | Framing, checksums, typed commands and reassembly | GUI and Bluetooth lifecycle |
| `flow8-ble` | Session, handshake decisions, transport interface, RX/TX tasks | WinUSB and FLOW codec duplication in a backend |
| `flow8-directhci` | FLOW device/UUID selection and passive notification forwarding | FLOW command parsing or DirectHCI daemon internals |
| `directhcid` (external) | Controller ownership, Raw HCI and recovery | FLOW 8 protocol and mixer state |

The FLOW characteristic has no CCCD in the observed device profile. The Windows adapter uses DirectHCI passive `listen()` before waiting for the first notification; it does not call standard `subscribe()`. Other platform transports may have a different RX setup, but all deliver raw values to the same FLOW parser and Store.

## Session and state

```text
Disconnected → Scanning → Connecting → TransportReady
→ session handshake → initial state synchronization
→ atomic Store apply → Ready
```

Only a complete valid initial MixerState applied to the Store permits Ready. The runtime is the live session-phase source; the GUI forwards its phase events to the Store rather than advancing the session on button presses or individual RX commands. The RX path keeps consuming notifications while one per-session WithResponse TX writer serializes writes. Device-reported state wins over pending GUI edits. Selecting MAIN/MON/FX changes the viewed mix destination; it does not silently select a physical output. Synthetic fixtures are offline-only, never presented as a production connection.

For the public protocol overview see [protocol](protocol.md), for observed hardware capability see [validation](validation.md), and for build/run setup see [DEVELOPMENT.md](DEVELOPMENT.md).
