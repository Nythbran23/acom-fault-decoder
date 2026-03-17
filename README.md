# ACOM Fault Decoder

**Professional desktop diagnostic tool for ACOM linear amplifiers.**  
Developed by GW4WND

---

## Overview

ACOM amplifiers log hard fault signatures at the moment of failure — a snapshot of every voltage, current, temperature, flag and I/O state in the firmware at the exact instant the fault occurred. This tool decodes those signatures into human-readable form and provides engineering-level diagnostic analysis to identify root causes.

The decode logic was reverse-engineered from the official ACOM Excel converter tools, including correction of a **DEC2HEX string-truncation bug** in the original Excel that caused incorrect flag decoding for any flag word below 0x1000.

---

## Supported Amplifiers

| Model | Series | Connection | Entry Method |
|-------|--------|-----------|--------------|
| ACOM 500S | S-Series | RS232 / USB-Serial | Automatic capture or manual |
| ACOM 600S | S-Series | RS232 / USB-Serial | Automatic capture or manual |
| ACOM 700S | S-Series | RS232 / USB-Serial | Automatic capture or manual |
| ACOM 1200S | S-Series | RS232 / USB-Serial | Automatic capture or manual |
| ACOM 1000 | Legacy | Front panel display | Manual entry |
| ACOM 1500 | Legacy | Front panel display | Manual entry |
| ACOM 2100 | Legacy | Front panel display | Manual entry |

---

## Downloads

Pre-built installers are available on the [Releases](https://github.com/Nythbran23/acom-fault-decoder/releases) page:

| Platform | File | Notes |
|----------|------|-------|
| macOS (Apple Silicon) | `ACOM.Fault.Decoder_x.x.x_aarch64.dmg` | M1/M2/M3 Macs |
| macOS (Intel) | `ACOM.Fault.Decoder_x.x.x_x64.dmg` | Intel Macs |
| Windows | `ACOM.Fault.Decoder_x.x.x_x64-setup.exe` | Windows 10/11 |
| Linux (Debian/Ubuntu) | `ACOM.Fault.Decoder_x.x.x_amd64.deb` | `sudo dpkg -i` |
| Linux (portable) | `ACOM.Fault.Decoder_x.x.x_amd64.AppImage` | Any distro |

---

## Installation

### macOS
1. Download the `.dmg` for your Mac type (Apple Silicon or Intel)
2. Open the DMG and drag **ACOM Fault Decoder** to Applications
3. On first launch, right-click → Open if Gatekeeper blocks it
4. Or from Terminal: `xattr -cr "/Applications/ACOM Fault Decoder.app"`

### Windows
1. Download and run the `-setup.exe` installer
2. If Windows Defender SmartScreen blocks it, click **More info → Run anyway**

### Linux (Debian/Ubuntu)
```bash
sudo dpkg -i ACOM.Fault.Decoder_x.x.x_amd64.deb
acom-fault-decoder
```

### Linux (AppImage)
```bash
chmod +x ACOM.Fault.Decoder_x.x.x_amd64.AppImage
./ACOM.Fault.Decoder_x.x.x_amd64.AppImage
```

---

## Usage

### S-Series (500S / 600S / 700S / 1200S) — RS232 Capture

The S-Series transmits fault signatures automatically over RS232 when you access the fault log.

**Hardware setup:**
- Connect a USB-to-RS232 adapter between your computer and the amplifier's RS232 port
- Settings: 9600 baud, 8N1 (configured automatically)

**Capture procedure:**
1. Select the **S-Series** tab
2. Select the correct serial port from the dropdown and click **Connect**
3. On the amplifier: navigate to `MENU → FAULTS LOG`
4. The 4-line signature is captured automatically and decoded immediately
5. Captures are saved automatically to `~/Downloads/ACOM_Signatures/`

**Manual entry:**
If you have a signature already (e.g. from a previous capture file), paste each of the 4 lines into the input fields and press **Decode**. Each line is 16 × 4-digit hex words. The checksum indicator (✓/✗) confirms each line is correctly transcribed.

---

### Legacy Models (A1000 / A1500 / A2100) — Manual Entry

These amplifiers display fault signatures on their front panel 7-segment display. The signature must be read and entered manually.

**Reading the signature:**

Consult your amplifier's operating manual for the signature display procedure. Typically:
- Hold a button combination during power-on, or
- Navigate to the fault log via the front panel menu

The display shows 7 groups of characters in sequence.

**Entry procedure:**
1. Select the **1000 / 1500 / 2100** tab
2. Select your amplifier model from the dropdown
3. Enter each group as displayed:
   - **State** (G0): 1-2 characters
   - **Groups 1-6** (G1-G6): 6 characters each
4. Press **Decode**

**7-segment character note:**  
The character `S` on the display represents the digit `5` — type either `s` or `5`, both are accepted. The characters `b`, `A`-`F` and `0`-`9` are standard.

---

## Decode Output

### S-Series

| Section | Contents |
|---------|----------|
| Amplifier State | Operating mode, jump state, total runtime |
| Active Fault Codes | All hard faults, soft faults, and warnings with descriptions |
| Diagnostic Analysis | Root cause analysis cross-correlating faults with voltages and runtime |
| RF Parameters | Frequency, forward/reflected power, SWR |
| PSU Voltages | VCC26, VCC5, HV1, HV2 with out-of-range highlighting |
| PAM Currents & Temperatures | Per-module currents and temperatures |
| DC Power | Input and dissipation per PAM module |
| Bias Voltages | Measured vs nominal for all 8 bias channels |
| CAT Interface | Protocol, baud rate, timing settings |
| Diagnostics | Error source code, LPF register, band data |
| User Flags | All user-configurable settings at fault time |
| Amp Flags 1 & 2 | Full firmware state flags |

### Legacy (1000 / 1500 / 2100)

| Section | Contents |
|---------|----------|
| Amplifier State | Operating phase and mode at fault time |
| Analog Parameters | HV plate voltage, plate current, RF power, temperatures |
| Digital Signal Registers | All 5 I/O register bytes decoded to individual signal names |
| Checksum | Validity confirmation |

**Analog scaling (all confirmed from ACOM Excel tools):**
- HV plate voltage: raw × 16 V
- Idle plate current: raw × 5 mA
- Forward power: raw² ÷ 32 W
- Reflected power: raw² ÷ 128 W
- Input drive: raw² ÷ 512 W
- PA anode voltage: raw × 12 V
- Screen grid current: raw ÷ 2 mA
- Temperature: raw × 2 − 273 °C

---

## Diagnostic Engine

The S-Series decoder includes an automatic diagnostic engine that cross-references fault codes with voltage readings, temperatures, runtime, and flag states to produce plain-English findings with recommended actions.

Examples of what it identifies automatically:

- **5V transient vs static failure** — distinguishes between a marginal capacitor causing a transient droop (rail reads normal after fault) and a sustained PSU failure
- **HV degradation vs protective shutdown** — identifies whether a zero HV reading means the supply failed or was shut down protectively after a different fault
- **Runtime-correlated faults** — flags when fault patterns at high runtime (>5000h) are consistent with electrolytic capacitor degradation
- **Service mode context** — warns when fault data was captured in service/test mode where protection thresholds may differ
- **Artefact identification** — recognises secondary flag states (e.g. HV_CTRL + HV_MON_DIS together) that are artefacts of the shutdown sequence rather than independent faults

---

## Technical Notes

### S-Series Signature Format

The fault signature is a 4-line × 16-word hex format transmitted at 9600 baud over RS232. Each line ends with a checksum word (sum of all 16 words mod 65536 = 0).

The decode covers all 160 fault codes across 10 error groups, all parameter fields from lines 1-4, and all flag registers.

### Legacy Signature Format

7 display groups (state + 6 × 6 characters). Checksum algorithm: `0xA5 XOR` cascade of 15 specific bytes (counter byte and GAMA display byte excluded). Verified against real captures from both ACOM 1000 and ACOM 1500.

### Known Limitations

**S-Series:**
- LPF register values not mapped to frequency bands (raw hex shown)
- ATU/ASEL packed fields not decoded (raw hex shown)
- Error source codes are ACOM-internal — include in support requests to ACOM
- CAT command set values for Yaesu/Kenwood/Elecraft unconfirmed (ICOM confirmed)

**Legacy:**
- ACOM 2100 signal maps assumed identical to 1000/1500 — unverified (no real captures)
- 7-seg characters `r` and `t` substitution unconfirmed

---

## Building from Source

### Prerequisites
- [Rust](https://rustup.rs/) (stable toolchain)
- Tauri CLI: `cargo install tauri-cli --version "^2.0"`

### Development
```bash
git clone https://github.com/Nythbran23/acom-fault-decoder.git
cd acom-fault-decoder
cargo tauri dev
```

### Run tests
```bash
cd src-tauri
cargo test --lib
```
All 30 tests should pass.

### Release build
```bash
cargo tauri build
```
Output: `src-tauri/target/release/bundle/`

### Cross-platform builds
Triggered automatically on GitHub Actions when a version tag is pushed:
```bash
git tag v1.x.x
git push origin v1.x.x
```
Builds macOS (ARM + Intel), Windows, and Linux in parallel.

---

## Contributing

Real-world fault captures help improve the decode tables, particularly for:
- Legacy models (1000/1500/2100) captured during transmission (non-zero RF power values)
- ACOM 2100 captures of any kind
- S-Series captures with ATU/ASEL units connected
- S-Series captures on Yaesu/Kenwood/Elecraft CAT

To submit a capture, open an issue on GitHub with the raw signature data and the amplifier model.

---

## Licence

For amateur radio diagnostic use. Not affiliated with ACOM.

*© 2026 The DX Shop — GW4WND*
