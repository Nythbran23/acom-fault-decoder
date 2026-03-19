#![allow(dead_code)]
// ============================================================================
// signature.rs — Parsing the 4-line ACOM hard-fault signature.
// ============================================================================

use serde::Serialize;
use thiserror::Error;

use crate::decoder::error_codes::{active_faults, FaultType};
use crate::decoder::flags::{UserFlags, AmpFlags1, AmpFlags2};
use crate::decoder::parameters::{
    AmpMode, BandData, BiasParams, BiasSet, AtuSettings, CatParams,
    CurrentParams, Disbalance, LpfStatus, PowerParams, RfParams,
    TempParams, VoltageParams, WorkingClock,
};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Line {line} has {actual} words, expected 16")]
    WrongWordCount { line: usize, actual: usize },
    #[error("Line {line} word {word}: invalid hex '{token}'")]
    InvalidHex { line: usize, word: usize, token: String },
    #[error("Line {line} checksum failed (sum mod 65536 = {sum})")]
    BadChecksum { line: usize, sum: u32 },
}

/// Serialisable fault summary — sent to frontend as part of AcomSignature JSON.
/// Computed at decode time so the frontend needs no bit-check logic.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveFaultSummary {
    pub code:       u8,
    pub condition:  &'static str,
    pub fault_type: FaultType,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignatureWords(pub [[u16; 16]; 4]);

impl SignatureWords {
    pub fn line(&self, n: usize) -> &[u16; 16] { &self.0[n] }
}

#[derive(Debug, Clone, Serialize)]
pub struct AcomSignature {
    pub raw_lines:     [String; 4],
    /// Computed at decode time — ready for frontend rendering.
    pub active_faults: Vec<ActiveFaultSummary>,
    pub error_words:   [u16; 10],
    pub amp_mode:      AmpMode,
    pub jump_state:    u16,
    pub clock:         WorkingClock,
    pub rf:            RfParams,
    pub disbalance:    Disbalance,
    pub voltages:      VoltageParams,
    pub currents:      CurrentParams,
    pub temperatures:  TempParams,
    pub power:         PowerParams,
    pub bias:          BiasParams,
    pub band_data:     BandData,
    pub cat:           CatParams,
    pub lpf_status:    LpfStatus,
    pub atu_settings:  AtuSettings,
    pub error_source:  u16,
    pub user_flags:    UserFlags,
    pub amp_flags1:    AmpFlags1,
    pub amp_flags2:    AmpFlags2,
}

pub fn parse_line(line: &str, line_num: usize) -> Result<[u16; 16], ParseError> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() != 16 {
        return Err(ParseError::WrongWordCount { line: line_num, actual: words.len() });
    }
    let mut result = [0u16; 16];
    for (i, token) in words.iter().enumerate() {
        result[i] = u16::from_str_radix(token, 16).map_err(|_| ParseError::InvalidHex {
            line: line_num, word: i, token: token.to_string(),
        })?;
    }
    let sum: u32 = result.iter().map(|&w| w as u32).sum();
    if sum % 65536 != 0 {
        return Err(ParseError::BadChecksum { line: line_num, sum: sum % 65536 });
    }
    Ok(result)
}

impl TryFrom<[String; 4]> for AcomSignature {
    type Error = ParseError;

    fn try_from(raw: [String; 4]) -> Result<Self, Self::Error> {
        let w0 = parse_line(&raw[0], 1)?;
        let w1 = parse_line(&raw[1], 2)?;
        let w2 = parse_line(&raw[2], 3)?;
        let w3 = parse_line(&raw[3], 4)?;

        let error_words: [u16; 10] = w0[0..10].try_into().unwrap();

        // Compute active faults as a serialisable field
        let active_faults: Vec<ActiveFaultSummary> = active_faults(&error_words)
            .into_iter()
            .map(|e| ActiveFaultSummary {
                code:       e.code,
                condition:  e.condition,
                fault_type: e.fault_type,
            })
            .collect();

        let amp_mode   = AmpMode::from_raw(w0[11]);
        let jump_state = w0[12];
        let clock      = WorkingClock::from_words(w0[13], w0[14]);
        let rf         = RfParams::from_words(&w1);
        let disbalance = Disbalance::from_word(w1[4]);
        let voltages   = VoltageParams::from_words(&w1);
        let currents   = CurrentParams::from_words(&w1);
        let temperatures = TempParams::from_words(&w1, &w2);
        let power      = PowerParams::from_words(&w1, &w2);
        let bias       = BiasParams {
            measured: BiasSet::from_words(&w2),
            nominal:  BiasSet::from_words(&w3),
        };
        let band_data    = BandData { millivolts: w2[11] };
        let cat          = CatParams::from_words(&w2);
        let error_source = w3[8];
        let atu_settings = AtuSettings { raw: w3[10] };
        let lpf_status   = LpfStatus { raw: w3[12] };
        let user_flags   = UserFlags::from_bits_truncate(w3[11]);
        let amp_flags1   = AmpFlags1::from_bits_truncate(w3[13]);
        let amp_flags2   = AmpFlags2::from_bits_truncate(w3[14]);

        Ok(Self {
            raw_lines: raw,
            active_faults,
            error_words,
            amp_mode, jump_state, clock,
            rf, disbalance,
            voltages, currents, temperatures, power,
            bias,
            band_data, cat, lpf_status, atu_settings,
            error_source,
            user_flags, amp_flags1, amp_flags2,
        })
    }
}

impl TryFrom<&[String; 4]> for AcomSignature {
    type Error = ParseError;
    fn try_from(raw: &[String; 4]) -> Result<Self, Self::Error> {
        Self::try_from(raw.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::flags::*;

    fn sample_lines() -> [String; 4] {
        [
            "0000 0000 0002 0000 0000 0000 0000 0000 0000 0000 0000 0041 0040 0000 0863 F71A".into(),
            "0000 0000 0000 0000 0000 00FC 13A0 01FE 0000 0000 0000 0129 0000 0000 0000 E83D".into(),
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0120 0000 01F4 FCEC".into(),
            "0000 0000 0000 0000 0000 0000 0000 0000 003E 0000 0000 8059 0080 0104 0291 7B54".into(),
        ]
    }

    #[test]
    fn parse_full_sample() {
        let sig = AcomSignature::try_from(sample_lines()).expect("sample should parse");

        // active_faults is now a proper serialised field
        assert_eq!(sig.active_faults.len(), 1);
        assert_eq!(sig.active_faults[0].code, 0x21);
        assert_eq!(sig.active_faults[0].condition, "PAM1 HV TOO HIGH");
        assert!(matches!(sig.active_faults[0].fault_type, FaultType::HardFault));

        assert_eq!(sig.clock.total_seconds, 2147);
        assert_eq!(sig.clock.hours,   0);
        assert_eq!(sig.clock.minutes, 35);
        assert_eq!(sig.clock.seconds, 47);

        assert_eq!(sig.voltages.vcc26_v, 25.2);
        assert_eq!(sig.voltages.vcc5_v,  5.024);
        assert_eq!(sig.voltages.hv1_v,   51.0);
        assert!(sig.voltages.hv2_v.is_none());
        assert_eq!(sig.temperatures.pam1_celsius, Some(24.0));

        assert_eq!(sig.cat.settings_raw, 0x0120);
        assert_eq!(sig.cat.command_set, 1);
        assert_eq!(sig.cat.baud_rate_code, 2);
        assert_eq!(sig.cat.poll_interval_ms, 500);

        assert!(sig.user_flags.contains(UserFlags::EXTRA_COOLING));
        assert!(sig.user_flags.contains(UserFlags::SHUTDOWN_HARD_FAULT));
        assert!(sig.amp_flags1.contains(AmpFlags1::TURN_ON_PWR_BTN));
        assert!(sig.amp_flags1.contains(AmpFlags1::BIAS_MON_DISABLED));
        assert!(!sig.amp_flags1.contains(AmpFlags1::HV_ON), "Excel lied — HV was NOT on");
        assert!(sig.amp_flags2.contains(AmpFlags2::KEYIN_MON));
        assert!(sig.amp_flags2.contains(AmpFlags2::TX_ACCESS));
        assert!(sig.amp_flags2.contains(AmpFlags2::ORC_MON));
        assert!(sig.amp_flags2.contains(AmpFlags2::LPF_TUNED));
        assert!(!sig.amp_flags2.contains(AmpFlags2::ORC),             "Excel lied");
        assert!(!sig.amp_flags2.contains(AmpFlags2::ASEL_TUNED),      "Excel lied");
        assert!(!sig.amp_flags2.contains(AmpFlags2::HF_ERR_DISABLED), "Excel lied");

        assert_eq!(sig.error_source, 0x003E);
        assert_eq!(sig.lpf_status.raw, 0x0080);
    }

    #[test]
    fn bad_checksum_rejected() {
        let mut lines = sample_lines();
        lines[0] = lines[0].replacen("F71A", "F71B", 1);
        assert!(AcomSignature::try_from(lines).is_err());
    }

    #[test]
    fn wrong_word_count_rejected() {
        let mut lines = sample_lines();
        lines[1] = "0000 0000".into();
        let err = AcomSignature::try_from(lines).unwrap_err();
        assert!(matches!(err, ParseError::WrongWordCount { line: 2, actual: 2 }));
    }

    #[test]
    fn real_capture_sig1_dec2025() {
        let lines = [
            "0000 0000 0001 0000 0000 0000 0000 0000 0000 0000 0000 0061 0060 0067 4482 BA55".into(),
            "0000 0000 0000 0000 0000 00FD 13DD 0185 0000 0000 0000 0127 0000 0000 0000 E87A".into(),
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0120 0000 012C FDB4".into(),
            "0000 0000 0000 0000 0000 0000 0000 0000 00A4 0000 0000 0059 0008 1D04 1291 CF66".into(),
        ];
        let sig = AcomSignature::try_from(lines).expect("should parse");

        // active_faults field must be populated — this is what broke the frontend
        assert_eq!(sig.active_faults.len(), 1);
        assert_eq!(sig.active_faults[0].code, 0x20);
        assert_eq!(sig.active_faults[0].condition, "PAM1 HV TOO LOW");
        assert!(matches!(sig.active_faults[0].fault_type, FaultType::HardFault));

        assert!((sig.voltages.hv1_v - 38.9).abs() < 0.05);
        assert_eq!(sig.clock.hours,   1879);
        assert_eq!(sig.clock.minutes, 55);
        assert_eq!(sig.clock.seconds, 46);
        assert_eq!(sig.temperatures.pam1_celsius, Some(22.0));
        assert_eq!(sig.cat.command_set, 1);
        assert_eq!(sig.cat.baud_rate_code, 2);
        assert_eq!(sig.cat.poll_interval_ms, 300);
        assert_eq!(sig.error_source, 0x00A4);
        assert!(sig.amp_flags1.contains(AmpFlags1::HV_ON));
        assert!(sig.amp_flags1.contains(AmpFlags1::TURN_ON_PWR_BTN));
        assert!(!sig.user_flags.contains(UserFlags::SHUTDOWN_HARD_FAULT));
    }
}
