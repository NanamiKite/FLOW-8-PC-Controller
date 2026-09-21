# FLOW 8 PC Controller

An independent C++20/Qt 6 desktop controller for the Behringer FLOW 8 mixer, with BLE as
the primary transport and USB MIDI as a secondary future transport.

Current development includes a cross-platform build, protocol codecs, a unified
evidence-aware state model, a Qt Bluetooth transport, BLE inspection tools, an
official-manual-calibrated functional model, a deterministic simulator, and a Qt Widgets
desktop mixer. Real FLOW 8 behavior is not claimed: hardware validation is currently
**BLOCKED: NEED_HARDWARE**.

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
- GCC/Clang on Linux or MSVC on Windows
- Qt 6.2+ with Core, Widgets, Test, Bluetooth/Connectivity, and Linguist tools

Ubuntu example:

```bash
sudo apt install qt6-base-dev qt6-base-dev-tools qt6-connectivity-dev qt6-l10n-tools ninja-build
```

## Build

Linux:

```bash
cmake --preset linux-debug -DFLOW8_REQUIRE_BLUETOOTH=ON
cmake --build --preset linux-debug
ctest --preset linux-debug
./build/linux-debug/flow8-controller
```

Use `linux-release` for Release. If Qt Connectivity is temporarily unavailable, omit
`FLOW8_REQUIRE_BLUETOOTH=ON`; the simulator/UI/protocol targets still build and CMake
clearly reports that BLE targets were omitted.

Windows, from an MSVC environment with Qt discoverable by CMake:

```powershell
cmake --preset windows-debug -DFLOW8_REQUIRE_BLUETOOTH=ON
cmake --build --preset windows-debug
ctest --preset windows-debug
```

Use `windows-release` for Release. No Qt, compiler, user-directory, adapter, or device
address path is hard-coded.

## Simulator

Run `flow8-controller` and choose Assisted Setup, Load Snapshot, Start New, or Continue
Session. The bundled transport is explicitly `Simulator (SYNTHETIC)`. The mixer provides
seven correctly typed input strips (`1`, `2`, `3`, `4`, `5/6`, `7/8`, `USB/BT`), five
buses, two FX engines, 15 hardware snapshot slots, routing, animated meters, channel and
bus EQ, compressor amount, and send controls. Simulator values are functional test data,
not FLOW 8 captures or protocol evidence.

The Mixer follows the APK-derived source-to-destination model. One shared set of seven
channel strips is controlled by a MAIN/MON1/MON2/FX1/FX2 destination selector. Switching
the destination changes only the route faders; input Gain/Pan/Mute/Solo/EQ/Compressor
state stays global. The right side shows either the selected destination Master/FX detail
or an in-context Input Inspector. Stage reads the same matrix rather than maintaining a
second mixer state. Setup remains a distinct sidebar workspace for Configure Inputs, the
local Snapshot Library, 15 device Snapshot slots, Preferences, Routing, and Info.

The canonical mixer state includes a 7×5 route matrix with confirmed/pending/error
values, independent destination masters, MON stereo link, and independent FX input
sends versus FX output returns. Meter state is high-rate transient data kept outside
ordinary control state. Simulator data uses explicit `SYNTHETIC` evidence.

The simulator models channel names/icons/hide-show, Gain/Phase/Low Cut, phantom power where
applicable, MON pre/post sends, expanded USB/FX/headphone routing, MON stereo link,
app-snapshot store/load/rename/delete, Assisted Setup, EZ-GAIN, global Tap Tempo, and
output preferences. Hardware commands for these additions remain
`UNKNOWN / BLOCKED: NEED_HARDWARE`; the simulator never emits guessed packets.

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
- MAIN, MON1, and MON2 have 9-band EQ and limiter capability; FX buses do not.
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
- [Protocol](docs/protocol.md)
- [Conservative SysEx offset catalog](docs/sysex-offsets.md)
- [USB MIDI and SysEx](docs/midi.md)
- [State model](docs/state-model.md)
- [FLOW Mix feature matrix](docs/flow-mix-feature-matrix.md)
- [Reverse-engineering record](docs/reverse-engineering.md)
- [FLOW Mix APK semantic record](docs/reverse-engineering-apk.md)

`reference/flow-8-midi/` is research evidence, not an authority or a substitute for this
project's hardware validation.

Current product-capability calibration also uses the official
[FLOW Mix App Store page](https://apps.apple.com/us/app/flow-mix/id1473097916),
[Google Play listing](https://play.google.com/store/apps/details?id=com.musicgroup.xairbt),
[FLOW 8 product page](https://www.behringer.com/ja/products/0603-AEW), and official Quick
Start Guide. Store/manual capability descriptions never promote a BLE mapping to
`VERIFIED`.
