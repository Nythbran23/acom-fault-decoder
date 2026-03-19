#![allow(dead_code)]
// ============================================================================
// flags.rs — Bit-flag words from Line 4 of the ACOM hard-fault signature.
//
// IMPORTANT: Excel spreadsheet bug note
// ======================================================
// The original Excel tool (ACOM_500S_Hard_Faults_Signatures_Converter_Tool.xls)
// contains a string-slicing bug. It uses DEC2HEX() which drops leading zeros,
// then splits the result with LEFT()/RIGHT() to get high/low bytes.
//
// For any flag word < 0x1000 (the common case), DEC2HEX produces a 3-character
// string, so LEFT/RIGHT split on the WRONG byte boundary, shifting all bits
// above bit 7 by 4 positions and producing entirely wrong flag decodes.
//
// Example — AmpFlags1 sample value 0x0104:
//   Excel:  DEC2HEX(260) = "104" → LEFT = "10", RIGHT = "04"
//           0x10 high byte sets bit 12 → Excel falsely reports HV_ON
//   Rust:   u16::from_str_radix("0104", 16) = 260 = 0b0000_0001_0000_0100
//           Bit 2 = TURN_ON_PWR_BTN ✓   Bit 8 = BIAS_MON_DISABLED ✓   HV_ON = 0 ✓
//
// The sequential bit definitions below (0 = LSB) are correct as per ACOM
// hardware documentation. Only the Excel display was wrong.
// ============================================================================

use bitflags::bitflags;
use serde::Serialize;

// ----------------------------------------------------------------------------
// UserFlags — line4[11], e.g. sample 0x8059
// This one was NOT affected by the Excel bug (0x8059 >= 0x1000).
// ----------------------------------------------------------------------------
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
    pub struct UserFlags: u16 {
        /// FLAG_ExtraCooling — Force temperature cooling before shutdown
        const EXTRA_COOLING        = 1 << 0;
        /// FLAG_AutoOper — Auto-enter Operate after power-on or soft fault
        const AUTO_OPERATE         = 1 << 1;
        /// FLAG_TempUnit — Temperature units: 0 = Celsius, 1 = Fahrenheit
        const TEMP_UNIT_FAHRENHEIT = 1 << 2;
        /// FLAG_Buzzer — Audible alarm enabled
        const BUZZER_ENABLED       = 1 << 3;
        /// FLAG_ATAC_Type — ATAC procedure type: 0 = Manual, 1 = Auto
        const ATAC_TYPE_AUTO       = 1 << 4;
        /// FLAG_PWRBTN_DUR — Power button: 0 = Long ON / Short OFF
        const PWR_BTN_DUR_LONG_ON  = 1 << 5;
        /// FLAG_OPER_ACC — Operate mode access: 1 = Unlocked
        const OPERATE_UNLOCKED     = 1 << 6;
        // Bits 7-14: spare / reserved
        /// FLAG_SHDN_RSN — Last shutdown reason: 1 = Hard Fault, 0 = Normal
        const SHUTDOWN_HARD_FAULT  = 1 << 15;
    }
}

// ----------------------------------------------------------------------------
// AmpFlags1 — line4[13], e.g. sample 0x0104
// Excel bug: DEC2HEX(260) = "104" → falsely reported HV_ON.
// Correct decode: TURN_ON_PWR_BTN (bit 2) + BIAS_MON_DISABLED (bit 8) only.
// ----------------------------------------------------------------------------
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
    pub struct AmpFlags1: u16 {
        /// FLAG_PSU1_TEMPHI — PSU1 thermal condition elevated
        const PSU1_TEMP_HIGH      = 1 << 0;
        /// FLAG_PSU2_TEMPHI — PSU2 thermal condition elevated
        const PSU2_TEMP_HIGH      = 1 << 1;
        /// FLAG_PWRON_BTN — Amplifier turned ON via front-panel button
        const TURN_ON_PWR_BTN     = 1 << 2;
        /// FLAG_PWRON_RMT — Amplifier turned ON via remote CAT signal
        const TURN_ON_REMOTE      = 1 << 3;
        /// FLAG_PWRON_RS — Amplifier turned ON via RS232
        const TURN_ON_RS232       = 1 << 4;
        /// FLAG_Soft_PwrOff — Active firmware-initiated turn-off request
        const SOFT_POWER_OFF_REQ  = 1 << 5;
        /// (unnamed) — Power button turn-off request
        const PWR_BTN_OFF_REQ     = 1 << 6;
        /// FLAG_BIAS_CTRL — Bias voltage ON/OFF procedure requested
        const BIAS_CTRL_REQ       = 1 << 7;
        /// FLAG_BIAS_MON — Bias monitoring temporarily disabled
        const BIAS_MON_DISABLED   = 1 << 8;
        /// FLAG_BIASEN — Bias voltage control signal: 1 = ON
        const BIAS_ENABLED        = 1 << 9;
        /// FLAG_HV_CTRL — High voltage ON/OFF procedure requested
        const HV_CTRL_REQ         = 1 << 10;
        /// FLAG_HV_MON — HV monitoring temporarily disabled
        const HV_MON_DISABLED     = 1 << 11;
        /// FLAG_HV_ON — High voltage control signal: 1 = ON
        const HV_ON               = 1 << 12;
        /// FLAG_ATAC_REQ — ATAC procedure actively requested
        const ATAC_REQ            = 1 << 13;
        /// FLAG_ATAC_InProg — ATAC procedure currently in progress
        const ATAC_IN_PROGRESS    = 1 << 14;
        /// FLAG_Last_Com — Last command source: 0 = Local panel, 1 = RS232
        const LAST_CMD_RS232      = 1 << 15;
    }
}

// ----------------------------------------------------------------------------
// AmpFlags2 — line4[14], e.g. sample 0x0291
// Excel bug: DEC2HEX(657) = "291" → falsely reported ORC, ASEL_TUN, HF_ERR_DIS.
// Correct decode: KEYIN_MON (0) + TX_ACCESS (4) + ORC_MON (7) + LPF_TUNED (9).
// ----------------------------------------------------------------------------
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
    pub struct AmpFlags2: u16 {
        /// FLAG_KEYIN_MON — KEYIN signal monitoring enabled
        const KEYIN_MON           = 1 << 0;
        /// FLAG_KEYIN — KEYIN signal active (TX request present)
        const KEYIN_TX_REQUEST    = 1 << 1;
        /// FLAG_INR_CTRL — Input relay ON/OFF procedure requested
        const INR_CTRL_REQ        = 1 << 2;
        /// FLAG_INR — Input relay status: 1 = Closed
        const INPUT_RELAY_CLOSED  = 1 << 3;
        /// FLAG_TX_ACCESS — TX mode access permitted
        const TX_ACCESS           = 1 << 4;
        /// FLAG_OUTR_CTRL — Output relay ON/OFF procedure requested
        const OUTR_CTRL_REQ       = 1 << 5;
        /// FLAG_OUTR — Output relay status: 1 = Closed
        const OUTPUT_RELAY_CLOSED = 1 << 6;
        /// FLAG_ORC_MON — ORC (Output Relay Closed) signal monitoring enabled
        const ORC_MON             = 1 << 7;
        /// FLAG_ORC — ORC signal status: 1 = Output relay confirmed closed
        const ORC                 = 1 << 8;
        /// FLAG_LPF_TUN (typo in Excel: LAG_LPF_TUN) — LPF band tuned
        const LPF_TUNED           = 1 << 9;
        /// FLAG_ATU_TUN — Antenna tuner tuned (valid only when ATU fitted)
        const ATU_TUNED           = 1 << 10;
        /// FLAG_ASEL_TUN — Antenna selector tuned (valid only when ASEL fitted)
        const ASEL_TUNED          = 1 << 11;
        /// FLAG_I_LKG_MON — Drain current monitoring enabled
        const DRAIN_CURRENT_MON   = 1 << 12;
        /// FLAG_HF_ERR_DIS — Hard fault error generation disabled
        const HF_ERR_DISABLED     = 1 << 13;
        /// FLAG_SF_ERR_DIS — Soft fault error generation disabled
        const SF_ERR_DISABLED     = 1 << 14;
        /// FLAG_WRN_DIS — Warning generation disabled
        const WRN_DISABLED        = 1 << 15;
    }
}

// ============================================================================
// Helper — human-readable list of active flag names for display/report output.
// Separate from Serialize (which gives compact u16) to avoid coupling the wire
// format to the display format.
// ============================================================================

pub trait ActiveFlagNames {
    fn active_names(&self) -> Vec<&'static str>;
}

impl ActiveFlagNames for UserFlags {
    fn active_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.contains(Self::EXTRA_COOLING)        { v.push("Extra Cooling Enabled"); }
        if self.contains(Self::AUTO_OPERATE)         { v.push("Auto Operate Enabled"); }
        if self.contains(Self::TEMP_UNIT_FAHRENHEIT) { v.push("Temperature: Fahrenheit"); }
        if self.contains(Self::BUZZER_ENABLED)       { v.push("Buzzer Enabled"); }
        if self.contains(Self::ATAC_TYPE_AUTO)       { v.push("ATAC: Auto"); }
        if self.contains(Self::PWR_BTN_DUR_LONG_ON)  { v.push("Power Button: Long ON / Short OFF"); }
        if self.contains(Self::OPERATE_UNLOCKED)     { v.push("Operate Access: Unlocked"); }
        if self.contains(Self::SHUTDOWN_HARD_FAULT)  { v.push("Last Shutdown: Hard Fault"); }
        v
    }
}

impl ActiveFlagNames for AmpFlags1 {
    fn active_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.contains(Self::PSU1_TEMP_HIGH)     { v.push("PSU1 Temperature High"); }
        if self.contains(Self::PSU2_TEMP_HIGH)     { v.push("PSU2 Temperature High"); }
        if self.contains(Self::TURN_ON_PWR_BTN)    { v.push("Turn ON Source: Front Panel Button"); }
        if self.contains(Self::TURN_ON_REMOTE)     { v.push("Turn ON Source: Remote CAT"); }
        if self.contains(Self::TURN_ON_RS232)      { v.push("Turn ON Source: RS232"); }
        if self.contains(Self::SOFT_POWER_OFF_REQ) { v.push("Firmware Turn-Off Requested"); }
        if self.contains(Self::PWR_BTN_OFF_REQ)    { v.push("Power Button Turn-Off Requested"); }
        if self.contains(Self::BIAS_CTRL_REQ)      { v.push("Bias Control Request Active"); }
        if self.contains(Self::BIAS_MON_DISABLED)  { v.push("Bias Monitoring Disabled"); }
        if self.contains(Self::BIAS_ENABLED)       { v.push("Bias Voltages: ON"); }
        if self.contains(Self::HV_CTRL_REQ)        { v.push("HV Control Request Active"); }
        if self.contains(Self::HV_MON_DISABLED)    { v.push("HV Monitoring Disabled"); }
        if self.contains(Self::HV_ON)              { v.push("High Voltage: ON"); }
        if self.contains(Self::ATAC_REQ)           { v.push("ATAC Procedure Requested"); }
        if self.contains(Self::ATAC_IN_PROGRESS)   { v.push("ATAC Procedure In Progress"); }
        if self.contains(Self::LAST_CMD_RS232)     { v.push("Last Command: RS232 (Remote)"); }
        v
    }
}

impl ActiveFlagNames for AmpFlags2 {
    fn active_names(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.contains(Self::KEYIN_MON)           { v.push("KEYIN Monitoring Enabled"); }
        if self.contains(Self::KEYIN_TX_REQUEST)    { v.push("KEYIN: TX Request Active"); }
        if self.contains(Self::INR_CTRL_REQ)        { v.push("Input Relay Control Request Active"); }
        if self.contains(Self::INPUT_RELAY_CLOSED)  { v.push("Input Relay: Closed"); }
        if self.contains(Self::TX_ACCESS)           { v.push("TX Access: Enabled"); }
        if self.contains(Self::OUTR_CTRL_REQ)       { v.push("Output Relay Control Request Active"); }
        if self.contains(Self::OUTPUT_RELAY_CLOSED) { v.push("Output Relay: Closed"); }
        if self.contains(Self::ORC_MON)             { v.push("ORC Monitoring Enabled"); }
        if self.contains(Self::ORC)                 { v.push("ORC: Output Relay Confirmed Closed"); }
        if self.contains(Self::LPF_TUNED)           { v.push("LPF: Tuned"); }
        if self.contains(Self::ATU_TUNED)           { v.push("ATU: Tuned"); }
        if self.contains(Self::ASEL_TUNED)          { v.push("ASEL: Tuned"); }
        if self.contains(Self::DRAIN_CURRENT_MON)   { v.push("Drain Current Monitoring Enabled"); }
        if self.contains(Self::HF_ERR_DISABLED)     { v.push("Hard Fault Generation: DISABLED"); }
        if self.contains(Self::SF_ERR_DISABLED)     { v.push("Soft Fault Generation: Disabled"); }
        if self.contains(Self::WRN_DISABLED)        { v.push("Warning Generation: Disabled"); }
        v
    }
}

// ============================================================================
// Tests — ground truth from the Excel sample capture.
// These also serve as regression tests proving the Excel bug is NOT replicated.
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_flags_sample_0x8059() {
        // Excel was correct here (0x8059 >= 0x1000, no truncation bug)
        let f = UserFlags::from_bits_truncate(0x8059);
        assert!(f.contains(UserFlags::EXTRA_COOLING),       "bit 0");
        assert!(f.contains(UserFlags::BUZZER_ENABLED),      "bit 3");
        assert!(f.contains(UserFlags::ATAC_TYPE_AUTO),      "bit 4");
        assert!(f.contains(UserFlags::OPERATE_UNLOCKED),    "bit 6");
        assert!(f.contains(UserFlags::SHUTDOWN_HARD_FAULT), "bit 15");
        assert!(!f.contains(UserFlags::AUTO_OPERATE),         "bit 1 clear");
        assert!(!f.contains(UserFlags::TEMP_UNIT_FAHRENHEIT), "bit 2 clear");
        assert!(!f.contains(UserFlags::PWR_BTN_DUR_LONG_ON),  "bit 5 clear");
    }

    #[test]
    fn amp_flags1_sample_0x0104_excel_bug_regression() {
        // Excel DEC2HEX(260) = "104" → LEFT="10", RIGHT="04"
        // Excel falsely reported HV_ON (bit 12).
        // Correct: TURN_ON_PWR_BTN (bit 2) + BIAS_MON_DISABLED (bit 8) only.
        let f = AmpFlags1::from_bits_truncate(0x0104);
        assert!(f.contains(AmpFlags1::TURN_ON_PWR_BTN),   "bit 2 — confirmed");
        assert!(f.contains(AmpFlags1::BIAS_MON_DISABLED), "bit 8 — confirmed");
        assert!(!f.contains(AmpFlags1::HV_ON),            "bit 12 — Excel lied, HV was NOT on");
        assert!(!f.contains(AmpFlags1::PSU1_TEMP_HIGH),   "bit 0 clear");
        assert!(!f.contains(AmpFlags1::BIAS_ENABLED),     "bit 9 clear");
        assert_eq!(f.bits() & !0x0104, 0,                 "no extra bits set");
    }

    #[test]
    fn amp_flags2_sample_0x0291_excel_bug_regression() {
        // Excel DEC2HEX(657) = "291" → LEFT="29", RIGHT="91"
        // 0x29 high byte → Excel falsely reported ORC (bit 8), ASEL_TUNED (bit 11),
        // HF_ERR_DISABLED (bit 13).
        // Correct: KEYIN_MON (0) + TX_ACCESS (4) + ORC_MON (7) + LPF_TUNED (9).
        let f = AmpFlags2::from_bits_truncate(0x0291);
        assert!(f.contains(AmpFlags2::KEYIN_MON),      "bit 0 — confirmed");
        assert!(f.contains(AmpFlags2::TX_ACCESS),      "bit 4 — confirmed");
        assert!(f.contains(AmpFlags2::ORC_MON),        "bit 7 — confirmed");
        assert!(f.contains(AmpFlags2::LPF_TUNED),      "bit 9 — LPF is tuned (Excel missed this)");
        assert!(!f.contains(AmpFlags2::ORC),           "bit 8 — Excel lied, ORC relay was OPEN");
        assert!(!f.contains(AmpFlags2::ASEL_TUNED),    "bit 11 — Excel lied, ASEL not tuned");
        assert!(!f.contains(AmpFlags2::HF_ERR_DISABLED), "bit 13 — Excel lied, HF errors were ENABLED");
        assert_eq!(f.bits() & !0x0291, 0,              "no extra bits set");
    }
}
