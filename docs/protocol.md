# FLOW 8 protocol overview

FLOW 8 uses a proprietary Bluetooth Low Energy (BLE) protocol for mixer control, state synchronization, and device notifications. FLOW 8 PC Controller implements this native BLE protocol directly; USB MIDI is not its primary mixer-control path.

## Communication model

```text
Connect → session handshake → initial mixer-state synchronization
        → Ready → bidirectional live control
```

The transport carries characteristic values. The FLOW protocol layer parses incoming messages and encodes outgoing commands; the Store holds the confirmed mixer state shown by the GUI.

## Capabilities and evidence

| Capability | Public status |
|---|---|
| Session establishment | `VERIFIED_FROM_DEVICE` on the reported Windows setup. |
| Full initial mixer-state synchronization | `VERIFIED_FROM_DEVICE` for a complete state applied before Ready. |
| Incremental device notifications | Reception is `VERIFIED_FROM_DEVICE`; end-to-end coverage for every control is not claimed. |
| Mixer control writes | Selected write/response behavior is `VERIFIED_FROM_DEVICE`; this does not validate every parameter. |
| Snapshot operations | Implemented in the application; operation-by-operation hardware acceptance remains pending. |
| Fragmented state messages | Observed on a device; fragment count is not a protocol constant. |

`VERIFIED_FROM_DEVICE` denotes a physical-device observation. `VERIFIED_FROM_APK` denotes documented official-app behavior, not proof that this client's operation was accepted by hardware. `INFERRED` identifies an interpretation that still needs confirmation. These labels describe evidence; they are separate from whether a feature is implemented or exposed in the UI. See [validation](validation.md) for the scope and limits of current hardware observations.

## Implementation principles

- Reassembly follows message headers; the parser does not assume a fixed fragment count.
- Incomplete or invalid state never updates the confirmed Store or marks a session Ready.
- Unknown fields and unsupported behavior are not assigned guessed meanings.
- A receive-side schema does not authorize sending the same message.
- Protocol evidence, codec coverage, device acceptance, and UI availability are distinct claims.

For component responsibilities, see [architecture](architecture.md). For connection behavior, see [BLE transport](ble.md).
