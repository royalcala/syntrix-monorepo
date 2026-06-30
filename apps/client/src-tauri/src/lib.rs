use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use syntrix_logging::{LogHandle, LogQuery, LogRecord, LogSummary};

pub mod identity;
pub mod events;
pub mod sync;
pub mod invite;
mod seed;
pub mod audit;
pub mod indexes;
pub mod search;
pub mod gossip;
pub mod catchup;

use syntrix_schema::build_registry;

#[derive(Debug, Serialize, Deserialize)]
pub struct SchemaFilter {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaQuery {
    pub filters: Option<Vec<SchemaFilter>>,
    pub sort: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub use identity::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrgInfo { pub id: String, pub name: String, pub role: String }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Invoice { pub id: String, pub customer_id: String, pub total: f64, pub date: String }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product { pub id: String, pub name: String, pub price: f64 }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Customer { pub id: String, pub name: String }

#[tauri::command]
fn get_node_id(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(hex::encode(s.node_id()))
}

#[tauri::command]
fn list_orgs(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<OrgInfo>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.list_orgs())
}

#[tauri::command]
fn set_active_org(state: tauri::State<'_, Mutex<AppState>>, org_id: String) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.set_active_org(&org_id)
}

#[tauri::command]
fn commit_event(state: tauri::State<'_, Mutex<AppState>>, app: tauri::AppHandle, event_type: String, payload: String) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let res = events::commit_event(&s, &event_type, &payload).map_err(|e| e.to_string());
    if res.is_ok() {
        let _ = app.emit("entity_changed", ());
    }
    res
}

#[tauri::command]
async fn join_org(state: tauri::State<'_, Mutex<AppState>>, invite_json: String, org_name: Option<String>) -> Result<OrgInfo, String> {
    let name = org_name.unwrap_or_else(|| "org-unknown".into());
    let invite: invite::InvitePayload = serde_json::from_str(&invite_json)
        .map_err(|e| format!("invalid invite json: {}", e))?;

    let topic_id_bytes = hex::decode(&invite.topic_id).map_err(|e| format!("invalid topic_id: {e}"))?;
    let mut topic_id = [0u8; 32];
    let len = topic_id_bytes.len().min(32);
    topic_id[..len].copy_from_slice(&topic_id_bytes[..len]);
    let role = invite.role.clone();
    let admin_addr = invite.admin_addr.clone();
    let can_open = invite.can_open.clone();
    let can_write = invite.can_write.clone();
    let final_org_id = hex::encode(&topic_id[..4]);

    {
        let mut s = state.lock().map_err(|e| e.to_string())?;

        s.add_org(&final_org_id, &name, &role, topic_id, admin_addr.clone());

        let node_id = s.node_id();

        if let Ok(mut reg) = s.registry().write() {
            let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
            reg.set_topic_id(final_org_id.clone(), iroh_topic_id);
            reg.upsert_device(
                final_org_id.clone(), node_id,
                syntrix_core::registry::Device {
                    node_id, active: true, role: role.clone(),
                    person: hex::encode(node_id),
                    name: format!("Device {}", &hex::encode(node_id)[..8]),
                },
            );
            reg.upsert_role(final_org_id.clone(), role.clone(),
                syntrix_core::registry::RoleGrants { can_open: can_open.clone(), can_write: can_write.clone() });
            eprintln!("[join_org] registered device org={} node_hex={} role={} can_open={:?}", final_org_id, hex::encode(node_id), role, can_open);
        }

        // Write role to redb so the frontend can query it via query_entity("roles")
        let role_json = serde_json::json!({
            "name": role,
            "can_open": can_open,
            "can_write": can_write,
        });
        let _ = s.indexer.upsert_role_cfg(&final_org_id, &role, &role_json);

        s.save_org_config(identity::ClientOrgConfig {
            org_id: final_org_id.clone(),
            name: name.clone(),
            role: role.clone(),
            topic_id: hex::encode(topic_id),
            admin_addr: admin_addr.clone(),
            can_open: can_open.clone(),
            can_write: can_write.clone(),
        }).ok();
    }

    // Join gossip topic with admin as bootstrap
    let (gossip_bus, indexer, node_id_hex, iroh_topic_id, bootstrap) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let gossip_bus = s.gossip_bus.clone();
        let indexer = s.indexer.clone();
        let node_id_hex = hex::encode(s.node_id());
        let iroh_topic_id = iroh_gossip::TopicId::from_bytes(topic_id);
        let bootstrap = if let Some(ref addr) = admin_addr {
            if let Some(ep) = syntrix_core::parse_device_addr(addr) {
                vec![ep.id]
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        (gossip_bus, indexer, node_id_hex, iroh_topic_id, bootstrap)
    };
    {
        let mut bus = gossip_bus.write().await;
        let _ = bus.join_org(&final_org_id, iroh_topic_id, bootstrap, indexer.clone(), node_id_hex).await;
    }

    // Catch-up from admin
    if let Some(ref addr) = admin_addr {
        let endpoint = {
            let s = state.lock().map_err(|e| e.to_string())?;
            s.endpoint().clone()
        };
        let indexer = {
            let s = state.lock().map_err(|e| e.to_string())?;
            s.indexer.clone()
        };
        let _ = catchup::CatchupProtocol::request_catchup(
            &endpoint, addr, &final_org_id, 0, &indexer,
        ).await;
    }

    // Write self as member to redb and start heartbeat
    let node_id_hex2 = hex::encode(state.lock().map_err(|e| e.to_string())?.node_id());
    let self_member = serde_json::json!({
        "node_id": node_id_hex2,
        "active": true,
        "role": role,
        "person": node_id_hex2,
        "name": format!("Device {}", &node_id_hex2[..8]),
    });
    let _ = indexer.upsert_member(&final_org_id, &node_id_hex2, &self_member);

    let hb_broadcast = {
        let bus = gossip_bus.clone();
        let org = final_org_id.clone();
        Arc::new(move |json: &str| {
            let mut guard = bus.blocking_write();
            let bytes = bytes::Bytes::copy_from_slice(json.as_bytes());
            let handle = tokio::runtime::Handle::current();
            let _ = handle.block_on(guard.broadcast(&org, bytes));
        })
    };
    let hb_store = {
        let idx = indexer.clone();
        let org = final_org_id.clone();
        Arc::new(move |ts: i64, nid: &str| {
            let hb = serde_json::json!({"ts": ts, "status": "online", "node_id": nid});
            let _ = idx.upsert_heartbeat(&org, nid, &hb);
        })
    };
    syntrix_core::heartbeat::start_heartbeat_with_resync(
        node_id_hex2.clone(), hb_broadcast, hb_store,
    );

    Ok(OrgInfo { id: final_org_id, name, role })
}

#[tauri::command]
fn sync_push(state: tauri::State<'_, Mutex<AppState>>, app: tauri::AppHandle, org_id: String, batch: Vec<sync::SyncEventEncoded>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let res = sync::sync_push(&mut s, &org_id, batch).map_err(|e| e.to_string());
    if res.is_ok() {
        let _ = app.emit("entity_changed", ());
    }
    res
}

#[tauri::command]
fn sync_pull(state: tauri::State<'_, Mutex<AppState>>, org_id: String, cursor: Option<sync::HlcCursor>) -> Result<sync::SyncPullResult, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(sync::sync_pull(&s, &org_id, cursor))
}

#[tauri::command]
fn sync_ping(state: tauri::State<'_, Mutex<AppState>>, org_id: String) -> Result<sync::ConnectionState, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sync::sync_ping(&s, &org_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn sync_status(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(sync::sync_status(&s))
}

#[tauri::command]
fn get_sync_info(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<sync::SyncInfo, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(sync::get_sync_info_impl(&s, &org)).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_invites(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<invite::InvitePayload>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.invite_handler.get_pending())
}

#[tauri::command]
fn debug_invite_handler(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let pending = s.invite_handler.get_pending().len();
    Ok(format!("invite handler active, {} pending, node {}", pending, hex::encode(s.node_id())[..8].to_string()))
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

#[tauri::command]
fn check_entity_access(state: tauri::State<'_, Mutex<AppState>>, org_id: String, entity: String) -> Result<bool, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(check_read_access(&s, &org_id, &entity).is_ok())
}

fn check_read_access(state: &AppState, org_id: &str, entity: &str) -> Result<(), String> {
    if entity == "roles" { return Ok(()); }
    let node_id = state.node_id();
    let reg = state.registry().read().map_err(|e| e.to_string())?;
    let openable: std::collections::HashSet<String> = reg.openable_namespaces(
        &org_id.to_string(),
        &node_id,
    );
    if openable.contains(entity) || openable.contains("*") {
        Ok(())
    } else {
        Err(format!("Access denied: role cannot read {}", entity))
    }
}

pub fn list_orgs_impl(state: &AppState) -> Vec<OrgInfo> {
    state.list_orgs()
}

pub fn commit_event_impl(state: &AppState, event_type: &str, payload: &str) -> Result<String, String> {
    events::commit_event(state, event_type, payload).map_err(|e| e.to_string())
}

pub fn sync_push_impl(state: &mut AppState, org_id: &str, batch: Vec<sync::SyncEventEncoded>) -> Result<(), String> {
    sync::sync_push(state, org_id, batch).map_err(|e| e.to_string())
}

pub fn sync_pull_impl(state: &AppState, org_id: &str, cursor: Option<sync::HlcCursor>) -> sync::SyncPullResult {
    sync::sync_pull(state, org_id, cursor)
}

pub fn sync_ping_impl(state: &AppState, org_id: &str) -> Result<sync::ConnectionState, String> {
    sync::sync_ping(state, org_id).map_err(|e| e.to_string())
}

pub fn sync_status_impl(state: &AppState) -> String {
    sync::sync_status(state)
}

pub fn get_sync_info_impl(state: &AppState, org: &str) -> Result<sync::SyncInfo, String> {
    tauri::async_runtime::block_on(sync::get_sync_info_impl(state, org)).map_err(|e| e.to_string())
}

pub fn get_invites_impl(state: &AppState) -> Vec<invite::InvitePayload> {
    state.invite_handler.get_pending()
}

pub fn query_entity_impl(
    state: &AppState,
    org_id: Option<&str>,
    entity: &str,
    filter_field: Option<&str>,
    filter_value: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let resolved_org_id = org_id.map(String::from).or_else(|| state.active_org().ok().map(String::from)).unwrap_or_default();
    if resolved_org_id.is_empty() { return Ok(vec![]); }

    if entity != "roles" {
        let mut filters = vec![];
        if let (Some(field), Some(value)) = (filter_field, filter_value) {
            filters.push(indexes::QueryFilter { field: field.to_string(), value: value.to_string() });
        }
        let options = indexes::QueryOptions { filters, sort: None, limit: None, offset: None };
        return state.indexer.query(&resolved_org_id, entity, &options).map_err(|e| e.to_string());
    }

    let indexer = state.indexer();
    let roles = indexer.get_roles(&resolved_org_id).map_err(|e| e.to_string())?;
    Ok(roles)
}

pub fn query_entity_advanced_impl(
    state: &AppState,
    org_id: Option<&str>,
    entity: &str,
    query: Option<SchemaQuery>,
) -> Result<Vec<serde_json::Value>, String> {
    let q = query.unwrap_or_default();
    let resolved_org_id = org_id.map(String::from).or_else(|| state.active_org().ok().map(String::from)).unwrap_or_default();
    if resolved_org_id.is_empty() { return Ok(vec![]); }

    let options = indexes::QueryOptions {
        filters: q.filters.unwrap_or_default().into_iter().map(|f| indexes::QueryFilter { field: f.field, value: f.value }).collect(),
        sort: q.sort,
        limit: q.limit,
        offset: q.offset,
    };
    state.indexer.query(&resolved_org_id, entity, &options).map_err(|e| e.to_string())
}

pub fn search_entity_impl(
    state: &AppState,
    org_id: Option<&str>,
    query: &str,
    entities: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<search::SearchResult>, String> {
    let resolved_org_id = org_id.map(String::from).or_else(|| state.active_org().ok().map(String::from)).unwrap_or_default();
    if resolved_org_id.is_empty() { return Ok(vec![]); }

    let limit_val = limit.unwrap_or(20);
    state.indexer.search_engine.search(&resolved_org_id, query, entities, limit_val).map_err(|e| e.to_string())
}

// ===== Thin Tauri command wrappers =====

#[tauri::command]
fn query_entity(state: tauri::State<'_, Mutex<AppState>>, org_id: Option<String>, entity: String, filter_field: Option<String>, filter_value: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    query_entity_impl(&s, org_id.as_deref(), &entity, filter_field.as_deref(), filter_value.as_deref())
}

#[tauri::command]
fn query_entity_advanced(
    state: tauri::State<'_, Mutex<AppState>>,
    org_id: Option<String>,
    entity: String,
    query: Option<SchemaQuery>,
) -> Result<Vec<serde_json::Value>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    query_entity_advanced_impl(&s, org_id.as_deref(), &entity, query)
}

#[tauri::command]
fn search_entity(
    state: tauri::State<'_, Mutex<AppState>>,
    org_id: Option<String>,
    query: String,
    entities: Option<Vec<String>>,
    limit: Option<usize>,
) -> Result<Vec<search::SearchResult>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    search_entity_impl(&s, org_id.as_deref(), &query, entities, limit)
}

#[tauri::command]
fn get_schema_registry() -> Result<serde_json::Value, String> {
    let registry = build_registry();
    Ok(registry.export_json())
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
fn seed_dev_data(state: tauri::State<'_, Mutex<AppState>>) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    seed::seed_dev_data(&s).map_err(|e| e.to_string())
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

    let log_handle = syntrix_logging::init_logging("client", data_dir.clone());

    let app_state = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize iroh")
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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

            let state = app.state::<Mutex<AppState>>();
            let s = state.lock().unwrap();
            if let Some(rx) = s.invite_rx.lock().unwrap().take() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::sync::mpsc::UnboundedReceiver;
                    let mut rx: UnboundedReceiver<crate::invite::InvitePayload> = rx;
                    while let Some(invite) = rx.recv().await {
                        let _ = handle.emit("invite-received", invite);
                    }
                });
            }
            drop(s);
            Ok(())
        })
        .manage(Mutex::new(app_state))
        .manage(log_handle)
        .invoke_handler(tauri::generate_handler![
            get_node_id, list_orgs, set_active_org, join_org, get_invites, debug_invite_handler, get_endpoint_addr,
            commit_event, sync_status, sync_push, sync_pull, sync_ping, check_entity_access,
            query_entity, query_entity_advanced, search_entity, seed_dev_data, get_sync_info,
            get_schema_registry, audit_query,
            query_logs, summarize_logs, start_tail_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-client");
}
