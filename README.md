# FLOW 8 PC Controller

An independent C++20/Qt 6 desktop controller for the Behringer FLOW 8 mixer, with BLE as
the primary transport and USB MIDI as a secondary future transport.

Current development includes a cross-platform build, a 31-command bidirectional
APK-equivalent protocol codec/parser, a unified
evidence-aware state model, a Qt Bluetooth transport, BLE inspection tools, an
official-manual-calibrated functional model, a deterministic simulator, and a Qt Widgets
desktop mixer. Real FLOW 8 behavior is not claimed: a retained Windows hardware session has not yet
been executed, so real-device results remain **NOT RUN / WINDOWS_HOST_RUN_REQUIRED**.

```text
Qt Widgets GUI → Flow8Device ↔ Flow8State
                       ├── FLOW 8 protocol → Flow8Transport
                       │                        ├── FakeTransport
                       │                        └── BLE Transport
                       └── MidiParameterMap (official map; I/O backend future)
```

## Requirements

- CMake 3.22+
- Ninja
- GCC/Clang on Linux or Qt MinGW-w64 (GCC) on Windows
- Qt 6.2+ with Core, Widgets, Test, Bluetooth/Connectivity, and Linguist tools

Ubuntu example:

```bash
sudo apt install qt6-base-dev qt6-base-dev-tools qt6-connectivity-dev qt6-l10n-tools ninja-build
```

## Build

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

## Simulator

Run `flow8-controller` and choose Assisted Setup, Load Snapshot, Start New, or Continue
Session. The bundled transport is explicitly `Simulator (SYNTHETIC)`. The mixer provides
seven conventional input strips (`1`, `2`, `3`, `4`, `5/6`, `7/8`, `USB/BT`), three
independent Mix Buses (MAIN/MON1/MON2), two FX engines, four physical output sinks,
15 hardware snapshot slots, routing, animated meters, channel/bus EQ, compressor amount,
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

The simulator models channel names/icons/hide-show, Gain/Phase/Low Cut, phantom power where
applicable, MON pre/post sends, two independent USB Audio endpoints,
USB/FX/headphone routing, physical outputs, and the MON1/MON2 stereo-link relationship,
app-snapshot store/load/rename/delete, Assisted Setup, EZ-GAIN, global Tap Tempo, and
output preferences. Simulator behavior remains `SYNTHETIC`; it never serves as device
evidence and does not emit a guessed hardware response.

## Bidirectional APK protocol implementation

The protocol registry implements exact TX and typed RX for the 31 requested native
commands, including Pan/Solo/Gain, input PEQ, output GEQ, HPF, Route/Master, Mute,
Compressor amount, Limiter, Phantom, FX, meter, settings, snapshots, output selection,
delay, and compound state. Each requested command has at least one exact APK-native byte
vector test. The full native descriptor catalog records 51 non-empty schemas; companion
commands without an established PC workflow are not given speculative product APIs.

`0x38` reassembly handles the retained 388-byte payload as 251/147-byte raw frames,
rejects incomplete or inconsistent sequences, then atomically applies 7 input states,
3 output states, 2 FX states, and the routing/control tail. Atomic notifications and
compound state update the same confirmed model. Outbound control values remain pending
until RX confirms them; a conflicting RX value wins. On `0x36`, the device layer queues
the exact `0x37` state request and waits for complete `0x38` before Ready.

These are `VERIFIED_FROM_APK` codec/parser facts and `IMPLEMENTED` PC behavior. No BLE
packet has been accepted by a physical FLOW 8 in this project yet. The mixer is now
available to the Windows host, but Windows BLE execution remains
`NOT RUN / WINDOWS_HOST_RUN_REQUIRED` until a retained capture exists.

The desktop UI uses custom HiDPI-aware faders (drag, wheel, Shift fine adjustment,
keyboard and double-click reset), smooth meters, EQ graphs, and responsive layouts while
preserving the architectural rule that Widgets call only `Flow8Device`.

## Languages

The complete desktop UI supports:

- 简体中文 (`zh-CN`)
- English (`en-US`)

The first launch follows the system locale (`zh_*` selects Simplified Chinese; all other
locales select English). The language can be changed at runtime in
**Setup → Preferences → Language** and is remembered for later launches. Qt Linguist
`.ts` sources live in `translations/`;
CMake compiles and deploys the corresponding `.qm` files without hard-coded Qt paths.

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

When Qt Bluetooth is installed:

```bash
./build/linux-debug/flow8-ble-scanner --json
./build/linux-debug/flow8-ble-monitor --json --decode
```

On the Windows 11 host, first hardware bring-up uses the manual-by-default logger:

```powershell
.\build\windows-mingw-debug\flow8-hardware-bringup.exe `
  --output captures\hardware\bringup-001.jsonl `
  --capture-dir captures\hardware\mixer-state
```

The monitor preserves every payload byte and does not automatically send the unverified
FLOW 8 handshake.

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
