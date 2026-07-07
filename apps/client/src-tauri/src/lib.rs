use std::sync::{Arc, Mutex};
use std::io::{BufRead, Write};
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
pub mod storage;
pub mod catchup;
pub mod live;
pub mod cdc_sync;
pub mod ai_provider;

use syntrix_core::ENTITY_NAMES;
use syntrix_ai::tools::{AiContext, QueryResult, SearchResult, ViewDefinition, ViewMeta};
use syntrix_ai::{AiChatRequest, AiChatResponse, AiStatusInfo, router, ai_chat_impl, ai_status_impl};

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
fn join_org(state: tauri::State<'_, Mutex<AppState>>, invite_json: String, org_name: Option<String>) -> Result<OrgInfo, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(join_org_impl(&mut s, &invite_json, org_name.as_deref()))
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
fn get_updates_since(state: tauri::State<'_, Mutex<AppState>>, org_id: String, since_change_id: i64) -> Result<serde_json::Value, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    get_updates_since_impl(&s, &org_id, since_change_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn drizzle_execute(state: tauri::State<'_, Mutex<AppState>>, sql: String, params: Vec<String>) -> Result<serde_json::Value, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    drizzle_execute_impl(&s, &sql, &params).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_invites(state: tauri::State<'_, Mutex<AppState>>) -> Result<Vec<invite::InvitePayload>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.invite_handler.get_pending())
}

#[tauri::command]
fn get_endpoint_addr(state: tauri::State<'_, Mutex<AppState>>) -> Result<String, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(get_endpoint_addr_impl(&s))
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

pub fn sync_status_impl(state: &AppState) -> String {
    sync::sync_status(state)
}

pub fn get_sync_info_impl(state: &AppState, org: &str) -> Result<sync::SyncInfo, String> {
    tauri::async_runtime::block_on(sync::get_sync_info_impl(state, org)).map_err(|e| e.to_string())
}

pub fn get_updates_since_impl(state: &AppState, org_id: &str, since_change_id: i64) -> Result<serde_json::Value, String> {
    let (events, new_cursor) = state.indexer().read_cdc_events(
        since_change_id as u64, 500, None,
    ).map_err(|e| e.to_string())?;
    let org_events: Vec<serde_json::Value> = events.into_iter()
        .filter(|e| e.org_id() == Some(org_id))
        .map(|e| {
            serde_json::json!({
                "change_type": e.change_type,
                "table": e.table,
                "change_time": e.change_time,
                "columns": e.columns,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "events": org_events,
        "max_change_id": new_cursor,
    }))
}

pub fn drizzle_execute_impl(state: &AppState, sql: &str, params: &[String]) -> Result<serde_json::Value, String> {
    state.indexer().drizzle_execute(sql, params)
}

impl AiContext for AppState {
    fn check_read_access(&self, org_id: &str, entity: &str) -> Result<(), String> {
        check_read_access(self, org_id, entity)
    }

    fn query_entity(&self, _org_id: &str, sql: &str, _params: &[String]) -> Result<QueryResult, String> {
        let row_objects = self.indexer().execute_sql_query(sql).map_err(|e| e.to_string())?;
        let columns: Vec<String> = row_objects
            .first()
            .map(|obj| {
                obj.as_object()
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let rows: Vec<Vec<serde_json::Value>> = row_objects
            .into_iter()
            .map(|obj| {
                columns
                    .iter()
                    .map(|col| {
                        obj.as_object()
                            .and_then(|m| m.get(col).cloned())
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .collect()
            })
            .collect();
        Ok(QueryResult { columns, rows })
    }

    fn search_entity(
        &self,
        org_id: &str,
        query: &str,
        entities: Option<Vec<String>>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, String> {
        let results = search_entity_impl(self, Some(org_id), query, entities, Some(limit))?;
        Ok(results.into_iter().map(|r| SearchResult {
            entity: r.entity,
            doc_id: r.doc_id,
            score: r.score as f64,
            snippet: r.snippet,
        }).collect())
    }

    fn save_view(&self, view: &ViewDefinition) -> Result<(), String> {
        let meta_json = serde_json::to_string(&view.meta).unwrap_or_default();
        let components_json = serde_json::to_string(&view.components).unwrap_or_default();
        let tags = view.meta.tags.join(",");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let node_id_hex = hex::encode(self.node_id());
        let sql = "INSERT OR REPLACE INTO view_definitions (org_id, doc_id, sql, entity, components_json, root, meta_json, created_by, tags, change_time, node_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)";
        let params = vec![
            view.org_id.clone(),
            view.id.clone(),
            view.sql.clone(),
            view.entity.clone(),
            components_json,
            view.root.clone(),
            meta_json,
            view.meta.created_by.clone(),
            tags,
            now.to_string(),
            node_id_hex,
        ];
        drizzle_execute_impl(self, sql, &params)?;
        Ok(())
    }

    fn list_views(&self, org_id: &str, _tags: Option<&[String]>) -> Result<Vec<ViewDefinition>, String> {
        let sql = "SELECT doc_id, sql, entity, components_json, root, meta_json, created_by, tags FROM view_definitions WHERE org_id = ?1 ORDER BY change_time DESC";
        let row_objects = self.indexer().execute_sql_query(sql).map_err(|e| e.to_string())?;
        let mut views = Vec::new();
        for row in row_objects {
            let obj = row.as_object().ok_or_else(|| "invalid row".to_string())?;
            let id = obj.get("doc_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let sql_text = obj.get("sql").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let entity = obj.get("entity").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let root = obj.get("root").and_then(|v| v.as_str()).unwrap_or("root").to_string();
            let components = obj.get("components_json")
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            let meta: ViewMeta = obj.get("meta_json")
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(ViewMeta {
                    natural_language_query: String::new(),
                    created_by: obj.get("created_by").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    created_at: 0,
                    tags: obj.get("tags").and_then(|v| v.as_str())
                        .map(|s| s.split(',').filter(|t| !t.is_empty()).map(String::from).collect())
                        .unwrap_or_default(),
                });
            views.push(ViewDefinition {
                id,
                org_id: org_id.to_string(),
                sql: sql_text,
                entity,
                components,
                root,
                meta,
            });
        }
        Ok(views)
    }
}

pub fn get_invites_impl(state: &AppState) -> Vec<invite::InvitePayload> {
    state.invite_handler.get_pending()
}

pub async fn join_org_impl(
    state: &mut AppState,
    invite_json: &str,
    org_name: Option<&str>,
) -> Result<OrgInfo, String> {
    let name = org_name.unwrap_or("org-unknown");
    let invite: invite::InvitePayload = serde_json::from_str(invite_json)
        .map_err(|e| format!("invalid invite json: {}", e))?;

    let topic_id_str = format!("syntrix-org-{}", &invite.org_name);
    let role = invite.role.clone();
    let admin_addr = invite.admin_addr.clone();
    let can_open = invite.can_open.clone();
    let can_write = invite.can_write.clone();
    let final_org_id = invite.org_name.clone();

    state.add_org(&final_org_id, &name, &role, topic_id_str.clone(), admin_addr.clone());

    let node_id = state.node_id();
    let node_id_hex = hex::encode(node_id);

    if let Ok(mut reg) = state.registry().write() {
        reg.set_topic_id(final_org_id.clone(), topic_id_str.clone());
        reg.upsert_device(
            final_org_id.clone(), node_id,
            syntrix_core::registry::Device {
                node_id, active: true, role: role.clone(),
                person: node_id_hex.clone(),
                name: format!("Device {}", &node_id_hex[..8]),
            },
        );
        reg.upsert_role(final_org_id.clone(), role.clone(),
            syntrix_core::registry::RoleGrants { can_open: can_open.clone(), can_write: can_write.clone() });
    }

    let role_json = serde_json::json!({
        "name": role,
        "can_open": can_open,
        "can_write": can_write,
    });
    let _ = state.indexer.upsert_role_cfg(&final_org_id, &role, &role_json);

    state.save_org_config(identity::ClientOrgConfig {
        org_id: final_org_id.clone(),
        name: name.to_string(),
        role: role.clone(),
        topic_id: hex::encode(node_id),
        admin_addr: admin_addr.clone(),
        can_open: can_open.clone(),
        can_write: can_write.clone(),
    }).ok();

    let _ = state.p2p().join_topic(&topic_id_str);

    // Catch-up after joining
    let p2p_catchup = state.p2p().clone();
    let indexer = state.indexer.clone();
    let admin_ok = if let Some(ref addr) = admin_addr {
        if let Some(peer_id) = syntrix_core::parse_device_addr(addr) {
            crate::catchup::request_catchup(&p2p_catchup, peer_id, &final_org_id, 0, &indexer).await.is_ok()
        } else {
            false
        }
    } else {
        false
    };

    if !admin_ok {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let hb = indexer.get_heartbeats(&final_org_id).unwrap_or_default();
        let now = chrono::Utc::now().timestamp_millis();
        if let Some((peer_hex, _)) = hb.into_iter()
            .find(|(nid, ts)| nid != &node_id_hex && *ts > 0 && now - *ts < 60_000)
        {
            if let Ok(peer_bytes) = hex::decode(&peer_hex) {
                if peer_bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&peer_bytes);
                    if let Ok(peer_id) = libp2p::PeerId::from_bytes(&arr) {
                        let _ = crate::catchup::request_catchup(&p2p_catchup, peer_id, &final_org_id, 0, &indexer).await;
                    }
                }
            }
        }
    }

    let self_member = serde_json::json!({
        "node_id": node_id_hex.clone(),
        "active": true,
        "role": role.clone(),
        "person": node_id_hex.clone(),
        "name": format!("Device {}", &node_id_hex[..8]),
    });
    let _ = indexer.upsert_member(&final_org_id, &node_id_hex, &self_member);

    let hb_broadcast = {
        let p2p = state.p2p().clone();
        let topic = topic_id_str.clone();
        Arc::new(move |json: &str| {
            let p2p = p2p.clone();
            let t = topic.clone();
            let data = json.as_bytes().to_vec();
            let _ = p2p.publish(&t, data);
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
        node_id_hex.clone(), hb_broadcast, hb_store,
    );

    Ok(OrgInfo { id: final_org_id, name: name.to_string(), role: role.to_string() })
}

pub fn get_endpoint_addr_impl(state: &AppState) -> String {
    let peer_id = state.p2p().local_peer_id();
    let addrs = tauri::async_runtime::block_on(state.p2p().listen_addrs());
    serde_json::json!({
        "node_id": hex::encode(state.node_id()),
        "peer_id": peer_id.to_base58(),
        "addrs": addrs,
    }).to_string()
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
    state.indexer().search(&resolved_org_id, query, entities, limit_val).map_err(|e| e.to_string())
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
fn ai_chat(
    state: tauri::State<'_, Mutex<AppState>>,
    ai: tauri::State<'_, ai_provider::OllamaProvider>,
    org_id: String,
    messages: Vec<syntrix_ai::AiMessage>,
    model: Option<String>,
) -> Result<AiChatResponse, String> {
    let ctx = state.lock().map_err(|e| e.to_string())?;
    let router = router::DefaultRouter;
    let req = AiChatRequest { org_id, messages, model };
    ai_chat_impl(&*ctx, &*ai, &router, &req)
}

#[tauri::command]
fn ai_status(ai: tauri::State<'_, ai_provider::OllamaProvider>) -> Result<AiStatusInfo, String> {
    let uptime = ai.uptime();
    Ok(ai_status_impl(uptime))
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
fn live_subscribe(
    state: tauri::State<'_, Mutex<AppState>>,
    sql: String,
    depends_on: Vec<String>,
) -> Result<u64, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.live_manager.subscribe(sql, depends_on))
}

#[tauri::command]
fn live_unsubscribe(
    state: tauri::State<'_, Mutex<AppState>>,
    id: u64,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    s.live_manager.unsubscribe(id);
    Ok(())
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
            .join("syntrix")
    };

    let log_handle = syntrix_logging::init_logging("client", data_dir.clone());

    let (app_state, mut invite_rx) = tauri::async_runtime::block_on(async {
        AppState::new().await.expect("failed to initialize libp2p")
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_state_setup = app.state::<Mutex<AppState>>();
            if let Ok(state) = app_state_setup.lock() {
                state.live_manager.set_app_handle(app.handle().clone());
            }
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

            // Spawn invite listener: forwards P2P invites to the frontend
            // via the "invite-received" Tauri event.
            let invite_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(invite) = invite_rx.recv().await {
                    let _ = invite_app_handle.emit("invite-received", &invite);
                    tracing::info!(
                        target: "syntrix",
                        org = %invite.org_name,
                        role = %invite.role,
                        "invite emitted to frontend"
                    );
                }
            });

            Ok(())
        })
        .manage(Mutex::new(app_state))
        .manage(log_handle)
        .manage(ai_provider::OllamaProvider::new(None))
        .invoke_handler(tauri::generate_handler![
            get_node_id, list_orgs, set_active_org, join_org, get_invites, get_endpoint_addr,
            commit_event, sync_status, check_entity_access,
            query_entity, query_entity_advanced, search_entity, seed_dev_data, get_sync_info,
            get_updates_since, drizzle_execute, get_schema_registry, audit_query,
            query_logs, summarize_logs, start_tail_logs,
            live_subscribe, live_unsubscribe,
            ai_chat, ai_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running syntrix-client");
}

/// Headless mode: no WebView, reads JSON commands from stdin, writes JSON
/// responses to stdout. Used by binary E2E tests.
fn run_headless() {
    let data_dir = if let Ok(custom_path) = std::env::var("SYNTRIX_DATA_DIR") {
        std::path::PathBuf::from(custom_path)
    } else {
        dirs_next::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("syntrix")
    };

    let _ = syntrix_logging::init_logging("client-headless", data_dir.clone());

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let (mut state, _invite_rx) = rt.block_on(async {
        AppState::new().await.expect("failed to initialize libp2p")
    });

    let ai_provider = ai_provider::OllamaProvider::new(None);
    let router = router::DefaultRouter;

    let stdin = std::io::stdin();
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    eprintln!("[headless] client ready, reading commands from stdin...");

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
        let result: Result<serde_json::Value, String> = match cmd {
            "get_invites" => {
                Ok(serde_json::to_value(get_invites_impl(&state)).unwrap_or_default())
            }
            "get_endpoint_addr" => {
                let peer_id = state.p2p().local_peer_id();
                let addrs = rt.block_on(state.p2p().listen_addrs());
                let addr = serde_json::json!({
                    "node_id": hex::encode(state.node_id()),
                    "peer_id": peer_id.to_base58(),
                    "addrs": addrs,
                }).to_string();
                Ok(serde_json::Value::String(addr))
            }
            "join_org" => {
                let invite_json = req["invite_json"].as_str().unwrap_or("");
                let org_name = req["org_name"].as_str();
                let result = rt.block_on(join_org_impl(&mut state, invite_json, org_name));
                result.map(|org| serde_json::to_value(org).unwrap_or_default())
            }
            "list_orgs" => {
                Ok(serde_json::to_value(list_orgs_impl(&state)).unwrap_or_default())
            }
            "set_active_org" => {
                let org_id = req["org_id"].as_str().unwrap_or("");
                state.set_active_org(org_id).map(|_| serde_json::json!({"ok": true}))
            }
            "set_org_role" => {
                let org_id = req["org_id"].as_str().unwrap_or("");
                let role = req["role"].as_str().unwrap_or("");
                let node_id = hex::encode(state.node_id());
                let member_json = serde_json::json!({"node_id": node_id, "role": role, "active": true});
                let _ = state.indexer().upsert_member(org_id, &node_id, &member_json);
                state.set_org_role(org_id, role);
                Ok(serde_json::json!({"ok": true}))
            }
            "get_org_role" => {
                let org_id = req["org_id"].as_str().unwrap_or("");
                let org_role = state.get_org(org_id).map(|o| o.role).unwrap_or_default();
                let node_id = hex::encode(state.node_id());
                let member_role = state.indexer().get_member_role(org_id, &node_id).unwrap_or_default();
                Ok(serde_json::json!({"org_role": org_role, "member_role": member_role}))
            }
            "commit_event" => {
                let event_type = req["event_type"].as_str().unwrap_or("");
                let payload = req["payload"].as_str().unwrap_or("");
                commit_event_impl(&state, event_type, payload)
                    .map(|id| serde_json::json!({"id": id}))
            }
            "query_entity" => {
                let org_id = req["org_id"].as_str();
                let entity = req["entity"].as_str().unwrap_or("");
                query_entity_impl(&state, org_id, entity, None, None)
                    .map(|docs| serde_json::Value::Array(docs))
            }
            "ai_chat" => {
                let org_id = req["org_id"].as_str().unwrap_or("");
                let msg_text = req["text"].as_str().unwrap_or("");
                let ai_req = syntrix_ai::AiChatRequest {
                    org_id: org_id.to_string(),
                    messages: vec![syntrix_ai::AiMessage {
                        role: "user".to_string(),
                        content: msg_text.to_string(),
                    }],
                    model: req["model"].as_str().map(String::from),
                };
                let result = syntrix_ai::ai_chat_impl(&state, &ai_provider, &router, &ai_req);
                result.map(|r| serde_json::to_value(r).unwrap_or_default())
            }
            "ai_status" => {
                let info = syntrix_ai::ai_status_impl(ai_provider.uptime());
                Ok(serde_json::to_value(info).unwrap_or_default())
            }
            "drizzle_execute" => {
                let sql = req["sql"].as_str().unwrap_or("");
                let params: Vec<String> = req["params"].as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                drizzle_execute_impl(&state, sql, &params)
            }
            "ping" => Ok(serde_json::json!("pong")),
            _ => Err(format!("unknown command: {}", cmd)),
        };

        let response = match result {
            Ok(data) => serde_json::json!({"ok": true, "data": data}),
            Err(e) => serde_json::json!({"ok": false, "error": e}),
        };
        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }

    eprintln!("[headless] client shutting down");
}
