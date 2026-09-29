# FLOW 8 PC Controller

Rust/egui is the active implementation of FLOW 8 PC Controller. The completed C++20/Qt 6 application remains in this repository as the UI, state-model, protocol, and exact-vector reference during migration; it is not deleted or treated as a second product. Real-device evidence shows that FLOW 8 has no CCCD and emits unsolicited notifications. The production Windows path now consumes the external DirectHCI runtime through the `directhci-ble` Rust SDK and passive `listen()`; its first complete `0x35 -> Ready` application run remains pending.

```text
egui GUI → SemanticCommand → CommandQueue → Protocol Encoder → Transport
    ▲                                                            │
    └──── Store / confirmed state ← Parser / reassembly ← BLE RX ┘

Transport: Linux/generic = btleplug; Windows/FLOW 8 = `flow8-directhci` SDK adapter → external `directhcid`
```

## Rust quick start

Requirements: Rust 1.95+, Cargo, eframe/winit desktop libraries, and BlueZ/D-Bus for optional Linux BLE tools.

```bash
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
cargo build --workspace
cargo test --workspace
cargo run -p flow8-gui
```

When the checkout is on a VMware Shared Folder, keep LLVM artifacts off that mount:

```bash
export CARGO_TARGET_DIR=/tmp/flow8-rust-target
cargo run -p flow8-gui
```

The GUI starts in explicit `Simulator / SYNTHETIC` mode and needs no BLE adapter. The production Windows FLOW path uses a normal Rust toolchain plus the external `directhcid` service through the Rust SDK; it does not require Qt and does not manage a Windows Bluetooth driver itself.

## Rust workspace

| Package | Responsibility |
|---|---|
| `flow8-model` | product model, `StateValue`, evidence, parameter specs |
| `flow8-protocol` | 31/31 TX, typed atomic/composite RX, FIX8, framing and `0x38` reassembly |
| `flow8-core` | Store, full semantic queue, state applier, pending/confirmed, Simulator |
| `flow8-directhci` | thin FLOW profile adapter over the external `directhci-ble` SDK; no protocol parser or controller implementation |
| `flow8-ble` | transport-neutral session/handshake coordinator, platform runtime bridge, and async GUI channels |
| `flow8-gui` | egui rendering and semantic intents only |
| `flow8-cli` | offline protocol utility |
| `flow8-hardware-bringup` | scan/GATT/notify/handshake/state/capture and safe typed controls |

The Rust protocol implementation now covers all 31 requested TX schemas and their
APK-confirmed typed RX paths. Atomic events, `0x17/0x18/0x30`, and fragmented `0x38`
apply to the same Store. The GUI can switch between deterministic Simulator mode and the
Tokio device runtime without blocking egui. The Qt/C++ implementation remains
the visual/reference implementation while final parity and hardware calibration continue.

## Legacy C++/Qt reference

The following requirements and commands build the retained reference implementation. New product work belongs in Rust.


### Requirements

- CMake 3.22+
- Ninja
- GCC/Clang on Linux or Qt MinGW-w64 (GCC) on Windows
- Qt 6.2+ with Core, Widgets, Test, Bluetooth/Connectivity, and Linguist tools

Ubuntu example:

```bash
sudo apt install qt6-base-dev qt6-base-dev-tools qt6-connectivity-dev qt6-l10n-tools ninja-build
```

### Build

Linux:

```bash
cmake --preset linux-debug
cmake --build --preset linux-debug
ctest --preset linux-debug
./build/linux-debug/flow8-controller
```

Use `linux-release` for Release. The default Linux preset permits Qt Bluetooth to be
absent, so simulator/UI/protocol targets continue to build; pass
`-DFLOW8_REQUIRE_BLUETOOTH=ON` only on a Linux host where Bluetooth is required.

Windows uses ordinary PowerShell or cmd; Visual Studio and MSVC are not required. Put
the Qt Online Installer MinGW-w64 toolchain, the matching Qt `mingw_64` kit,
CMake, and Ninja on `PATH`, then verify:

```powershell
where.exe gcc.exe
where.exe g++.exe
where.exe cmake.exe
where.exe ninja.exe
where.exe qmake.exe
g++.exe --version
qmake.exe -query QMAKE_XSPEC
```

`QMAKE_XSPEC` must be `win32-g++`. If Qt is not discovered, provide the matching
`mingw_64` kit through `CMAKE_PREFIX_PATH` or `Qt6_DIR`; no machine-specific absolute
path is stored in the repository. Build both configurations independently:

```powershell
cmake --preset windows-mingw-debug
cmake --build --preset windows-mingw-debug
ctest --preset windows-mingw-debug

cmake --preset windows-mingw-release
cmake --build --preset windows-mingw-release
ctest --preset windows-mingw-release
```

## Rust Simulator

Run `cargo run -p flow8-gui`. The application starts explicitly in
`Simulator / SYNTHETIC` mode and provides seven conventional input strips (`1`, `2`,
`3`, `4`, `5/6`, `7/8`, `USB/BT`), three independent Mix Buses (MAIN/MON1/MON2), two FX
engines, four physical output sinks, 15 device-snapshot slots, a separate local simulator
snapshot list, routing, deterministic animated meters, channel/bus EQ, compressor amount,
and send controls. Simulator values are functional test data, not FLOW 8 captures or
protocol evidence.

The Mixer follows the APK-derived source-to-destination model. One shared set of seven
channel strips is controlled by a MAIN/MON1/MON2/FX1/FX2 destination selector. Switching
the destination changes only the route faders; input Gain/Pan/Mute/Solo/EQ/Compressor
state stays global. The right side shows either the selected destination Master/FX detail
or an in-context Input Inspector. Stage reads the same matrix rather than maintaining a
second mixer state. Setup remains a distinct sidebar workspace for Configure Inputs, the
local Snapshot Library, 15 device Snapshot slots, Preferences, Routing, and Info.

The 7×5 matrix is deliberately scoped to seven conventional Mixer inputs feeding
MAIN/MON1/MON2/FX1/FX2. It is not the device's complete routing graph. USB 1/2 and USB
3/4 are represented separately as USB Audio loopback endpoints; they are not Mixer
sources and are not MON buses. Physical outputs separately represent MAIN OUT, MON OUT
1, MON OUT 2, and HEADPHONES. USB input assignment, physical monitor-output feed
selection, headphone MAIN/MON plus Pre/Post selection, output pads, MON1/MON2 stereo
link, and six FX return routes are independent state. Meter state is high-rate transient
data kept outside ordinary control state. Simulator data uses explicit `SYNTHETIC`
evidence.

The Rust simulator models Gain/Phase/Low Cut, phantom power only where capability allows,
PEQ/GEQ, limiter, independent USB Audio endpoints, USB/FX/headphone routing, physical
outputs, MON1/MON2 link state, device/local snapshot presentation, and global tempo.
It deliberately does not invent MON link propagation. Deterministic synthetic activity
exercises meters, FX values, and snapshot indications while retaining `SYNTHETIC`
evidence.

## Bidirectional APK protocol implementation

The protocol registry implements exact TX and typed RX for the 31 requested native
commands, including Pan/Solo/Gain, input PEQ, output GEQ, HPF, Route/Master, Mute,
Compressor amount, Limiter, Phantom, FX, meter, settings, snapshots, output selection,
delay, and compound state. Each requested command has at least one exact APK-native byte
vector test. The full native descriptor catalog records 51 non-empty schemas; companion
commands without an established PC workflow are not given speculative product APIs.

`0x38` reassembly is driven by the protocol fragment header rather than a fixed MTU or
fragment count. It handles both the retained APK 251/147-byte fixture and the real-device
MTU-131 four-fragment shape, rejects incomplete or inconsistent sequences, then atomically
applies 7 input states, 3 output states, 2 FX states, and the routing/control tail. Atomic notifications and
compound state update the same confirmed model. Outbound control values remain pending
until RX confirms them; a conflicting RX value wins. On `0x36`, the device layer queues
the exact `0x37` state request and waits for complete `0x38` before Ready.

These are `VERIFIED_FROM_APK` codec/parser facts and `IMPLEMENTED` PC behavior. Windows
has verified the target service/characteristic, `WRITE | NOTIFY`, MTU 131 and successful
`WithResponse` characteristic writes, while standard subscription fails with ATT Write
Not Permitted. Android HCI evidence establishes that the physical characteristic has no
CCCD and sends unsolicited ATT notifications on value handle `0x000B`. The production
Windows transport reads the selected device's optional `DeviceInstanceId`, `ContainerId`,
parent identity and Bluetooth address for diagnostics only. Service discovery is scoped to
the selected `BluetoothLEDevice`: its `BluetoothDeviceId` and the FLOW service UUID build a
`GattDeviceService` device selector. The sole returned service `DeviceInformation.Id` is
forwarded verbatim to `CreateFileW`; the resulting native handle is verified with
`BluetoothGATTGetServices` / `BluetoothGATTGetCharacteristics` (without assuming
`0x000B`). It registers `BluetoothGATTRegisterEvent` and
never calls btleplug `subscribe()` or writes a synthetic CCCD. This backend is
`IMPLEMENTED` and source-level Windows checked; successful Windows RX/handshake remains
`NOT VERIFIED` until the next hardware run.

The desktop UI uses custom HiDPI-aware faders (drag, wheel, Shift fine adjustment,
keyboard and double-click reset), smooth meters, EQ graphs, a live 85–140% UI scale,
adaptive channel widths, responsive inspector placement and scrolling long pages while
preserving the architectural rule that Widgets call only `Flow8Device`.

## Languages

The migrated Rust pages provide English and Simplified Chinese labels and can be switched
at runtime from the top bar. egui keeps its Latin faces first and appends an installed
system CJK face (Microsoft YaHei/DengXian/SimSun on Windows, Noto/Source Han/WenQuanYi on
Linux) to both proportional and monospace fallback chains. No font file is bundled; a
missing system CJK face produces a warning rather than a crash. Locale detection and
persistence are still migration work.
The retained Qt reference continues to use its Qt Linguist `.ts/.qm` catalogs; those Qt
translation files are not the Rust runtime implementation.

## Official capability model

Product capabilities come from the Behringer FLOW 8 product page and official Quick
Start Guide/MIDI Implementation. This is intentionally separate from protocol evidence:
an official statement that a feature exists does not establish its BLE address or prove
that this project operated it on hardware.

- Inputs 5/6 and 7/8 are stereo pairs; USB/BT is a separate digital input strip.
- Phantom power is exposed only for inputs 1 and 2.
- MAIN, MON1, and MON2 are Mix Buses with 9-band EQ and limiter capability; FX1/FX2 are
  separate engines rather than output buses.
- FX1 and FX2 are independent engines with 16 documented preset numbers each.
- Hardware snapshots have 15 slots; the app-library concept is separate.
- The MIDI channel/CC/PC/Note map is represented in `MidiParameterMap` with source
  `OfficialManual`, but runtime MIDI behavior remains untested.

Official sources: [FLOW 8 product page](https://www.behringer.com/ja/products/0603-AEW)
and [worldwide Quick Start Guide](https://mediadl.musictribe.com/media/PLM/data/docs/P0DNM/QSG_BE_0603-AEW_FLOW-8_WW.pdf).

## BLE tools

The Windows runtime uses DirectHCI by default; the retained native GATT backend is legacy/experimental and opt-in only. The separately running `directhcid` owns controller acquisition, Windows driver transitions, raw-HCI access, and recovery. The linked `directhci-ble` SDK reaches that runtime through `directhci-client` and Named Pipe IPC, then exposes generic BLE/GATT operations to FLOW 8. The thin `flow8-directhci` crate owns only FLOW device/UUID selection and passive-listen policy; the protocol codec and session state machine remain in the existing FLOW runtime. Provide the paired 16-byte client UUID (this is semantic identity data, not a prebuilt packet):

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
$env:FLOW8_CLIENT_UUID = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
$env:RUST_LOG = "flow8_directhci=trace,flow8_ble=trace"
cargo run -p flow8-gui
```

For this first integration stage, external `directhcid.exe run` must already be available. The FLOW application does not spawn it as a child and never opens WinUSB or changes a driver. The `directhci-ble` SDK requests a BLE session from the daemon (`DIRECTHCI_CONTROLLER_ID` is required only when more than one exists), scans for `FLOW 8 LE`, discovers the exact FLOW service/characteristic, arms generic passive listening without CCCD, then lets the existing `Flow8Session` perform `0x35 -> 0x39 -> 0x36 -> 0x37 -> 0x38 -> Ready`. The previous ASCII `FLOW8-PC-RUST001` value is not used by this backend. Automatic DirectHCI service startup is a later lifecycle task after the first production Ready run.

DirectHCI is the default Windows FLOW backend when `FLOW8_BLE_BACKEND` is unset. Set `FLOW8_BLE_BACKEND=windows-native` only to compare the retained legacy/experimental implementation. Keep build products on
the Windows-local filesystem, enable useful transport logging, launch the GUI, select BLE
mode and press **Connect**:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
$env:RUST_LOG = "flow8_ble=debug"
cargo run -p flow8-gui
```

With `flow8_ble=debug` or `flow8_ble=trace`, the same production lifecycle log is also
written to `captures/hardware/windows-production-connection.log`. Use `flow8_ble=trace`
when full raw packet hex is needed. The separate bring-up tool and
older probes remain available for evidence comparison:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\flow8-rust-target"
cargo run -p flow8-hardware-bringup -- scan --seconds 5
cargo run -p flow8-hardware-bringup -- inspect-gatt --output captures\hardware\gatt-inspection.log
cargo run -p flow8-hardware-bringup -- winrt-gatt-probe --output captures\hardware\winrt-gatt-probe.log
cargo run -p flow8-hardware-bringup -- subscribe --seconds 15
cargo run -p flow8-hardware-bringup -- capture captures\bringup-001.log --seconds 30
cargo run -p flow8-hardware-bringup -- handshake --seconds 20
cargo run -p flow8-hardware-bringup -- probe-handshake-order --output captures\handshake-order.log
cargo run -p flow8-hardware-bringup -- passive-handshake --output captures\hardware\passive-handshake.log
cargo run -p flow8-hardware-bringup -- prearmed-handshake --output captures\hardware\prearmed-handshake.log
```

`subscribe` and `capture` without `--handshake` are observation-only. `handshake` follows
the APK-confirmed `0x35/0x39/0x36/0x37/0x38` state machine, but real acceptance is still
NOT RUN. Explicit `route`, `gain`, `mute`, `pan`, and `solo` subcommands are available
only for staged manual validation; Phantom, snapshot deletion and reset are omitted.

`probe-handshake-order` is a narrow Windows/GATT diagnostic for subscription failures. It
opens a fresh connection for each of five cases, compares subscribe-only with APK-exact
`0x35`/`0x36` writes before subscription, retains positive and negative backend outcomes,
and never sends a mixer-control command. Its deliberate emission of normally receive-side
handshake schemas is confined to this evidence tool and does not alter normal session
semantics.

`inspect-gatt` is strictly read-only at the GATT application boundary: it connects,
discovers the complete public btleplug service/characteristic/descriptor hierarchy, and
attempts safe descriptor reads. It never subscribes, writes a descriptor or
characteristic, or sends a FLOW 8 protocol packet. The report explicitly states whether
the target characteristic exposes the standard `0x2902` CCCD through the current backend;
btleplug's public API does not expose Windows attribute handles or WinRT object IDs.

`winrt-gatt-probe` is Windows-only and bypasses btleplug's GATT abstraction. It performs
cached and uncached WinRT discovery, compares the target characteristic's descriptor
lists, reports every communication status and ATT protocol error exposed by WinRT, and
then performs one controlled standard CCCD Notify/None cycle with a `ValueChanged`
handler attached. It sends no FLOW 8 protocol or mixer-control packet. The CCCD
Notify/None operations are the only writes made by this diagnostic.

`passive-handshake` is the hardware-correct no-CCCD path. It first opens btleplug's
notification receiver without calling `subscribe()`. If that public stream cannot expose
unsolicited Windows notifications, it reconnects through WinRT, attaches
`GattCharacteristic.ValueChanged` directly, and never calls a CCCD API. A complete
`0x35` causes the production codec to emit `0x39`; `0x36` causes production-codec `0x37`.
All `0x38` fragments go through the normal header-driven reassembler and the session is
reported Ready only after the complete state is atomically applied. No mixer-control
command is permitted by this diagnostic.

`prearmed-handshake` is the Windows timing experiment for the official app's immediate
and roughly 500 ms-retried `0x35`. It uses direct WinRT, resolves only cached GATT
objects, attaches `GattCharacteristic.ValueChanged` and logs `listener=ARMED` before it
creates/maintains a `GattSession`. It never performs uncached discovery, subscribes, or
writes a CCCD. Connection-status transitions and handshake milestones are timestamped
relative to the observed Connected transition. If `FromIdAsync` or a cached lookup
connects before the listener is armed, the run is explicitly marked compromised and a
later timeout is not treated as evidence that Windows suppresses unsolicited values.
Only production-codec `0x39` and `0x37` responses are eligible for transmission.

The retained Qt reference tools remain available when Qt Bluetooth is installed:

```bash
./build/linux-debug/flow8-ble-scanner --json
./build/linux-debug/flow8-ble-monitor --json --decode
```

The Rust capture command preserves every payload byte. Captures are evidence only after
they come from a real device and include the test context; offline APK vectors remain
`VERIFIED_FROM_APK`.

Offline capture analysis:

```bash
./build/linux-debug/flow8-packet-decoder 370138 0601010fff16
./build/linux-debug/flow8-state-dump-decoder --json dump.syx
./build/linux-debug/flow8-state-dump-decoder --hex --json dump.hex
```

The state-dump decoder reports input SHA-256 and keeps all extracted offsets marked
`INFERRED`.

## Evidence and design documents

- [Architecture](docs/architecture.md)
- [BLE](docs/ble.md)
- [Windows hardware validation](docs/hardware-validation.md)
- [Protocol](docs/protocol.md)
- [Conservative SysEx offset catalog](docs/sysex-offsets.md)
- [USB MIDI and SysEx](docs/midi.md)
- [State model](docs/state-model.md)
- [FLOW Mix feature matrix](docs/flow-mix-feature-matrix.md)
- [Reverse-engineering record](docs/reverse-engineering.md)

`docs/reverse-engineering.md` is the current APK/native fact source used by the PC model.

`reference/flow-8-midi/` is research evidence, not an authority or a substitute for this
project's hardware validation.

Current product-capability calibration also uses the official
[FLOW Mix App Store page](https://apps.apple.com/us/app/flow-mix/id1473097916),
[Google Play listing](https://play.google.com/store/apps/details?id=com.musicgroup.xairbt),
[FLOW 8 product page](https://www.behringer.com/ja/products/0603-AEW), and official Quick
Start Guide. Store/manual capability descriptions never promote a BLE mapping to
`VERIFIED`.
