# USB MIDI reference (not the application's control transport)

The FLOW 8 hardware has a separately documented USB MIDI interface. The current Rust
FLOW 8 PC Controller does **not** use USB MIDI to control the mixer: its live
connection and control path is native BLE. This page records official MIDI
mapping information for comparison only; it is not a guide to a working Rust
MIDI backend.

## Official MIDI implementation

Source: Behringer [FLOW 8 Quick Start Guide](https://mediadl.musictribe.com/media/PLM/data/docs/P0DNM/QSG_BE_0603-AEW_FLOW-8_WW.pdf),
**MIDI Implementation**, pp. 44-45 (also present in localized official guides).

The historical `c++` branch implemented a MIDI mapping model; the active Rust
workspace has not yet implemented a USB MIDI transport. Channels in the table
are zero-based in code and one-based when displayed.

| Target | Display channel | Code channel |
|---|---:|---:|
| Input 1, 2, 3, 4 | 1-4 | 0-3 |
| Input 5/6 | 5 | 4 |
| Input 7/8 | 6 | 5 |
| USB/BT | 7 | 6 |
| MAIN, MON1, MON2 | 8-10 | 7-9 |
| FX1 bus, FX2 bus | 11-12 | 10-11 |
| unused | 13 | 12 |
| FX1 engine, FX2 engine | 14-15 | 13-14 |
| global | 16 | 15 |

Input CC mapping is: EQ gains `1-4`, mute `5`, solo `6`, level `7`, gain `8`,
low cut `9`, balance `10`, compressor `11`, phantom `12`, and MON1/MON2/FX1/FX2 sends
`14-17`. Official exceptions are encoded: USB/BT has no gain, low-cut, compressor, or
phantom mapping; phantom is only available on Input 1 and 2.

Bus mapping is level `CC 7`, limiter `CC 8`, MAIN-only balance `CC 10`, and nine EQ gains
at `CC 11-19` (62 Hz through 16 kHz). FX buses expose only level from that set.

FX engines use Program Change `1-16`, `CC 1` for Parameter 1, and `CC 2` for Parameter 2.
Global channel 16 uses Program Change `1-15` for hardware snapshots, Program Change `16`
for reset, `CC 1` for common FX mute, and Note On 0 with non-zero velocity for tap tempo.

The official mapping keeps three MIDI concepts separate:

- bus control uses displayed channels 8-12 (MAIN, MON1, MON2, FX1 bus, FX2 bus);
- independent FX engine control uses displayed channels 14-15;
- snapshot, common FX mute, and global Tap Tempo use displayed channel 16.

The desktop Layer Bar does not change this documented mapping: `FX1`/`FX2` pages address the
engine model, while monitor/main pages use their bus/send models. UI layer names are not
MIDI-channel assignments, and no control is synthesized merely because a page exists.

The official MIDI chart does not establish messages for the project's USB routing,
monitor-link, or output settings. Assisted Setup and remote EZ GAIN are not implemented
in the Rust GUI; no MIDI or BLE command for remote EZ GAIN is currently identified.

These mappings are **officially documented capabilities**, not proof that this project's
future MIDI backend transmitted them successfully. Runtime validation remains
**BLOCKED: NEED_HARDWARE**.

## Value ranges in the official chart

- level/send: 0 = off; 1-127 maps -70 dB to +10 dB;
- gain: -20 dB to +60 dB;
- EQ: -15 dB to +15 dB;
- pan/balance: left to right with 64 centered;
- compressor and FX parameter 1: 0-100%;
- low cut: 20-600 Hz;
- limiter: -30 dB to 0 dB;
- switches: 0 off/A, 1-127 on/B.

## SysEx status

The active Rust application does not implement a USB MIDI/SysEx transport or
state-application path. Historical SysEx observations are not evidence that
this application's BLE connection depends on USB MIDI.

## UNKNOWN / BLOCKED

- portable Rust MIDI I/O backend and cross-platform port naming;
- actual Windows/Linux enumeration and device acceptance;
- feedback behavior and timing;
- Program Change numbering as exposed by a future library API;
- SysEx length/offset stability across firmware;
- whether every documented command is available in every firmware revision.
