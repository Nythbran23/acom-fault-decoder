#![allow(dead_code)]
// ============================================================================
// legacy.rs — ACOM 1000 / 1500 / 2100 hard-fault signature decoder.
//
// Scaling formulas reverse-engineered from official ACOM Excel converter tools.
// All formulas verified against real captures.
//
// INPUT FORMAT: 7 display groups
//   Group 0 (state):  2 chars  — sequence counter + mode code prefix
//   Groups 1-6:       6 chars each = 3 bytes each = 18 bytes total
//
// BYTE LAYOUT (after parsing):
//   Group 1: [counter] [mode_code(2chars)] [secondary_state]
//   Group 2: [pfwd_raw] [rfl_raw] [inp_raw]
//   Group 3: [paav_raw] [g2c_raw] [ipm_raw]
//   Group 4: [??] [hvm_raw] [temp_raw]
//   Groups 5-6: digital registers (buffer0, buffer1, port1, port3, port4)
//
// SCALING (all confirmed from Excel formulas):
//   pfwd  (W)  = raw² / 32
//   rfl   (W)  = raw² / 128
//   inp   (W)  = raw² / 512
//   paav  (V)  = raw × 12
//   g2c  (mA)  = raw / 2
//   ipm  (mA)  = raw × 5
//   hvm   (V)  = raw × 16
//   temp  (°C) = raw × 2 − 273
// ============================================================================

use serde::Serialize;
use thiserror::Error;

// ============================================================================
// Model selector
// ============================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LegacyModel {
    Acom1000,
    Acom1500,
    Acom2100,
}

impl LegacyModel {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Acom1000 => "ACOM 1000",
            Self::Acom1500 => "ACOM 1500",
            Self::Acom2100 => "ACOM 2100",
        }
    }
}

// ============================================================================
// Errors
// ============================================================================
#[derive(Debug, Error)]
pub enum LegacyParseError {
    #[error("Group {group} has wrong length: got {actual} chars, expected {expected}")]
    WrongGroupLength { group: usize, actual: usize, expected: usize },

    #[error("Group {group} contains unrecognised character '{ch}'")]
    UnrecognisedChar { group: usize, ch: char },
}

// ============================================================================
// 7-segment normalisation
//
// Group 1 chars 3-4 are a LITERAL mode code ('sb', 'tr', 'pn', 'pr') —
// these are NOT hex bytes. We handle them separately in parse_mode_code().
//
// For all other groups, 's' → '5' is the only non-standard 7-seg char
// confirmed from real captures.
// ============================================================================
fn normalise_7seg(ch: char) -> Result<char, char> {
    match ch {
        '0'..='9' | 'a'..='f' | 'A'..='F' => Ok(ch),
        'S' | 's' => Ok('5'),  // confirmed: s ≡ 5 on 7-seg
        'r' | 'R' => Ok('5'),  // unconfirmed best guess
        't' | 'T' => Ok('4'),  // unconfirmed best guess
        other => Err(other),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CharSubstitution {
    pub group:     usize,
    pub position:  usize,
    pub original:  char,
    pub assumed:   char,
    pub confirmed: bool,
}

fn parse_hex_byte(s: &str, group: usize, substitutions: &mut Vec<CharSubstitution>)
    -> Result<u8, LegacyParseError>
{
    let chars: Vec<char> = s.chars().collect();
    let mut nibbles = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        match normalise_7seg(ch) {
            Ok(n) => {
                if n != ch {
                    substitutions.push(CharSubstitution {
                        group, position: i,
                        original: ch, assumed: n,
                        confirmed: ch == 's' || ch == 'S',
                    });
                }
                nibbles.push(n);
            }
            Err(_) => return Err(LegacyParseError::UnrecognisedChar { group, ch }),
        }
    }
    u8::from_str_radix(&nibbles, 16)
        .map_err(|_| LegacyParseError::UnrecognisedChar { group, ch: chars[0] })
}

// ============================================================================
// Mode code decode
//
// Group 1, chars 3-4 are literal 2-character mode codes displayed as text.
// 'sb', 'tr', 'pn', 'pr' — NOT hex values.
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct AmpMode {
    pub code:        String,
    pub description: &'static str,
}

fn parse_mode_code(s: &str) -> AmpMode {
    let code = s.to_lowercase();
    let description = match code.as_str() {
        "pn" => "Power On",
        "sb" => "Stand By",
        "pr" => "Operate",
        "tr" => "Oper T/R (Transmit/Receive)",
        _    => "Unknown",
    };
    AmpMode { code, description }
}

// ============================================================================
// State decode (group 0 + group 1 chars 5-6)
// ============================================================================
// ============================================================================
// Group 1 full decode
//
// Group 1 chars: [0-1]=trip_number  [2-3]=mode_code  [4]=sub_state  [5]=fault_signal
//
// Sub-state meanings:
//   PN0=Power-On before HV, PN2=Power-On after HV+1s
//   SB0=StandBy entering/warmup, SB2=StandBy after warmup
//   PR0=entering Operate, PR2=during Operate
//   TR0=Tx→Rx relay test, TR2=Rx→Tx relay test
//   TR4=relay test during Tx, TR6=relay test during Rx
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct FaultSignal {
    pub code:        char,
    pub signal_name: &'static str,
    pub signal_type: &'static str,   // "analogue" or "logic"
    pub description: &'static str,
}

fn decode_fault_signal(ch: char) -> FaultSignal {
    let (name, sig_type, desc) = match ch {
        '1' => ("pfwd",    "analogue", "Peak forward power"),
        '2' => ("rfl",     "analogue", "Reflected power"),
        '3' => ("inp",     "analogue", "Input (drive) power"),
        '4' => ("paav",    "analogue", "Peak anode alternate voltage"),
        '5' => ("g2c",     "analogue", "Screen grid current"),
        '6' => ("ipm",     "analogue", "Plate current"),
        '7' => ("hvm",     "analogue", "High voltage"),
        '8' => ("temp",    "analogue", "Exhaust air temperature"),
        '9' => ("*GRIDRF", "logic",    "Drive power present"),
        'a' => ("*PANT",   "logic",    "Antenna power present"),
        'b' => ("ORC",     "logic",    "Output relay closed"),
        'c' => ("ARCF",    "logic",    "Arc fault"),
        'd' => ("G1C",     "logic",    "Control grid current too high"),
        'e' => ("PSE",     "logic",    "24VDC power supply error"),
        'f' => ("LAIR",    "logic",    "Low airflow"),
        _    => ("??",      "unknown",  "Unknown signal"),
    };
    FaultSignal { code: ch, signal_name: name, signal_type: sig_type, description: desc }
}

#[derive(Debug, Clone, Serialize)]
pub struct AmpState {
    /// Trip sequence number as displayed e.g. "1A" = last event
    pub trip_number: String,
    /// Mode code: "pn", "sb", "pr", "tr"
    pub mode:        AmpMode,
    /// Sub-state digit: "0", "2", "4", "6"
    pub sub_state:   char,
    /// Human-readable description of operating state at fault time
    pub state_description: &'static str,
    /// The signal that caused the protection to trip
    pub fault_signal: FaultSignal,
}

fn decode_state_description(mode: &str, sub: char) -> &'static str {
    match (mode, sub) {
        ("pn", '0') => "Power-On test — before HV on",
        ("pn", '2') => "Power-On test — after HV on, 1s after step-start closed",
        ("sb", '0') => "Stand-By — warm-up period or entering Stand-By",
        ("sb", '2') => "Stand-By — after warm-up period",
        ("pr", '0') => "Operate — entering Operate",
        ("pr", '2') => "Operate — during normal operation",
        ("tr", '0') => "T/R relay test — Tx to Rx transition",
        ("tr", '2') => "T/R relay test — Rx to Tx transition",
        ("tr", '4') => "T/R relay test — during Tx (Operate)",
        ("tr", '6') => "T/R relay test — during Rx (Operate)",
        _            => "Unknown operating state",
    }
}

// ============================================================================
// Digital signal registers (identical for 1000/1500/2100)
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct SignalBit {
    pub name:       &'static str,
    pub active:     bool,
    pub active_low: bool,
    pub meaningful: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DigitalRegisters {
    pub buffer0: Vec<SignalBit>,
    pub buffer1: Vec<SignalBit>,
    pub port1:   Vec<SignalBit>,
    pub port3:   Vec<SignalBit>,
    pub port4:   Vec<SignalBit>,
}

fn decode_byte_signals(byte: u8, defs: &[(&'static str, bool, bool); 8]) -> Vec<SignalBit> {
    (0..8).map(|i| {
        let bit_set = (byte >> (7 - i)) & 1 == 1;
        let (name, active_low, meaningful) = defs[i];
        SignalBit {
            name,
            active: if active_low { !bit_set } else { bit_set },
            active_low,
            meaningful,
        }
    }).collect()
}

fn decode_registers(b0: u8, b1: u8, p1: u8, p3: u8, p4: u8) -> DigitalRegisters {
    let buf0_defs: [(&str, bool, bool); 8] = [
        ("n/u",     false, false),
        ("EG2ON",   false, true),
        ("*BYPASS", true,  true),
        ("OPRled",  false, true),
        ("OFFled",  false, true),
        ("Onled",   false, true),
        ("STST",    false, true),
        ("PWRON",   false, true),
    ];
    let buf1_defs: [(&str, bool, bool); 8] = [
        ("ATTled",  false, true),
        ("FANHI",   false, true),
        ("FANON",   false, true),
        ("KEYOUT",  false, true),
        ("ATN",     false, true),
        ("n/u",     false, false),
        ("ENAB",    false, true),
        ("T/*R",    true,  true),
    ];
    let port1_defs: [(&str, bool, bool); 8] = [
        ("SDA",    false, false),
        ("SCL",    false, false),
        ("F",      false, true),
        ("F/64",   false, true),
        ("*PANT",  true,  true),
        ("*ARCF",  true,  true),
        ("ORC",    false, true),
        ("KEYIN",  false, true),
    ];
    let port3_defs: [(&str, bool, bool); 8] = [
        ("*RD",     true,  false),
        ("*WR",     true,  false),
        ("*OLE",    true,  false),
        ("n/u",     false, false),
        ("PSE",     false, true),
        ("*GRIDRF", true,  true),
        ("n/u",     false, false),
        ("n/u",     false, false),
    ];
    let port4_defs: [(&str, bool, bool); 8] = [
        ("FH",       false, true),
        ("n/u",      false, false),
        ("*G1C",     true,  true),
        ("*LOWAIR",  true,  true),
        ("*PREV",    true,  true),
        ("*NEXTbtn", true,  true),
        ("*OPRbtn",  true,  true),
        ("*Onbtn",   true,  true),
    ];
    DigitalRegisters {
        buffer0: decode_byte_signals(b0, &buf0_defs),
        buffer1: decode_byte_signals(b1, &buf1_defs),
        port1:   decode_byte_signals(p1, &port1_defs),
        port3:   decode_byte_signals(p3, &port3_defs),
        port4:   decode_byte_signals(p4, &port4_defs),
    }
}

// ============================================================================
// Analog values — all scaling confirmed from Excel formulas
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct LegacyAnalog {
    /// Forward power W = raw² / 32
    pub pfwd_w:   f32,
    pub pfwd_raw: u8,
    /// Reflected power W = raw² / 128
    pub rfl_w:    f32,
    pub rfl_raw:  u8,
    /// Input drive W = raw² / 512
    pub inp_w:    f32,
    pub inp_raw:  u8,
    /// PA anode voltage V = raw × 12
    pub paav_v:   f32,
    pub paav_raw: u8,
    /// Screen grid current mA = raw / 2
    pub g2c_ma:   f32,
    pub g2c_raw:  u8,
    /// Idle plate current mA = raw × 5
    pub ipm_ma:   u16,
    pub ipm_raw:  u8,
    /// HV plate voltage V = raw × 16
    pub hvm_v:    u16,
    pub hvm_raw:  u8,
    /// Temperature °C = raw × 2 − 273
    pub temp_c:   i16,
    pub temp_raw: u8,
}

fn decode_analog(bytes: &[u8; 18]) -> LegacyAnalog {
    // Group 2 → bytes[3..=5]: pfwd, rfl, inp
    let pfwd_raw = bytes[3];
    let rfl_raw  = bytes[4];
    let inp_raw  = bytes[5];
    // Group 3 → bytes[6..=8]: paav, g2c, ipm
    let paav_raw = bytes[6];
    let g2c_raw  = bytes[7];
    let ipm_raw  = bytes[8];
    // Group 4 → bytes[9..=11]: ??, hvm, temp
    let hvm_raw  = bytes[10];
    let temp_raw = bytes[11];

    LegacyAnalog {
        pfwd_w:   (pfwd_raw as f32 * pfwd_raw as f32) / 32.0,
        pfwd_raw,
        rfl_w:    (rfl_raw as f32 * rfl_raw as f32) / 128.0,
        rfl_raw,
        inp_w:    (inp_raw as f32 * inp_raw as f32) / 512.0,
        inp_raw,
        paav_v:   paav_raw as f32 * 12.0,
        paav_raw,
        g2c_ma:   g2c_raw as f32 / 2.0,
        g2c_raw,
        ipm_ma:   ipm_raw as u16 * 5,
        ipm_raw,
        hvm_v:    hvm_raw as u16 * 16,
        hvm_raw,
        temp_c:   (temp_raw as i16 * 2) - 273,
        temp_raw,
    }
}

// ============================================================================
// Full decoded signature
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct LegacySignature {
    pub model:         LegacyModel,
    pub raw_groups:    Vec<String>,
    pub state:         AmpState,
    pub analog:        LegacyAnalog,
    pub registers:     DigitalRegisters,
    pub substitutions: Vec<CharSubstitution>,
    /// True if checksum verified (confirmed algorithm: seed=0xA5 XOR cascade)
    pub checksum_ok:   bool,
    pub raw_bytes:     Vec<u8>,
}

// ============================================================================
// Parse entry point
// ============================================================================
pub fn parse_legacy(
    model: LegacyModel,
    groups: &[String],
) -> Result<LegacySignature, LegacyParseError> {

    if groups.len() != 6 {
        return Err(LegacyParseError::WrongGroupLength {
            group: 0, actual: groups.len(), expected: 6
        });
    }

    let mut substitutions: Vec<CharSubstitution> = Vec::new();
    let mut all_bytes: Vec<u8> = Vec::new();

    // ── Group 0: sequence counter (1-2 chars) ────────────────────────────────
    let seq_str = groups[0].trim();
    let _sequence = u8::from_str_radix(seq_str, 16).unwrap_or(0);

    // ── Groups 1-6: parse each into 3 bytes, but group 1 chars 3-4 is a
    //    literal mode code not a hex byte
    for (gi, group) in groups.iter().enumerate() {
        let group_idx = gi + 1;
        let trimmed = group.trim().to_lowercase();

        if trimmed.len() != 6 {
            return Err(LegacyParseError::WrongGroupLength {
                group: group_idx, actual: trimmed.len(), expected: 6
            });
        }

        let chars: Vec<char> = trimmed.chars().collect();

        if gi == 0 {
            // Byte 0: chars 0-1 (hex)
            let b0_str: String = chars[0..2].iter().collect();
            all_bytes.push(parse_hex_byte(&b0_str, group_idx, &mut substitutions)?);

            // Bytes 1: chars 2-3 are the MODE CODE ('sb', 'tr', 'pn', 'pr') — NOT hex
            // Store as raw bytes by mapping to a sentinel or just store 0x00
            // We decode mode separately
            all_bytes.push(0x00); // placeholder

            // Byte 2: chars 4-5 (hex)
            let b2_str: String = chars[4..6].iter().collect();
            all_bytes.push(parse_hex_byte(&b2_str, group_idx, &mut substitutions)?);
        } else {
            // Normal groups: 3 hex byte pairs
            for pair in 0..3 {
                let pair_str: String = chars[pair*2..pair*2+2].iter().collect();
                all_bytes.push(parse_hex_byte(&pair_str, group_idx, &mut substitutions)?);
            }
        }
    }

    while all_bytes.len() < 18 { all_bytes.push(0); }
    let bytes: &[u8; 18] = all_bytes[..18].try_into().unwrap();

    // ── Group 1 full decode ───────────────────────────────────────────────────
    // chars [0-1]=trip_number  [2-3]=mode  [4]=sub_state  [5]=fault_signal
    let g1_lower: Vec<char> = groups[0].trim().to_lowercase().chars().collect();
    let trip_number: String = if g1_lower.len() >= 2 {
        format!("{}{}", g1_lower[0], g1_lower[1]).to_uppercase()
    } else { "??".to_string() };

    let mode_chars: String = if g1_lower.len() >= 4 {
        g1_lower[2..4].iter().collect()
    } else { "??".to_string() };
    // sequence is the trip counter from chars 0-1
    let mode = parse_mode_code(&mode_chars);

    let sub_state = if g1_lower.len() >= 5 { g1_lower[4] } else { '?' };
    let state_description = decode_state_description(&mode.code, sub_state);

    let fault_signal_char = if g1_lower.len() >= 6 { g1_lower[5] } else { '?' };
    let fault_signal = decode_fault_signal(fault_signal_char);

    let state = AmpState { trip_number, mode, sub_state, state_description, fault_signal };

    // ── Analog ────────────────────────────────────────────────────────────────
    let analog = decode_analog(bytes);

    // ── Registers: bytes 12-16 ────────────────────────────────────────────────
    let registers = decode_registers(bytes[12], bytes[13], bytes[14], bytes[15], bytes[16]);

    // ── Checksum (confirmed algorithm) ────────────────────────────────────────
    // seed=0xA5 XOR mode_numeric XOR secondary XOR pfwd XOR rfl XOR inp
    //           XOR paav XOR g2c XOR ipm XOR hvm XOR temp
    //           XOR buffer0 XOR buffer1 XOR port1 XOR port3 XOR port4
    // Counter byte (bytes[0]) and GAMA byte (bytes[9]) are NOT included.
    let mode_numeric: u8 = match state.mode.code.as_str() {
        "pn" => 0x01, "pr" => 0x02, "sb" => 0x03, "tr" => 0x04, _ => 0x00,
    };
    let computed_cs = 0xA5u8
        ^ mode_numeric
        ^ bytes[2]   // secondary
        ^ bytes[3]   // pfwd
        ^ bytes[4]   // rfl
        ^ bytes[5]   // inp
        ^ bytes[6]   // paav
        ^ bytes[7]   // g2c
        ^ bytes[8]   // ipm
        ^ bytes[10]  // hvm (bytes[9] = GAMA display byte, not checksummed)
        ^ bytes[11]  // temp
        ^ bytes[12]  // buffer0
        ^ bytes[13]  // buffer1
        ^ bytes[14]  // port1
        ^ bytes[15]  // port3
        ^ bytes[16]; // port4
    let checksum_ok = computed_cs == bytes[17];

    Ok(LegacySignature {
        model,
        raw_groups: groups.to_vec(),
        state,
        analog,
        registers,
        substitutions,
        checksum_ok,
        raw_bytes: all_bytes,
    })
}


// ============================================================================
// Legacy diagnostic engine
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct LegacyFinding {
    pub severity:    &'static str,   // "critical", "warning", "info"
    pub title:       String,
    pub explanation: String,
    pub action:      String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyDiagnostic {
    pub summary:  String,
    pub findings: Vec<LegacyFinding>,
}

pub fn diagnose_legacy(sig: &LegacySignature) -> LegacyDiagnostic {
    let mut findings: Vec<LegacyFinding> = Vec::new();
    let mode  = sig.state.mode.code.as_str();
    let sub   = sig.state.sub_state;
    let fsig  = sig.state.fault_signal.code;
    let hvm   = sig.analog.hvm_v;
    let temp  = sig.analog.temp_c;
    let ipm   = sig.analog.ipm_ma;

    // ── Arc fault — always critical, stop operating ──────────────────────────
    if fsig == 'c' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "Arc fault detected".to_string(),
            explanation: format!(
                "The protection tripped due to an arc fault (ARCF) during {}.                  Internal arcing in the tank circuit, output network, or tube envelope.                  This is a serious condition that can cause progressive damage.", 
                sig.state.state_description),
            action: "Do not operate the amplifier. Inspect the tank circuit,                     output network, and tube for signs of arcing or carbonisation.                     Check for internal contamination or damaged components.".to_string(),
        });
    }

    // ── Low airflow ──────────────────────────────────────────────────────────
    if fsig == 'f' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "Low airflow fault".to_string(),
            explanation: "The protection tripped because airflow through the amplifier                          fell below the minimum threshold.                          Without adequate cooling, the tube and components will overheat rapidly.".to_string(),
            action: "Check the cooling fan — is it running? Check for blockage of                     air inlet and exhaust vents. Measure fan current.                     Clean any dust accumulation from the airflow path.".to_string(),
        });
    }

    // ── 24V PSU error ────────────────────────────────────────────────────────
    if fsig == 'e' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "24VDC control supply fault".to_string(),
            explanation: "The 24VDC control power supply failed.                          This powers the control PCB and relay logic.".to_string(),
            action: "Check the 24V regulator and its output capacitors.                     Measure 24V rail voltage. Check for short circuits on the 24V bus.".to_string(),
        });
    }

    // ── HV fault ─────────────────────────────────────────────────────────────
    if fsig == '7' {
        let hvm_context = if hvm == 0 {
            "HV reads zero — protective shutdown completed before readout.".to_string()
        } else {
            format!("HV reads {}V at time of capture.", hvm)
        };

        match (mode, sub) {
            ("pn", '0') => findings.push(LegacyFinding {
                severity: "critical",
                title: "HV failed Power-On self-test (before HV enabled)".to_string(),
                explanation: format!("HV was expected to be absent during this test phase but                              a voltage was detected, or HV failed to reach threshold. {}",
                             hvm_context),
                action: "Check HV PSU. Verify HV enable relay operation.                         Check for HV leakage paths.".to_string(),
            }),
            ("pn", '2') => findings.push(LegacyFinding {
                severity: "critical",
                title: "HV failed to stabilise 1 second after step-start".to_string(),
                explanation: format!("HV did not reach the required level within 1 second                              of the step-start relay closing. {}                              This typically indicates a slow or failing HV PSU.", hvm_context),
                action: "Check HV PSU output under load.                         Check step-start relay contacts.                         Inspect HV filter capacitors.".to_string(),
            }),
            _ => findings.push(LegacyFinding {
                severity: "critical",
                title: "High voltage fault".to_string(),
                explanation: format!("HV fault during {}. {}", 
                    sig.state.state_description, hvm_context),
                action: "Check HV PSU and associated components.                         Verify HV regulation under load.".to_string(),
            }),
        }
    }

    // ── Plate current ────────────────────────────────────────────────────────
    if fsig == '6' {
        let ipm_context = if ipm == 0 {
            "Plate current reads zero at capture time (may have recovered).".to_string()
        } else {
            format!("Plate current reads {}mA at capture time.", ipm)
        };

        let explanation = match (mode, sub) {
            ("sb", _) => format!(
                "Plate current fault during {}. {}                  In Stand-By, the tube draws a small idle current set by the bias voltage.                  A fault here usually indicates a bias supply problem or tube fault.",
                sig.state.state_description, ipm_context),
            ("pr", _) | ("tr", _) => format!(
                "Plate current fault during {}. {}                  Excess plate current during operation indicates the tube is drawing                  more current than permitted — possible drive overdrive,                  bias drift, or tube degradation.",
                sig.state.state_description, ipm_context),
            _ => format!("Plate current fault during {}. {}", 
                sig.state.state_description, ipm_context),
        };

        findings.push(LegacyFinding {
            severity: "critical",
            title: "Plate current protection tripped".to_string(),
            explanation,
            action: "Check bias voltage adjustment. Verify drive level is within                     specification. Check tube condition.                     Measure plate current under controlled conditions.".to_string(),
        });
    }

    // ── Reflected power / SWR ────────────────────────────────────────────────
    if fsig == '2' {
        findings.push(LegacyFinding {
            severity: "warning",
            title: "Excessive reflected power".to_string(),
            explanation: format!(
                "Reflected power exceeded the protection threshold during {}.                  This is almost always an external issue — antenna mismatch,                  connector fault, or feedline problem.",
                sig.state.state_description),
            action: "Check antenna SWR with an external meter.                     Inspect all RF connectors and feedline.                     Verify ATU has a valid tune for the operating frequency.".to_string(),
        });
    }

    // ── Temperature ──────────────────────────────────────────────────────────
    if fsig == '8' {
        let temp_context = if temp < 0 {
            "Temperature reads below 0°C — sensor may be faulty or reading not valid at capture time.".to_string()
        } else {
            format!("Exhaust temperature reads {}°C at capture time.", temp)
        };

        findings.push(LegacyFinding {
            severity: "critical",
            title: "Exhaust air temperature fault".to_string(),
            explanation: format!(
                "Exhaust temperature exceeded the protection limit during {}. {}                  Note: the temperature reading is sampled after fault detection —                  the peak that triggered the fault may have already reduced.",
                sig.state.state_description, temp_context),
            action: "Check fan operation and airflow path.                     Check for blocked vents or high ambient temperature.                     Inspect tube and tank circuit for signs of excessive heating.".to_string(),
        });
    }

    // ── Output relay (ORC) ───────────────────────────────────────────────────
    if fsig == 'b' {
        let during_tr = mode == "tr";
        findings.push(LegacyFinding {
            severity: "critical",
            title: if during_tr { 
                "Output relay (ORC) fault during T/R switching".to_string()
            } else {
                "Output relay (ORC) fault".to_string()
            },
            explanation: format!(
                "The output relay closed (ORC) signal was the cause of protection trip during {}.                  {} The relay either failed to close when expected,                  failed to open, or the ORC sense circuit has a fault.",
                sig.state.state_description,
                if during_tr { 
                    "T/R relay timing faults are common after many switching cycles. " 
                } else { "" }),
            action: "Check output relay coil and contacts.                     Measure relay switching time.                     Inspect ORC sense resistor and comparator circuit on the control PCB.".to_string(),
        });
    }

    // ── Control grid current ─────────────────────────────────────────────────
    if fsig == 'd' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "Control grid current too high (G1C)".to_string(),
            explanation: format!(
                "Excessive control grid current detected during {}.                  High G1 current can indicate grid emission, tube contamination,                  or overdrive of the input stage.",
                sig.state.state_description),
            action: "Check input drive level — reduce to minimum and test.                     Check tube for grid emission.                     Inspect input circuit for excessive grid loading.".to_string(),
        });
    }

    // ── Anode voltage ────────────────────────────────────────────────────────
    if fsig == '4' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "Peak anode voltage fault (PAAV)".to_string(),
            explanation: format!(
                "Peak anode alternate voltage exceeded limits during {}.                  This protects against excessive RF voltage swing on the anode.",
                sig.state.state_description),
            action: "Check HV level. Verify the tank circuit is correctly tuned.                     Check for mismatch between amplifier output and load.".to_string(),
        });
    }

    // ── Screen grid current ──────────────────────────────────────────────────
    if fsig == '5' {
        findings.push(LegacyFinding {
            severity: "critical",
            title: "Screen grid current fault (G2C)".to_string(),
            explanation: format!(
                "Screen grid current exceeded protection threshold during {}.                  High screen current indicates tube problems or incorrect operating conditions.",
                sig.state.state_description),
            action: "Check screen voltage. Reduce drive and retest.                     Check tube condition — high G2 current is often an early sign of tube failure.".to_string(),
        });
    }

    // ── Build summary ─────────────────────────────────────────────────────────
    let summary = format!(
        "Trip {} — {}{}{} — fault: {} ({}) — {}",
        sig.state.trip_number,
        sig.state.mode.code.to_uppercase(),
        sig.state.sub_state,
        sig.state.fault_signal.code.to_uppercase(),
        sig.state.fault_signal.signal_name,
        sig.state.fault_signal.description,
        sig.state.state_description,
    );

    LegacyDiagnostic { summary, findings }
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn groups(g1: &str, g2: &str, g3: &str, g4: &str, g5: &str, g6: &str)
        -> Vec<String>
    {
        vec![g1, g2, g3, g4, g5, g6].into_iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn acom1000_hvm_ipm_temp() {
        let g = groups("3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.analog.hvm_v,  2720, "hvm should be 2720V");
        assert_eq!(sig.analog.ipm_ma, 40,   "ipm should be 40mA");
        assert_eq!(sig.analog.temp_c, 23,   "temp should be 23°C");
    }

    #[test]
    fn acom1500_hvm_temp() {
        let g = groups("1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &g).unwrap();
        assert_eq!(sig.analog.hvm_v, 2592, "hvm should be 2592V");
        assert_eq!(sig.analog.temp_c, 41,  "temp should be 41°C");
    }

    #[test]
    fn mode_code_sb() {
        let g = groups("3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.state.mode.code, "sb");
        assert_eq!(sig.state.trip_number, "3A");
        assert_eq!(sig.state.sub_state, '2');
        assert_eq!(sig.state.fault_signal.code, '6');
        assert_eq!(sig.state.fault_signal.signal_name, "ipm");
        assert_eq!(sig.state.state_description, "Stand-By — after warm-up period");
    }

    #[test]
    fn mode_code_tr() {
        let g = groups("1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &g).unwrap();
        assert_eq!(sig.state.mode.code, "tr");
        assert_eq!(sig.state.trip_number, "1A");
        assert_eq!(sig.state.sub_state, '6');
        assert_eq!(sig.state.fault_signal.code, 'b');
        assert_eq!(sig.state.fault_signal.signal_name, "ORC");
        assert_eq!(sig.state.state_description, "T/R relay test — during Rx (Operate)");
    }

    #[test]
    fn registers_1000() {
        let g = groups("3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        let pwron = sig.registers.buffer0.iter().find(|s| s.name == "PWRON").unwrap();
        assert!(pwron.active, "PWRON should be active");
        let fanon = sig.registers.buffer1.iter().find(|s| s.name == "FANON").unwrap();
        assert!(fanon.active, "FANON should be active");
    }

    #[test]
    fn pfwd_scaling() {
        // pfwd raw=8: 8²/32 = 2W
        // Inject via crafting a group2 with '08' in chars 1-2
        let g = groups("3asb26", "080000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert!((sig.analog.pfwd_w - 2.0).abs() < 0.01, "pfwd 8²/32 = 2W");
    }

    #[test]
    fn checksum_valid_1000() {
        let g = groups("3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert!(sig.checksum_ok, "1000 sample checksum should be valid");
    }

    #[test]
    fn checksum_valid_1500() {
        let g = groups("1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &g).unwrap();
        assert!(sig.checksum_ok, "1500 sample checksum should be valid");
    }

    #[test]
    fn temp_formula() {
        // temp raw=0x94=148: 148×2-273=23°C
        let g = groups("3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.analog.temp_c, 23);
        // temp raw=0x9D=157: 157×2-273=41°C
        let g2 = groups("1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig2 = parse_legacy(LegacyModel::Acom1500, &g2).unwrap();
        assert_eq!(sig2.analog.temp_c, 41);
    }
}
