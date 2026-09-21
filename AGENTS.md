# FLOW 8 PC Controller

## 1. Project Overview

This project is an independent PC control application for the Behringer FLOW 8 digital mixer.

The goal is to provide functionality comparable to the official FLOW Control mobile application while running natively on a PC.

The application must operate without requiring a mobile phone.

Primary communication:

* Bluetooth Low Energy (BLE)

Secondary communication:

* USB MIDI

Primary development environment:

* Linux

Primary runtime, hardware testing, and release environment:

* Windows

Supported platforms:

* Windows
* Linux

The project should remain cross-platform at the architecture level.

---

# 2. Project Goals

The final application should be capable of:

* Discovering FLOW 8 devices
* Connecting to FLOW 8 through BLE
* Performing the required BLE initialization/authentication
* Reading device state
* Synchronizing mixer state
* Modifying mixer parameters
* Controlling channels
* Controlling buses
* Controlling EQ
* Controlling compressors
* Controlling FX
* Controlling sends
* Controlling snapshots
* Receiving device-side state changes
* Updating the GUI in real time
* Recovering from BLE disconnection
* Supporting USB MIDI as an additional transport
* Running on Windows
* Running on Linux

The application should eventually provide a usable PC-oriented mixer interface rather than merely reproducing the mobile application's UI.

---

# 3. Technology Stack

Primary language:

C++20

GUI:

Qt 6

Build system:

CMake

Build generator:

Ninja

Testing:

Qt Test or Catch2, depending on project requirements.

Compilers:

Linux:

* GCC or Clang

Windows:

* MSVC

BLE:

Qt Bluetooth should be the first choice for the cross-platform BLE layer.

Linux:

Qt Bluetooth → BlueZ

Windows:

Qt Bluetooth → Windows Bluetooth stack

Do not implement a custom Bluetooth stack unless there is a demonstrated technical requirement.

---

# 4. Development Environment

The project is developed primarily on Linux.

Linux is used for:

* coding
* static analysis
* unit tests
* protocol development
* reverse-engineering tooling
* general development
* CI validation

Windows is the primary target platform and must be treated as a first-class platform throughout development.

Windows is used for:

* real FLOW 8 hardware testing
* BLE testing
* GUI testing
* release testing
* packaging
* final user operation

Do not assume that successful Linux BLE behavior guarantees successful Windows BLE behavior.

Hardware-related functionality must eventually be verified on Windows.

---

# 5. Cross-Platform Build Policy

The project must use a single CMake-based build system.

Provide CMake presets for:

```text
linux-debug
linux-release
windows-debug
windows-release
```

Expected Linux workflow:

```bash
cmake --preset linux-debug
cmake --build --preset linux-debug
ctest --test-dir build/linux-debug
```

Expected Linux release workflow:

```bash
cmake --preset linux-release
cmake --build --preset linux-release
```

Expected Windows workflow:

```powershell
cmake --preset windows-debug
cmake --build --preset windows-debug
```

Windows release:

```powershell
cmake --preset windows-release
cmake --build --preset windows-release
```

The exact generator/compiler configuration may be adjusted to the actual environment.

Do not hard-code:

* Qt installation paths
* compiler paths
* user directories
* Bluetooth adapter names
* device addresses
* machine-specific paths

Use CMake configuration, environment variables, presets, or platform-specific configuration where appropriate.

---

# 6. Repository Structure

The project should approximately follow:

```text
flow8-pc/
│
├── AGENTS.md
├── README.md
├── LICENSE
├── CMakeLists.txt
├── CMakePresets.json
│
├── cmake/
│   ├── Dependencies.cmake
│   ├── CompilerWarnings.cmake
│   └── Platform.cmake
│
├── docs/
│   ├── architecture.md
│   ├── protocol.md
│   ├── ble.md
│   ├── midi.md
│   ├── state-model.md
│   ├── reverse-engineering.md
│   └── troubleshooting.md
│
├── reference/
│   └── flow-8-midi/
│
├── src/
│   ├── app/
│   │
│   ├── core/
│   │   ├── flow8_device.*
│   │   ├── flow8_state.*
│   │   ├── parameter.*
│   │   └── events.*
│   │
│   ├── model/
│   │   ├── channel.*
│   │   ├── bus.*
│   │   ├── eq.*
│   │   ├── compressor.*
│   │   ├── fx.*
│   │   └── snapshot.*
│   │
│   ├── protocol/
│   │   ├── flow8_protocol.*
│   │   ├── packet.*
│   │   ├── codec.*
│   │   └── sysex.*
│   │
│   ├── ble/
│   │   ├── ble_transport.*
│   │   ├── ble_device.*
│   │   └── ble_services.*
│   │
│   ├── midi/
│   │   ├── midi_transport.*
│   │   ├── midi_device.*
│   │   └── midi_protocol.*
│   │
│   ├── platform/
│   │   ├── windows/
│   │   └── linux/
│   │
│   └── ui/
│       ├── main_window.*
│       ├── mixer/
│       ├── channel/
│       ├── eq/
│       ├── fx/
│       ├── snapshot/
│       └── settings/
│
├── tests/
│   ├── unit/
│   │   ├── protocol/
│   │   ├── model/
│   │   └── codec/
│   │
│   ├── integration/
│   │   ├── ble/
│   │   └── midi/
│   │
│   └── fixtures/
│
├── tools/
│   ├── ble_scanner/
│   ├── ble_monitor/
│   ├── midi_monitor/
│   ├── packet_decoder/
│   └── state_dump_decoder/
│
├── logs/
│
└── packaging/
    ├── windows/
    └── linux/
```

The exact structure may evolve as implementation requirements become clearer.

Do not create unnecessary directories merely to satisfy this document.

---

# 7. Architecture

The architecture should follow:

```text
                         Qt GUI
                            │
                            ▼
                       Flow8State
                            │
                            ▼
                       Flow8Device
                            │
                  ┌─────────┴─────────┐
                  │                   │
                  ▼                   ▼
             BLE Transport       MIDI Transport
                  │                   │
                  ▼                   ▼
             BLE Protocol        MIDI Protocol
                  │                   │
                  └─────────┬─────────┘
                            │
                            ▼
                         FLOW 8
```

The GUI must not directly communicate with BLE or MIDI.

The GUI must not construct raw FLOW 8 protocol packets.

The GUI should interact with high-level device/state APIs.

The protocol layer must not depend on Qt Widgets.

Transport implementations must not contain mixer business logic.

---

# 8. Device State

The application must maintain a unified device state.

The conceptual model includes:

```text
Flow8State
 ├── ChannelState
 ├── BusState
 ├── MainState
 ├── MonitorState
 ├── FXState
 └── SnapshotState
```

The exact fields must be derived from verified FLOW 8 behavior.

Do not create protocol fields merely because a GUI widget needs them.

If a field is not known:

```text
UNKNOWN
```

or explicitly mark it as unverified.

---

# 9. Protocol Research

Protocol research is a core part of this project.

The reference implementation:

```text
reference/flow-8-midi/
```

may be used as a technical reference.

It must not automatically be treated as authoritative.

For every protocol feature, distinguish between:

```text
VERIFIED
INFERRED
UNKNOWN
```

Verification sources include:

1. Actual FLOW 8 hardware behavior
2. Actual BLE/MIDI captures
3. Existing open-source reverse-engineering implementations
4. Official documentation
5. Other reliable technical references

When possible, record the source.

For source-code-based findings, record:

* repository
* file
* relevant function/class
* explanation

---

# 10. No Protocol Guessing

Never invent:

* BLE UUIDs
* Characteristic UUIDs
* parameter IDs
* packet layouts
* packet lengths
* checksums
* authentication values
* handshake sequences
* SysEx structures
* mixer state layouts
* encoding rules

Do not infer protocol semantics solely because a value "looks reasonable".

If evidence is insufficient:

```text
UNKNOWN
```

If there is a hypothesis, explicitly label it:

```text
HYPOTHESIS
```

and explain the evidence.

Do not silently turn a hypothesis into implementation.

---

# 11. BLE

BLE is the primary transport.

The BLE layer must support, as required by actual FLOW 8 behavior:

```text
Scan
Connect
Disconnect
Service discovery
Characteristic discovery
Read
Write
Write Without Response
Notify
Connection state
Reconnection
Error handling
```

The implementation must record useful debugging information.

BLE debug logs should be able to capture:

```text
timestamp
direction
service UUID
characteristic UUID
raw payload
decoded message
```

Example:

```text
BLE RX
characteristic=...
data=01 02 03 ...
```

Raw packet logging must not be enabled at excessive verbosity by default.

---

# 12. BLE Protocol Layer

BLE transport and FLOW 8 protocol must remain separate.

Conceptually:

```text
BLE Transport
      │
      ▼
Raw packet
      │
      ▼
FLOW 8 Protocol Decoder
      │
      ▼
Flow8 Message
      │
      ▼
Flow8State
```

For outbound operations:

```text
User operation
      │
      ▼
Flow8Device
      │
      ▼
FLOW 8 Protocol Encoder
      │
      ▼
BLE Transport
      │
      ▼
FLOW 8
```

Authentication and handshake must be implemented only after they are sufficiently understood.

---

# 13. USB MIDI

USB MIDI is a secondary transport.

It should be implemented independently of BLE.

The MIDI layer should support whatever FLOW 8 functionality is actually verified to be available through MIDI.

Potential capabilities include:

* device discovery
* input
* output
* MIDI messages
* SysEx
* parameter control

Do not assume that all BLE functionality exists through USB MIDI.

The protocol documentation must explicitly record transport limitations.

---

# 14. Transport Abstraction

The device layer should not depend directly on platform-specific transport APIs.

Conceptually:

```cpp
class Flow8Transport
{
public:
    virtual ~Flow8Transport() = default;

    virtual bool connect() = 0;
    virtual void disconnect() = 0;
    virtual bool send(const QByteArray& data) = 0;
};
```

The actual interface may be redesigned when implementation requirements become clearer.

BLE and MIDI should be interchangeable from the protocol/device layer where practical.

---

# 15. Platform Abstraction

Prefer Qt cross-platform APIs.

Do not write separate Windows and Linux implementations unless necessary.

Only use:

```text
src/platform/windows/
src/platform/linux/
```

when Qt cannot provide the required functionality or when a platform-specific behavior has been verified.

The platform layer must not leak into:

```text
core/
model/
protocol/
```

---

# 16. GUI

The GUI should be designed for PC use.

It does not need to visually reproduce the official FLOW Control application.

The GUI should prioritize:

* mixer visibility
* efficient mouse interaction
* keyboard shortcuts where useful
* clear parameter editing
* real-time state updates
* usable layout on desktop displays

Expected functionality eventually includes:

```text
Mixer
Channel strips
Faders
Mute
Solo
Gain
Pan
EQ
Compressor
Sends
FX
Main
Monitor
Snapshot
Settings
Connection status
```

Do not implement GUI features before the corresponding device functionality is understood.

A GUI control for an unimplemented protocol feature should not pretend that the feature works.

---

# 17. State Synchronization

Device-originated changes must update the application state.

Conceptually:

```text
FLOW 8
  ↓
BLE/MIDI
  ↓
Protocol decoder
  ↓
Flow8State
  ↓
State event
  ↓
Specific UI component
```

Do not rebuild the entire GUI for every packet.

Update only the affected state and UI components.

---

# 18. Connection State

The application should represent connection states such as:

```text
Disconnected
Scanning
Connecting
Authenticating
Synchronizing
Connected
Ready
Error
```

Actual states may be adjusted based on the protocol.

Handle:

* device unavailable
* BLE disconnect
* reconnection
* timeout
* malformed packet
* authentication failure
* transport failure

---

# 19. Testing

Separate:

```text
Unit tests
Integration tests
Hardware tests
```

Unit tests should cover:

* packet parsing
* packet encoding
* parameter conversion
* codecs
* state parsing
* SysEx parsing
* protocol logic

Integration tests should cover:

* BLE transport
* MIDI transport

Hardware-dependent tests must be clearly identified.

Protocol tests should use real verified captures whenever possible.

Do not create arbitrary fake protocol data merely to make tests pass.

---

# 20. Reverse-Engineering Fixtures

Store verified captures in:

```text
tests/fixtures/
```

For each fixture, document:

```text
source
date
device state
user action
transport
expected interpretation
verification status
```

For example:

```text
tests/fixtures/ble/
tests/fixtures/midi/
```

Never commit sensitive personal data or unrelated machine information into fixtures.

---

# 21. Documentation

Maintain:

```text
docs/protocol.md
docs/ble.md
docs/midi.md
docs/reverse-engineering.md
docs/state-model.md
docs/architecture.md
```

Documentation must evolve together with the implementation.

When a protocol fact is discovered, update the documentation.

When an earlier assumption is disproved, update the documentation and identify the correction.

---

# 22. Development Phases

## Phase 0 — Repository and Research

Tasks:

* inspect repository
* inspect reference implementation
* inspect build environment
* inspect Qt installation
* inspect CMake
* inspect Bluetooth capabilities
* analyze FLOW 8 protocol references
* document verified and unknown protocol information

Do not build the complete GUI.

---

## Phase 1 — Cross-Platform Build Foundation

Establish:

* CMake
* CMakePresets
* Linux build
* Windows build configuration
* Qt 6 integration
* unit-test framework
* GitHub Actions

Both Linux and Windows must be first-class build targets.

---

## Phase 2 — BLE Discovery

Implement:

* BLE scanner
* FLOW 8 detection
* connection
* GATT discovery
* characteristic inspection
* notification monitoring
* raw BLE logging

Goal:

```text
PC → BLE → FLOW 8
```

and:

```text
FLOW 8 → BLE → PC
```

must be observable.

---

## Phase 3 — BLE Protocol

Implement only verified protocol functionality.

Potential areas:

* initialization
* authentication
* handshake
* packet decoding
* packet encoding
* state dump
* state synchronization

Do not implement unknown portions.

---

## Phase 4 — Flow8State

Implement the unified state model.

Verify that real device data can populate the model.

---

## Phase 5 — Basic Device Control

Implement verified basic controls.

For example:

```text
Fader
Mute
Solo
Gain
Pan
```

Actual feature order depends on protocol findings.

Every control must be tested against real hardware.

---

## Phase 6 — Extended Mixer Functions

Implement verified:

```text
EQ
Compressor
Sends
FX
Bus
Monitor
Main
Snapshot
```

Do not assume all features are supported by every transport.

---

## Phase 7 — USB MIDI

Implement the MIDI backend and transport-specific functionality.

---

## Phase 8 — GUI

Build the desktop mixer interface on top of the existing state/device model.

---

## Phase 9 — Synchronization and Recovery

Implement:

* real-time updates
* reconnection
* synchronization
* error handling
* state recovery

---

## Phase 10 — Packaging

Windows is the primary release platform.

Provide a practical Windows distribution.

Potential forms:

```text
Portable ZIP
Installer
```

Linux release may use:

```text
AppImage
```

or another appropriate distribution format.

Do not add packaging complexity before the application itself is stable.

---

# 23. Windows Priority

Windows is the primary runtime platform.

Therefore:

* Windows builds must remain functional
* Windows BLE must be tested
* Windows GUI must be tested
* Windows packaging must be tested
* Windows hardware integration must be tested

Linux remains a fully supported development/runtime platform.

Do not sacrifice cross-platform architecture merely because Windows is the primary target.

---

# 24. CI

GitHub Actions should eventually verify:

```text
Ubuntu
    configure
    build
    unit tests

Windows
    configure
    build
    unit tests
```

Hardware-dependent BLE tests should not be falsely represented as CI tests unless physical hardware is actually available to the runner.

---

# 25. Codex Operating Rules

Codex is expected to perform substantial implementation work autonomously.

It may:

* inspect the repository
* inspect reference code
* create and modify files
* refactor code
* run builds
* run tests
* create documentation
* create tools
* inspect compiler errors
* fix implementation issues
* maintain CMake configuration

However, Codex must not fabricate hardware observations.

When physical FLOW 8 interaction is required, explicitly stop and tell the user what physical action is required.

For example:

```text
需要真实设备验证：

1. 打开 FLOW 8
2. 开启 Bluetooth
3. 不要连接手机
4. 运行 tools/ble_scanner
5. 将输出提供给我
```

Do not claim that the device was tested when it was not.

---

# 26. Work Incrementally

Do not generate the entire application in one step.

Each phase should produce a working and testable result.

After each significant change:

1. Build
2. Run tests
3. Inspect git diff
4. Update documentation
5. Identify verified/unknown protocol information
6. Summarize what changed

Avoid huge speculative implementations.

Prefer small, verifiable increments.

---

# 27. Error Handling

When blocked by missing information:

Do not invent an answer.

Instead report:

```text
BLOCKED

Missing information:
...

Required verification:
...

Possible ways to obtain it:
...
```

Continue with independent work that does not depend on the missing information.

---

# 28. Code Quality

Prefer:

* clear C++20
* RAII
* strong ownership semantics
* const correctness
* small focused classes
* explicit error handling
* testable interfaces
* minimal global state

Avoid:

* unnecessary singleton usage
* giant manager classes
* UI/protocol coupling
* platform-specific code in core modules
* speculative abstractions
* unnecessary dependencies

Do not over-engineer the project before real protocol requirements are known.

---

# 29. Final Architecture

The intended final architecture is approximately:

```text
                         FLOW 8
                       /       \
                     BLE      USB MIDI
                      │          │
                      ▼          ▼
                BLE Transport  MIDI Transport
                      │          │
                      ▼          ▼
                FLOW8 Protocol Layer
                       │
                       ▼
                  Flow8State
                       │
                       ▼
                  Flow8Device
                       │
                       ▼
                    Qt GUI
```

Platform:

```text
Linux
 └── Qt Bluetooth
       └── BlueZ

Windows
 └── Qt Bluetooth
       └── Windows Bluetooth
```

The protocol and state model must remain independent from the platform.

---

# 30. Final Completion Criteria

The project is considered functionally complete only when the application can, using real FLOW 8 hardware:

* discover FLOW 8
* connect through BLE
* perform required initialization
* synchronize verified device state
* display mixer state
* modify verified mixer parameters
* receive device-side state changes
* update the GUI accordingly
* recover from normal disconnection scenarios
* provide verified USB MIDI functionality where supported
* build on Linux
* build on Windows
* run on Windows
* run on Linux
* provide reproducible build instructions
* provide protocol documentation
* provide meaningful automated tests
* provide Windows packaging

No undocumented or fabricated protocol behavior should be required for the final application.
