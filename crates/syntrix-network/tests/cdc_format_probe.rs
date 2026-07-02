//! Phase 0 investigation (see .kilo/plans/1782949593655-relational-cdc-migration.md):
//! experimentally determine the encoding of `turso_cdc.before/after/updates` so that
//! `read_cdc_events`/`apply_cdc_events` can decode real row images instead of the current
//! hardcoded-empty stubs.
//!
//! Findings are asserted below and also printed via `--nocapture` for inspection.

use std::sync::Arc;
use turso_core::{types::ImmutableRecord, StepResult, Value, IO};

fn exec(conn: &Arc<turso_core::Connection>, sql: &str) {
    conn.execute(sql)
        .unwrap_or_else(|e| panic!("exec failed: {sql}: {e}"));
}

fn temp_conn() -> (tempfile::TempDir, Arc<turso_core::Connection>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("cdc_probe.db");
    let io: Arc<dyn IO> = Arc::new(turso_core::PlatformIO::new().unwrap());
    let db = turso_core::Database::open_file_with_flags(
        io,
        db_path.to_str().unwrap(),
        turso_core::OpenFlags::default(),
        turso_core::DatabaseOpts::new().with_index_method(true),
        None,
    )
    .unwrap();
    let conn = db.connect().unwrap();
    (dir, conn)
}

struct CdcRow {
    change_id: i64,
    #[allow(dead_code)]
    change_time: i64,
    change_type: i64,
    table_name: String,
    id: Value,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
    updates: Option<Vec<u8>>,
}

fn read_all_cdc_rows(conn: &Arc<turso_core::Connection>) -> Vec<CdcRow> {
    let sql = "SELECT change_id, change_time, change_type, table_name, id, before, after, updates \
               FROM turso_cdc ORDER BY change_id";
    let mut stmt = conn.prepare(sql).unwrap();
    let mut rows = Vec::new();
    loop {
        match stmt.step().unwrap() {
            StepResult::Row => {
                let row = stmt.row().unwrap();
                let change_id: i64 = row.get(0).unwrap();
                let change_time: i64 = row.get(1).unwrap();
                let change_type: i64 = row.get(2).unwrap();
                let table_name: &Value = row.get(3).unwrap();
                let table_name = match table_name {
                    Value::Text(t) => t.as_str().to_string(),
                    other => format!("{other:?}"),
                };
                let id: &Value = row.get(4).unwrap();
                let id = id.clone();
                let before: &Value = row.get(5).unwrap();
                let before = before.clone();
                let after: &Value = row.get(6).unwrap();
                let after = after.clone();
                let updates: &Value = row.get(7).unwrap();
                let updates = updates.clone();

                let as_blob = |v: Value| match v {
                    Value::Blob(b) => Some(b),
                    _ => None,
                };

                rows.push(CdcRow {
                    change_id,
                    change_time,
                    change_type,
                    table_name,
                    id,
                    before: as_blob(before),
                    after: as_blob(after),
                    updates: as_blob(updates),
                });
            }
            StepResult::Done => break,
            StepResult::IO | StepResult::Yield => {
                stmt._io().step().unwrap();
            }
            other => panic!("unexpected step result reading turso_cdc: {other:?}"),
        }
    }
    rows
}

fn decode_record(blob: &[u8]) -> Vec<Value> {
    let mut rec = ImmutableRecord::new(blob.len()).unwrap();
    rec.start_serialization(blob).unwrap();
    rec.get_values_owned().unwrap()
}

/// FINDING (Fase 0, punto 1): `after`/`before`/`updates` son BLOBs que contienen un
/// **record binario SQLite/turso estándar** (el mismo formato que una fila de tabla),
/// serializado con `MakeRecord` sobre TODAS las columnas no-generadas de la tabla, en el
/// orden de columnas del schema. Para un INSERT, `after` trae todas las columnas con sus
/// valores nuevos; `before` es NULL. Para un UPDATE, `after` trae el estado completo
/// post-cambio y `updates` trae un record del mismo shape con NULL en las columnas que NO
/// cambiaron (heurística: para distinguir "no cambió" de "cambió a NULL" haría falta
/// antes/después combinados; para nuestro caso de uso basta leer `after` completo, que ya
/// trae el row completo post-cambio). Para un DELETE, `before` trae el estado previo y
/// `after` es NULL.
///
/// FINDING CRÍTICO (contradice el código actual): `change_type` NO es 0=insert/1=update/2=delete
/// como asumía `cdc.rs` (`if event.change_type == 2 { continue }` tratándolo como delete).
/// El mapeo real observado es:
///   - `change_type == 1` → INSERT
///   - `change_type == 0` → UPDATE
///   - `change_type == -1` → DELETE
///   - `change_type == 2` → marcador de **COMMIT de transacción** (fila sintética con
///     `table_name = NULL`, `id = NULL`, `before/after/updates` todos NULL). Aparece una vez
///     por cada transacción confirmada (en este test, cada `conn.execute` individual es su
///     propia transacción implícita, de ahí una fila de commit tras cada INSERT/UPDATE/DELETE).
///     Esta fila debe ser **ignorada** por `read_cdc_events` (no es una fila de datos), pero
///     su `change_id` sí cuenta para el cursor de avance.
///
/// Por tanto: `read_cdc_events` debe (a) saltar filas con `table_name IS NULL` (marcadores de
/// commit) y (b) decodificar `after`/`before` completos con `ImmutableRecord` según el orden
/// declarado de columnas de la tabla (que Rust ya conoce vía schema.json, Fase 1) para
/// reconstruir org_id/doc_id/columnas de negocio, en vez de dejarlos vacíos. `apply_cdc_events`
/// debe usar `change_type == -1` (no `== 2`) para detectar deletes.
#[test]
fn cdc_after_blob_is_standard_record_format_with_full_row() {
    let (_dir, conn) = temp_conn();
    exec(
        &conn,
        "CREATE TABLE customers (org_id TEXT NOT NULL, doc_id TEXT NOT NULL, name TEXT, \
         email TEXT, change_time INTEGER, node_id TEXT, PRIMARY KEY (org_id, doc_id))",
    );
    exec(&conn, "PRAGMA capture_data_changes_conn='full'");

    exec(
        &conn,
        "INSERT INTO customers (org_id, doc_id, name, email, change_time, node_id) \
         VALUES ('org1', 'c1', 'Alice', 'alice@test.com', 1000, 'node-a')",
    );
    exec(
        &conn,
        "UPDATE customers SET name='Alice B', change_time=2000 \
         WHERE org_id='org1' AND doc_id='c1'",
    );
    exec(&conn, "DELETE FROM customers WHERE org_id='org1' AND doc_id='c1'");

    let all_rows = read_all_cdc_rows(&conn);
    for (i, r) in all_rows.iter().enumerate() {
        eprintln!(
            "row[{i}] change_id={} change_type={} table={} id={:?} before={} after={} updates={}",
            r.change_id, r.change_type, r.table_name, r.id,
            r.before.is_some(), r.after.is_some(), r.updates.is_some()
        );
    }

    // Every real data change is immediately followed by a synthetic commit marker
    // (table_name NULL, change_type == 2) because each `exec` is its own transaction.
    let commit_markers: Vec<_> = all_rows.iter().filter(|r| r.table_name == "Null").collect();
    assert_eq!(commit_markers.len(), 3, "one commit marker per autocommit transaction");
    for marker in &commit_markers {
        assert_eq!(marker.change_type, 2, "commit marker change_type must be 2");
        assert!(marker.before.is_none() && marker.after.is_none() && marker.updates.is_none());
    }

    let rows: Vec<_> = all_rows.iter().filter(|r| r.table_name == "customers").collect();
    assert_eq!(rows.len(), 3, "expected insert+update+delete data rows in turso_cdc");

    // --- INSERT ---
    let insert_row = rows[0];
    assert_eq!(insert_row.change_type, 1, "insert change_type must be 1");
    assert!(insert_row.before.is_none(), "before should be NULL on insert");
    let after_values = decode_record(insert_row.after.as_ref().expect("after present on insert"));
    eprintln!("INSERT after decoded: {after_values:?}");
    // Columns in schema order: org_id, doc_id, name, email, change_time, node_id
    assert_eq!(after_values.len(), 6);
    assert_eq!(after_values[0], Value::from_text("org1".to_string()));
    assert_eq!(after_values[1], Value::from_text("c1".to_string()));
    assert_eq!(after_values[2], Value::from_text("Alice".to_string()));
    assert_eq!(after_values[3], Value::from_text("alice@test.com".to_string()));
    assert_eq!(after_values[4], Value::from_i64(1000));
    assert_eq!(after_values[5], Value::from_text("node-a".to_string()));

    // --- UPDATE ---
    let update_row = rows[1];
    assert_eq!(update_row.change_type, 0, "update change_type must be 0");
    let after_values = decode_record(update_row.after.as_ref().expect("after present on update"));
    eprintln!("UPDATE after decoded: {after_values:?}");
    assert_eq!(after_values[2], Value::from_text("Alice B".to_string()));
    assert_eq!(after_values[4], Value::from_i64(2000));
    if let Some(updates) = &update_row.updates {
        let updates_values = decode_record(updates);
        eprintln!("UPDATE updates-patch decoded: {updates_values:?}");
    }

    // --- DELETE ---
    let delete_row = rows[2];
    assert_eq!(delete_row.change_type, -1, "delete change_type must be -1 (NOT 2, which is the commit marker)");
    assert!(delete_row.after.is_none(), "after should be NULL on delete");
    let before_values = decode_record(delete_row.before.as_ref().expect("before present on delete"));
    eprintln!("DELETE before decoded: {before_values:?}");
    assert_eq!(before_values[0], Value::from_text("org1".to_string()));
    assert_eq!(before_values[1], Value::from_text("c1".to_string()));
    assert_eq!(before_values[2], Value::from_text("Alice B".to_string()));
}

/// FINDING (Fase 0, punto 1 - detalle del patch record `updates`): el blob `updates` en un
/// UPDATE NO tiene el mismo shape que `before`/`after` (N columnas). Tiene **2*N columnas**:
///   - Las primeras N son flags `0`/`1` indicando si la columna en esa posición cambió.
///   - Las siguientes N son los valores nuevos de las columnas que cambiaron (`NULL` para las
///     que no cambiaron).
/// Ejemplo real (tabla de 6 columnas, cambiaron `name` (idx 2) y `change_time` (idx 4)):
/// `[0,0,1,0,1,0, NULL,NULL,"Alice B",NULL,2000,NULL]`.
/// Para nuestro caso de uso (LWW por fila completa, no merge de columnas) no necesitamos
/// `updates`: basta leer `after` completo. Se documenta por si en el futuro se requiere merge
/// column-level.
#[test]
fn cdc_updates_blob_is_changed_flags_followed_by_new_values() {
    let (_dir, conn) = temp_conn();
    exec(
        &conn,
        "CREATE TABLE customers (org_id TEXT, doc_id TEXT, name TEXT, email TEXT, \
         change_time INTEGER, node_id TEXT, PRIMARY KEY (org_id, doc_id))",
    );
    exec(&conn, "PRAGMA capture_data_changes_conn='full'");
    exec(
        &conn,
        "INSERT INTO customers (org_id, doc_id, name, email, change_time, node_id) \
         VALUES ('org1', 'c1', 'Alice', 'alice@test.com', 1000, 'node-a')",
    );
    exec(
        &conn,
        "UPDATE customers SET name='Alice B', change_time=2000 WHERE org_id='org1' AND doc_id='c1'",
    );

    let all_rows = read_all_cdc_rows(&conn);
    let update_row = all_rows
        .iter()
        .find(|r| r.table_name == "customers" && r.change_type == 0)
        .expect("update row present");
    let updates = decode_record(update_row.updates.as_ref().expect("updates present on update"));
    assert_eq!(updates.len(), 12, "updates record has 2*N entries (flags + values)");
    let flags = &updates[0..6];
    let values = &updates[6..12];
    assert_eq!(flags[2], Value::from_i64(1), "name flag set (changed)");
    assert_eq!(flags[4], Value::from_i64(1), "change_time flag set (changed)");
    assert_eq!(flags[0], Value::from_i64(0), "org_id flag unset (unchanged)");
    assert_eq!(values[2], Value::from_text("Alice B".to_string()));
    assert_eq!(values[4], Value::from_i64(2000));
    assert_eq!(values[0], Value::Null, "unchanged column value is NULL in patch record");
}

/// FINDING (Fase 0, punto 2 parcial): `turso_cdc` es una tabla normal append-only; turso NO
/// la poda automáticamente. `change_id` es un contador monotónico (`INTEGER PRIMARY KEY`,
/// probablemente AUTOINCREMENT) compartido entre TODAS las tablas con CDC activo (no es
/// por-tabla), lo que confirma que un cursor único `since_change_id` global (o por-org si
/// filtramos por org_id decodificado del row) es correcto para el loop de sync, y que hace
/// falta un mecanismo explícito de retención/poda (borrar filas con change_id < cursor
/// confirmado por todos los peers) para que `turso_cdc` no crezca indefinidamente.
#[test]
fn cdc_change_id_is_monotonic_across_tables_and_table_is_not_auto_pruned() {
    let (_dir, conn) = temp_conn();
    exec(
        &conn,
        "CREATE TABLE customers (org_id TEXT, doc_id TEXT, name TEXT, PRIMARY KEY (org_id, doc_id))",
    );
    exec(
        &conn,
        "CREATE TABLE suppliers (org_id TEXT, doc_id TEXT, name TEXT, PRIMARY KEY (org_id, doc_id))",
    );
    exec(&conn, "PRAGMA capture_data_changes_conn='full'");

    exec(&conn, "INSERT INTO customers (org_id, doc_id, name) VALUES ('o', 'c1', 'a')");
    exec(&conn, "INSERT INTO suppliers (org_id, doc_id, name) VALUES ('o', 's1', 'b')");
    exec(&conn, "INSERT INTO customers (org_id, doc_id, name) VALUES ('o', 'c2', 'c')");

    let all_rows = read_all_cdc_rows(&conn);
    let rows: Vec<_> = all_rows.iter().filter(|r| r.table_name != "Null").collect();
    assert_eq!(rows.len(), 3);
    // change_id strictly increasing across tables => single global sequence.
    assert!(rows[0].change_id < rows[1].change_id);
    assert!(rows[1].change_id < rows[2].change_id);
    assert_eq!(rows[0].table_name, "customers");
    assert_eq!(rows[1].table_name, "suppliers");
    assert_eq!(rows[2].table_name, "customers");

    // No automatic pruning: rows remain readable after further unrelated writes.
    exec(&conn, "INSERT INTO customers (org_id, doc_id, name) VALUES ('o', 'c3', 'd')");
    let all_rows_after = read_all_cdc_rows(&conn);
    let rows_after: Vec<_> = all_rows_after.iter().filter(|r| r.table_name != "Null").collect();
    assert_eq!(rows_after.len(), 4, "turso_cdc must not auto-prune old rows");
}
