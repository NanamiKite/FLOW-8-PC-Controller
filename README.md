# FLOW 8 PC Controller

[简体中文](README.zh-CN.md)

An unofficial Rust desktop controller for the Behringer FLOW 8 digital mixer. It shows the mixer's reported state and provides mixer, routing, FX, and snapshot controls over Bluetooth.

This project is independent of Behringer and Music Tribe.

## Current status

The application is under active development. On a tested Windows/DirectHCI setup, real-device logs have reached the complete initial state sync and Ready state, and have shown control writes and device notifications. This is not a guarantee that every Windows Bluetooth controller or FLOW 8 firmware is supported. The Linux btleplug backend exists, but a Linux real-device acceptance result is not recorded here. See [hardware validation](docs/hardware-validation.md) for the evidence and remaining gaps.

The mixer interface is visible while disconnected; device controls become available only after a complete state sync.

## Windows installation

Versioned Windows x64 installers will be distributed as release assets. 

The FLOW 8 installer installs the GUI only. Windows Bluetooth control also requires the separately installed [DirectHCI runtime](https://github.com/NanamiKite/DirectHCI). Before connecting, start its service and select a supported controller in DirectHCI Control Panel; if the panel reports **Not prepared**, follow DirectHCI's controller preparation instructions. The installer does not do this setup. The current DirectHCI SDK also requires administrator rights when the GUI acquires a controller session. See [Windows installation](docs/installation.md).

On the first connection with a new client identity, enable **PAIR REMOTE / PAIR APP** on the mixer, then connect in the GUI. The application creates and reuses a client identity at `%LOCALAPPDATA%\FLOW 8 PC Controller\client-id.txt`. Keep this file private; deleting it creates a new identity and may require pairing again. Normal GUI logs are written under `%LOCALAPPDATA%\FLOW 8 PC Controller\logs`.

## Build from source

Rust 1.95 or newer is required. The workspace currently uses a path dependency on the independent DirectHCI Rust SDK, so place the two source checkouts side by side:

```text
parent/
├── FLOW 8 PC Controller/
└── DirectHCI/
```

From this repository:

```sh
cargo run -p flow8-gui
```

Windows needs the external DirectHCI runtime; Linux uses the system Bluetooth stack through BlueZ/btleplug. If the source is on a VMware shared folder, keep Cargo build artifacts on a local filesystem. Platform setup, build commands, and validation guidance are in [DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Documentation

- [Windows installer and prerequisites](docs/installation.md)
- [Development setup](docs/DEVELOPMENT.md)
- [Architecture](docs/architecture.md)
- [Protocol implementation](docs/protocol.md)
- [BLE transport](docs/ble.md)
- [Hardware validation](docs/hardware-validation.md)

The historical C++/Qt version is kept on the separate `c++` branch; this branch contains the Rust application.

## License

FLOW 8 PC Controller is licensed under GPL-3.0-only. See [LICENSE.txt](LICENSE.txt). DirectHCI is a separate project with its own distribution and dependency notices.
