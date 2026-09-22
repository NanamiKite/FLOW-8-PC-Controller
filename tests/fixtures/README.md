# Protocol fixtures

Formal fixtures live here; uncurated local captures remain under the ignored
`captures/` directory. Every fixture consists of immutable raw bytes (`.hex`)
and a JSON manifest containing its origin, description, evidence level,
SHA-256 digest, and expected decoder output.

`reference_capture` means the bytes are copied from the bundled reference
project. It does **not** mean this project verified them against a FLOW 8.
`synthetic` fixtures test codecs and replay plumbing only and can never upgrade
protocol evidence.

Current catalog:

- `protocol/reference-session-start.json` — reference 0x37 packet.
- `protocol/reference-authentication.json` — reference 0x39 packet.
- `sysex/synthetic-reference-layout.json` — synthetic full-size reference
  layout with matching state expectations in `state/`.

Future hardware fixtures must use `source = hardware capture`, record date,
firmware/device context and user action, and be reviewed before moving out of
`captures/`.
