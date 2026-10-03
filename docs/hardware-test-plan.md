# FLOW 8 hardware test plan

This is a manual acceptance checklist for the production application. It does not require an independent BLE probe, experimental Windows backend, or direct controller operations from the FLOW project. Record the app version, FLOW 8 firmware if known, OS, Bluetooth controller, DirectHCI version on Windows, time, and the outcome of each step. Keep raw logs private until reviewed for device state or client identifiers.

## Preconditions

- A physical FLOW 8 is powered and available for remote Bluetooth control.
- Windows: DirectHCI is installed, its service is running, and a supported controller is prepared/selected in DirectHCI Control Panel. FLOW 8 PC Controller does not switch drivers.
- A new client identity has been authorized with **PAIR REMOTE / PAIR APP** on the mixer. Preserve the existing identity file between reconnect attempts.
- No other app is simultaneously controlling the same FLOW 8.

## Production acceptance

1. Start the real GUI and, while disconnected, confirm the mixer page remains visible but device controls are disabled.
2. Click **Scan**, then **Connect**. Record whether `FLOW 8 LE` is found and which backend/error is shown.
3. Without touching any mixer control, wait for the production chain: target characteristic selected → passive listener active → session handshake → complete initial mixer state → Store apply → Ready. Fragment count is determined by the received message, not prescribed by the test.
4. At Ready, compare a few displayed values to the physical mixer before making a change. This checks initial synchronization, not just protocol parsing.
5. Make one safe, reversible GUI change such as Mute on an appropriate channel. Record GUI intent, TX completion, physical result, RX confirmation, and final displayed value; restore the original setting.
6. Change a hardware control and record whether the GUI updates without a PC-originated command. This is the separate hardware-to-PC live-sync check.
7. Disconnect, then reconnect. Confirm stale values do not become authoritative before a new full state sync. Observe whether the controller returns to its prior ownership state when the FLOW session ends, as appropriate to DirectHCI's lifecycle.

Meter requests, FX catalog calibration, snapshot mutation, output routing changes, phantom power, and factory reset are **not** part of this baseline sequence. Test them separately, with explicit recording of original settings and a recovery plan. Do not run destructive operations merely to fill a status table.

## Result record

For each item, record **pass / fail / not run**, the first failing stage or command, whether the device physically changed, and whether the app's confirmed Store matched the device. Keep a distinction between transport write success and device acceptance. Update [validation status](validation.md) only for attributable real-device evidence; an offline build or simulator result does not count.
