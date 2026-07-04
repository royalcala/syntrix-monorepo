use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::Emitter;

#[derive(Debug, Clone)]
pub struct LiveSubscription {
    pub id: u64,
    pub sql: String,
    pub depends_on: Vec<String>,
}

#[derive(Default)]
pub struct LiveManager {
    subscriptions: std::sync::RwLock<HashMap<u64, LiveSubscription>>,
    next_id: AtomicU64,
    app_handle: std::sync::RwLock<Option<tauri::AppHandle>>,
}

impl LiveManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        if let Ok(mut h) = self.app_handle.write() {
            *h = Some(handle);
        }
    }

    pub fn subscribe(&self, sql: String, depends_on: Vec<String>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let sub = LiveSubscription { id, sql, depends_on };
        if let Ok(mut subs) = self.subscriptions.write() {
            subs.insert(id, sub);
        }
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        if let Ok(mut subs) = self.subscriptions.write() {
            subs.remove(&id);
        }
    }

    pub fn notify_table_changed(
        &self,
        db_lock: &Mutex<()>,
        conn: &Arc<turso_core::Connection>,
        changed_tables: &[String],
    ) {
        let _lock = db_lock.lock().unwrap();
        let subs = match self.subscriptions.read() {
            Ok(s) => s.clone(),
            Err(_) => return,
        };
        let handle = match self.app_handle.read() {
            Ok(h) => h.clone(),
            Err(_) => return,
        };
        let handle = match handle {
            Some(h) => h,
            None => return,
        };

        for (_id, sub) in &subs {
            let has_overlap = sub.depends_on.iter().any(|t| changed_tables.contains(t));
            if !has_overlap {
                continue;
            }
            if let Ok(rows) = execute_live_query(conn, &sub.sql) {
                let payload = serde_json::json!({
                    "id": sub.id,
                    "rows": rows,
                });
                let _ = handle.emit("live_update", payload);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_and_unsubscribe() {
        let mgr = LiveManager::new();
        let id = mgr.subscribe("SELECT * FROM customers".into(), vec!["customers".into()]);
        assert_eq!(id, 0);
        let id2 = mgr.subscribe("SELECT * FROM invoices".into(), vec!["invoices".into()]);
        assert_eq!(id2, 1);

        mgr.unsubscribe(id);
        let subs = mgr.subscriptions.read().unwrap();
        assert!(!subs.contains_key(&id));
        assert!(subs.contains_key(&id2));
    }

    #[test]
    fn test_subscription_id_increments() {
        let mgr = LiveManager::new();
        let ids: Vec<u64> = (0..5).map(|i| {
            mgr.subscribe(format!("query_{}", i), vec!["customers".into()])
        }).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_unsubscribe_nonexistent() {
        let mgr = LiveManager::new();
        mgr.unsubscribe(42);
        let subs = mgr.subscriptions.read().unwrap();
        assert!(subs.is_empty());
    }

    #[test]
    fn test_live_subscription_struct() {
        let sub = LiveSubscription {
            id: 1,
            sql: "SELECT * FROM t".into(),
            depends_on: vec!["customers".into(), "invoices".into()],
        };
        assert_eq!(sub.id, 1);
        assert!(sub.depends_on.contains(&"customers".into()));
    }
}

fn execute_live_query(
    conn: &Arc<turso_core::Connection>,
    sql: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(sql)?;
    use turso_core::StepResult;

    let num_cols = stmt.num_columns();
    let col_names: Vec<String> = (0..num_cols).map(|i| stmt.get_column_name(i).to_string()).collect();

    let mut rows = Vec::new();
    loop {
        match stmt.step()? {
            StepResult::Row => {
                if let Some(row) = stmt.row() {
                    let mut obj = serde_json::Map::new();
                    for i in 0..num_cols {
                        let col_name = col_names.get(i).cloned().unwrap_or_else(|| format!("col_{}", i));
                        if let Ok(val) = row.get::<String>(i) {
                            obj.insert(col_name, serde_json::Value::String(val));
                        } else if let Ok(val) = row.get::<i64>(i) {
                            obj.insert(col_name, serde_json::json!(val));
                        } else if let Ok(val) = row.get::<f64>(i) {
                            obj.insert(col_name, serde_json::json!(val));
                        } else {
                            obj.insert(col_name, serde_json::Value::Null);
                        }
                    }
                    rows.push(serde_json::Value::Object(obj));
                }
            }
            StepResult::Done => break,
            StepResult::IO | StepResult::Yield => {
                stmt._io().step()?;
            }
            StepResult::Interrupt | StepResult::Busy => {
                anyhow::bail!("live query interrupted or busy");
            }
        }
    }
    Ok(rows)
}
