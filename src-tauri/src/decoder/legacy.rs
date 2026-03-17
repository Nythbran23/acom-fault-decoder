// ============================================================================
// legacy.rs — ACOM 1000 / 1500 / 2100 hard-fault signature decoder.
//
// These older tube/hybrid amplifiers display fault signatures on their
// front-panel 7-segment display. The operator reads and types them manually.
//
// FORMAT: 1 state group + 6 data groups × 6 characters = 19 display groups
// After 7-seg normalisation: 18 data bytes + 1 checksum byte
//
// Byte layout (confirmed against real captures):
//   [0]      state_hi  — upper nibble of operating phase
//   [1]      state_lo  — lower nibble / sub-state
//   [2-3]    pfwd      — forward power (2 bytes, scaling unconfirmed)
//   [4]      rfl       — reflected power (scaling unconfirmed)
//   [5]      inp       — input drive power (scaling unconfirmed)
//   [6]      paav      — PA anode voltage average (scaling unconfirmed)
//   [7]      g2c       — screen grid current mA (scaling unconfirmed)
//   [8]      ipm       — idle plate current mA, scale = raw × 5  ✓ confirmed
//   [9]      0x00      — padding
//   [10]     hvm       — HV plate voltage V, scale = raw × 16   ✓ confirmed
//   [11]     temp      — temperature °C (thermistor lookup, unconfirmed)
//   [12]     buffer0   — digital I/O register
//   [13]     buffer1   — digital I/O register
//   [14]     port1     — digital I/O register
//   [15]     port3     — digital I/O register
//   [16]     port4     — digital I/O register
//   [17]     checksum  — control sum (algorithm unconfirmed)
//
// Signal register maps are IDENTICAL across all three models.
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
// Error types
// ============================================================================
#[derive(Debug, Error)]
pub enum LegacyParseError {
    #[error("Group {group} has wrong length: got {actual} chars, expected 6")]
    WrongGroupLength { group: usize, actual: usize },

    #[error("Group {group} contains unrecognised character '{ch}' after 7-seg normalisation")]
    UnrecognisedChar { group: usize, ch: char },

    #[error("Checksum mismatch (algorithm unconfirmed — treat as warning)")]
    ChecksumMismatch,
}

// ============================================================================
// 7-segment character normalisation
//
// The ACOM display renders certain byte values using 7-seg segments that look
// like non-hex letters. Map them back to their numeric equivalents.
//
//  Display  →  Hex value  Notes
//  'S'/'s'  →  '5'        5 and S look identical on 7-seg
//  'b'      →  'b'        kept as-is (valid hex)
//  'r'      →  'r'        UNCONFIRMED — flag for user verification
//  't'      →  't'        UNCONFIRMED — flag for user verification
// ============================================================================
fn normalise_7seg(ch: char) -> Result<char, char> {
    match ch {
        '0'..='9' | 'a'..='f' | 'A'..='F' => Ok(ch),
        'S' | 's' => Ok('5'),  // confirmed: s ≡ 5 on 7-seg
        'r' | 'R' => Ok('5'),  // UNCONFIRMED: best guess, flag in output
        't' | 'T' => Ok('4'),  // UNCONFIRMED: best guess, flag in output
        other => Err(other),
    }
}

/// Unconfirmed substitutions that occurred — returned alongside the decoded value
/// so the UI can warn the user to verify those characters.
#[derive(Debug, Clone, Serialize)]
pub struct CharSubstitution {
    pub group:    usize,
    pub position: usize,
    pub original: char,
    pub assumed:  char,
    pub confirmed: bool,
}

// ============================================================================
// Digital I/O signal bit maps
// (Identical for 1000, 1500, 2100)
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct SignalBit {
    pub name:   &'static str,
    pub active: bool,
    /// True = this signal is active-low (asterisk prefix in ACOM docs)
    pub active_low: bool,
    /// True = this is a meaningful diagnostic signal (not n/u)
    pub meaningful: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DigitalRegisters {
    pub buffer0: Vec<SignalBit>,  // byte[12]
    pub buffer1: Vec<SignalBit>,  // byte[13]
    pub port1:   Vec<SignalBit>,  // byte[14]
    pub port3:   Vec<SignalBit>,  // byte[15]
    pub port4:   Vec<SignalBit>,  // byte[16]
}

fn decode_byte_signals(byte: u8, defs: &[(&'static str, bool, bool); 8]) -> Vec<SignalBit> {
    // bit 7 = MSB = col2 in spreadsheet (leftmost signal name)
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

fn decode_registers(bytes: &[u8; 18]) -> DigitalRegisters {
    // buffer0: bits 7→0 = n/u, EG2ON, *BYPASS, OPRled, OFFled, Onled, STST, PWRON
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

    // buffer1: ATTled, FANHI, FANON, KEYOUT, ATN, n/u, ENAB, T/*R
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

    // port1: SDA, SCL, F, F/64, *PANT, *ARCF, ORC, KEYIN
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

    // port3: *RD, *WR, *OLE, n/u, PSE, *GRIDRF, n/u, n/u
    let port3_defs: [(&str, bool, bool); 8] = [
        ("*RD",      true,  false),
        ("*WR",      true,  false),
        ("*OLE",     true,  false),
        ("n/u",      false, false),
        ("PSE",      false, true),
        ("*GRIDRF",  true,  true),
        ("n/u",      false, false),
        ("n/u",      false, false),
    ];

    // port4: FH, n/u, *G1C, *LOWAIR, *PREV, *NEXTbtn, *OPRbtn, *Onbtn
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
        buffer0: decode_byte_signals(bytes[12], &buf0_defs),
        buffer1: decode_byte_signals(bytes[13], &buf1_defs),
        port1:   decode_byte_signals(bytes[14], &port1_defs),
        port3:   decode_byte_signals(bytes[15], &port3_defs),
        port4:   decode_byte_signals(bytes[16], &port4_defs),
    }
}

// ============================================================================
// State decode
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct AmpState {
    pub raw_hi:  u8,
    pub raw_lo:  u8,
    pub phase:   &'static str,
    pub mode:    &'static str,
}

fn decode_state(hi: u8, lo: u8) -> AmpState {
    let phase = match hi {
        0x0 => "Entering",
        0x1 => "During Rx",
        0x2 => "During Tx",
        0x3 => "During",
        0x4 => "Exiting",
        0x5 => "In",
        0x6 => "During",
        0x7 => "In",
        0x8 => "After",
        0x9 => "During",
        0xA => "During",
        0xB => "During",
        0xC => "In",
        0xD => "After",
        0xE => "Error",
        0xF => "Fatal",
        _   => "Unknown",
    };

    let mode = match lo {
        0x0 => "Unknown",
        0x1 => "Oper-T/R",
        0x2 => "StandBy",
        0x3 => "Operate",
        0x4 => "Oper-T/R",
        0x5 => "StandBy",
        0x6 => "Oper-T/R",
        0x7 => "StandBy",
        0x8 => "Operate",
        0x9 => "StandBy",
        0xA => "Oper-T/R",
        0xB => "Operate",
        0xC => "StandBy",
        0xD => "Oper-T/R",
        0xE => "HardFault",
        0xF => "Shutdown",
        _   => "Unknown",
    };

    AmpState { raw_hi: hi, raw_lo: lo, phase, mode }
}

// ============================================================================
// Analog values
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct LegacyAnalog {
    /// Forward power — raw bytes only (scaling unconfirmed)
    pub pfwd_raw:   [u8; 2],
    /// Reflected power — raw byte only (scaling unconfirmed)
    pub rfl_raw:    u8,
    /// Input drive power — raw byte (scaling unconfirmed)
    pub inp_raw:    u8,
    /// PA anode voltage avg — raw byte (scaling unconfirmed)
    pub paav_raw:   u8,
    /// Screen grid current — raw byte (scaling unconfirmed)
    pub g2c_raw:    u8,
    /// Idle plate current mA — confirmed: raw × 5
    pub ipm_ma:     u16,
    /// HV plate voltage V — confirmed: raw × 16
    pub hvm_v:      u16,
    /// Temperature — raw byte (thermistor lookup, unconfirmed)
    pub temp_raw:   u8,
}

// ============================================================================
// Full decoded signature
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct LegacySignature {
    pub model:           LegacyModel,
    pub raw_groups:      Vec<String>,
    pub state:           AmpState,
    pub analog:          LegacyAnalog,
    pub registers:       DigitalRegisters,
    /// Characters that required unconfirmed 7-seg substitution
    pub substitutions:   Vec<CharSubstitution>,
    /// True if checksum verified (algorithm confirmed)
    pub checksum_ok:     Option<bool>,
    /// Raw bytes for reference
    pub raw_bytes:       Vec<u8>,
}

// ============================================================================
// Parsing entry point
// ============================================================================

/// Parse a legacy ACOM signature from 7 display groups.
///
/// groups[0] = state display (2 chars typically, e.g. "62")
/// groups[1..=5] = data groups (6 chars each)
/// groups[6] = final group containing checksum as last 2 chars
pub fn parse_legacy(
    model: LegacyModel,
    groups: &[String],
) -> Result<LegacySignature, LegacyParseError> {

    if groups.len() != 7 {
        return Err(LegacyParseError::WrongGroupLength {
            group: 0,
            actual: groups.len(),
        });
    }

    let mut substitutions: Vec<CharSubstitution> = Vec::new();
    let mut all_bytes: Vec<u8> = Vec::new();

    // ── State group (groups[0]) — 1 or 2 hex chars ───────────────────────────
    let state_str = groups[0].trim().to_string();
    let (state_hi, state_lo) = if state_str.len() >= 2 {
        let hi_ch = state_str.chars().nth(0).unwrap_or('0');
        let lo_ch = state_str.chars().nth(1).unwrap_or('0');
        (
            u8::from_str_radix(&hi_ch.to_string(), 16).unwrap_or(0),
            u8::from_str_radix(&lo_ch.to_string(), 16).unwrap_or(0),
        )
    } else if state_str.len() == 1 {
        let hi_ch = state_str.chars().next().unwrap_or('0');
        (u8::from_str_radix(&hi_ch.to_string(), 16).unwrap_or(0), 0)
    } else {
        (0, 0)
    };

    // ── Data groups (groups[1..=6]) — 6 chars each = 3 bytes each ────────────
    for (gi, group) in groups[1..].iter().enumerate() {
        let group_idx = gi + 1;
        let trimmed = group.trim().to_lowercase();

        if trimmed.len() != 6 {
            return Err(LegacyParseError::WrongGroupLength {
                group: group_idx,
                actual: trimmed.len(),
            });
        }

        let chars: Vec<char> = trimmed.chars().collect();

        // Process 3 byte pairs
        for pair in 0..3 {
            let c0 = chars[pair * 2];
            let c1 = chars[pair * 2 + 1];

            let n0 = normalise_7seg(c0).map_err(|_| LegacyParseError::UnrecognisedChar {
                group: group_idx, ch: c0,
            })?;
            let n1 = normalise_7seg(c1).map_err(|_| LegacyParseError::UnrecognisedChar {
                group: group_idx, ch: c1,
            })?;

            // Track unconfirmed substitutions
            let confirmed_subs = ['s'];
            if c0 != n0 {
                substitutions.push(CharSubstitution {
                    group: group_idx, position: pair * 2,
                    original: c0, assumed: n0,
                    confirmed: confirmed_subs.contains(&c0),
                });
            }
            if c1 != n1 {
                substitutions.push(CharSubstitution {
                    group: group_idx, position: pair * 2 + 1,
                    original: c1, assumed: n1,
                    confirmed: confirmed_subs.contains(&c1),
                });
            }

            let byte_str = format!("{}{}", n0, n1);
            let byte = u8::from_str_radix(&byte_str, 16).map_err(|_| {
                LegacyParseError::UnrecognisedChar { group: group_idx, ch: c0 }
            })?;
            all_bytes.push(byte);
        }
    }

    // all_bytes should now be 18 bytes
    while all_bytes.len() < 18 {
        all_bytes.push(0);
    }

    let bytes: &[u8; 18] = all_bytes[..18].try_into().unwrap();

    // ── Checksum — algorithm unconfirmed, attempt sum check ──────────────────
    let data_sum: u16 = bytes[..17].iter().map(|&b| b as u16).sum::<u16>()
        + state_hi as u16 + state_lo as u16;
    let computed_cs = (!data_sum.wrapping_add(1) & 0xFF) as u8;
    let checksum_ok = Some(computed_cs == bytes[17]);

    // ── Analog values ─────────────────────────────────────────────────────────
    let analog = LegacyAnalog {
        pfwd_raw: [bytes[2], bytes[3]],
        rfl_raw:   bytes[4],
        inp_raw:   bytes[5],
        paav_raw:  bytes[6],
        g2c_raw:   bytes[7],
        ipm_ma:    bytes[8] as u16 * 5,
        hvm_v:     bytes[10] as u16 * 16,
        temp_raw:  bytes[11],
    };

    // ── Digital registers ─────────────────────────────────────────────────────
    let registers = decode_registers(bytes);

    // ── State ─────────────────────────────────────────────────────────────────
    let state = decode_state(state_hi, state_lo);

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
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn make_groups(state: &str, g1: &str, g2: &str, g3: &str, g4: &str, g5: &str, g6: &str)
        -> Vec<String>
    {
        vec![state, g1, g2, g3, g4, g5, g6].into_iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn acom1000_sample_hvm_ipm() {
        // Real 1000 sample: hvm=2720V, ipm=40mA
        let groups = make_groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &groups).expect("should parse");

        // HV plate voltage: byte[10]=0xAA, 0xAA*16=2720 ✓
        assert_eq!(sig.analog.hvm_v, 2720, "hvm should be 2720V");

        // Idle plate current: byte[8]=0x08, 0x08*5=40 ✓
        assert_eq!(sig.analog.ipm_ma, 40, "ipm should be 40mA");
    }

    #[test]
    fn acom1500_sample_hvm() {
        // Real 1500 sample: hvm=2592V
        let groups = make_groups("b6", "1atr6b", "000000", "009000", "1BA29D", "F723DE", "D47FC4");
        let sig = parse_legacy(LegacyModel::Acom1500, &groups).expect("should parse");

        // HV plate voltage: byte[10]=0xA2, 0xA2*16=2592 ✓
        assert_eq!(sig.analog.hvm_v, 2592, "hvm should be 2592V");
    }

    #[test]
    fn acom1000_registers_decoded() {
        // 1000 sample: buffer0=03, buffer1=20, port1=DC, port3=D7, port4=7F
        // buffer0=0x03=00000011 → STST and PWRON active
        // buffer1=0x20=00100000 → FANON active
        // port1=0xDC=11011100 → SDA,SCL,F/64,*PANT,*ARCF active
        let groups = make_groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &groups).expect("should parse");

        // PWRON should be active (bit 0 of buffer0=0x03)
        let pwron = sig.registers.buffer0.iter().find(|s| s.name == "PWRON").unwrap();
        assert!(pwron.active, "PWRON should be active");

        // STST should be active (bit 1 of buffer0=0x03)
        let stst = sig.registers.buffer0.iter().find(|s| s.name == "STST").unwrap();
        assert!(stst.active, "STST should be active");

        // FANON should be active (buffer1=0x20=00100000, bit 5 from MSB = FANON)
        let fanon = sig.registers.buffer1.iter().find(|s| s.name == "FANON").unwrap();
        assert!(fanon.active, "FANON should be active");
    }

    #[test]
    fn s_substitution_tracked() {
        let groups = make_groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &groups).unwrap();
        // 's' in '3asb26' should be tracked as a substitution
        let s_sub = sig.substitutions.iter().find(|s| s.original == 's');
        assert!(s_sub.is_some(), "'s' substitution should be tracked");
        assert!(s_sub.unwrap().confirmed, "'s'→'5' should be confirmed");
    }

    #[test]
    fn state_decoded() {
        let groups = make_groups("62", "3asb26", "000000", "000008", "3baa94", "0320dc", "d77fe1");
        let sig = parse_legacy(LegacyModel::Acom1000, &groups).unwrap();
        assert_eq!(sig.state.raw_hi, 6);
        assert_eq!(sig.state.raw_lo, 2);
        assert_eq!(sig.state.mode, "StandBy");
    }
}
