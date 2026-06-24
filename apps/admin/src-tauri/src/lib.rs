use std::sync::Mutex;
use tracing_subscriber::{Layer, prelude::*};

use serde::{Deserialize, Serialize};

mod identity;
mod admin;

pub use identity::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceInfo {
    pub node_id: String,
    pub active: bool,
    pub role: String,
    pub person: String,
    pub name: String,
    pub device_addr: String, // full addr JSON for QUIC connections
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoleInfo {
    pub name: String,
    pub can_open: Vec<String>,
    pub can_write: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrgInfo {
    pub name: String,
}

// All commands use block_on for iroh async ops

#[tauri::command]
fn get_node_id(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(hex::encode(state.node_id()))
}

#[tauri::command]
fn list_orgs(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<OrgInfo>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(state.list_orgs().into_iter().map(|name| OrgInfo { name }).collect())
}

#[tauri::command]
fn create_org(state: tauri::State<'_, Mutex<AppState>>, name: String) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::create_org(&mut state, &name)).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_device(
    state: tauri::State<'_, Mutex<AppState>>,
    org: String, node_id: String, name: String, person: String, role: String, device_addr: Option<String>,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(
        admin::add_device(&mut state, &org, &node_id, &name, &person, &role, &device_addr.unwrap_or_default())
    ).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_device(
    state: tauri::State<'_, Mutex<AppState>>,
    org: String, node_id: String, active: bool, role: Option<String>, name: Option<String>, person: Option<String>,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(
        admin::update_device(&mut state, &org, &node_id, active, role, name, person)
    ).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_devices(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<Vec<DeviceInfo>, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::list_devices(&mut state, &org)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_roles(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<Vec<RoleInfo>, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::list_roles(&mut state, &org)).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_role(
    state: tauri::State<'_, Mutex<AppState>>,
    org: String, name: String, can_open: Vec<String>, can_write: Vec<String>,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::create_role(&mut state, &org, &name, can_open, can_write))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_role(
    state: tauri::State<'_, Mutex<AppState>>,
    org: String, key: String, changes: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::update_role(&mut state, &org, &key, changes))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn network_status(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(admin::network_status(&state))
}

#[tauri::command]
fn get_sync_info(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<admin::SyncInfo, String> {
    let (doc, store, node_id) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let org_state = s.get_org(&org).ok_or_else(|| format!("org {} not found", org))?;
        (org_state.control_doc.clone(), s.store().clone(), s.node_id())
    };
    tauri::async_runtime::block_on(admin::get_sync_info(doc, store, node_id)).map_err(|e| e.to_string())
}

#[tauri::command]
fn send_invite(
    state: tauri::State<'_, Mutex<AppState>>,
    org: String,
    endpoint_addr_json: String,
    role: String,
    name: String,
    person: String,
) -> Result<(), String> {
    let (control_doc, catalogs_doc, operational_doc, payroll_doc, endpoint) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let org_state = s.get_org(&org).ok_or_else(|| format!("org {} not found", org))?;
        (
            org_state.control_doc.clone(),
            org_state.catalogs_doc.clone(),
            org_state.operational_doc.clone(),
            org_state.payroll_doc.clone(),
            s.endpoint().clone(),
        )
    };

    let (node_id_hex, device_addr) = tauri::async_runtime::block_on(admin::send_invite(
        control_doc,
        catalogs_doc,
        operational_doc,
        payroll_doc,
        endpoint,
        &org,
        &endpoint_addr_json,
        &role,
    )).map_err(|e| e.to_string())?;

    let mut s = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::add_device(
        &mut s,
        &org,
        &node_id_hex,
        &name,
        &person,
        &role,
        &device_addr,
    )).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn share_org(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<Vec<String>, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::share_org_tickets(&mut state, &org)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_logs() -> Result<String, String> {
    let log_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path).join("logs")
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix")
            .join("logs")
    };
    let log_file = log_dir.join("syntrix-admin.log");
    if log_file.exists() {
        std::fs::read_to_string(log_file).map_err(|e| e.to_string())
    } else {
        Ok("No logs yet.".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // File logging (rotating daily, kept for 7 days)
    let log_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path).join("logs")
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix")
            .join("logs")
    };
    std::fs::create_dir_all(&log_dir).ok();
    
    let file_appender = tracing_appender::rolling::daily(&log_dir, "syntrix-admin.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Console subscriber
    let console_layer = tracing_subscriber::fmt::layer()
        .with_filter(tracing_subscriber::EnvFilter::new("iroh=debug,syntrix=debug"));
    
    // File subscriber
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_filter(tracing_subscriber::EnvFilter::new("iroh=debug,syntrix=debug"));
    
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();
    
    // Leak the guard to keep the file writer alive for the app lifetime
    std::mem::forget(_guard);

    let app_state = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize iroh")
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(app_state))
        .invoke_handler(tauri::generate_handler![
            get_node_id, list_orgs, create_org,
            add_device, update_device, list_devices, list_roles, network_status,
            share_org, send_invite, get_logs,
            create_role, update_role, get_sync_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-admin");
}
