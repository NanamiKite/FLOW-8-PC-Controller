# Hardware validation status

This page states what has actually been observed with a physical FLOW 8. It is not a hardware compatibility list. **Verified** below means an attributable user-supplied device run or capture, not universal firmware/controller support. The referenced raw Windows GUI capture is not distributed in this repository.

| Capability | Current result | Evidence boundary |
|---|---|---|
| Discover `FLOW 8 LE` and its FLOW GATT UUIDs | Verified on the reported Windows setup | Device scan and GATT discovery. |
| Connect and arm passive notifications | Verified on the reported DirectHCI setup | FLOW characteristic has `WRITE \| NOTIFY` and no CCCD; passive notifications received. |
| Native handshake | Verified on the reported setup | The device and host completed the session exchange. |
| Full initial MixerState and Ready | Verified on the reported setup | A complete fragmented state was decoded, atomically applied to Store, then Ready. |
| Control write and device echo | Partially verified | A completed WithResponse control write and device update were reported. This does not calibrate every parameter or control. |
| FX preset switching | Partially observed | A GUI run showed a preset write and subsequent device responses; the full preset catalog/name mapping is not calibrated. |
| Hardware control → live PC state | Not accepted as a complete milestone | Individual RX notifications have been observed, but the current ledger lacks a retained end-to-end action/state/UI acceptance record. |
| Meter request and sustained updates | Implemented; hardware acceptance pending | The request/update path exists, but stream timing and scale are not confirmed by a retained hardware run. |
| Device snapshot save/load/delete/rename | Implemented; operation-by-operation hardware acceptance pending | No complete retained record for all four actions and resulting device state. |
| GUI disconnect and reconnect | Partial | A client-identity hardware test reported Ready on two fresh connections; this is not a general GUI reconnect/recovery acceptance. |
| Linux/BlueZ backend | Implemented; hardware acceptance not recorded | Do not infer Linux support from offline compilation. |

## Observed FLOW 8 connection facts

The observed device advertises as `FLOW 8 LE`. Its service UUID is `14839ad4-8d7e-415c-9a42-167340cf2339`; its `WRITE | NOTIFY` characteristic UUID is `0034594a-a8e7-4b1a-a6b1-cd5243059a57`. Android HCI and Windows DirectHCI observations show unsolicited notifications without a standard CCCD subscription. One observed session negotiated an ATT MTU of 131 and received the initial state in several FLOW protocol fragments; neither value is hard-coded as a universal requirement.

Windows production BLE uses the independent [DirectHCI](https://github.com/NanamiKite/DirectHCI) runtime. The legacy WinRT/Win32 path and btleplug subscription failures are **not** alternative production acceptance evidence. The no-CCCD property remains relevant to current operation, even though old backend experiments are not part of this status page.

## Evidence rule

`VERIFIED_FROM_DEVICE` is reserved for physical-device observations. `VERIFIED_FROM_APK` (documented official-app behavior), `INFERRED`, implemented code, offline tests, and UI affordances are different claims. A user-supplied log excerpt proves only that reported run; absent source capture, firmware identification, or repetition must be stated as such. Do not promote a command's existence in the codec to hardware acceptance.

For the next safe, repeatable acceptance sequence and what to record, see [hardware test plan](hardware-test-plan.md). For the public protocol overview, see [protocol](protocol.md).
