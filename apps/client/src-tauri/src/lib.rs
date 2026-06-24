use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Emitter};
use tracing_subscriber::{Layer, prelude::*};

mod identity;
mod events;
mod sync;
mod invite;
mod seed;
pub mod indexes;
pub mod search;


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
        use tauri::Emitter;
        let _ = app.emit("entity_changed", ());
    }
    res
}

#[tauri::command]
fn join_org(state: tauri::State<'_, Mutex<AppState>>, invite_json: String, org_name: Option<String>) -> Result<OrgInfo, String> {
    let name = org_name.unwrap_or_else(|| "org-unknown".into());

    let invite: invite::InvitePayload = serde_json::from_str(&invite_json)
        .map_err(|e| format!("invalid invite json: {}", e))?;

    let api = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.api().clone()
    };

    let mut final_org_id = String::new();
    let mut docs = Vec::new();

    for ti in &invite.tickets {
        let ticket: iroh_docs::DocTicket = ti.ticket
            .parse()
            .map_err(|e| format!("invalid ticket for {}: {}", ti.ns, e))?;

        if final_org_id.is_empty() {
            final_org_id = hex::encode(&ticket.capability.id().as_bytes()[..4]);
        }

        let doc = tauri::async_runtime::block_on(api.import(ticket))
            .map_err(|e| format!("import ticket for {}: {}", ti.ns, e))?;

        docs.push((ti.ns.clone(), doc));
    }

    let mut s = state.lock().map_err(|e| e.to_string())?;
    let role = invite.role.clone();

    for (ns, doc) in docs {
        s.add_org_docs(&ns, &final_org_id, &name, &role, doc);
    }

    if let Some(org_state) = s.get_org_docs(&final_org_id) {
        let cfg = identity::ClientOrgConfig {
            org_id: final_org_id.clone(),
            name: name.clone(),
            role: role.clone(),
            control_id: org_state.control_doc.id().to_string(),
            catalogs_id: org_state.catalogs_doc.id().to_string(),
            operational_id: org_state.operational_doc.id().to_string(),
            payroll_id: org_state.payroll_doc.id().to_string(),
        };
        let _ = s.save_org_config(cfg);
        
        sync::start_heartbeat(org_state.control_doc.clone(), s.author(), hex::encode(s.node_id()));
    }

    Ok(OrgInfo { id: final_org_id, name, role })
}

#[tauri::command]
fn sync_push(state: tauri::State<'_, Mutex<AppState>>, app: tauri::AppHandle, org_id: String, batch: Vec<sync::SyncEventEncoded>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let res = sync::sync_push(&mut s, &org_id, batch).map_err(|e| e.to_string());
    if res.is_ok() {
        use tauri::Emitter;
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
    let (doc, store, node_id) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let org_state = s.get_org_docs(&org).ok_or_else(|| format!("org {} not found", org))?;
        (org_state.control_doc.clone(), s.store().clone(), s.node_id())
    };
    tauri::async_runtime::block_on(sync::get_sync_info(doc, store, node_id)).map_err(|e| e.to_string())
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
fn query_entity(state: tauri::State<'_, Mutex<AppState>>, org_id: Option<String>, entity: String, filter_field: Option<String>, filter_value: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let (_oid, doc, store) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let resolved_org_id = org_id.or_else(|| s.active_org().ok().map(String::from)).unwrap_or_default();
        if resolved_org_id.is_empty() { return Ok(vec![]); }

        if entity != "roles" {
            return s.indexer.query(&resolved_org_id, &entity, filter_field.as_deref(), filter_value.as_deref()).map_err(|e| e.to_string());
        }

        let org_state = s.get_org_docs(&resolved_org_id).ok_or_else(|| "Org docs not found".to_string())?;
        (resolved_org_id, org_state.control_doc.clone(), s.store().clone())
    };

    tauri::async_runtime::block_on(async move {
        let mut results = vec![];
        let stream_raw = doc.get_many(iroh_docs::store::Query::key_prefix("roles/")).await.map_err(|e| e.to_string())?;
        let mut stream = Box::pin(stream_raw);
        use futures_util::stream::StreamExt;
        while let Some(entry_res) = stream.next().await {
            if let Ok(entry) = entry_res {
                let hash = entry.content_hash();
                if let Ok(bytes) = store.blobs().get_bytes(hash).await {
                    let bytes_ref: &[u8] = bytes.as_ref();
                    if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(bytes_ref) {
                        let key = String::from_utf8_lossy(entry.key()).to_string();
                        let name = key.replace("roles/", "");
                        if let Some(obj) = json.as_object_mut() {
                            obj.insert("name".into(), serde_json::Value::String(name));
                        }
                        results.push(json);
                    }
                }
            }
        }
        Ok(results)
    })
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
    let resolved_org_id = org_id.or_else(|| s.active_org().ok().map(String::from)).unwrap_or_default();
    if resolved_org_id.is_empty() { return Ok(vec![]); }

    let limit_val = limit.unwrap_or(20);
    s.indexer
        .search_engine
        .search(&resolved_org_id, &query, entities, limit_val)
        .map_err(|e| e.to_string())
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
    let log_file = log_dir.join("syntrix-client.log");
    if log_file.exists() {
        std::fs::read_to_string(log_file).map_err(|e| e.to_string())
    } else {
        Ok("No logs yet.".into())
    }
}

#[tauri::command]
fn seed_dev_data(state: tauri::State<'_, Mutex<AppState>>) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    seed::seed_dev_data(&s).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // File logging (rotating daily)
    let log_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path).join("logs")
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix")
            .join("logs")
    };
    std::fs::create_dir_all(&log_dir).ok();
    
    let file_appender = tracing_appender::rolling::daily(&log_dir, "syntrix-client.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    let console_layer = tracing_subscriber::fmt::layer()
        .with_filter(tracing_subscriber::EnvFilter::new("iroh=debug,syntrix=info"));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_filter(tracing_subscriber::EnvFilter::new("iroh=debug,syntrix=debug"));
    
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();
    std::mem::forget(_guard);

    let app_state = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize iroh")
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
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
        .invoke_handler(tauri::generate_handler![
            get_node_id, list_orgs, set_active_org, join_org, get_invites, debug_invite_handler, get_endpoint_addr,
            commit_event, sync_status, sync_push, sync_pull, sync_ping,
            query_entity, search_entity, seed_dev_data, get_logs, get_sync_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-client");
}
