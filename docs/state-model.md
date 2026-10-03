# State model

The active Rust state types are in `flow8-model` and `flow8-core`. This document describes the implemented model, not the former C++/Qt design. The public protocol overview and device observations are recorded separately in [protocol.md](protocol.md) and [validation.md](validation.md).

## State ownership

`flow8-core::Flow8Store` owns one `Flow8State` for the GUI and BLE runtime. The GUI sends `SemanticCommand` values; it does not construct packets or keep a second copy of confirmed mixer state. The physical mixer remains authoritative.

`flow8-model::StateValue<T>` carries a confirmed value, an optional pending local request, an optional error, and an `EvidenceStatus`. A device observation replaces the confirmed value and clears pending/error, including when it disagrees with the requested value. The evidence labels are `VerifiedFromDevice`, `VerifiedFromApk`, `Inferred`, `Unknown`, `Blocked`, and `Synthetic`.

The production Store starts with `Flow8State::unconfirmed()`: the mixer layout is available, but device values are unknown and controls requiring Ready remain disabled. `Flow8Store::simulator()` exists for deterministic offline tests only; it is not a connection mode in the GUI.

## Mixer topology

`Flow8State` contains:

- seven input strips: Input 1–4, Input 5/6, Input 7/8, and USB/Bluetooth;
- three independent mix buses: MAIN, MON1, and MON2;
- two FX engines: FX1 and FX2;
- routing/device settings, 15 device snapshot slots, headphone volume, global tempo, and the currently selected GUI destination.

Each input has five route levels, one for each destination (MAIN, MON1, MON2, FX1, FX2). Selecting a destination changes which route levels are shown; it does not duplicate the input state. USB Audio 1/2 and 3/4 are separate routing endpoints, not extra mixer strips. MON stereo-link state is recorded without assuming how the hardware propagates individual parameters.

Input capabilities limit controls such as analog gain and 48 V phantom power to applicable channels. Buses hold their own master, EQ, limiter, delay, and meter state. FX engines have independent preset, parameter, return, mute, and meter state. The GUI's output selection is separate from the device's selected physical output.

The model stores output delay as ticks; the GUI displays milliseconds using `ticks / 48.0`. The accepted hardware range remains unverified.

## Commands and device updates

`Flow8Store::dispatch` validates a semantic request and records pending state before the runtime encodes a `flow8-protocol::TxCommand`. Device notifications become typed `RxCommand` values and enter `Flow8Store::apply_rx`. An atomic update changes only its addressed state.

A complete initial MixerState is validated and applied to a candidate Store before replacing the confirmed state. Missing or malformed fragments cannot partially update the visible mixer or make the session Ready. The production session reaches Ready only after that full state has been applied.

Meter observations live in the existing input, bus, and FX meter fields. GUI smoothing affects rendering only; it does not alter confirmed meter samples or generate control commands.

## Snapshots and application preferences

The production model has 15 device snapshot slots and no persistent PC snapshot library. Slot names come from device responses when available. The GUI-local `AppPreferences` covers display and interaction choices such as mute-button visibility, channel icons, channel visibility, EQ editing mode, and UI scale; it is not a device protocol state.

Assisted Setup and remote EZ GAIN are not implemented in the current Rust application. The FLOW 8 hardware has an EZ GAIN function, but this project has no identified remote start/stop command. Do not infer one from ordinary Gain, level, meter, or phantom-power messages.
