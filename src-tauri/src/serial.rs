// ============================================================================
// serial.rs — RS232 capture and line grouping.
//
// Architecture:
//   connect() spawns a dedicated OS thread.  The thread owns the SerialPort,
//   reads bytes into a buffer, assembles complete lines, validates them as
//   64-char hex strings, groups 4 valid lines into a FourLineSignature, then
//   fires Tauri events directly via the AppHandle.
//
//   A shared AtomicBool (stop_flag) lets the command layer cleanly terminate
//   the thread without any locks on the hot path.
// ============================================================================

use std::io::Read;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use serialport::SerialPort;
use tauri::{AppHandle, Emitter};

/// Emitted when a single valid hex line arrives.
#[derive(Clone, serde::Serialize)]
pub struct LineReceivedPayload {
    pub line_number: usize,
    pub line: String,
}

/// Emitted when all 4 lines of a complete signature have been captured.
#[derive(Clone, serde::Serialize)]
pub struct SignatureCompletePayload {
    pub lines: [String; 4],
}

/// Returns a stop-flag + join-handle pair.
pub struct SerialHandle {
    pub stop_flag: Arc<AtomicBool>,
    pub thread: Option<thread::JoinHandle<()>>,
}

impl SerialHandle {
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Open a serial port and start the capture thread.
///
/// On success returns a `SerialHandle` that can be used to stop capture.
/// Errors are returned as a String suitable for display in the frontend.
pub fn connect(port_path: &str, app: AppHandle) -> Result<SerialHandle, String> {
    let port = serialport::new(port_path, 9600)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .timeout(Duration::from_millis(500))
        .open()
        .map_err(|e| format!("Failed to open {port_path}: {e}"))?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();

    let thread = thread::spawn(move || {
        capture_loop(port, app, stop_clone);
    });

    Ok(SerialHandle {
        stop_flag,
        thread: Some(thread),
    })
}

/// The inner loop — runs entirely on the capture thread.
fn capture_loop(mut port: Box<dyn SerialPort>, app: AppHandle, stop: Arc<AtomicBool>) {
    let mut byte_buf  = [0u8; 256];
    let mut line_buf  = String::with_capacity(80);
    let mut collected: Vec<String> = Vec::with_capacity(4);

    while !stop.load(Ordering::Relaxed) {
        match port.read(&mut byte_buf) {
            Ok(0) => continue,
            Ok(n) => {
                let chunk = String::from_utf8_lossy(&byte_buf[..n]);
                for ch in chunk.chars() {
                    if ch == '\n' || ch == '\r' {
                        let candidate = line_buf.trim().to_string();
                        line_buf.clear();

                        if is_valid_signature_line(&candidate) {
                            collected.push(candidate.clone());

                            let _ = app.emit("serial-line-received", LineReceivedPayload {
                                line_number: collected.len(),
                                line: candidate,
                            });

                            if collected.len() == 4 {
                                let sig_lines: [String; 4] = [
                                    collected[0].clone(),
                                    collected[1].clone(),
                                    collected[2].clone(),
                                    collected[3].clone(),
                                ];
                                let _ = app.emit("serial-signature-complete",
                                    SignatureCompletePayload { lines: sig_lines });
                                collected.clear();
                            }
                        }
                    } else {
                        line_buf.push(ch);
                        // Safety valve — discard pathologically long lines
                        if line_buf.len() > 200 {
                            line_buf.clear();
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => {
                eprintln!("Serial read error: {e}");
                let _ = app.emit("serial-error", e.to_string());
                break;
            }
        }
    }

    eprintln!("Serial capture thread exiting");
    let _ = app.emit("serial-disconnected", ());
}

/// A valid signature line: 16 4-hex-digit groups separated by spaces.
/// Total length with spaces: 16 × 4 + 15 spaces = 79 chars.
/// Without spaces (raw concat): 64 hex chars.
fn is_valid_signature_line(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() == 64 && compact.chars().all(|c| c.is_ascii_hexdigit())
}

// ============================================================================
// Port enumeration helper (used by list_serial_ports command)
// ============================================================================
#[derive(Debug, Clone, serde::Serialize)]
pub struct PortInfo {
    pub path:          String,
    pub manufacturer:  Option<String>,
    pub serial_number: Option<String>,
    pub product:       Option<String>,
}

pub fn list_ports() -> Vec<PortInfo> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let (manufacturer, serial_number, product) =
                if let serialport::SerialPortType::UsbPort(ref info) = p.port_type {
                    (info.manufacturer.clone(), info.serial_number.clone(), info.product.clone())
                } else {
                    (None, None, None)
                };
            PortInfo {
                path: p.port_name,
                manufacturer,
                serial_number,
                product,
            }
        })
        .collect()
}
