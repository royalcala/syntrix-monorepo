use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use syntrix_logging::{LogHandle, LogQuery, LogRecord, LogSummary};

pub mod identity;
pub mod admin;
pub mod audit;

use syntrix_schema::build_registry;

pub use identity::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceInfo {
    pub node_id: String,
    pub active: bool,
    pub role: String,
    pub person: String,
    pub name: String,
    pub device_addr: String,
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
    let (ctrl_doc, entity_docs, store, registry, secret) = {
        let mut state = state.lock().map_err(|e| e.to_string())?;
        tauri::async_runtime::block_on(
            admin::add_device(&mut state, &org, &node_id, &name, &person, &role, &device_addr.clone().unwrap_or_default())
        ).map_err(|e| e.to_string())?;
        let org_state = state.get_org(&org).ok_or_else(|| format!("org {} not found", org))?;
        (
            org_state.control_doc.clone(),
            org_state.entity_docs.clone(),
            state.store().clone(),
            state.registry().clone(),
            state.secret().clone(),
        )
    };
    let entity_docs_vec: Vec<iroh_docs::api::Doc> = entity_docs.into_values().collect();
    tauri::async_runtime::block_on(identity::sync_and_populate_org_members_impl(
        ctrl_doc, entity_docs_vec, store, registry, secret, org,
    )).map_err(|e| e.to_string())
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

fn parse_invite_endpoint(endpoint_addr_json: &str) -> Result<(String, String), String> {
    if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(endpoint_addr_json) {
        let node_id_hex = addr_data["node_id"]
            .as_str()
            .ok_or_else(|| "invalid endpoint: missing node_id".to_string())?
            .to_string();
        Ok((node_id_hex, endpoint_addr_json.to_string()))
    } else {
        Ok((endpoint_addr_json.to_string(), endpoint_addr_json.to_string()))
    }
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
    let (node_id_hex, device_addr) = parse_invite_endpoint(&endpoint_addr_json)?;

    {
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
    }

    let (control_doc, entity_docs, endpoint) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let org_state = s.get_org(&org).ok_or_else(|| format!("org {} not found", org))?;
        (
            org_state.control_doc.clone(),
            org_state.entity_docs.clone(),
            s.endpoint().clone(),
        )
    };

    tauri::async_runtime::block_on(admin::send_invite(
        control_doc.clone(),
        entity_docs.clone(),
        endpoint,
        &org,
        &endpoint_addr_json,
        &role,
    )).map_err(|e| e.to_string())?;

    // Bootstrap P2P sync with the new device so it doesn't have to wait for the heartbeat
    let (store, registry, secret) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.store().clone(), s.registry().clone(), s.secret().clone())
    };
    let entity_docs_vec: Vec<iroh_docs::api::Doc> = entity_docs.into_values().collect();
    let _ = tauri::async_runtime::block_on(identity::sync_and_populate_org_members_impl(
        control_doc, entity_docs_vec, store, registry, secret, org,
    ));

    Ok(())
}

#[tauri::command]
fn share_org(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<Vec<String>, String> {
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::share_org_tickets(&mut state, &org)).map_err(|e| e.to_string())
}

#[tauri::command]
fn query_logs(handle: tauri::State<'_, LogHandle>, query: LogQuery) -> Result<Vec<LogRecord>, String> {
    Ok(syntrix_logging::query_logs_impl(&handle, &query))
}

#[tauri::command]
fn summarize_logs(handle: tauri::State<'_, LogHandle>, window_secs: u64) -> Result<LogSummary, String> {
    Ok(syntrix_logging::summarize_logs_impl(
        &handle,
        std::time::Duration::from_secs(window_secs),
    ))
}

#[tauri::command]
fn start_tail_logs(_app: tauri::AppHandle, _handle: tauri::State<'_, LogHandle>) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn audit_query(
    state: tauri::State<'_, Mutex<AppState>>,
    org_id: String,
    filter: Option<audit::AuditFilter>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<audit::AuditEntry>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let f = filter.unwrap_or_default();
    let entries = audit::audit_query(&s, &org_id, &f, limit.unwrap_or(50), offset.unwrap_or(0));
    Ok(entries)
}

#[tauri::command]
fn get_schema_registry() -> Result<serde_json::Value, String> {
    let registry = build_registry();
    Ok(registry.export_json())
}

#[tauri::command]
fn get_endpoint_addr(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let addr = s.endpoint().addr();
    let addrs: Vec<String> = addr.addrs.iter().map(|a| a.to_string()).collect();
    Ok(serde_json::json!({
        "node_id": hex::encode(s.node_id()),
        "addrs": addrs,
    }).to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path)
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix")
    };

    let log_handle = syntrix_logging::init_logging("admin", data_dir.clone());

    let app_state = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize iroh")
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(app_state))
        .manage(log_handle)
        .setup(|app| {
            let handle = app.state::<LogHandle>();
            let rx = handle.subscribe_tail();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use std::time::Duration;
                let mut batch: Vec<LogRecord> = Vec::new();
                loop {
                    match rx.recv_timeout(Duration::from_millis(250)) {
                        Ok(record) => {
                            batch.push(record);
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if !batch.is_empty() {
                                for record in batch.drain(..) {
                                    let _ = app_handle.emit("log_event", &record);
                                }
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    if batch.len() >= 50 {
                        for record in batch.drain(..) {
                            let _ = app_handle.emit("log_event", &record);
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_node_id, list_orgs, create_org,
            add_device, update_device, list_devices, list_roles, network_status,
            share_org, send_invite, get_endpoint_addr,
            create_role, update_role, get_sync_info, get_schema_registry,
            audit_query,
            query_logs, summarize_logs, start_tail_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-admin");
}
