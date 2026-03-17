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
#[derive(Debug, Clone, Serialize)]
pub struct AmpState {
    pub sequence:    u8,       // group 0 digit
    pub mode:        AmpMode,  // group 1 chars 3-4
    pub secondary:   u8,       // group 1 chars 5-6
    pub phase:       &'static str,
}

fn decode_phase(mode_code: &str, secondary: u8) -> &'static str {
    match mode_code {
        "tr" => match secondary >> 4 {
            0x0 => "Switching to RX",
            0x2 => "Switching to TX",
            0x4 => "During TX",
            0x6 => "During RX",
            _   => "T/R transition",
        },
        "sb" => "Stand By",
        "pr" => "Operating",
        "pn" => "Powering On",
        _    => "Unknown",
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
    pub raw_bytes:     Vec<u8>,
}

// ============================================================================
// Parse entry point
// ============================================================================
pub fn parse_legacy(
    model: LegacyModel,
    groups: &[String],
) -> Result<LegacySignature, LegacyParseError> {

    if groups.len() != 7 {
        return Err(LegacyParseError::WrongGroupLength {
            group: 0, actual: groups.len(), expected: 7
        });
    }

    let mut substitutions: Vec<CharSubstitution> = Vec::new();
    let mut all_bytes: Vec<u8> = Vec::new();

    // ── Group 0: sequence counter (1-2 chars) ────────────────────────────────
    let seq_str = groups[0].trim();
    let sequence = u8::from_str_radix(seq_str, 16).unwrap_or(0);

    // ── Groups 1-6: parse each into 3 bytes, but group 1 chars 3-4 is a
    //    literal mode code not a hex byte
    for (gi, group) in groups[1..].iter().enumerate() {
        let group_idx = gi + 1;
        let trimmed = group.trim().to_lowercase();

        if trimmed.len() != 6 {
            return Err(LegacyParseError::WrongGroupLength {
                group: group_idx, actual: trimmed.len(), expected: 6
            });
        }

        let chars: Vec<char> = trimmed.chars().collect();

        if group_idx == 1 {
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

    // ── Mode code from group 1 chars 2-3 ─────────────────────────────────────
    let mode_chars: String = groups[1].trim().to_lowercase().chars().skip(2).take(2).collect();
    let mode = parse_mode_code(&mode_chars);
    let secondary = bytes[2];
    let phase = decode_phase(&mode.code, secondary);

    let state = AmpState { sequence, mode, secondary, phase };

    // ── Analog ────────────────────────────────────────────────────────────────
    let analog = decode_analog(bytes);

    // ── Registers: bytes 12-16 ────────────────────────────────────────────────
    let registers = decode_registers(bytes[12], bytes[13], bytes[14], bytes[15], bytes[16]);

    Ok(LegacySignature {
        model,
        raw_groups: groups.to_vec(),
        state,
        analog,
        registers,
        substitutions,
        raw_bytes: all_bytes,
    })
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn groups(s: &str, g1: &str, g2: &str, g3: &str, g4: &str, g5: &str, g6: &str)
        -> Vec<String>
    {
        vec![s, g1, g2, g3, g4, g5, g6].into_iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn acom1000_hvm_ipm_temp() {
        let g = groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.analog.hvm_v,  2720, "hvm should be 2720V");
        assert_eq!(sig.analog.ipm_ma, 40,   "ipm should be 40mA");
        assert_eq!(sig.analog.temp_c, 23,   "temp should be 23°C");
    }

    #[test]
    fn acom1500_hvm_temp() {
        let g = groups("b6", "1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &g).unwrap();
        assert_eq!(sig.analog.hvm_v, 2592, "hvm should be 2592V");
        assert_eq!(sig.analog.temp_c, 41,  "temp should be 41°C");
    }

    #[test]
    fn mode_code_sb() {
        let g = groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.state.mode.code, "sb");
        assert_eq!(sig.state.mode.description, "Stand By");
    }

    #[test]
    fn mode_code_tr() {
        let g = groups("b6", "1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &g).unwrap();
        assert_eq!(sig.state.mode.code, "tr");
        assert_eq!(sig.state.mode.description, "Oper T/R (Transmit/Receive)");
    }

    #[test]
    fn registers_1000() {
        let g = groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
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
        let g = groups("62", "3asb26", "080000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert!((sig.analog.pfwd_w - 2.0).abs() < 0.01, "pfwd 8²/32 = 2W");
    }

    #[test]
    fn temp_formula() {
        // temp raw=0x94=148: 148×2-273=23°C
        let g = groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &g).unwrap();
        assert_eq!(sig.analog.temp_c, 23);
        // temp raw=0x9D=157: 157×2-273=41°C
        let g2 = groups("b6", "1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig2 = parse_legacy(LegacyModel::Acom1500, &g2).unwrap();
        assert_eq!(sig2.analog.temp_c, 41);
    }
}
