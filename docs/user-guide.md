# User guide

[简体中文](user-guide.zh-CN.md)

FLOW 8 PC Controller controls a physical FLOW 8 over its native Bluetooth LE protocol. It does not use USB MIDI for mixer control. The mixer surface is visible without a connection, but the values are placeholders and device controls remain disabled until synchronization finishes.

## Install and connect

On Windows, install FLOW 8 PC Controller and the separate [DirectHCI runtime](https://github.com/NanamiKite/DirectHCI). In DirectHCI Control Panel, start the service and prepare/select a supported Bluetooth controller. The FLOW installer does not install drivers or prepare the controller. For the current controller-acquisition path, run the FLOW GUI as administrator. See [Windows installation](installation.md) for details. On Linux, use the system Bluetooth service; this path has not yet been recorded as hardware-accepted.

1. Turn on the mixer and make it available for remote Bluetooth control. For a new client identity, enable **PAIR REMOTE / PAIR APP** on the mixer.
2. Open the application. Optionally click **Scan** while disconnected to look for `FLOW 8 LE`; then click **Connect**.
3. Wait for **Ready**. The app must receive and apply a complete mixer state before controls become active. Scanning/connecting/handshaking/syncing are not Ready.

The app creates a stable client identity on first use and reuses it later. Do not regenerate or share it casually. A new identity can require another mixer pairing action.

## Working with the mixer

The top navigation switches between **MIXER**, **STAGE**, **FX1**, **FX2**, **MON1**, **MON2**, **MAIN**, and **MAIN OUT**. The **Setup** menu opens **Preferences**, **Snapshots**, and **Routing**. Clicking a channel strip selects it for editing; use the destination tabs to choose which bus's levels you are viewing.

- **MIXER / input detail:** inspect and change input level, mute/solo, gain, HPF, EQ, compressor and applicable input controls. Only controls relevant to the selected input are available. Enabling 48 V phantom power asks for confirmation; check the connected equipment first.
- **MAIN / MON1 / MON2:** show each mix bus and its output controls. Bus selection does not silently change a physical output route.
- **FX1 / FX2:** select an effect preset and adjust the exposed effect parameters. Preset names and some parameter meanings still need device calibration; use the device-reported state as the confirmation.
- **Routing:** inspect the signal relationships and adjust the controls that are enabled for the active connection.
- **STAGE:** a visual view of channel and meter state. **MAIN OUT** is a signal-topology view, not a second set of bus controls.
- **Preferences:** language, UI scale, display choices, and supported device settings. Unsupported or unidentified device settings remain unavailable rather than sending guessed commands.

Requests may take time over Bluetooth. A pending value is not proof that the mixer accepted it. During a fader gesture and briefly after release, known intermediate echoes of that gesture may be absorbed while displaying its latest target. A different device value or a failed command ends this hold; the confirmed device state remains authoritative.

## Device snapshots

Open **Setup → Snapshots** and use **Refresh slots** if names have not yet appeared. The page shows device slots only; it does not maintain a PC-local snapshot library. Each slot may be shown as empty, occupied, or unknown until device information arrives.

**Save** stores the current mixer state in the selected device slot and may overwrite its previous contents. **Load** applies a stored snapshot to the live mixer. **Delete** removes a device snapshot, and **Rename** changes its device-stored name. Review the confirmation dialog before any of these actions; cancel if the slot or scope is not what you intended. Snapshot behavior is implemented, but the [validation table](validation.md) does not claim a full hardware acceptance for every snapshot operation.

## Disconnect and common problems

Click **Disconnect** before scanning again or closing a session. A new connection must sync state again; stale readings from the old session are not reused as device truth.

| Symptom | What to check |
|---|---|
| `FLOW 8 LE` is not found | Mixer power/remote availability, PC Bluetooth, and (on Windows) the DirectHCI controller selected in its Control Panel. |
| Windows backend initialization fails | Check DirectHCI service/controller status and follow its [troubleshooting documentation](https://github.com/NanamiKite/DirectHCI). FLOW 8 PC Controller cannot repair driver preparation. |
| Connection never reaches Ready | Leave the mixer in its pairing mode if this is a new client identity; review the app's connection error and log. A partial state sync does not enable controls. |
| Controls are disabled | Confirm the connection says Ready. The disconnected interface is intentionally read-only. |
| A control moves back | The mixer reported a different confirmed value or has not confirmed the pending write yet. Check the connection and device state before repeating the action. |

Normal Windows logs are under `%LOCALAPPDATA%\FLOW 8 PC Controller\logs`. Linux logs are under `$XDG_STATE_HOME/flow8-pc-controller`, or `~/.local/state/flow8-pc-controller` if `XDG_STATE_HOME` is unset. Detailed diagnostic logs are automatically cleaned up at startup and every hour: up to 7 days, 6 files and 96 MiB total are retained, excluding the current log. Copy a log elsewhere if you need to keep it. This does not delete hardware captures or the client identity file. Logs can contain mixer state and device identifiers; review them before sharing. For current hardware support claims, see [validation status](validation.md).
