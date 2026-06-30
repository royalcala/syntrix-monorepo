use std::sync::Mutex;
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
pub mod ai;

use syntrix_schema::build_registry;
use iroh_docs::api::Doc;
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
    let res = commit_event_impl(&s, &event_type, &payload);
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

        let doc = api.import(ticket).await
            .map_err(|e| format!("import ticket for {}: {}", ti.ns, e))?;

        docs.push((ti.ns.clone(), doc));
    }

    let role = invite.role.clone();
    let (ctrl_doc, entity_docs, store, registry, secret) = {
        let mut s = state.lock().map_err(|e| e.to_string())?;

        let mut doc_imports = Vec::new();
        for (ns, doc) in docs {
            doc_imports.push((ns.clone(), doc.clone()));
        }

        let _result = join_org_state_impl(&mut s, &final_org_id, &name, &role, doc_imports)?;

        let org_state = s.get_org_docs(&final_org_id).ok_or_else(|| "Failed to get org docs".to_string())?;
        (
            org_state.control_doc.clone(),
            org_state.entity_docs.clone(),
            s.store().clone(),
            s.registry().clone(),
            s.secret().clone(),
        )
    };

    // Bootstrap P2P sync with the admin who sent the invite
    if let Some(ref admin_addr_str) = invite.admin_addr {
        if let Some(admin_endpoint) = syntrix_core::parse_device_addr(admin_addr_str) {
            let peer_vec = vec![admin_endpoint];
            let _ = ctrl_doc.start_sync(peer_vec.clone()).await;
            for (_ns, doc) in &entity_docs {
                let _ = doc.start_sync(peer_vec.clone()).await;
            }
        }
    }

    // Give the sync a moment to deliver control doc entries
    let mut member_found = false;
    for _attempt in 0..4 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let check = {
            let s = state.lock().map_err(|e| e.to_string())?;
            if let Some(org) = s.get_org_docs(&final_org_id) {
                let ctrl = org.control_doc.clone();
                let members = tauri::async_runtime::block_on(async {
                    if let Ok(stream) = ctrl.get_many(iroh_docs::store::Query::key_prefix("members/")).await {
                        let mut pinned = Box::pin(stream);
                        use futures_util::StreamExt;
                        pinned.next().await.is_some()
                    } else {
                        false
                    }
                });
                if members {
                    true
                } else {
                    tauri::async_runtime::block_on(async {
                        if let Ok(stream) = ctrl.get_many(iroh_docs::store::Query::key_prefix("roles/")).await {
                            let mut pinned = Box::pin(stream);
                            use futures_util::StreamExt;
                            pinned.next().await.is_some()
                        } else {
                            false
                        }
                    })
                }
            } else {
                false
            }
        };
        if check {
            member_found = true;
            break;
        }
    }

    let entity_docs_vec: Vec<iroh_docs::api::Doc> = entity_docs.values().cloned().collect();
    let _ = identity::sync_and_populate_org_members_impl(
        ctrl_doc.clone(),
        entity_docs_vec,
        store,
        registry.clone(),
        secret,
        final_org_id.clone(),
    ).await;

    if !member_found {
        eprintln!(
            "join_org: no members synced yet for org {}. Heartbeat loop will retry.",
            &final_org_id[..8]
        );
    }

    // Ensure the current device is always registered in the namespace registry.
    let node_id;
    let registry_arc = {
        let s = state.lock().map_err(|e| e.to_string())?;
        node_id = s.node_id();
        s.registry().clone()
    };
    let node_id_hex = hex::encode(node_id);
    if let Ok(mut reg) = registry_arc.write() {
        reg.upsert_device(
            final_org_id.clone(),
            node_id,
            syntrix_core::registry::Device {
                node_id,
                active: true,
                role: role.clone(),
                person: node_id_hex.clone(),
                name: format!("Device {}", &node_id_hex[..8]),
            },
        );
        let openable = reg.openable_namespaces(&final_org_id, &node_id);
        if openable.is_empty() {
            let (can_open, can_write): (Vec<String>, Vec<String>) = match role.as_str() {
                "admin" => (vec!["*".into()], vec!["*".into()]),
                "sales" => (
                    vec!["customers".into(), "products".into(), "invoices".into(), "orders".into()],
                    vec!["customers".into(), "invoices".into(), "orders".into()],
                ),
                "contabilidad" => (
                    vec!["invoices".into(), "customers".into()],
                    vec![],
                ),
                _ => (vec![], vec![]),
            };
            reg.upsert_role(
                final_org_id.clone(),
                role.clone(),
                syntrix_core::registry::RoleGrants {
                    can_open,
                    can_write,
                },
            );
        }
    }

    Ok(OrgInfo { id: final_org_id, name, role })
}

#[tauri::command]
fn sync_push(state: tauri::State<'_, Mutex<AppState>>, app: tauri::AppHandle, org_id: String, batch: Vec<sync::SyncEventEncoded>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let res = sync_push_impl(&mut s, &org_id, batch);
    if res.is_ok() {
        let _ = app.emit("entity_changed", ());
    }
    res
}

#[tauri::command]
fn sync_pull(state: tauri::State<'_, Mutex<AppState>>, org_id: String, cursor: Option<sync::HlcCursor>) -> Result<sync::SyncPullResult, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(sync_pull_impl(&s, &org_id, cursor))
}

#[tauri::command]
fn sync_ping(state: tauri::State<'_, Mutex<AppState>>, org_id: String) -> Result<sync::ConnectionState, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    sync_ping_impl(&s, &org_id)
}

#[tauri::command]
fn sync_status(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(sync_status_impl(&s))
}

#[tauri::command]
fn get_sync_info(state: tauri::State<'_, Mutex<AppState>>, org: String) -> Result<sync::SyncInfo, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    get_sync_info_impl(&s, &org)
}

#[tauri::command]
fn get_invites(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<invite::InvitePayload>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(get_invites_impl(&s))
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

/// Check if the current device has read access to the given entity.
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

// ===== Extracted impl functions (no Tauri types) =====

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
    let (doc, store, node_id) = {
        let org_state = state.get_org_docs(org).ok_or_else(|| format!("org {} not found", org))?;
        (org_state.control_doc.clone(), state.store().clone(), state.node_id())
    };
    tauri::async_runtime::block_on(sync::get_sync_info(doc, store, node_id)).map_err(|e| e.to_string())
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
        check_read_access(state, &resolved_org_id, entity)?;
        let mut filters = vec![];
        if let (Some(field), Some(value)) = (filter_field, filter_value) {
            filters.push(indexes::QueryFilter { field: field.to_string(), value: value.to_string() });
        }
        let options = indexes::QueryOptions { filters, sort: None, limit: None, offset: None };
        return state.indexer.query(&resolved_org_id, entity, &options).map_err(|e| e.to_string());
    }

    let org_state = state.get_org_docs(&resolved_org_id).ok_or_else(|| "Org docs not found".to_string())?;
    let doc = org_state.control_doc.clone();
    let store = state.store().clone();
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

pub fn query_entity_advanced_impl(
    state: &AppState,
    org_id: Option<&str>,
    entity: &str,
    query: Option<SchemaQuery>,
) -> Result<Vec<serde_json::Value>, String> {
    let q = query.unwrap_or_default();
    let resolved_org_id = org_id.map(String::from).or_else(|| state.active_org().ok().map(String::from)).unwrap_or_default();
    if resolved_org_id.is_empty() { return Ok(vec![]); }

    check_read_access(state, &resolved_org_id, entity)?;

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

/// Sync state manipulation after org join (async doc imports already done).
pub fn join_org_state_impl(
    state: &mut AppState,
    final_org_id: &str,
    name: &str,
    role: &str,
    docs: Vec<(String, Doc)>,
) -> Result<OrgInfo, String> {
    for (ns, doc) in docs {
        state.add_org_docs(&ns, final_org_id, name, role, doc);
    }

    let org_state = state.get_org_docs(final_org_id).ok_or_else(|| "Failed to get org docs".to_string())?;

    let namespace_ids: std::collections::HashMap<String, String> = org_state.entity_docs.iter().map(|(k, v)| (k.clone(), v.id().to_string())).collect();

    let cfg = identity::ClientOrgConfig {
        org_id: final_org_id.to_string(),
        name: name.to_string(),
        role: role.to_string(),
        control_id: org_state.control_doc.id().to_string(),
        namespace_ids,
    };
    let _ = state.save_org_config(cfg);

    let entity_docs_vec: Vec<iroh_docs::api::Doc> = org_state.entity_docs.values().cloned().collect();
    sync::start_heartbeat_with_resync(
        org_state.control_doc.clone(),
        entity_docs_vec.clone(),
        state.author(),
        hex::encode(state.node_id()),
        state.store().clone(),
        state.secret().clone(),
        state.registry().clone(),
    );

    identity::start_doc_subscriptions(
        org_state.control_doc.clone(),
        entity_docs_vec,
        state.author(),
        state.store().clone(),
        state.indexer(),
        final_org_id.to_string(),
    );

    Ok(OrgInfo { id: final_org_id.to_string(), name: name.to_string(), role: role.to_string() })
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

#[tauri::command]
async fn ai_chat(
    state: tauri::State<'_, Mutex<AppState>>,
    handle: tauri::State<'_, LogHandle>,
    app: tauri::AppHandle,
    org_id: String,
    messages: Vec<syntrix_ai::tool::ChatMessage>,
    provider_config: Option<syntrix_ai::tool::ProviderConfig>,
) -> Result<(), String> {
    let (executor, tools, org_role) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let indexer = s.indexer.clone();
        let role = s.get_org_docs(&org_id).map(|o| o.role.as_str()).unwrap_or("user");
        (ai::AiToolExecutor { indexer, log_handle: handle.clone() },
         syntrix_ai::tool::default_tool_definitions(),
         role.to_string())
    };

    let schema_json = syntrix_schema::build_registry().export_json();
    let schema_str = serde_json::to_string(&schema_json).unwrap_or_default();

    let system_prompt = syntrix_ai::tool::ChatMessage {
        role: "system".into(),
        content: Some(format!(
            r#"You are an AI assistant for Syntrix, a peer-to-peer ERP system.
Active organization ID: {}
Your role: {}
You are READ-ONLY — you can query data but never create, update, or delete records.
Always respect entity access permissions.

Available entities and their schemas:
{}

When displaying data, use tables for structured results, cards for summaries, and metrics for KPIs.
Respond in the same language the user writes in. Be concise and data-driven."#,
            org_id, org_role, schema_str
        )),
        tool_calls: None,
        tool_call_id: None,
    };

    let mut full_messages = vec![system_prompt];
    full_messages.extend(messages);

    let config = provider_config.unwrap_or_default();
    let provider = syntrix_ai::provider::OpenAICompatibleProvider::new(
        config.base_url, config.model, config.api_key,
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        syntrix_ai::chat::ai_chat_impl(
            &provider, &executor, &org_id, &full_messages, &tools, tx,
        ).await.ok();
    });

    while let Some(event) = rx.recv().await {
        let _ = app.emit("ai_chat_event", &event);
    }

    Ok(())
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
            commit_event, sync_status, sync_push, sync_pull, sync_ping,
            query_entity, query_entity_advanced, search_entity, seed_dev_data, get_sync_info,
            get_schema_registry, audit_query,
            query_logs, summarize_logs, start_tail_logs,
            ai_chat,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-client");
}
