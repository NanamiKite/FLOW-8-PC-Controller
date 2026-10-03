# Windows installation

The Windows x64 FLOW 8 PC Controller installer installs the GUI only. It does not include DirectHCI, Bluetooth drivers, or controller preparation. Windows BLE control depends on the separately installed [DirectHCI runtime](https://github.com/NanamiKite/DirectHCI) and on that runtime's controller compatibility.

## Install

1. Download a published `FLOW-8-PC-Controller-<version>-windows-x64.exe` from this repository's release assets. If there is no release asset yet, the [development guide](DEVELOPMENT.md) describes building an installer; users do not need Cargo for a published installer.
2. Install DirectHCI separately. In its Control Panel, start the service and select a supported Bluetooth controller. If the panel says **Not prepared**, follow [DirectHCI's instructions](https://github.com/NanamiKite/DirectHCI) for that controller. FLOW 8 PC Controller cannot prepare or repair it by itself.
3. Run the FLOW installer and choose an installation directory. The default is under the current user's Programs directory; installation itself does not request elevation.
4. For the current DirectHCI controller-acquisition path, run the installed FLOW 8 PC Controller shortcut as administrator. This does not replace DirectHCI's own setup. Start the mixer, enable **PAIR REMOTE / PAIR APP** if this is a new client identity, and connect. Wait for Ready before making changes.

The installer can warn that the DirectHCI service is absent, but it does not start, configure or repair that service and does not switch a Windows Bluetooth driver.

## User data and logs

The app creates `%LOCALAPPDATA%\FLOW 8 PC Controller\client-id.txt` and reuses it across runs. It is a private client identity, separate from the device ID. Upgrading or uninstalling the GUI leaves this file in place. Deleting it generates a new identity and can require pairing again.

Normal GUI logs are under `%LOCALAPPDATA%\FLOW 8 PC Controller\logs\flow8-gui-*.log`. The latest 12 normal logs are retained. More detailed logs requested through `RUST_LOG` use separate files and are not automatically removed. Logs can contain mixer state or identifiers; review them before sharing. The installed release GUI has no console window.

For first use and common problems, see the [user guide](user-guide.md). If DirectHCI reports a controller, service or driver-preparation failure, use [DirectHCI's troubleshooting documentation](https://github.com/NanamiKite/DirectHCI) rather than FLOW protocol diagnostics.
