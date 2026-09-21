# FLOW 8 PC Controller

An independent C++20/Qt 6 desktop controller for the Behringer FLOW 8 mixer, with BLE as
the primary transport and USB MIDI as a secondary future transport.

Current development includes a cross-platform build, protocol codecs, a unified optional
state model, a Qt Bluetooth transport, BLE inspection tools, a deterministic simulator,
and a Qt Widgets mixer shell. Real FLOW 8 behavior is not claimed: hardware validation is
currently **BLOCKED: NEED_HARDWARE**.

```text
Qt Widgets GUI → Flow8Device ↔ Flow8State
                       │
                  Flow8Transport
                 /              \
        FakeTransport         BLE Transport
                                  │
                             FLOW 8 protocol
```

## Requirements

- CMake 3.22+
- Ninja
- GCC/Clang on Linux or MSVC on Windows
- Qt 6.2+ with Core, Widgets, Test, and Bluetooth/Connectivity

Ubuntu example:

```bash
sudo apt install qt6-base-dev qt6-base-dev-tools qt6-connectivity-dev ninja-build
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

Run `flow8-controller`, select `Simulator (SYNTHETIC)`, and connect. The mixer provides
seven channel strips and Main control. Channel name, gain, fader, mute, solo, and pan can
be exercised without hardware; deterministic remote fader updates test inbound state.
EQ, Compressor, Sends, FX, Snapshots, and Settings are visible placeholders only.

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
- [Reverse-engineering record](docs/reverse-engineering.md)

`reference/flow-8-midi/` is research evidence, not an authority or a substitute for this
project's hardware validation.
