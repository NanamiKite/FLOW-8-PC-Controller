# BLE transport and evidence

The Rust GUI starts disconnected. Device pages stay visible but their controls are disabled until a complete hardware state sync; values displayed before then are placeholders, not device readings. Synthetic fixtures are not exposed as a production connection mode. Generic/Linux BLE uses btleplug. The production Windows FLOW 8 connection uses the independent DirectHCI runtime through its Rust SDK and the thin `flow8-directhci` adapter. The Rust `windows_native.rs` path is retained only as an opt-in experiment.

## Verified FLOW 8 profile

| Item | Observation | Evidence |
|---|---|---|
| Advertisement name | `FLOW 8 LE` | real Windows/DirectHCI observation |
| Service | `14839ad4-8d7e-415c-9a42-167340cf2339` | APK and device discovery |
| Characteristic | `0034594a-a8e7-4b1a-a6b1-cd5243059a57` | APK and device discovery |
| Properties | `WRITE | NOTIFY` | real device |
| CCCD | absent; standard subscribe fails | Android HCI and Windows diagnostics |
| Notification behavior | unsolicited ATT notifications | Android HCI and DirectHCI |
| Example negotiated MTU | 131 | one observed device session; not a constant |

An observed characteristic handle is evidence, not a production constant. DirectHCI GATT discovery supplies the actual handle. FLOW 8's no-CCCD behavior is a device-profile choice in the FLOW adapter; DirectHCI remains generic.

## Windows production path

```text
flow8-gui → flow8-ble runtime → flow8-directhci
→ directhci-ble SDK → directhci-client IPC → directhcid
→ Bluetooth controller → FLOW 8
```

A GUI Scan uses the DirectHCI SDK and releases its temporary scan session before Connect. Connect creates one long-lived SDK BLE connection, discovers the FLOW UUID pair, arms passive `listen()` before the session handshake, and forwards every characteristic value to the common FLOW parser. Neither the adapter nor the daemon parses FLOW commands. No standard `subscribe()` or synthetic CCCD write is used.

One serialized WithResponse writer handles outbound FLOW frames while notification consumption continues independently. On disconnect, the listener and BLE connection are closed; controller ownership and recovery remain with `directhcid`. A repeated Connect must not replace an active session.

## Handshake and state

The session handshake is followed by fragmented initial mixer-state synchronization. The runtime uses the shared production codec for TX and decoder/reassembler for RX. A complete decoded MixerState is atomically applied before Ready. Fragment count and MTU are learned from actual frames rather than hard-coded.

A malformed or incomplete state cannot make the GUI Ready. Device-originated atomic changes update the same confirmed Store after Ready. Offline synthetic fixture events never count as device evidence.

After Ready, each connection requests channel labels. The response is decoded and applied to the existing confirmed Store, so device names/icons replace the corresponding input labels without a second UI state. A new Ready session requests labels again; this request is not part of the handshake or the Ready gate.

After Ready, the GUI requests meter updates for the conventional inputs and the currently displayed MAIN/MON/FX destination. Changing the displayed destination issues a new request without changing the device's physical output selection. Incoming updates use the existing protocol parser and confirmed Store for input and selected-output meters; the initial mixer state does not supply live meter levels. The GUI alone smooths painted input/output/Stage meter bars with fast attack and slower release; it never changes confirmed values or masks BLE delivery latency. The request is sent once per Ready session/output selection, not on every repaint. Whether one request starts a sustained device stream, and the exact output stereo flags for each FX destination, still require hardware validation; this code path is implemented but must not be labeled device-verified until a real request/update capture is observed.

For the two FX preset catalogs and their evidence levels, see [fx-presets.md](fx-presets.md).

For current observed outcomes and unresolved questions, see [validation.md](validation.md). The protocol implementation is summarized in [protocol.md](protocol.md).
