use std::sync::Arc;

/// Store a gossip data event into the admin's redb EVENT_LOG table.
///
/// Called from the main event loop in identity.rs.
pub fn write_event_to_redb(db: &redb::Database, org_id: &str, val: &serde_json::Value) {
    const EVENT_LOG: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("event_log");
    let event_type = val["type"].as_str().unwrap_or("unknown");
    let hlc_val = &val["hlc"];
    let hlc_ts = hlc_val["ts"].as_u64().unwrap_or(0);
    let hlc_count = hlc_val["count"].as_u64().unwrap_or(0);
    let hlc_node = hlc_val["node"].as_str().unwrap_or("");

    let key = format!(
        "evt:{}:{:020}:{:08}:{}",
        org_id, hlc_ts, hlc_count, hlc_node
    );

    let write_result = (|| -> anyhow::Result<()> {
        let write_txn = db.begin_write()?;
        {
            let mut event_log = write_txn.open_table(EVENT_LOG)?;
            event_log.insert(key.as_str(), serde_json::to_vec(val)?.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        eprintln!(
            "[admin-gossip] failed to write event {} to EVENT_LOG: {e}",
            event_type
        );
    } else {
        tracing::info!(
            target: "syntrix",
            org = %org_id,
            event_type = %event_type,
            "admin-audit: stored gossip event in EVENT_LOG"
        );
    }
}
