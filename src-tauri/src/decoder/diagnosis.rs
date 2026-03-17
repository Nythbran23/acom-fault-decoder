// ============================================================================
// diagnosis.rs — High-level diagnostic interpretation engine.
//
// Cross-correlates decoded fault codes, voltage readings, temperatures,
// runtime, and flag states to produce human-readable findings with
// explanations and recommended actions.
//
// This is intentionally separate from the raw decode logic — it adds
// engineering judgement on top of the data.
// ============================================================================

use serde::Serialize;
use crate::decoder::error_codes::FaultType;
use crate::decoder::flags::{AmpFlags1, AmpFlags2};
use crate::decoder::signature::{AcomSignature, ActiveFaultSummary};

// ============================================================================
// Output types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FindingSeverity {
    /// Actionable hardware fault requiring repair
    Critical,
    /// Degradation or condition requiring attention
    Warning,
    /// Contextual information that affects interpretation
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity:    FindingSeverity,
    pub title:       &'static str,
    pub explanation: String,
    pub action:      String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    /// One-line summary of overall amplifier condition
    pub summary:        String,
    /// Overall severity — worst of all findings
    pub overall:        FindingSeverity,
    /// Ordered list of findings, most severe first
    pub findings:       Vec<Finding>,
    /// True if the signature may underreport faults (HF_ERR_DIS set)
    pub incomplete_data: bool,
}

// ============================================================================
// Engine
// ============================================================================

pub fn diagnose(sig: &AcomSignature) -> DiagnosticReport {
    let mut findings: Vec<Finding> = Vec::new();

    let has_fault = |code: u8| -> bool {
        sig.active_faults.iter().any(|f| f.code == code)
    };

    let has_fault_in_range = |lo: u8, hi: u8| -> bool {
        sig.active_faults.iter().any(|f| f.code >= lo && f.code <= hi)
    };

    let hard_faults: Vec<&ActiveFaultSummary> = sig.active_faults.iter()
        .filter(|f| matches!(f.fault_type, FaultType::HardFault))
        .collect();

    let soft_faults: Vec<&ActiveFaultSummary> = sig.active_faults.iter()
        .filter(|f| matches!(f.fault_type, FaultType::SoftFault))
        .collect();

    let hv_on = sig.amp_flags1.contains(AmpFlags1::HV_ON);
    let hf_disabled = sig.amp_flags2.contains(AmpFlags2::HF_ERR_DISABLED);
    let sf_disabled = sig.amp_flags2.contains(AmpFlags2::SF_ERR_DISABLED);
    let runtime_hours = sig.clock.hours;

    // ── Rule: Hard faults disabled ────────────────────────────────────────────
    if hf_disabled {
        findings.push(Finding {
            severity: FindingSeverity::Warning,
            title: "Hard fault generation was disabled",
            explanation: "AmpFlags2 shows HF_ERR_DIS was set at the time this signature was captured. \
                         This means hard fault conditions that existed may NOT appear in the error words. \
                         The signature may underreport the true fault state.".to_string(),
            action: "Treat fault data with caution. Confirm whether the amplifier was in a \
                    service or calibration procedure. Re-capture with HF generation enabled if possible.".to_string(),
        });
    }

    // ── Rule: Soft faults disabled ────────────────────────────────────────────
    if sf_disabled {
        findings.push(Finding {
            severity: FindingSeverity::Info,
            title: "Soft fault generation was disabled",
            explanation: "Soft fault conditions may have existed without being logged.".to_string(),
            action: "Re-capture with SF generation enabled for a complete picture.".to_string(),
        });
    }

    // ── Rule: Service/factory mode ────────────────────────────────────────────
    let mode_val = match sig.amp_mode {
        crate::decoder::parameters::AmpMode::ServiceTest => Some(0u16),
        crate::decoder::parameters::AmpMode::Unknown(v) => Some(v),
        _ => None,
    };
    if let Some(v) = mode_val {
        if v >= 0x0040 {
            findings.push(Finding {
                severity: FindingSeverity::Info,
                title: "Fault captured in service or factory mode",
                explanation: format!(
                    "Amp mode 0x{:04X} indicates the amplifier was not in normal operating mode \
                     when the fault was logged. Protection thresholds and operating limits \
                     may differ from standard operation.", v),
                action: "Confirm with the operator whether a service procedure was in progress. \
                         Fault data may reflect test conditions rather than real-world failure.".to_string(),
            });
        }
    }

    // ── Rule: 5V fault with voltage reading normal (transient droop) ──────────
    if has_fault(0x10) {
        let vcc5 = sig.voltages.vcc5_v;
        if vcc5 >= 4.9 && vcc5 <= 5.2 {
            findings.push(Finding {
                severity: FindingSeverity::Critical,
                title: "5V supply: transient undervoltage (rail recovered by readout time)",
                explanation: format!(
                    "Hard fault 0x10 (5V TOO LOW) was triggered, but the VCC5 reading in this \
                     signature is {:.3}V — within normal range. This is a timing artefact: the \
                     fault threshold is detected by hardware comparator, but the ADC voltage \
                     reading is sampled slightly later after the rail has recovered. The 5V supply \
                     experienced a real transient dip that crossed the fault threshold.", vcc5),
                action: format!(
                    "The 5V rail is marginal. At {} hours runtime, electrolytic capacitors on the \
                     5V regulator output are the most likely cause — degraded ESR allows ripple \
                     voltage to exceed the fault threshold during load transients. \
                     Measure the 5V rail under load with an oscilloscope to observe the droop. \
                     Inspect and replace output capacitors on the 5V regulator.", runtime_hours),
            });
        } else if vcc5 < 4.8 {
            findings.push(Finding {
                severity: FindingSeverity::Critical,
                title: "5V supply: static undervoltage",
                explanation: format!(
                    "Hard fault 0x10 (5V TOO LOW) is active and VCC5 reads {:.3}V — \
                     statically below the 5.0V nominal. This is a sustained failure, \
                     not a transient.", vcc5),
                action: "Replace the 5V regulator. Before replacement, verify there is no \
                         short circuit load on the 5V bus — check for shorted bypass capacitors. \
                         Measure current draw on the 5V rail.".to_string(),
            });
        }
    }

    // ── Rule: 26V fault ────────────────────────────────────────────────────────
    if has_fault(0x12) || has_fault(0x13) {
        let vcc26 = sig.voltages.vcc26_v;
        let direction = if has_fault(0x12) { "low" } else { "high" };
        findings.push(Finding {
            severity: FindingSeverity::Critical,
            title: "26V supply fault",
            explanation: format!(
                "26V supply is reading {:.1}V (nominal 26.0V) and has triggered a \
                 {} fault. This rail powers the driver stage.", vcc26, direction),
            action: "Check the 26V regulator and its associated filter capacitors. \
                    Verify load current on the 26V rail is within specification.".to_string(),
        });
    }

    // ── Rule: HV low but non-zero (PSU degradation) ───────────────────────────
    if has_fault(0x20) || has_fault(0x40) {
        let hv = sig.voltages.hv1_v;
        if hv > 1.0 && hv < 48.0 {
            findings.push(Finding {
                severity: FindingSeverity::Critical,
                title: "HV supply undervoltage — PSU component degradation likely",
                explanation: format!(
                    "HV1 is reading {:.1}V against a nominal 50V. The rail is present but \
                     producing reduced output — this is characteristic of a PSU component \
                     fault rather than a complete failure. The consistent low reading across \
                     multiple captures (if this is recurring) points to a degraded component \
                     rather than an intermittent fault.", hv),
                action: format!(
                    "At {} hours runtime, the most likely causes in order: \
                     (1) Degraded electrolytic capacitors in the HV supply — measure ESR. \
                     (2) Failing rectifier diode with increased forward voltage drop. \
                     (3) Resistor drift in the voltage feedback divider. \
                     Measure HV supply output under load and compare to schematic nominal.", runtime_hours),
            });
        } else if hv == 0.0 && !hv_on {
            findings.push(Finding {
                severity: FindingSeverity::Info,
                title: "HV reads zero — protective shutdown completed before readout",
                explanation: "HV fault was triggered and the protective shutdown completed before \
                             the voltage readings were sampled. The zero reading reflects post-shutdown \
                             state. The actual HV level at the moment of fault is not captured here.".to_string(),
                action: "Investigate the root cause of the HV fault from the error codes and \
                        other voltage/flag data. The HV supply itself may be functional.".to_string(),
            });
        }
    }

    // ── Rule: Temperature faults ──────────────────────────────────────────────
    let temp_fault = has_fault_in_range(0x1C, 0x1F) || has_fault_in_range(0x32, 0x33)
                  || has_fault_in_range(0x52, 0x53);

    if temp_fault {
        let pam1_temp = sig.temperatures.pam1_celsius;
        let pam2_temp = sig.temperatures.pam2_celsius;
        let max_temp = [pam1_temp, pam2_temp]
            .iter().filter_map(|&t| t)
            .fold(0.0f32, f32::max);

        if max_temp > 70.0 {
            findings.push(Finding {
                severity: FindingSeverity::Critical,
                title: "PAM overtemperature — thermal management failure",
                explanation: format!(
                    "PAM temperature reached {:.0}°C. This exceeds safe operating limits \
                     and indicates a thermal management problem.", max_temp),
                action: "Check and replace thermal compound between PAM modules and heatsink. \
                        Verify fan operation (fault codes 0x15-0x17). \
                        Check heatsink fins for obstruction. \
                        Verify the amplifier is not being overdriven beyond rated input power.".to_string(),
            });
        } else if max_temp < 50.0 && max_temp > 0.0 {
            findings.push(Finding {
                severity: FindingSeverity::Warning,
                title: "Temperature fault — peak not captured in signature",
                explanation: format!(
                    "A temperature fault was logged but the current PAM temperature reads {:.0}°C. \
                     The signature voltage/temperature readings are sampled after fault detection — \
                     the peak temperature that triggered the fault has already reduced by readout time.", max_temp),
                action: "Check fan operation and thermal compound condition. \
                        Consider whether the fault correlates with high ambient temperature \
                        or sustained high-power operation.".to_string(),
            });
        }
    }

    // ── Rule: Fan faults ──────────────────────────────────────────────────────
    if has_fault_in_range(0x15, 0x17) {
        let which = if has_fault(0x15) { "PAM1" }
                    else if has_fault(0x16) { "PAM2" }
                    else { "LPF" };
        findings.push(Finding {
            severity: FindingSeverity::Critical,
            title: "Fan speed fault",
            explanation: format!(
                "The {} fan speed fell below the minimum threshold. Fan faults frequently \
                 precede thermal faults — if unaddressed, PAM overtemperature damage is likely.", which),
            action: format!(
                "Inspect and clean the {} fan. Check fan connector and wiring. \
                 Measure fan current to determine if the motor is failing or stalled. \
                 Replace fan if current is abnormal or fan does not spin freely.", which),
        });
    }

    // ── Rule: Bias faults ─────────────────────────────────────────────────────
    if has_fault_in_range(0x26, 0x2D) || has_fault_in_range(0x46, 0x4D) {
        findings.push(Finding {
            severity: FindingSeverity::Critical,
            title: "Bias voltage fault",
            explanation: "One or more PAM bias voltages are outside the expected range. \
                         Bias faults indicate a problem with the gate bias supply circuit \
                         or a failed transistor in the PAM module.".to_string(),
            action: "Compare measured bias voltages (in Bias section above) against \
                    nominal set values. A bias reading at zero or significantly off nominal \
                    points to a failed transistor or bias regulator. \
                    Contact ACOM service — PAM module replacement may be required.".to_string(),
        });
    }

    // ── Rule: SWR / RF faults ─────────────────────────────────────────────────
    if has_fault(0x05) || has_fault(0x0D) {
        findings.push(Finding {
            severity: FindingSeverity::Warning,
            title: "Excessive reflected power / SWR fault",
            explanation: "The amplifier detected excessive reflected power from the antenna system. \
                         This is almost always an external issue — antenna mismatch, connector fault, \
                         or feedline problem — rather than an amplifier fault.".to_string(),
            action: "Check antenna SWR with an external meter. Inspect all RF connectors \
                    between amplifier and antenna. Check for open or short circuit in feedline. \
                    Ensure the ATU (if fitted) has a valid tune for the current frequency.".to_string(),
        });
    }

    // ── Rule: CAT communication faults ───────────────────────────────────────
    if has_fault(0x70) {
        findings.push(Finding {
            severity: FindingSeverity::Warning,
            title: "CAT interface communication error",
            explanation: "The amplifier lost CAT communication with the transceiver. \
                         This may cause incorrect frequency tracking and could prevent \
                         the amplifier from selecting the correct LPF band.".to_string(),
            action: "Check CAT cable connections. Verify baud rate and protocol settings \
                    match between amplifier and transceiver. Try power-cycling both units.".to_string(),
        });
    }

    // ── Rule: ATU/ASEL comms faults ────────────────────────────────────────────
    if has_fault_in_range(0x80, 0x8B) {
        findings.push(Finding {
            severity: FindingSeverity::Warning,
            title: "ATU or ASEL communication fault",
            explanation: "Communication with the antenna tuner or antenna selector has failed. \
                         The amplifier may be operating without proper tuning data.".to_string(),
            action: "Check ATU/ASEL interconnect cables. Power cycle the ATU/ASEL unit. \
                    Verify the ATU is compatible with this amplifier firmware version.".to_string(),
        });
    }

    // ── Rule: HV control sequence mid-flight ─────────────────────────────────
    if sig.amp_flags1.contains(AmpFlags1::HV_CTRL_REQ)
    && sig.amp_flags1.contains(AmpFlags1::HV_MON_DISABLED) {
        findings.push(Finding {
            severity: FindingSeverity::Info,
            title: "HV control sequence was in progress at capture time",
            explanation: "AmpFlags1 shows both HV_CTRL_REQ and HV_MON_DISABLED active — \
                         the firmware was executing an HV on/off procedure when this signature \
                         was logged. This is a secondary artefact of the fault response, \
                         not an independent fault.".to_string(),
            action: "Focus on the primary fault code(s) above. \
                    This flag combination is expected during a protective shutdown sequence.".to_string(),
        });
    }

    // ── Rule: High runtime advisory ───────────────────────────────────────────
    if runtime_hours > 5000 && !sig.active_faults.is_empty() {
        findings.push(Finding {
            severity: FindingSeverity::Info,
            title: format!("High runtime: {} hours — electrolytic capacitors may be degraded",
                          runtime_hours).leak(),
            explanation: format!(
                "At {} hours, electrolytic capacitors throughout the amplifier are approaching \
                 or beyond their typical service life (3000-5000h at operating temperature). \
                 Many fault modes at this runtime — 5V transients, HV regulation drift, \
                 thermal issues — have capacitor degradation as the root cause.", runtime_hours),
            action: "Consider a preventive electrolytic capacitor replacement service, \
                    particularly targeting: HV supply bulk and filter caps, \
                    5V and 26V regulator output caps, bias supply filter caps. \
                    This is often more cost-effective than fault-chasing individual components.".to_string(),
        });
    }

    // ── Rule: Error source context ────────────────────────────────────────────
    if sig.error_source != 0 {
        findings.push(Finding {
            severity: FindingSeverity::Info,
            title: "Firmware diagnostic code logged",
            explanation: format!(
                "The firmware logged internal diagnostic code 0x{:04X} alongside this fault. \
                 This is an ACOM-internal reference that identifies the specific firmware \
                 code path that triggered the event.", sig.error_source),
            action: format!(
                "Include 'Error Source: 0x{:04X}' when submitting a fault report to ACOM \
                 technical support — it helps them identify the exact trigger condition.", sig.error_source),
        });
    }

    // ── Build summary ─────────────────────────────────────────────────────────
    let overall = if findings.iter().any(|f| f.severity == FindingSeverity::Critical) {
        FindingSeverity::Critical
    } else if findings.iter().any(|f| f.severity == FindingSeverity::Warning) {
        FindingSeverity::Warning
    } else {
        FindingSeverity::Info
    };

    let summary = build_summary(sig, &hard_faults, &soft_faults, runtime_hours, hf_disabled);

    // Sort: Critical first, then Warning, then Info
    findings.sort_by_key(|f| match f.severity {
        FindingSeverity::Critical => 0,
        FindingSeverity::Warning  => 1,
        FindingSeverity::Info     => 2,
    });

    DiagnosticReport {
        summary,
        overall,
        findings,
        incomplete_data: hf_disabled || sf_disabled,
    }
}

fn build_summary(
    sig: &AcomSignature,
    hard: &[&ActiveFaultSummary],
    soft: &[&ActiveFaultSummary],
    runtime_hours: u32,
    hf_disabled: bool,
) -> String {
    if sig.active_faults.is_empty() && !hf_disabled {
        return format!(
            "No active faults detected at {} hours runtime. Amplifier was operating normally.",
            runtime_hours
        );
    }

    let mut parts: Vec<String> = Vec::new();

    if !hard.is_empty() {
        let names: Vec<&str> = hard.iter().map(|f| f.condition).collect();
        parts.push(format!("{} hard fault{}: {}",
            hard.len(),
            if hard.len() == 1 { "" } else { "s" },
            names.join(", ")
        ));
    }

    if !soft.is_empty() {
        let names: Vec<&str> = soft.iter().map(|f| f.condition).collect();
        parts.push(format!("{} soft fault{}: {}",
            soft.len(),
            if soft.len() == 1 { "" } else { "s" },
            names.join(", ")
        ));
    }

    let warnings_count = sig.active_faults.iter()
        .filter(|f| matches!(f.fault_type, crate::decoder::error_codes::FaultType::Warning))
        .count();
    if warnings_count > 0 {
        parts.push(format!("{} warning{}", warnings_count,
            if warnings_count == 1 { "" } else { "s" }));
    }

    if hf_disabled {
        parts.push("⚠ hard fault generation was disabled — data may be incomplete".to_string());
    }

    format!("{} hours runtime — {}", runtime_hours, parts.join("; "))
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::signature::AcomSignature;

    fn parse(lines: [&str; 4]) -> AcomSignature {
        let owned: [String; 4] = lines.map(|s| s.to_string());
        AcomSignature::try_from(owned).expect("parse failed")
    }

    #[test]
    fn five_volt_transient_identified() {
        // Real capture: 5V fault with VCC5 reading normal at 5.068V
        let sig = parse([
            "0000 0001 0000 0000 0000 0000 0000 0000 0000 0000 0000 0051 0050 016A B4E2 4912",
            "0000 0000 0000 0000 0000 00FB 13CC 0000 0000 0000 0000 0130 0000 0000 0000 EA09",
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0130 0032 012C FD72",
            "0000 0000 0000 0000 0000 0000 0000 0000 0001 0000 0000 0059 0002 0904 0291 F40F",
        ]);
        let report = diagnose(&sig);
        assert!(report.findings.iter().any(|f|
            f.title.contains("transient")),
            "Should identify 5V transient droop");
        assert_eq!(report.overall, FindingSeverity::Critical);
    }

    #[test]
    fn hv_low_identified() {
        // December 2025 captures: HV at 38.9V
        let sig = parse([
            "0000 0000 0001 0000 0000 0000 0000 0000 0000 0000 0000 0061 0060 0067 4482 BA55",
            "0000 0000 0000 0000 0000 00FD 13DD 0185 0000 0000 0000 0127 0000 0000 0000 E87A",
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0120 0000 012C FDB4",
            "0000 0000 0000 0000 0000 0000 0000 0000 00A4 0000 0000 0059 0008 1D04 1291 CF66",
        ]);
        let report = diagnose(&sig);
        assert!(report.findings.iter().any(|f|
            f.title.contains("PSU component degradation")),
            "Should identify HV degradation");
    }

    #[test]
    fn no_faults_gives_clean_report() {
        let sig = parse([
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0002 0000 0000 0863 F79B",
            "0000 0000 0000 0000 0000 00FC 13A0 01FE 0000 0000 0000 0129 0000 0000 0000 E83D",
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0120 0000 01F4 FCEC",
            "0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 0000 8059 0080 0104 0291 7B92",
        ]);
        let report = diagnose(&sig);
        assert!(!report.findings.iter().any(|f|
            f.severity == FindingSeverity::Critical));
    }
}
