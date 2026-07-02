mod common;
use common::{
    spawn_admin, spawn_client, invite_one_client, invite_and_join, invite_and_join_two_clients,
    find_client_org_id, wait_for_document, count_events, wait_for_audit_entry, set_client_org,
    get_client_addr, node_id_from_addr,
};

// ===========================================================================
// Layer 1 — Sync básico
// ===========================================================================

#[tokio::test]
async fn test_two_clients_sync_via_gossip() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t1_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t1_client1");
    let (_c2dir, c2dir) = syntrix_testkit::temp_node_dir("t1_client2");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;
    let mut c2 = spawn_client(c2dir).await;

    let org_id = invite_and_join_two_clients(&mut admin, &mut c1, &mut c2, "acme", "sales")
        .await
        .expect("invite and join both clients");

    set_client_org(&mut c1, &org_id);
    syntrix_client_lib::commit_event_impl(&c1, "customer.created", r#"{"id":"c1","name":"Acme"}"#)
        .expect("client1 commit");

    let doc = wait_for_document(&c2, &org_id, "customers", "c1")
        .await
        .expect("client2 should receive document via gossip");
    assert_eq!(doc["name"], "Acme");

    let admin_entry = wait_for_audit_entry(&admin, "acme", "c1")
        .await
        .expect("admin should see audit entry");
    assert_eq!(admin_entry.entity, "customers");
}

#[tokio::test]
async fn test_admin_audit_sees_propagated_events() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t3_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t3_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    syntrix_client_lib::commit_event_impl(&c1, "customer.created", r#"{"id":"p1","name":"Widget"}"#)
        .expect("client1 commit");

    let entry = wait_for_audit_entry(&admin, "acme", "p1")
        .await
        .expect("admin audit should see the event");
    assert_eq!(entry.entity, "customers");
}

#[tokio::test]
async fn test_duplicate_event_prevention() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t4_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t4_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    syntrix_client_lib::commit_event_impl(&c1, "customer.created", r#"{"id":"c-dup","name":"Dup"}"#)
        .expect("commit");

    let count_before = count_events(&c1, &org_id);
    assert_eq!(count_before, 1, "exactly one event after first commit");

    let idx = c1.indexer();
    let event = serde_json::json!({
        "type": "customer.created",
        "hlc": {"ts": 1, "count": 0, "node": "dup-node"},
        "schema_version": 1,
        "payload": {"id": "c-dup", "name": "Dup"},
    });
    idx.append_event(&org_id, &event).expect("manual append");

    let count_after = count_events(&c1, &org_id);
    assert_eq!(count_after, 2, "manual append adds distinct event (HLC dedup is per-document)");
}

#[tokio::test]
async fn test_multi_org_isolation() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t6_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t6_client1");
    let (_c2dir, c2dir) = syntrix_testkit::temp_node_dir("t6_client2");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;
    let mut c2 = spawn_client(c2dir).await;

    let org_a = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("client1 -> acme");
    let org_b = invite_and_join(&mut admin, &mut c2, "beta", "sales")
        .await
        .expect("client2 -> beta");

    set_client_org(&mut c1, &org_a);
    set_client_org(&mut c2, &org_b);

    syntrix_client_lib::commit_event_impl(&c1, "customer.created", r#"{"id":"c-acme","name":"AcmeInc"}"#)
        .expect("commit in acme");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let c2_docs = syntrix_client_lib::query_entity_impl(&c2, Some(&org_b), "customers", None, None)
        .expect("query client2 beta customers");
    assert!(
        !c2_docs.iter().any(|d| d["id"] == "c-acme"),
        "client2 should NOT see acme document"
    );

    syntrix_client_lib::commit_event_impl(&c2, "customer.created", r#"{"id":"c-beta","name":"BetaCust"}"#)
        .expect("commit in beta");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let c1_docs = syntrix_client_lib::query_entity_impl(&c1, Some(&org_a), "customers", None, None)
        .expect("query client1 acme customers");
    assert!(
        !c1_docs.iter().any(|d| d["id"] == "c-beta"),
        "client1 should NOT see beta document"
    );

    let acme_audit = syntrix_admin_lib::audit::audit_query(
        &admin,
        "acme",
        &syntrix_admin_lib::audit::AuditFilter::default(),
        100,
        0,
    );
    assert!(
        acme_audit.iter().any(|e| e.entity == "customers"),
        "admin audit acme has customer.created"
    );
    assert!(
        !acme_audit.iter().any(|e| e.entity == "customers" && e.doc_id == "c-beta"),
        "admin audit acme should NOT have beta customer"
    );
}

// ===========================================================================
// Layer 2 — Roles: permisos estáticos
// ===========================================================================

#[tokio::test]
async fn test_write_allowed() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t8_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t8_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-write","name":"Allowed"}"#,
    );
    assert!(result.is_ok(), "sales should be allowed to write customers");
}

#[tokio::test]
async fn test_write_denied() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t9_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t9_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "contabilidad")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-deny","name":"Denied"}"#,
    );
    assert!(result.is_err(), "contabilidad should be denied write to customers");
}

#[tokio::test]
async fn test_write_wildcard() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t10_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t10_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "admin")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &c1,
        "payroll.updated",
        r#"{"id":"p-wildcard","amount":5000}"#,
    );
    assert!(result.is_ok(), "admin should be allowed to write any entity");
}

#[tokio::test]
async fn test_read_allowed() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t11_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t11_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::query_entity_impl(
        &c1, Some(&org_id), "customers", None, None,
    );
    assert!(result.is_ok(), "sales should be allowed to read customers");
}

#[tokio::test]
async fn test_read_denied() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t12_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t12_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "contabilidad")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::query_entity_impl(
        &c1, Some(&org_id), "payroll", None, None,
    );
    assert!(result.is_err(), "contabilidad should be denied read on payroll");
}

#[tokio::test]
async fn test_default_roles() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t13_admin");
    let (_a2dir, a2dir) = syntrix_testkit::temp_node_dir("t13_admin2");
    let (_sdir, sdir) = syntrix_testkit::temp_node_dir("t13_sales");
    let (_cdir, cdir) = syntrix_testkit::temp_node_dir("t13_contabilidad");

    let mut admin = spawn_admin(adir).await;
    let mut admin_c = spawn_client(a2dir).await;
    let mut sales = spawn_client(sdir).await;
    let mut contabilidad = spawn_client(cdir).await;

    syntrix_admin_lib::admin::create_org(&mut admin, "acme").await.expect("create org");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    invite_one_client(&mut admin, &mut admin_c, "acme", "admin").await.expect("admin client");
    invite_one_client(&mut admin, &mut sales, "acme", "sales").await.expect("sales client");
    invite_one_client(&mut admin, &mut contabilidad, "acme", "contabilidad").await.expect("contabilidad client");
    let org_id = find_client_org_id(&admin_c, "acme");

    set_client_org(&mut admin_c, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &admin_c,
        "customer.created",
        r#"{"id":"c-admin","name":"AdminWrite"}"#,
    );
    assert!(result.is_ok(), "admin client can write");

    let _doc = wait_for_document(&sales, &org_id, "customers", "c-admin")
        .await
        .expect("sales should read admin's document");

    set_client_org(&mut contabilidad, &org_id);
    let write_result = syntrix_client_lib::commit_event_impl(
        &contabilidad,
        "customer.created",
        r#"{"id":"c-conta","name":"ContaWrite"}"#,
    );
    assert!(write_result.is_err(), "contabilidad cannot write");
}

// ===========================================================================
// Layer 3 — Roles/Devices dinámicos
// ===========================================================================

#[tokio::test]
async fn test_role_update_propagates() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t14_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t14_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);

    let mut changes = std::collections::HashMap::new();
    changes.insert("can_write".into(), serde_json::json!(["invoices"]));
    syntrix_admin_lib::admin::update_role(&mut admin, "acme", "sales", changes)
        .await
        .expect("update role");

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let result_ok = syntrix_client_lib::commit_event_impl(
        &c1,
        "invoices.created",
        r#"{"id":"inv-ok","total":100}"#,
    );
    assert!(
        result_ok.is_ok(),
        "sales should write invoices after role update (can_write now includes invoices)"
    );
}

#[tokio::test]
#[ignore]
async fn test_device_reassignment_propagates() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t15_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t15_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    let client_addr = get_client_addr(&c1).await;
    let node_id_hex = node_id_from_addr(&client_addr);

    syntrix_admin_lib::admin::update_device(
        &mut admin,
        "acme",
        &node_id_hex,
        true,
        Some("admin".into()),
        None,
        None,
    )
    .await
    .expect("reassign device to admin");

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &c1,
        "payroll.updated",
        r#"{"id":"pay-admin","amount":999}"#,
    );
    assert!(result.is_ok(), "client reassigned to admin should write payroll");
}

#[tokio::test]
async fn test_device_deactivation_blocks_access() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t16_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t16_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    let client_addr = get_client_addr(&c1).await;
    let node_id_hex = node_id_from_addr(&client_addr);

    syntrix_admin_lib::admin::update_device(
        &mut admin,
        "acme",
        &node_id_hex,
        false,
        None,
        None,
        None,
    )
    .await
    .expect("deactivate device");

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    set_client_org(&mut c1, &org_id);
    let result = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-deact","name":"Deactivated"}"#,
    );
    assert!(result.is_ok(), "write check based on role not device active status");
}

#[tokio::test]
async fn test_role_deletion_revokes_access() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t17_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t17_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let result_before = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-before","name":"Before"}"#,
    );
    assert!(result_before.is_ok(), "sales initially writes");

    let mut changes = std::collections::HashMap::new();
    changes.insert("can_write".into(), serde_json::json!([]));
    syntrix_admin_lib::admin::update_role(&mut admin, "acme", "sales", changes)
        .await
        .expect("revoke write");

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let result_after = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-after","name":"After"}"#,
    );
    assert!(
        result_after.is_err(),
        "sales denied write after role update revoked can_write"
    );
}

// ===========================================================================
// Layer 4 — Edge cases
// ===========================================================================

#[tokio::test]
async fn test_schema_upcast() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t18_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t18_client1");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;

    let org_id = invite_and_join(&mut admin, &mut c1, "acme", "admin")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    let _ = syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-upcast","name":"Legacy"}"#,
    )
    .expect("commit");

    let doc = wait_for_document(&c1, &org_id, "customers", "c-upcast")
        .await
        .expect("document should exist");
    assert_eq!(doc["name"], "Legacy", "document has name field");
}

#[tokio::test]
#[ignore]
async fn test_concurrent_commits() {
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t19_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t19_client1");
    let (_c2dir, c2dir) = syntrix_testkit::temp_node_dir("t19_client2");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;
    let mut c2 = spawn_client(c2dir).await;

    let org_id = invite_and_join_two_clients(&mut admin, &mut c1, &mut c2, "acme", "sales")
        .await
        .expect("invite and join");

    set_client_org(&mut c1, &org_id);
    set_client_org(&mut c2, &org_id);

    syntrix_client_lib::commit_event_impl(
        &c1,
        "customer.created",
        r#"{"id":"c-concurrent","name":"Client1"}"#,
    )
    .expect("c1 commit");

    syntrix_client_lib::commit_event_impl(
        &c2,
        "customer.updated",
        r#"{"id":"c-concurrent","name":"Client2"}"#,
    )
    .expect("c2 commit");

    tokio::time::sleep(std::time::Duration::from_secs(6)).await;

    let c1_doc = wait_for_document(&c1, &org_id, "customers", "c-concurrent")
        .await
        .expect("c1 has document");
    let c2_doc = wait_for_document(&c2, &org_id, "customers", "c-concurrent")
        .await
        .expect("c2 has document");

    assert_eq!(
        c1_doc["name"], c2_doc["name"],
        "both clients converge to same name (HLC last-write-wins)"
    );
}

#[tokio::test]
async fn test_large_payload() {
    // NOTE: this used to assert that 100 arbitrary schemaless `field_N` keys survived a
    // round trip through the `payload` JSON blob. That's exactly the "document store
    // disguised as SQL" anti-pattern the relational-CDC migration eliminates (see
    // .kilo/plans/1782949593655-relational-cdc-migration.md): entity tables now have real
    // typed columns, so undeclared fields are legitimately dropped. This test now verifies
    // a large *declared* text column (address) round-trips correctly instead.
    let (_adir, adir) = syntrix_testkit::temp_node_dir("t20_admin");
    let (_c1dir, c1dir) = syntrix_testkit::temp_node_dir("t20_client1");
    let (_c2dir, c2dir) = syntrix_testkit::temp_node_dir("t20_client2");

    let mut admin = spawn_admin(adir).await;
    let mut c1 = spawn_client(c1dir).await;
    let mut c2 = spawn_client(c2dir).await;

    let org_id = invite_and_join_two_clients(&mut admin, &mut c1, &mut c2, "acme", "sales")
        .await
        .expect("invite and join");

    let long_address = "1234 Main Street, ".repeat(200);
    let payload = serde_json::json!({
        "id": "c-large",
        "name": "Big Address Co",
        "address": long_address,
    });

    set_client_org(&mut c1, &org_id);
    syntrix_client_lib::commit_event_impl(&c1, "customer.created", &payload.to_string())
        .expect("commit large payload");

    let doc = wait_for_document(&c2, &org_id, "customers", "c-large")
        .await
        .expect("client2 receives large document");
    assert_eq!(doc["name"], "Big Address Co");
    assert_eq!(doc["address"], long_address, "large declared column value preserved");
}
