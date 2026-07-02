use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use syntrix_logging::{LogHandle, LogQuery, LogRecord, LogSummary};

pub mod identity;
pub mod admin;
pub mod audit;
pub mod catchup;
pub mod gossip;
pub mod storage;
pub mod sql_console;

use syntrix_core::ENTITY_NAMES;

pub use identity::AppState;
use identity::default_role_grants;

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
    let mut state = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(
        admin::add_device(&mut state, &org, &node_id, &name, &person, &role, &device_addr.clone().unwrap_or_default())
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
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(admin::get_sync_info(&s, &org))
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

    let (topic_id, admin_addr, can_open, can_write) = {
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

        let org_state = s.get_org(&org).ok_or_else(|| format!("org {} not found", org))?;
        let roles = s.list_org_roles(&org);
        let (co, cw) = roles.iter()
            .find(|r| r.name == role)
            .map(|r| (r.can_open.clone(), r.can_write.clone()))
            .unwrap_or_else(|| {
                let g = default_role_grants(&role);
                (g.can_open, g.can_write)
            });
        let addr = get_admin_addr_string(&s);
        (org_state.topic_id.clone(), addr, co, cw)
    };

    // Dial the client peer BEFORE sending the invite. libp2p request-response
    // requires an active connection — without dial, the invite is silently
    // dropped (OutboundFailure). We do this OUTSIDE the state lock so we don't
    // block other commands during the QUIC handshake wait.
    let mut dialed_any = false;
    if let Ok(addr_data) = serde_json::from_str::<serde_json::Value>(&endpoint_addr_json) {
        let peer_id_b58 = addr_data["peer_id"].as_str().unwrap_or("");
        if let Some(addrs) = addr_data["addrs"].as_array() {
            let p2p_node = {
                let s = state.lock().map_err(|e| e.to_string())?;
                s.p2p().clone()
            };
            for addr_val in addrs {
                if let Some(addr_str) = addr_val.as_str() {
                    let with_p2p = format!("{}/p2p/{}", addr_str, peer_id_b58);
                    match libp2p::Multiaddr::from_str(&with_p2p) {
                        Ok(addr) => {
                            tracing::info!(
                                target: "syntrix",
                                addr = %addr_str,
                                peer = %peer_id_b58,
                                "dialing client peer before invite"
                            );
                            match p2p_node.dial(addr) {
                                Ok(()) => { dialed_any = true; }
                                Err(e) => {
                                    tracing::warn!(
                                        target: "syntrix",
                                        addr = %addr_str,
                                        error = %e,
                                        "dial failed for address"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(target: "syntrix", addr = %addr_str, error = %e, "invalid multiaddr");
                        }
                    }
                }
            }
        }
    }

    if !dialed_any {
        tracing::warn!(
            target: "syntrix",
            "no valid addresses to dial — invite will likely fail"
        );
    }

    // Wait for QUIC handshake + identify to complete (outside the lock).
    // std::thread::sleep is safe here because we're not holding the state lock
    // and don't need a Tokio runtime context.
    std::thread::sleep(Duration::from_millis(1000));

    // Now send the invite via request-response
    {
        let s = state.lock().map_err(|e| e.to_string())?;
        tauri::async_runtime::block_on(admin::send_invite(
            &*s,
            &org,
            &endpoint_addr_json,
            &role,
            topic_id,
            admin_addr,
            can_open,
            can_write,
        )).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn get_admin_addr_string(state: &AppState) -> String {
    let peer_id = state.p2p().local_peer_id();
    let addrs = tauri::async_runtime::block_on(state.p2p().listen_addrs());
    syntrix_core::build_device_addr_string(peer_id, &addrs)
}

#[tauri::command]
fn get_invite_info(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let info = tauri::async_runtime::block_on(admin::get_invite_info(&s, &org)).map_err(|e| e.to_string())?;
    Ok(serde_json::to_string(&info).map_err(|e| e.to_string())?)
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

// Kept as a Tauri command even though the frontend no longer calls it directly (`AuditTrail.tsx`
// now delegates to `run_sql`/`SqlConsole`, Fase 4 tarea 23): it's still a lower-level, filtered
// API exercised by the Rust integration tests (`tests/sync_test.rs`) and available for any
// future consumer that needs structured filtering instead of raw SQL.
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
fn run_sql(
    state: tauri::State<'_, Mutex<AppState>>,
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<sql_console::SqlResult, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sql_console::run_sql_impl(&s, &query, limit.unwrap_or(200), offset.unwrap_or(0)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_saved_views(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<sql_console::SavedView>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sql_console::list_saved_views_impl(&s).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_saved_view(
    state: tauri::State<'_, Mutex<AppState>>,
    name: String,
    sql_query: String,
) -> Result<sql_console::SavedView, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sql_console::create_saved_view_impl(&s, &name, &sql_query).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_saved_view(state: tauri::State<'_, Mutex<AppState>>, id: String) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sql_console::delete_saved_view_impl(&s, &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_schema_registry() -> Result<serde_json::Value, String> {
    let entities: Vec<serde_json::Value> = ENTITY_NAMES.iter().map(|name| {
        serde_json::json!({
            "name": name,
            "version": 1,
            "fields": [],
            "indexes": [],
        })
    }).collect();
    Ok(serde_json::to_value(entities).unwrap_or_default())
}

#[tauri::command]
fn get_endpoint_addr(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(admin::get_endpoint_addr_impl(&s))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path)
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix-admin")
    };

    let log_handle = syntrix_logging::init_logging("admin", data_dir.clone());

    let app_state = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize libp2p")
    });
    if let Err(e) = sql_console::seed_default_saved_views(&app_state) {
        tracing::warn!(target: "syntrix", error = %e, "failed to seed default saved views");
    }

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
            send_invite, get_invite_info, get_endpoint_addr,
            create_role, update_role, get_sync_info, get_schema_registry,
            audit_query,
            run_sql, list_saved_views, create_saved_view, delete_saved_view,
            query_logs, summarize_logs, start_tail_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-admin");
}
