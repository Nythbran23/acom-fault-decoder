use tauri::{AppHandle, State};
use crate::AppState;
use crate::serial::{self, PortInfo, SerialHandle};
use crate::decoder::{AcomSignature, ParseError, diagnose, DiagnosticReport};

#[tauri::command]
pub async fn list_serial_ports() -> Vec<PortInfo> {
    serial::list_ports()
}

#[tauri::command]
pub async fn connect_serial(
    state: State<'_, AppState>,
    port: String,
    app: AppHandle,
) -> Result<(), String> {
    let mut handle = state.serial_handle.lock().unwrap();
    if let Some(h) = handle.as_mut() {
        let h: &mut SerialHandle = h;
        h.stop();
    }
    let new_handle = serial::connect(&port, app)?;
    *handle = Some(new_handle);
    Ok(())
}

#[tauri::command]
pub async fn disconnect_serial(state: State<'_, AppState>) -> Result<(), String> {
    let mut handle = state.serial_handle.lock().unwrap();
    if let Some(h) = handle.as_mut() {
        let h: &mut SerialHandle = h;
        h.stop();
    }
    *handle = None;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct DecodeResponse {
    pub signature: AcomSignature,
    pub diagnosis: DiagnosticReport,
}

#[tauri::command]
pub async fn decode_signature(lines: [String; 4]) -> Result<DecodeResponse, String> {
    let sig = AcomSignature::try_from(lines).map_err(|e: ParseError| e.to_string())?;
    let report = diagnose(&sig);
    Ok(DecodeResponse { signature: sig, diagnosis: report })
}

#[tauri::command]
pub async fn auto_save_signature(
    app: AppHandle,
    data: String,
    filename: String,
) -> Result<String, String> {
    use std::fs;
    use tauri::Manager;
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let save_dir = downloads.join("ACOM_Signatures");
    fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
    let full_path = save_dir.join(&filename);
    fs::write(&full_path, &data).map_err(|e| e.to_string())?;
    Ok(full_path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn save_report(
    app: AppHandle,
    content: String,
    default_name: String,
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file()
        .set_file_name(&default_name)
        .add_filter("JSON Report", &["json"])
        .add_filter("Text Report", &["txt"])
        .blocking_save_file();
    match path {
        Some(p) => {
            let p = p.to_string();
            std::fs::write(&p, content).map_err(|e| e.to_string())?;
            Ok(p)
        }
        None => Err("Cancelled".into()),
    }
}

#[derive(serde::Serialize)]
pub struct LegacyDecodeResponse {
    pub signature:  crate::decoder::LegacySignature,
    pub diagnosis:  crate::decoder::LegacyDiagnostic,
}

#[tauri::command]
pub async fn decode_legacy(
    model: String,
    groups: Vec<String>,
) -> Result<LegacyDecodeResponse, String>
{
    use crate::decoder::{LegacyModel, parse_legacy, diagnose_legacy};
    let model = match model.as_str() {
        "1000" => LegacyModel::Acom1000,
        "1500" => LegacyModel::Acom1500,
        "2100" => LegacyModel::Acom2100,
        other  => return Err(format!("Unknown model: {}", other)),
    };
    let sig = parse_legacy(model, &groups).map_err(|e| e.to_string())?;
    let diagnosis = diagnose_legacy(&sig);
    Ok(LegacyDecodeResponse { signature: sig, diagnosis })
}

#[tauri::command]
pub async fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn open_signatures_folder(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use std::fs;
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let folder = downloads.join("ACOM_Signatures");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&folder).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer").arg(&folder).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(&folder).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_signatures_dir(app: AppHandle) -> Result<String, String> {
    use tauri::Manager;
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let folder = downloads.join("ACOM_Signatures");
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn read_signature_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_and_open_report(
    app: AppHandle,
    html: String,
    filename: String,
) -> Result<String, String> {
    use tauri::Manager;
    use std::fs;
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let folder = downloads.join("ACOM_Signatures");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join(&filename);
    fs::write(&path, html).map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().into_owned();
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&path_str).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd").args(["/C", "start", "", &path_str]).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(&path_str).spawn().map_err(|e| e.to_string())?;
    Ok(path_str)
}
