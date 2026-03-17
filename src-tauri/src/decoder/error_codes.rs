// ============================================================================
// error_codes.rs — Complete ACOM fault code table.
//
// 160 codes across 10 groups (0x00-0x9F), embedded as a compile-time const
// array.  Replaces the runtime error_codes.json from the Electron app.
//
// Error bits are stored in line1[0..=9] of the hard-fault signature.
// Group N corresponds to line1[N].  Within each word, bit B corresponds to
// fault code (N << 4) | B, i.e. the lower nibble of the code = bit index.
//
// Active check: (line1_words[code >> 4] >> (code & 0x0F)) & 1 == 1
// ============================================================================

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FaultType {
    HardFault,
    SoftFault,
    Warning,
    Reserved,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ErrorDef {
    pub code: u8,
    pub condition: &'static str,
    pub fault_type: FaultType,
}

impl ErrorDef {
    const fn new(code: u8, condition: &'static str, fault_type: FaultType) -> Self {
        Self { code, condition, fault_type }
    }
    const fn reserved(code: u8) -> Self {
        Self { code, condition: "Reserved", fault_type: FaultType::Reserved }
    }
}

// ============================================================================
// The table.  Every entry uses the code value as its own index (code == index)
// so lookup is O(1): ACOM_ERROR_TABLE[code as usize].
// ============================================================================
pub const ACOM_ERROR_TABLE: [ErrorDef; 160] = [
    // ── Group 0: RF / relay / power sequencing (line1[0]) ───────────────────
    ErrorDef::new(0x00, "HOT SWITCHING ATTEMPT",                        FaultType::Warning),
    ErrorDef::new(0x01, "OUTPUT RELAY CLOSED — SHOULD BE OPEN",         FaultType::Warning),
    ErrorDef::new(0x02, "OUTPUT RELAY OPEN — SHOULD BE CLOSED",         FaultType::HardFault),
    ErrorDef::new(0x03, "DRIVE POWER DETECTED AT WRONG TIME",           FaultType::HardFault),
    ErrorDef::new(0x04, "REFLECTED POWER WARNING",                      FaultType::Warning),
    ErrorDef::new(0x05, "EXCESSIVE REFLECTED POWER",                    FaultType::SoftFault),
    ErrorDef::new(0x06, "DRIVE POWER TOO HIGH",                         FaultType::Warning),
    ErrorDef::new(0x07, "EXCESSIVE DRIVE POWER",                        FaultType::SoftFault),
    ErrorDef::new(0x08, "HOT SWITCHING ATTEMPT",                        FaultType::SoftFault),
    ErrorDef::new(0x09, "DRIVE FREQUENCY OUT OF RANGE",                 FaultType::SoftFault),
    ErrorDef::new(0x0A, "FREQUENCY VIOLATION",                          FaultType::SoftFault),
    ErrorDef::new(0x0B, "OUTPUT DISBALANCE",                            FaultType::SoftFault),
    ErrorDef::new(0x0C, "DETECTED RF POWER AT WRONG TIME",              FaultType::Warning),
    ErrorDef::new(0x0D, "PA LOAD SWR TOO HIGH",                         FaultType::SoftFault),
    ErrorDef::new(0x0E, "STOP TRANSMISSION FIRST",                      FaultType::Warning),
    ErrorDef::new(0x0F, "REMOVE DRIVE POWER IMMEDIATELY",               FaultType::Warning),
    // ── Group 1: PSU voltages / fans / PAM temperatures (line1[1]) ──────────
    ErrorDef::new(0x10, "5V SUPPLY TOO LOW",                            FaultType::HardFault),
    ErrorDef::new(0x11, "5V SUPPLY TOO HIGH",                           FaultType::HardFault),
    ErrorDef::new(0x12, "26V SUPPLY TOO LOW",                           FaultType::HardFault),
    ErrorDef::new(0x13, "26V SUPPLY TOO HIGH",                          FaultType::HardFault),
    ErrorDef::reserved(0x14),
    ErrorDef::new(0x15, "PAM1 FAN SPEED TOO LOW",                       FaultType::HardFault),
    ErrorDef::new(0x16, "PAM2 FAN SPEED TOO LOW",                       FaultType::HardFault),
    ErrorDef::new(0x17, "LPF FAN SPEED TOO LOW",                        FaultType::HardFault),
    ErrorDef::new(0x18, "PAM1 DISSIPATION POWER TOO HIGH",              FaultType::SoftFault),
    ErrorDef::new(0x19, "PAM2 DISSIPATION POWER TOO HIGH",              FaultType::SoftFault),
    ErrorDef::new(0x1A, "PAM1 DISSIPATION POWER WARNING",               FaultType::Warning),
    ErrorDef::new(0x1B, "PAM2 DISSIPATION POWER WARNING",               FaultType::Warning),
    ErrorDef::new(0x1C, "PAM1 TEMPERATURE TOO HIGH",                    FaultType::Warning),
    ErrorDef::new(0x1D, "PAM2 TEMPERATURE TOO HIGH",                    FaultType::Warning),
    ErrorDef::new(0x1E, "PAM1 EXCESSIVE TEMPERATURE",                   FaultType::SoftFault),
    ErrorDef::new(0x1F, "PAM2 EXCESSIVE TEMPERATURE",                   FaultType::SoftFault),
    // ── Group 2: PAM1 HV / current / bias voltages (line1[2]) ───────────────
    ErrorDef::new(0x20, "PAM1 HV TOO LOW",                              FaultType::HardFault),
    ErrorDef::new(0x21, "PAM1 HV TOO HIGH",                             FaultType::HardFault),   // ACTIVE in sample!
    ErrorDef::new(0x22, "PAM1 CURRENT = 0 A (SHOULD BE NON-ZERO)",      FaultType::SoftFault),
    ErrorDef::new(0x23, "PAM1 IDLE CURRENT TOO LOW",                    FaultType::SoftFault),
    ErrorDef::new(0x24, "PAM1 CURRENT WARNING",                         FaultType::Warning),
    ErrorDef::new(0x25, "PAM1 EXCESSIVE CURRENT",                       FaultType::SoftFault),
    ErrorDef::new(0x26, "BIAS_1A VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x27, "BIAS_1B VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x28, "BIAS_1C VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x29, "BIAS_1D VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x2A, "BIAS_1A = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x2B, "BIAS_1B = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x2C, "BIAS_1C = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x2D, "BIAS_1D = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x2E, "PAM1 GAIN TOO LOW",                            FaultType::SoftFault),
    ErrorDef::new(0x2F, "PAM1 GAIN TOO HIGH",                           FaultType::SoftFault),
    // ── Group 3: PAM1 shutdown conditions (line1[3]) ─────────────────────────
    ErrorDef::new(0x30, "PAM1 HV PRESENT — SHOULD BE ZERO",             FaultType::HardFault),
    ErrorDef::new(0x31, "PAM1 CURRENT = 0 A — SHOULD BE ZERO",          FaultType::HardFault),
    ErrorDef::new(0x32, "PAM1 EXCESSIVE TEMPERATURE",                   FaultType::HardFault),
    ErrorDef::new(0x33, "PAM1 TEMPERATURE TOO HIGH",                    FaultType::Warning),
    ErrorDef::new(0x34, "BIAS_1A = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x35, "BIAS_1B = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x36, "BIAS_1C = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x37, "BIAS_1D = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x38, "PSU1 EXCESSIVE TEMPERATURE",                   FaultType::Warning),
    ErrorDef::new(0x39, "PAM1 EXCESSIVE CURRENT — CHECK SWR OR REDUCE DRIVE POWER", FaultType::SoftFault),
    ErrorDef::reserved(0x3A),
    ErrorDef::reserved(0x3B),
    ErrorDef::reserved(0x3C),
    ErrorDef::reserved(0x3D),
    ErrorDef::reserved(0x3E),
    ErrorDef::reserved(0x3F),
    // ── Group 4: PAM2 HV / current / bias voltages (line1[4]) ───────────────
    ErrorDef::new(0x40, "PAM2 HV TOO LOW",                              FaultType::HardFault),
    ErrorDef::new(0x41, "PAM2 HV TOO HIGH",                             FaultType::HardFault),
    ErrorDef::new(0x42, "PAM2 CURRENT = 0 A (SHOULD BE NON-ZERO)",      FaultType::SoftFault),
    ErrorDef::new(0x43, "PAM2 IDLE CURRENT TOO LOW",                    FaultType::SoftFault),
    ErrorDef::new(0x44, "PAM2 CURRENT WARNING",                         FaultType::Warning),
    ErrorDef::new(0x45, "PAM2 EXCESSIVE CURRENT",                       FaultType::SoftFault),
    ErrorDef::new(0x46, "BIAS_2A VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x47, "BIAS_2B VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x48, "BIAS_2C VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x49, "BIAS_2D VOLTAGE ERROR",                        FaultType::SoftFault),
    ErrorDef::new(0x4A, "BIAS_2A = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x4B, "BIAS_2B = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x4C, "BIAS_2C = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x4D, "BIAS_2D = 0 V (SHOULD BE NON-ZERO)",           FaultType::HardFault),
    ErrorDef::new(0x4E, "PAM2 GAIN TOO LOW",                            FaultType::SoftFault),
    ErrorDef::new(0x4F, "PAM2 GAIN TOO HIGH",                           FaultType::SoftFault),
    // ── Group 5: PAM2 shutdown conditions (line1[5]) ─────────────────────────
    ErrorDef::new(0x50, "PAM2 HV = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x51, "PAM2 CURRENT = 0 A — SHOULD BE ZERO",          FaultType::HardFault),
    ErrorDef::new(0x52, "PAM2 EXCESSIVE TEMPERATURE",                   FaultType::HardFault),
    ErrorDef::new(0x53, "PAM2 TEMPERATURE TOO HIGH",                    FaultType::Warning),
    ErrorDef::new(0x54, "BIAS_2A = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x55, "BIAS_2B = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x56, "BIAS_2C = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x57, "BIAS_2D = 0 V — SHOULD BE ZERO",               FaultType::HardFault),
    ErrorDef::new(0x58, "PSU2 EXCESSIVE TEMPERATURE",                   FaultType::Warning),
    ErrorDef::new(0x59, "PAM2 EXCESSIVE CURRENT — CHECK SWR OR REDUCE DRIVE POWER", FaultType::SoftFault),
    ErrorDef::reserved(0x5A),
    ErrorDef::reserved(0x5B),
    ErrorDef::reserved(0x5C),
    ErrorDef::reserved(0x5D),
    ErrorDef::reserved(0x5E),
    ErrorDef::reserved(0x5F),
    // ── Group 6: PSU / system / peripherals (line1[6]) ───────────────────────
    ErrorDef::new(0x60, "PSU1 / CONTROL MALFUNCTION",                   FaultType::HardFault),
    ErrorDef::new(0x61, "PSU2 / CONTROL MALFUNCTION",                   FaultType::HardFault),
    ErrorDef::new(0x62, "PSU1 EXCESSIVE TEMPERATURE",                   FaultType::SoftFault),
    ErrorDef::new(0x63, "PSU2 EXCESSIVE TEMPERATURE",                   FaultType::SoftFault),
    ErrorDef::new(0x64, "DISPLAY UNIT COMMUNICATION ERROR",             FaultType::Warning),
    ErrorDef::new(0x65, "ATU MODEM EXCESSIVE TEMPERATURE",              FaultType::Warning),
    ErrorDef::new(0x66, "ATU POWER SWITCH ALARM",                       FaultType::Warning),
    ErrorDef::new(0x67, "ATU POWER SWITCH ALARM AT POWER ON",           FaultType::Warning),
    ErrorDef::new(0x68, "ETHERNET CONTROLLER NOT RESPONDING",           FaultType::Warning),
    ErrorDef::new(0x69, "AUDIO MEMORY NOT RESPONDING",                  FaultType::Warning),
    ErrorDef::reserved(0x6A),
    ErrorDef::reserved(0x6B),
    ErrorDef::new(0x6C, "LOSS OF AUDIO DATA",                           FaultType::Warning),
    ErrorDef::new(0x6D, "LOSS OF ETHERNET SETTINGS",                    FaultType::Warning),
    ErrorDef::new(0x6E, "LOSS OF EEPROM DATA (WARNING)",                FaultType::Warning),
    ErrorDef::new(0x6F, "LOSS OF EEPROM DATA (SOFT FAULT)",             FaultType::SoftFault),
    // ── Group 7: CAT interface (line1[7]) ─────────────────────────────────────
    ErrorDef::new(0x70, "CAT INTERFACE ERROR",                          FaultType::Warning),
    ErrorDef::reserved(0x71),
    ErrorDef::reserved(0x72),
    ErrorDef::reserved(0x73),
    ErrorDef::reserved(0x74),
    ErrorDef::reserved(0x75),
    ErrorDef::reserved(0x76),
    ErrorDef::reserved(0x77),
    ErrorDef::reserved(0x78),
    ErrorDef::reserved(0x79),
    ErrorDef::reserved(0x7A),
    ErrorDef::reserved(0x7B),
    ErrorDef::reserved(0x7C),
    ErrorDef::reserved(0x7D),
    ErrorDef::reserved(0x7E),
    ErrorDef::reserved(0x7F),
    // ── Group 8: ATU / ASEL comms (line1[8]) ─────────────────────────────────
    ErrorDef::new(0x80, "ATU NOT RESPONDING",                           FaultType::Warning),
    ErrorDef::new(0x81, "ATU → AMP COMMUNICATION ERROR",                FaultType::Warning),
    ErrorDef::new(0x82, "AMP → ATU COMMUNICATION ERROR",                FaultType::Warning),
    ErrorDef::new(0x83, "ASEL NOT RESPONDING",                          FaultType::Warning),
    ErrorDef::new(0x84, "ASEL → AMP COMMUNICATION ERROR",               FaultType::Warning),
    ErrorDef::new(0x85, "AMP → ASEL COMMUNICATION ERROR",               FaultType::Warning),
    ErrorDef::new(0x86, "NO TUNING SETTINGS PREPARED",                  FaultType::Warning),
    ErrorDef::new(0x87, "NO ANTENNA SETTINGS PREPARED",                 FaultType::Warning),
    ErrorDef::new(0x88, "ATU CANNOT RE-TUNE WITHOUT RF POWER PRESENT",  FaultType::Warning),
    ErrorDef::new(0x89, "ANTENNA CANNOT CHANGE WITHOUT RF POWER PRESENT", FaultType::Warning),
    ErrorDef::new(0x8A, "ATU TUNING CYCLE UNSUCCESSFUL",                FaultType::Warning),
    ErrorDef::new(0x8B, "ATU MEMORY ERROR",                             FaultType::Warning),
    ErrorDef::reserved(0x8C),
    ErrorDef::reserved(0x8D),
    ErrorDef::reserved(0x8E),
    ErrorDef::reserved(0x8F),
    // ── Group 9: Bias threshold monitoring (line1[9]) ────────────────────────
    ErrorDef::new(0x90, "BIAS_1A OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x91, "BIAS_1B OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x92, "BIAS_1C OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x93, "BIAS_1D OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x94, "BIAS_2A OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x95, "BIAS_2B OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x96, "BIAS_2C OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x97, "BIAS_2D OUTSIDE THRESHOLD RANGE — SELECT OFF IF OK", FaultType::Warning),
    ErrorDef::new(0x98, "BIAS_1A ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x99, "BIAS_1B ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9A, "BIAS_1C ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9B, "BIAS_1D ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9C, "BIAS_2A ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9D, "BIAS_2B ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9E, "BIAS_2C ABOVE HIGH THRESHOLD",                 FaultType::Warning),
    ErrorDef::new(0x9F, "BIAS_2D ABOVE HIGH THRESHOLD",                 FaultType::Warning),
];

// ============================================================================
// Public helpers
// ============================================================================

/// Returns the ErrorDef for a given fault code.  O(1) table lookup.
pub fn lookup(code: u8) -> &'static ErrorDef {
    &ACOM_ERROR_TABLE[code as usize]
}

/// Returns true if the given code is active in the provided 10-word error group
/// array (line1[0..=9]).
///
/// Algorithm:
///   group = code >> 4   (upper nibble → which u16 word)
///   bit   = code & 0x0F (lower nibble → which bit in that word)
///   active = (error_words[group] >> bit) & 1 == 1
pub fn is_active(error_words: &[u16; 10], code: u8) -> bool {
    let group = (code >> 4) as usize;
    let bit   = (code & 0x0F) as u32;
    (error_words[group] >> bit) & 1 == 1
}

/// Collect all active fault codes from a full error group word array.
pub fn active_faults(error_words: &[u16; 10]) -> Vec<&'static ErrorDef> {
    (0x00u8..=0x9F)
        .filter(|&code| is_active(error_words, code))
        .filter(|&code| ACOM_ERROR_TABLE[code as usize].fault_type != FaultType::Reserved)
        .map(|code| &ACOM_ERROR_TABLE[code as usize])
        .collect()
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_correctly_indexed() {
        for (i, entry) in ACOM_ERROR_TABLE.iter().enumerate() {
            assert_eq!(entry.code as usize, i,
                "Entry at index {i} has wrong code value 0x{:02X}", entry.code);
        }
    }

    #[test]
    fn sample_0x0002_line1_word2_activates_0x21() {
        // From sample: line1 = [0,0,0x0002,0,...,0]
        // Code 0x21 = group 2, bit 1. Line1[2] = 0x0002 → bit 1 set → active.
        let mut words = [0u16; 10];
        words[2] = 0x0002;
        assert!(is_active(&words, 0x21), "0x21 PAM1 HV TOO HIGH should be active");
        assert!(!is_active(&words, 0x20), "0x20 should be clear");
        assert!(!is_active(&words, 0x22), "0x22 should be clear");
    }

    #[test]
    fn active_faults_from_sample_returns_0x21() {
        let mut words = [0u16; 10];
        words[2] = 0x0002;
        let faults = active_faults(&words);
        assert_eq!(faults.len(), 1);
        assert_eq!(faults[0].code, 0x21);
        assert_eq!(faults[0].fault_type, FaultType::HardFault);
    }
}
