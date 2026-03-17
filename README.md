# ACOM Fault Decoder

Professional desktop diagnostic tool for ACOM linear amplifiers.  
**The DX Shop — GW4WND**

---

## Supported Amplifiers

| Model | Connection | Method |
|-------|-----------|--------|
| ACOM 500S / 600S / 700S (S-Series) | RS232 / USB-Serial | Automatic capture or manual entry |
| ACOM 1000 | Front panel display | Manual entry |
| ACOM 1500 | Front panel display | Manual entry |
| ACOM 2100 | Front panel display | Manual entry |

---

## Features

- **Full parameter decode** — voltages, currents, temperatures, bias, CAT settings, runtime
- **Fault code analysis** — all 160 error codes with severity classification
- **Diagnostic engine** — cross-correlates fault codes with voltage readings and runtime to explain root causes
- **Digital signal registers** — complete I/O port state at time of fault (legacy models)
- **Capture history** — auto-saves signatures to `~/Downloads/ACOM_Signatures/`
- **Cross-platform** — macOS (Apple Silicon + Intel), Windows, Linux

---

## S-Series RS232 Setup

1. Connect USB-to-RS232 adapter between Mac/PC and amplifier's RS232 port
2. Settings: **9600 baud, 8N1** (configured automatically)
3. In the app: select the port and click **Connect**
4. On the amplifier: `MENU → FAULTS LOG`
5. The signature is captured automatically and decoded

---

## Legacy Models (1000 / 1500 / 2100)

1. Select the **1000 / 1500 / 2100** tab
2. Choose your amplifier model
3. Read the signature codes from the front panel display
4. Enter each group (State + Groups 1-6) and press **Decode**

**7-segment display note:** The character `S` on the display represents `5` — type `s` or `5`, both are accepted.

---

## Downloads

See [Releases](https://github.com/Nythbran23/acom-fault-decoder/releases) for the latest builds:

- **macOS**: `.dmg` (Apple Silicon and Intel)
- **Windows**: `.msi` installer
- **Linux**: `.AppImage` or `.deb`

---

## Building from Source

```bash
# Prerequisites: Rust stable, cargo tauri-cli
cargo install tauri-cli --version "^2.0"

# Run in development mode
cargo tauri dev

# Build release
cargo tauri build
```

---

## Technical Notes

The S-Series fault signature is a 4-line × 16-word hex format transmitted over RS232. Full decode specification was reverse-engineered from the official ACOM Excel converter tool, including correction of a DEC2HEX string-truncation bug in the original Excel that caused incorrect flag decoding for any flag word below 0x1000.

---

*Copyright © 2026 The DX Shop. For amateur radio use.*
