use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;
use std::io::{BufRead, Write};

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
    let mut s = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(admin::send_invite_full(
        &mut s, &org, &endpoint_addr_json, &role, &name, &person,
    )).map_err(|e| e.to_string())
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
    if std::env::args().any(|a| a == "--headless") {
        return run_headless();
    }

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

/// Headless mode: no WebView, reads JSON commands from stdin, writes JSON
/// responses to stdout. Used by binary E2E tests that need to test the real
/// binary (with dial, P2P, Tauri runtime) without a GUI.
fn run_headless() {
    let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path)
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix-admin")
    };

    let _ = syntrix_logging::init_logging("admin-headless", data_dir.clone());

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let mut state = rt.block_on(async {
        AppState::new().await.expect("failed to initialize libp2p")
    });

    let stdin = std::io::stdin();
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    eprintln!("[headless] admin ready, reading commands from stdin...");

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.is_empty() { continue; }

        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(stdout, "{{\"ok\":false,\"error\":\"parse: {e}\"}}");
                let _ = stdout.flush();
                continue;
            }
        };

        let cmd = req["cmd"].as_str().unwrap_or("");
        let result: anyhow::Result<serde_json::Value> = rt.block_on(async {
            match cmd {
                "create_org" => {
                    let name = req["name"].as_str().unwrap_or("");
                    admin::create_org(&mut state, name).await?;
                    Ok(serde_json::json!({"created": true}))
                }
                "create_role" => {
                    let org = req["org"].as_str().unwrap_or("");
                    let name = req["name"].as_str().unwrap_or("");
                    let can_open: Vec<String> = req["can_open"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let can_write: Vec<String> = req["can_write"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    admin::create_role(&mut state, org, name, can_open, can_write).await?;
                    Ok(serde_json::json!({"created": true}))
                }
                "list_roles" => {
                    let org = req["org"].as_str().unwrap_or("");
                    let roles = admin::list_roles(&mut state, org).await?;
                    Ok(serde_json::to_value(roles)?)
                }
                "list_orgs" => {
                    Ok(serde_json::to_value(state.list_orgs())?)
                }
                "get_endpoint_addr" => {
                    let peer_id = state.p2p().local_peer_id();
                    let addrs = state.p2p().listen_addrs().await;
                    let addr = serde_json::json!({
                        "node_id": hex::encode(state.node_id()),
                        "peer_id": peer_id.to_base58(),
                        "addrs": addrs,
                    }).to_string();
                    Ok(serde_json::Value::String(addr))
                }
                "send_invite" => {
                    let org = req["org"].as_str().unwrap_or("");
                    let endpoint = req["endpoint_addr_json"].as_str().unwrap_or("");
                    let role = req["role"].as_str().unwrap_or("");
                    let name = req["name"].as_str().unwrap_or("");
                    let person = req["person"].as_str().unwrap_or("");
                    admin::send_invite_full(&mut state, org, endpoint, role, name, person).await?;
                    Ok(serde_json::json!({"sent": true}))
                }
                "list_devices" => {
                    let org = req["org"].as_str().unwrap_or("");
                    let devs = admin::list_devices(&mut state, org).await?;
                    Ok(serde_json::to_value(devs)?)
                }
                "ping" => Ok(serde_json::json!("pong")),
                _ => Err(anyhow::anyhow!("unknown command: {}", cmd)),
            }
        });

        let response = match result {
            Ok(data) => serde_json::json!({"ok": true, "data": data}),
            Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}),
        };
        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }

    eprintln!("[headless] admin shutting down");
}
