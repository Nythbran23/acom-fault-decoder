// ============================================================================
// parameters.rs — Decoded operational parameter structs.
//
// Each struct maps to a named section of the ACOM hard-fault signature.
// All scaling (divide-by-10, /1000, -273, etc.) is applied here so callers
// only see real-world units.
// ============================================================================

use serde::Serialize;

// ============================================================================
// Amplifier mode — line1[11]
//
// NOTE: Mode encoding for values above 0x08 is not fully documented.
// 0x0041 from the sample capture is reported as "Service Mode" by the ACOM
// firmware but the exact nibble encoding is unconfirmed without firmware source.
// Additional captures with known states are needed to pin this down.
// ============================================================================
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AmpMode {
    Off,
    Initializing,
    Standby,
    WarmUp,
    Ready,
    Operate,
    Tuning,
    Fault,
    ServiceTest,
    Unknown(u16),
}

impl AmpMode {
    pub fn from_raw(val: u16) -> Self {
        match val {
            0x0000 => Self::Off,
            0x0001 => Self::Initializing,
            0x0002 => Self::Standby,
            0x0003 => Self::WarmUp,
            0x0004 => Self::Ready,
            0x0005 => Self::Operate,
            0x0006 => Self::Tuning,
            0x0007 => Self::Fault,
            0x0008 => Self::ServiceTest,
            _ => Self::Unknown(val),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Off          => "Off / Shutdown",
            Self::Initializing => "Initializing",
            Self::Standby      => "Standby",
            Self::WarmUp       => "Warm-Up",
            Self::Ready        => "Ready",
            Self::Operate      => "Operate / Transmit",
            Self::Tuning       => "Tuning",
            Self::Fault        => "Fault",
            Self::ServiceTest  => "Service / Test Mode",
            Self::Unknown(_)   => "Unknown — needs firmware verification",
        }
    }
}

// ============================================================================
// Working clock — line1[13] (high word) + line1[14] (low word), in seconds.
//
// BUG NOTE: The Electron app used line4[13] (= AmpFlags1) as the low word.
// Correct: clock_low = line1[14].
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WorkingClock {
    pub total_seconds: u32,
    pub hours:   u32,
    pub minutes: u8,
    pub seconds: u8,
}

impl WorkingClock {
    pub fn from_words(high: u16, low: u16) -> Self {
        let total = (high as u32) << 16 | (low as u32);
        Self {
            total_seconds: total,
            hours:   total / 3600,
            minutes: ((total % 3600) / 60) as u8,
            seconds: (total % 60) as u8,
        }
    }

    pub fn display(&self) -> String {
        format!("{}h {:02}m {:02}s", self.hours, self.minutes, self.seconds)
    }
}

// ============================================================================
// RF parameters — line2[0..=3]
// ============================================================================
#[derive(Debug, Clone, Serialize)]
pub struct RfParams {
    /// Carrier frequency in kHz (line2[0] / 10)
    pub frequency_khz: f32,
    /// Forward power in watts (line2[1] / 10)
    pub forward_power_w: f32,
    /// Reflected power in watts (line2[2] / 10)
    pub reflected_power_w: f32,
    /// Input (drive) power in watts (line2[3] / 10)
    pub input_power_w: f32,
    /// Calculated SWR (from fwd / refl).  None if forward power is zero.
    pub swr: Option<f32>,
}

impl RfParams {
    pub fn from_words(line2: &[u16; 16]) -> Self {
        let freq  = line2[0] as f32 / 10.0;
        let fwd   = line2[1] as f32 / 10.0;
        let refl  = line2[2] as f32 / 10.0;
        let input = line2[3] as f32 / 10.0;

        let swr = if fwd > 0.0 {
            let ratio = (refl / fwd).sqrt();
            if ratio < 1.0 {
                Some((1.0 + ratio) / (1.0 - ratio))
            } else {
                None // infinite SWR (full reflection)
            }
        } else {
            None
        };

        Self { frequency_khz: freq, forward_power_w: fwd, reflected_power_w: refl, input_power_w: input, swr }
    }

    pub fn frequency_mhz(&self) -> f32 {
        self.frequency_khz / 1000.0
    }
}

// ============================================================================
// PAMs disbalance — line2[4], raw mV
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Disbalance {
    pub millivolts: i16,
}

impl Disbalance {
    pub fn from_word(val: u16) -> Self {
        Self { millivolts: val as i16 }
    }
}

// ============================================================================
// PSU voltages — line2[5..=7]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VoltageParams {
    /// VCC26 supply rail in volts (line2[5] / 10, nominal 26.0 V)
    pub vcc26_v: f32,
    /// VCC5 supply rail in volts (line2[6] / 1000, nominal 5.0 V)
    pub vcc5_v:  f32,
    /// PAM1 high voltage in volts (line2[7] / 10, nominal 50 V)
    pub hv1_v:   f32,
    /// PAM2 high voltage in volts (line2[8] / 10).  None if PAM2 not fitted.
    pub hv2_v:   Option<f32>,
}

impl VoltageParams {
    pub fn from_words(line2: &[u16; 16]) -> Self {
        Self {
            vcc26_v: line2[5] as f32 / 10.0,
            vcc5_v:  line2[6] as f32 / 1000.0,
            hv1_v:   line2[7] as f32 / 10.0,
            hv2_v:   if line2[8] == 0 { None } else { Some(line2[8] as f32 / 10.0) },
        }
    }
}

// ============================================================================
// Drain currents — line2[9..=10]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CurrentParams {
    /// PAM1 drain current in amperes (line2[9] / 1000)
    pub pam1_a: f32,
    /// PAM2 drain current in amperes (line2[10] / 1000). None if PAM2 absent.
    pub pam2_a: Option<f32>,
}

impl CurrentParams {
    pub fn from_words(line2: &[u16; 16]) -> Self {
        Self {
            pam1_a: line2[9] as f32 / 1000.0,
            pam2_a: if line2[10] == 0 { None } else { Some(line2[10] as f32 / 1000.0) },
        }
    }
}

// ============================================================================
// Temperatures — line2[11..=12], line3[8..=9]
// Raw encoding: value - 273 = degrees Celsius.  0x0000 = not fitted (-273 °C).
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TempParams {
    /// PAM1 temperature in °C (line2[11] - 273). None if 0x0000 (not fitted).
    pub pam1_celsius: Option<f32>,
    /// PAM2 temperature in °C (line2[12] - 273). None if 0x0000.
    pub pam2_celsius: Option<f32>,
    /// PSU1 temperature in °C (line3[8] / 10). None if 0x0000.
    pub psu1_celsius: Option<f32>,
    /// PSU2 temperature in °C (line3[9] / 10). None if 0x0000.
    pub psu2_celsius: Option<f32>,
}

impl TempParams {
    pub fn from_words(line2: &[u16; 16], line3: &[u16; 16]) -> Self {
        let decode_kelvin = |val: u16| -> Option<f32> {
            if val == 0 { None } else { Some(val as f32 - 273.0) }
        };
        let decode_psu = |val: u16| -> Option<f32> {
            if val == 0 { None } else { Some(val as f32 / 10.0) }
        };
        Self {
            pam1_celsius: decode_kelvin(line2[11]),
            pam2_celsius: decode_kelvin(line2[12]),
            psu1_celsius: decode_psu(line3[8]),
            psu2_celsius: decode_psu(line3[9]),
        }
    }
}

// ============================================================================
// DC power consumption — line2[13..=14]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PowerParams {
    /// PAM1 DC power consumption in watts (line2[13] / 10)
    pub pam1_dc_w: f32,
    /// PAM2 DC power consumption in watts (line2[14] / 10)
    pub pam2_dc_w: f32,
    /// PAM1 dissipation in watts (line3[8])
    pub pam1_dis_w: f32,
    /// PAM2 dissipation in watts (line3[9])
    pub pam2_dis_w: f32,
}

impl PowerParams {
    pub fn from_words(line2: &[u16; 16], line3: &[u16; 16]) -> Self {
        Self {
            pam1_dc_w:  line2[13] as f32 / 10.0,
            pam2_dc_w:  line2[14] as f32 / 10.0,
            pam1_dis_w: line3[8]  as f32,
            pam2_dis_w: line3[9]  as f32,
        }
    }
}

// ============================================================================
// Bias voltages (measured) — line3[0..=7]
// Bias voltages (set/nominal) — line4[0..=7]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BiasSet {
    pub v_1a: f32, pub v_1b: f32, pub v_1c: f32, pub v_1d: f32,
    pub v_2a: f32, pub v_2b: f32, pub v_2c: f32, pub v_2d: f32,
}

impl BiasSet {
    pub fn from_words(words: &[u16; 16]) -> Self {
        let f = |i: usize| words[i] as f32 / 1000.0;
        Self {
            v_1a: f(0), v_1b: f(1), v_1c: f(2), v_1d: f(3),
            v_2a: f(4), v_2b: f(5), v_2c: f(6), v_2d: f(7),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BiasParams {
    pub measured: BiasSet,
    pub nominal:  BiasSet,
}

// ============================================================================
// Analog band data — line3[11], raw mV
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct BandData {
    pub millivolts: u16,
}

// ============================================================================
// CAT interface settings — line3[12..=14]
// line3[12]: packed — type / command set / baud rate
// line3[13]: byte spacing in µs
// line3[14]: polling interval in ms
//
// NOTE: The sub-field boundaries within line3[12] are unconfirmed — the Excel
// source was also affected by the DEC2HEX truncation bug.  These values need
// verification against known CAT configurations.
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CatParams {
    pub settings_raw:    u16,
    pub interface_type:  u8,   // (settings_raw >> 8) & 0x0F — UNVERIFIED field boundary
    pub command_set:     u8,   // (settings_raw >> 4) & 0x0F — 1 = ICOM/compat
    pub baud_rate_code:  u8,   // settings_raw & 0x0F        — 2 = 4800 bps
    pub byte_spacing_us: u16,
    pub poll_interval_ms: u16,
}

impl CatParams {
    pub fn from_words(line3: &[u16; 16]) -> Self {
        let raw = line3[12];
        Self {
            settings_raw:     raw,
            interface_type:   ((raw >> 12) & 0x0F) as u8,  // confirmed from real captures
            command_set:      ((raw >> 8)  & 0x0F) as u8,  // 1 = ICOM ✓
            baud_rate_code:   ((raw >> 4)  & 0x0F) as u8,  // 2 = 4800 bps ✓
            byte_spacing_us:  line3[13],
            poll_interval_ms: line3[14],
        }
    }

    pub fn baud_rate_hz(&self) -> Option<u32> {
        match self.baud_rate_code {
            1 => Some(1200),
            2 => Some(4800),
            3 => Some(9600),
            4 => Some(19200),
            _ => None,
        }
    }

    pub fn command_set_name(&self) -> &'static str {
        match self.command_set {
            1 => "ICOM and Compatibles",
            2 => "Yaesu",
            3 => "Kenwood",
            4 => "Elecraft",
            _ => "Unknown",
        }
    }
}

// ============================================================================
// LPF status — line4[12]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct LpfStatus {
    pub raw: u16,
}

// ============================================================================
// ATU / ASEL settings — line4[10]
// ============================================================================
#[derive(Debug, Clone, Copy, Serialize)]
pub struct AtuSettings {
    pub raw: u16,
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_clock_from_sample() {
        // Sample: line1[13]=0x0000, line1[14]=0x0863=2147 seconds
        let clock = WorkingClock::from_words(0x0000, 0x0863);
        assert_eq!(clock.total_seconds, 2147);
        assert_eq!(clock.hours,   0);
        assert_eq!(clock.minutes, 35);
        assert_eq!(clock.seconds, 47);
        assert_eq!(clock.display(), "0h 35m 47s");
    }

    #[test]
    fn working_clock_large_value() {
        // 3 hours 4 minutes 5 seconds = 3*3600 + 4*60 + 5 = 11045 seconds
        let clock = WorkingClock::from_words(0x0000, 11045);
        assert_eq!(clock.hours, 3);
        assert_eq!(clock.minutes, 4);
        assert_eq!(clock.seconds, 5);
    }

    #[test]
    fn working_clock_32bit_span() {
        // Use high word: 1 * 65536 + 0 = 65536 seconds = 18h 12m 16s
        let clock = WorkingClock::from_words(0x0001, 0x0000);
        assert_eq!(clock.total_seconds, 65536);
        assert_eq!(clock.hours, 18);
        assert_eq!(clock.minutes, 12);
        assert_eq!(clock.seconds, 16);
    }

    #[test]
    fn swr_calculated_correctly() {
        let mut line2 = [0u16; 16];
        line2[1] = 1000; // 100 W forward
        line2[2] = 100;  // 10 W reflected
        let rf = RfParams::from_words(&line2);
        let swr = rf.swr.unwrap();
        // rho = sqrt(10/100) = sqrt(0.1) ≈ 0.3162
        // SWR = (1+0.3162)/(1-0.3162) ≈ 1.924
        assert!((swr - 1.924).abs() < 0.01, "SWR = {swr}");
    }

    #[test]
    fn swr_is_none_when_no_forward_power() {
        let line2 = [0u16; 16];
        let rf = RfParams::from_words(&line2);
        assert!(rf.swr.is_none());
    }

    #[test]
    fn temp_zero_is_none() {
        let line2 = [0u16; 16];
        let line3 = [0u16; 16];
        let t = TempParams::from_words(&line2, &line3);
        assert!(t.pam1_celsius.is_none(), "PAM1 0x0000 = not fitted");
        assert!(t.pam2_celsius.is_none(), "PAM2 0x0000 = not fitted");
    }

    #[test]
    fn pam1_temp_from_sample() {
        // line2[11] = 0x0129 = 297 → 297 - 273 = 24 °C
        let mut line2 = [0u16; 16];
        line2[11] = 0x0129;
        let line3 = [0u16; 16];
        let t = TempParams::from_words(&line2, &line3);
        assert_eq!(t.pam1_celsius, Some(24.0));
    }

    #[test]
    fn vcc26_from_sample() {
        // line2[5] = 0x00FC = 252 → 252/10 = 25.2 V
        let mut line2 = [0u16; 16];
        line2[5] = 0x00FC;
        let v = VoltageParams::from_words(&line2);
        assert_eq!(v.vcc26_v, 25.2);
    }
}
