# Development

This document describes the development environment, repository layout, build workflow, Bluetooth backend setup, testing conventions, and local hardware workflow for the Rust implementation of FLOW 8 PC Controller.

Protocol details, architecture decisions, and hardware validation results are documented separately under `docs/`.

## Requirements

- Rust 1.95 or newer
- Cargo
- A supported Bluetooth environment:
  - Linux: BlueZ / btleplug
  - Windows: DirectHCI runtime
- A physical FLOW 8 is required for hardware validation

The current Rust branch does not require Qt or a C++ toolchain.

## Repository layout

The Rust workspace contains:

| Package | Purpose |
|---|---|
| `flow8-model` | Mixer model, parameter specifications, evidence metadata |
| `flow8-protocol` | Packet framing, encoding, typed RX parsing, numeric conversion, state-message reassembly |
| `flow8-core` | Store, pending/confirmed state, semantic command handling, deterministic fixtures |
| `flow8-directhci` | FLOW-specific adapter over the generic DirectHCI BLE SDK |
| `flow8-ble` | Platform transport boundary, BLE session coordination, handshake and runtime channels |
| `flow8-gui` | egui desktop application |
| `flow8-cli` | Small offline protocol utility |
| `flow8-hardware-bringup` | Explicit hardware diagnostics and manual hardware testing |

Research/reference material is kept outside the workspace.

Examples:

```text
reference/
captures/
```

`reference/flow-8-midi` is research material only and is not authoritative for FLOW 8 BLE behavior.

Local hardware captures should remain under the ignored `captures/` directory and should not be committed accidentally. Directories named `tests/`, including local fixtures and hardware tests, are also ignored by Git.

## DirectHCI SDK dependency

The Windows adapter uses `directhci-ble` from the independent DirectHCI
repository, pinned to commit `8e3c620dbee3663bc5dd8aef483941c8bf332333`.
Cargo fetches and caches the SDK; a sibling checkout is no longer required.
Keep `Cargo.lock` with the application so all builds resolve the same SDK and
dependencies.

This revision moves native worker-thread joining off the caller's async
executor. Existing `disconnect()` and `shutdown()` calls still await cleanup;
this does not make controller release instantaneous or guarantee Windows
Bluetooth restoration after an error.

The adapter and SDK compile only on Windows. Linux uses btleplug; Cargo may
still fetch the pinned source while resolving the cross-platform lockfile.
Offline builds require the dependencies to have been cached previously.

FLOW 8 PC Controller does not embed or own the DirectHCI daemon.

The expected process boundary is:

```text
FLOW 8 PC Controller
        ↓
flow8-directhci
        ↓
directhci-ble SDK
        ↓
directhci-client
        ↓
Named Pipe
        ↓
directhcid
```

The DirectHCI repository remains independently buildable and independently versioned.

Change the pinned revision deliberately when integrating an SDK update. This
does not install or start `directhcid`. If SDK development requires local
source changes, use a private Cargo patch override rather than changing the
default dependency back to a machine-specific path.

## Building

### Linux

On Debian/Ubuntu, install the build dependencies used by btleplug and the
platform device libraries:

```bash
sudo apt-get install pkg-config libdbus-1-dev libudev-dev
```

BlueZ must be available for hardware use. A running graphical session is
needed to launch the GUI, not for `cargo check`.

When working from a normal local filesystem:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo build --workspace
cargo run -p flow8-gui
```

### VMware Shared Folder

Do not place Cargo build artifacts inside the VMware shared folder.

Use a local target directory:

```bash
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
```

Then build normally:

```bash
cargo check --workspace
cargo run -p flow8-gui
```

Heavy build artifacts should remain outside the shared source tree.

### Windows PowerShell

When the repository is opened from a VMware shared folder, keep the target directory on the Windows local filesystem:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
```

Typical development environment:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
$env:RUST_LOG = "flow8_ble=debug,flow8_directhci=debug"

cargo run -p flow8-gui
```

Do not generate a new FLOW 8 client UUID on every run.

The client identity must remain stable for a paired FLOW 8 session.

### Linux to Windows GNU compile checks

```bash
sudo apt-get install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
cargo check -p flow8-gui --target x86_64-pc-windows-gnu
```

The Windows icon resource needs `windres` and `ar` even for `cargo check`.
`WINDRES` and `AR` can specify explicit tool paths. `WINDRES_PREPROCESSOR`
can select a standalone `cpp` for resource-only cross-checks. On a Windows
GNU host, build normally without adding `--target`; the installer script
uses the host toolchain.

## Bluetooth backends

### Linux

Linux uses the normal system Bluetooth stack through btleplug / BlueZ.

No DirectHCI runtime is required.

### Windows

The production Windows backend uses DirectHCI.

FLOW 8 PC Controller should not:

- manage Windows drivers directly
- manipulate WinUSB directly
- enumerate HCI controllers itself
- start `directhcid` as an ordinary child process

The application only consumes the DirectHCI SDK.

During development, `directhcid` must already be available as an external
process or Windows service. Use the separately installed DirectHCI Control
Panel to start and prepare the service; the FLOW project does not assume an
installation path for its executables. Running a console daemon from a
DirectHCI development checkout is an option only for DirectHCI developers.
Controller acquisition currently requires an appropriately elevated FLOW
client.

### Experimental Windows native backend

The Rust-only Windows native backend is retained for experimentation and regression work.

It is not the production default.

If enabled through an environment override, document the exact override in the relevant issue or private investigation record. Do not present an experimental run as production validation.

On Windows, `FLOW8_BLE_BACKEND=windows-native` (or `native`) selects this legacy
backend; leaving it unset uses DirectHCI. The override has no effect on Linux.
`FLOW8_DIRECTHCI_ADDRESS` optionally selects a specific advertised FLOW 8
address, for example `random:D9:5A:15:4B:FD:1B` or `public:AA:BB:CC:DD:EE:FF`.
It selects the BLE peripheral, not the PC's Bluetooth controller. Usually both
overrides should be left unset.

Do not silently fall back to it when DirectHCI fails.

## FLOW 8 client identity

FLOW 8 uses an application-level client identity during its remote-control handshake.
This is separate from ordinary Windows BLE pairing and from the device identity in the session handshake.

On first launch, the application generates a random UUIDv4 and persists it before
sending any handshake response. Later launches and reconnects reuse the same file:

- Windows: `%LOCALAPPDATA%\FLOW 8 PC Controller\client-id.txt`
- Linux: `${XDG_CONFIG_HOME:-$HOME/.config}/flow8-pc-controller/client-id.txt`

No `FLOW8_CLIENT_UUID` environment variable is needed for the GUI. To authorize
a new identity, use the mixer's PAIR REMOTE / PAIR APP flow. A failed or corrupt
identity file stops connection rather than silently generating a replacement.
Treat the file as a client credential; keep it private and back it up if needed.

The ordinary `flow8-hardware-bringup` handshake, state and control commands use
the same production runtime and identity file as the GUI. They do not invent a
separate client identity. Control commands require `--confirm-device-write` and
wait for Ready before writing. A successful transport write is reported
separately from a device state confirmation.

On Windows, the existing `subscribe` observation command uses the DirectHCI
passive listener without writing a CCCD or sending a FLOW handshake. A capture
without `--handshake` is passive as well. Captures with a handshake consume the
production runtime's raw RX events and redact client-identity TX bytes. Existing
WinRT and ordering experiments are historical diagnostics, not production
connection paths.

Local hardware comparison tests, if present in an existing development checkout,
may accept `FLOW8_CLIENT_UUID` for a known-ID control case. Those ignored files
are not included in a fresh clone and do not configure the production GUI.

## Runtime expectations

The GUI starts disconnected.

The mixer UI may be rendered before a device is connected, but placeholder values are not authoritative device state.

Controls that depend on confirmed mixer state should remain disabled until a complete device state has been synchronized.

The runtime should preserve the following separation:

```text
GUI intent
    ↓
SemanticCommand
    ↓
session / command handling
    ↓
protocol encoder
    ↓
BLE transport
```

and:

```text
BLE notification
    ↓
typed parser / reassembly
    ↓
confirmed Store
    ↓
GUI
```

The physical mixer remains the source of truth.

Do not make GUI-local state authoritative simply to make controls appear responsive.

## DirectHCI session lifecycle

A FLOW DirectHCI session should:

1. acquire a BLE-capable DirectHCI session
2. scan or connect to the target FLOW 8
3. discover the target service and characteristic
4. establish passive notification listening
5. run the FLOW handshake and state synchronization
6. keep RX and TX alive for the entire session
7. disconnect cleanly on user request or application shutdown

FLOW 8 PC Controller is not responsible for restoring Windows Bluetooth drivers.

Driver ownership and restoration belong to `directhcid`.

The daemon may remain running after FLOW 8 PC Controller exits.

## Logging

Useful development targets include:

```powershell
$env:RUST_LOG = "flow8_ble=debug,flow8_directhci=debug"
```

For deeper transport debugging:

```powershell
$env:RUST_LOG = "flow8_ble=trace,flow8_directhci=trace"
```

Trace logging is intentionally verbose and should not be treated as a normal production configuration.

Setting `flow8_ble` or `flow8_directhci` to `debug` or `trace` also writes a detailed log file. On Windows, these files are under `%LOCALAPPDATA%\FLOW 8 PC Controller\logs`; on Linux, they are under `$XDG_STATE_HOME/flow8-pc-controller`, or `~/.local/state/flow8-pc-controller` when `XDG_STATE_HOME` is unset. Detailed logs are cleaned up on startup and every hour. Retention is 7 days, at most 6 files and at most 96 MiB total; the current file is excluded and capped at 16 MiB. Hardware captures and client identity files are not part of this cleanup. Copy logs needed for an investigation elsewhere before they expire.

When reporting a hardware failure, preserve the smallest useful section covering:

```text
connect
→ handshake
→ state sync
→ failing operation
→ disconnect / recovery
```

Avoid pasting unrelated startup/build output unless relevant.

## Testing policy

Do not run the full test suite after every small edit.

Use the smallest check that gives useful confidence for the change.

Typical lightweight checks:

```bash
cargo fmt --all -- --check
cargo check -p flow8-protocol
cargo check -p flow8-core
cargo check -p flow8-ble
cargo check -p flow8-gui
```

Use broader checks for milestone changes:

```bash
cargo check --workspace
cargo test --workspace
```

Windows-target checks may be run from Linux when useful:

```bash
cargo check -p flow8-ble -p flow8-gui --target x86_64-pc-windows-gnu
```

Do not run Clippy sweeps or full hardware tests for every incremental change unless explicitly required.

## Offline tests

Offline tests should use deterministic fixtures.

Synthetic fixture data may be used for:

- parser tests
- encoder tests
- state reconciliation tests
- UI-independent Store tests
- fragment reassembly tests

Synthetic data must not leak into the production GUI as if it were real hardware state.

The production application does not have a Simulator device source.

## Hardware validation

Real-hardware behavior must be recorded separately from inferred or reverse-engineered behavior.

Use [validation.md](validation.md) for current results and
[hardware-test-plan.md](hardware-test-plan.md) for the manual acceptance
checklist. Record results such as:

- successful connection
- actual ATT MTU
- handshake behavior
- real notification sequences
- packet fragmentation
- confirmed command behavior
- reconnect behavior
- Windows-specific results

Do not promote an inferred protocol behavior to "verified" without physical-device evidence.

## Protocol and evidence records

Keep protocol facts separate from implementation details.

Use:

```text
docs/protocol.md
docs/ble.md
docs/validation.md
docs/state-model.md
```

for:

- the public protocol overview and implementation principles
- BLE transport behavior
- observed hardware capabilities and evidence levels
- known unknowns
- state model semantics

Do not copy APK source code into production implementation unless legally and technically appropriate.

Prefer independent protocol descriptions and independently written Rust implementations.

## Captures

Local packet captures may contain device-specific identifiers or other private information.

Store them under:

```text
captures/
```

and keep that directory ignored unless a capture has been intentionally sanitized for publication.

Public fixtures should contain only the minimum data required for deterministic tests or documentation.

## CI

[GitHub Actions](../.github/workflows/build.yml) checks out this repository.
Cargo resolves the SDK revision pinned in the adapter manifest and lockfile;
CI does not need a separate DirectHCI checkout.

Pushes to `main`, pull requests, and manual runs check formatting, the locked
workspace, and the workspace build on Linux and Windows. CI does not start
`directhcid`, access FLOW 8 hardware, package an installer, or run software tests.
Hardware acceptance remains a separate manual step.

## Development boundaries

Do not move FLOW-specific logic into DirectHCI.

DirectHCI must not contain:

- FLOW 8 UUIDs
- FLOW 8 command IDs
- FLOW handshake behavior
- mixer state parsing
- FLOW-specific packet formats

Likewise, FLOW 8 PC Controller should not contain:

- WinUSB ownership logic
- BTHUSB driver switching
- raw controller recovery logic
- DirectHCI daemon internals

The integration boundary should remain a thin BLE adapter.

## Windows packaging boundary

The FLOW installer packages the GUI. DirectHCI has its own installer and service; the current FLOW SDK connects to a running service rather than starting it. DirectHCI Control Panel must start the service and select and, if necessary, prepare a supported controller. The current controller-acquisition path requires administrator rights for the FLOW client. Cargo and its cached SDK source are build-time requirements only, not requirements for installing the packaged GUI.

Packaging keeps ownership boundaries explicit:

```text
FLOW 8 PC Controller installer → per-user GUI only
DirectHCI installer             → separate service/runtime
Controller driver setup         → separate privileged prerequisite
```

Build the Windows installer with Rust 1.95 or newer and Inno Setup 6.4 or newer.
Cargo downloads the pinned SDK on the first build; no sibling source checkout
is needed. From PowerShell at the repository root:

```powershell
.\scripts\windows\build-installer.ps1
```

The script builds the GUI with `cargo build --release --locked`, keeps Cargo
artifacts under `%LOCALAPPDATA%\flow8-rust-target`, and writes the installer
under `%LOCALAPPDATA%\FLOW 8 PC Controller\installer-output`. It prints SHA-256
hashes for the GUI and installer. Before publishing a release, verify a fresh
install, upgrade, launch, controller connection through Ready, disconnect,
uninstall, identity-file preservation, signing status and corresponding source.
The FLOW installer never installs or removes DirectHCI, a driver, or user credentials.
The user-facing installation path is in [installation.md](installation.md).

The packaged GUI does not require PowerShell, Cargo, a manually launched console daemon, or environment variables for normal use. It does currently require the separately installed DirectHCI service to be started and controller preparation to be completed through DirectHCI Control Panel. The FLOW application does not switch drivers itself.

## Related documentation

Use the dedicated documents for deeper details:

- `docs/architecture.md`
- `docs/ble.md`
- `docs/protocol.md`
- `docs/state-model.md`
- `docs/validation.md`
- `docs/hardware-test-plan.md`
- `docs/midi.md`

The historical C++/Qt implementation is kept on the separate `c++` branch.
