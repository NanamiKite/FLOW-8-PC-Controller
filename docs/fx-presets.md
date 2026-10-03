# FX1 / FX2 presets

FLOW 8 has two separate effects processors, each with 16 preset slots. The manufacturer's block diagram confirms the broad categories: FX1 has twelve reverbs, a flanger, and three choruses; FX2 has twelve delays/echoes, a flanger, and three choruses. The exact names/order below are taken from an independent [FLOW 8 controller's preset catalog](https://github.com/abelroes/flow-8-midi/blob/main/src/model/channels.rs), so their name-to-BLE-ID association is **INFERRED**, not yet `VERIFIED_FROM_DEVICE`. See the [FLOW 8 block diagram](https://manuals.plus/m/f9848cd1f1f0ddd4f1b0e27ea9d26220c14f2e1d37dae53ea364f17d33c2f500.pdf) for the manufacturer's category breakdown.

| UI slot | BLE preset ID (inferred) | FX1 | FX2 |
|---:|---:|---|---|
| 1 | 0 | Ambience | Delay 1/1 |
| 2 | 1 | Perc-Rev1 | Delay 1/2 |
| 3 | 2 | Perc-Rev2 | Delay 1/3 |
| 4 | 3 | Guit-Rev1 | Delay 2/1 |
| 5 | 4 | Guit-Rev2 | Echo 1/1 |
| 6 | 5 | Chamber | Echo 1/2 |
| 7 | 6 | Room | Echo 1/3 |
| 8 | 7 | Concert | Echo 2/1 |
| 9 | 8 | Church | Wide Echo |
| 10 | 9 | Cathedral | Ping Pong |
| 11 | 10 | Temple | Ping P 1/3 |
| 12 | 11 | Stadium | Ping P R>L |
| 13 | 12 | Flanger | Flanger |
| 14 | 13 | Soft Chor | Soft Chor |
| 15 | 14 | Warm Chor | Warm Chor |
| 16 | 15 | Deep Chor | Deep Chor |

## Switching in the Rust GUI

Open FX1 or FX2 and choose a preset from its dropdown. The GUI sends the existing `SemanticCommand::SetFxPreset` through the shared protocol encoder; it does not construct a packet locally. The selected value is pending until an RX update confirms it; the device's reported preset remains authoritative. In BLE mode, editing is unavailable until the mixer state has synchronized and the session is Ready. Offline synthetic fixture selection is not device evidence. The BLE preset numbers and names in the table still need a real-device switch-and-readback calibration.

The preset-specific meanings of FX parameter values remain unknown in this application; the GUI keeps their generic names rather than importing unverified parameter labels.
