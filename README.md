# FLOW 8 PC Controller

[简体中文](README.zh-CN.md)

An unofficial native FLOW 8 BLE desktop client for the Behringer FLOW 8 digital mixer. It connects over Bluetooth LE, synchronizes the mixer's state, and sends control changes through the FLOW 8 protocol. **This is not a USB-MIDI-based controller.**

This project is independent of Behringer and Music Tribe.

## Screenshots

![FLOW 8 PC Controller mixer view](docs/images/mixer.png)

| Input channel | Output details | FX settings |
| :---: | :---: | :---: |
| <img src="docs/images/channel-details.png" alt="Input channel configuration" width="260"> | <img src="docs/images/output-details.png" alt="Output settings" width="260"> | <img src="docs/images/fx-settings.png" alt="FX settings" width="260"> |

| Routing | Device snapshots |
| :---: | :---: |
| <img src="docs/images/routing.png" alt="Routing view" width="430"> | <img src="docs/images/snapshots.png" alt="Device snapshots" width="430"> |

## Key features

- Native BLE connection, handshake, full mixer-state sync, and live device notifications.
- Mixer layers for MAIN, MON1/2, and FX1/2, with input editing, routing, stage view, and device snapshots.
- English and Simplified Chinese interface. The mixer remains visible while disconnected; controls unlock after state sync reaches Ready.
- On Windows, the separately installed [DirectHCI runtime](https://github.com/NanamiKite/DirectHCI) provides Bluetooth access. Linux uses the system Bluetooth stack through BlueZ/btleplug.

Hardware acceptance and remaining calibration limits are recorded in the [validation summary](docs/validation.md); a visible control does not mean every operation has been hardware-accepted.

## Download and installation

Download the Windows x64 installer from this repository's [Releases](../../releases). The FLOW 8 installer contains the GUI, **not** DirectHCI or Bluetooth drivers. Windows users must install and prepare DirectHCI separately; see the [installation guide](docs/installation.md). Controller compatibility depends on DirectHCI and the individual PC.

## Quick start

1. On Windows, start DirectHCI in its Control Panel and prepare/select a supported Bluetooth controller. On Linux, ensure the system Bluetooth service is available.
2. Start FLOW 8 PC Controller. On Windows, the current DirectHCI acquisition path requires **Run as administrator**. For a new client identity, enable **PAIR REMOTE / PAIR APP** on the mixer.
3. Click **Connect**. The app scans for `FLOW 8 LE` as part of connecting; **Scan** is optional if you only want to view nearby devices first. Wait for **Ready** before changing mixer controls.

The first connection creates a reusable client identity in your user profile; do not delete it casually. The [user guide](docs/user-guide.md) covers the pages, snapshots, disconnect/reconnect, and common problems.


## Architecture and development

The Rust GUI sends semantic operations to one Store and session runtime. A shared FLOW codec handles packets; platform transports move raw characteristic values. Windows defaults to the DirectHCI SDK and external `directhcid`; FLOW 8 PC Controller does not manage Windows drivers or Raw HCI. See [architecture](docs/architecture.md), [protocol](docs/protocol.md), and [development setup](docs/DEVELOPMENT.md).

The historical C++/Qt implementation is on the separate `c++` branch. This branch is the Rust application.

## License

GPL-3.0-only; see [LICENSE.txt](LICENSE.txt). DirectHCI is a separate project with its own distribution and dependency notices.
