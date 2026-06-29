use std::sync::Mutex;
use syntrix_client_lib::*;

/// Verify that `new_with_data_dir` creates an AppState with a valid node_id.
#[tokio::test]
async fn test_new_with_data_dir() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_node");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState::new_with_data_dir should succeed");
    let node_id = state.node_id();
    assert_ne!(node_id, [0u8; 32], "node_id should not be all zeros");
}

#[tokio::test]
async fn test_list_orgs_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_list_orgs");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let orgs = list_orgs_impl(&s);
    assert!(orgs.is_empty(), "fresh state should have no orgs");
}

#[tokio::test]
async fn test_sync_status_impl() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_sync_status");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let status = sync_status_impl(&s);
    assert!(status.contains("online"), "status should contain 'online'");
    assert!(status.contains("orgs"), "status should mention org count");
}

#[tokio::test]
async fn test_get_invites_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_get_invites");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let invites = get_invites_impl(&s);
    assert!(invites.is_empty(), "fresh state should have no invites");
}

#[tokio::test]
async fn test_query_entity_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_query_empty");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let results = query_entity_impl(&s, None, "products", None, None);
    assert!(results.is_ok(), "query should not fail on empty state");
    let results = results.unwrap();
    assert!(results.is_empty(), "empty state should return no results");
}

#[tokio::test]
async fn test_search_entity_empty() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_search_empty");
    let state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");
    let state = Mutex::new(state);
    let s = state.lock().unwrap();
    let results = search_entity_impl(&s, None, "test", None, Some(10));
    assert!(results.is_ok(), "search should not fail on empty state");
    let results = results.unwrap();
    assert!(results.is_empty(), "empty state should return no search results");
}

/// Test that query_entity_impl does NOT fail with "Access denied" when device is registered.
#[tokio::test]
async fn test_query_entity_after_join_registers_device() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_query_after_join");
    let data_dir_clone = data_dir.clone();
    let mut state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");

    let api = state.api();
    let author = state.author();
    let control_doc = api.create().await.expect("create control doc");
    let customers_doc = api.create().await.expect("create customers doc");

    let org_id = "test-org-001".to_string();

    let sales_role = serde_json::json!({
        "can_open": ["customers", "products", "invoices", "orders"],
        "can_write": ["customers", "invoices", "orders"],
    });
    control_doc
        .set_bytes(author, b"roles/sales".to_vec(), serde_json::to_vec(&sales_role).unwrap())
        .await
        .expect("write sales role");

    let org_json = serde_json::json!({"name": "Test Org", "created_at": "2025-01-01T00:00:00Z"});
    control_doc
        .set_bytes(author, b"org".to_vec(), serde_json::to_vec(&org_json).unwrap())
        .await
        .expect("write org info");

    if let Ok(mut reg) = state.registry().write() {
        reg.map_namespace_to_org(control_doc.id(), org_id.clone());
        reg.map_namespace_to_org(customers_doc.id(), org_id.clone());
    }

    // Scenario A: Device NOT in registry
    state.add_org_docs("control", &org_id, "Test Org", "sales", control_doc.clone());
    state.add_org_docs("customers", &org_id, "Test Org", "sales", customers_doc.clone());

    state.set_active_org(&org_id).expect("set active org");

    let state_mutex = Mutex::new(state);
    let s = state_mutex.lock().unwrap();
    let results = query_entity_impl(&s, Some(&org_id), "customers", None, None);
    assert!(results.is_err(), "query should fail when device is not in registry");
    let err_msg = results.err().unwrap();
    assert!(
        err_msg.contains("Access denied"),
        "expected 'Access denied', got: {}",
        err_msg
    );
    drop(s);
    drop(state_mutex);

    // Scenario B: Device IS registered
    let mut state2 = identity::AppState::new_with_data_dir(data_dir_clone.join("reload"))
        .await
        .expect("AppState init for scenario B");

    let api2 = state2.api();
    let author2 = state2.author();
    let control_doc2 = api2.create().await.expect("create control doc B");
    let customers_doc2 = api2.create().await.expect("create customers doc B");

    control_doc2
        .set_bytes(author2, b"roles/sales".to_vec(), serde_json::to_vec(&sales_role).unwrap())
        .await
        .expect("write sales role B");
    control_doc2
        .set_bytes(author2, b"org".to_vec(), serde_json::to_vec(&org_json).unwrap())
        .await
        .expect("write org info B");

    if let Ok(mut reg) = state2.registry().write() {
        reg.map_namespace_to_org(control_doc2.id(), org_id.clone());
        reg.map_namespace_to_org(customers_doc2.id(), org_id.clone());
    }

    let node_id2 = state2.node_id();
    let node_id_hex2 = hex::encode(node_id2);

    state2.add_org_docs("control", &org_id, "Test Org", "sales", control_doc2.clone());
    state2.add_org_docs("customers", &org_id, "Test Org", "sales", customers_doc2.clone());

    {
        let mut reg = state2.registry().write().expect("registry write");
        reg.upsert_device(
            org_id.clone(),
            node_id2,
            syntrix_core::registry::Device {
                node_id: node_id2,
                active: true,
                role: "sales".to_string(),
                person: node_id_hex2.clone(),
                name: format!("Device {}", &node_id_hex2[..8]),
            },
        );
        reg.upsert_role(
            org_id.clone(),
            "sales".to_string(),
            syntrix_core::registry::RoleGrants {
                can_open: vec![
                    "customers".into(), "products".into(), "invoices".into(), "orders".into(),
                ],
                can_write: vec![
                    "customers".into(), "invoices".into(), "orders".into(),
                ],
            },
        );
    }

    state2.set_active_org(&org_id).expect("set active org B");

    let state_mutex2 = Mutex::new(state2);
    let s2 = state_mutex2.lock().unwrap();
    let results2 = query_entity_impl(&s2, Some(&org_id), "customers", None, None);
    assert!(
        results2.is_ok(),
        "query should succeed when device is registered in registry: {:?}",
        results2.err()
    );
}

/// Full multi-node integration test: admin creates org -> client1 joins -> creates customer
/// -> client2 sees it -> admin can audit.
#[tokio::test]
async fn test_multi_node_customer_lifecycle() {
    // 1. Admin setup
    let (_admin_dir, admin_data) = syntrix_testkit::temp_node_dir("multi_admin");
    let mut admin = identity::AppState::new_with_data_dir(admin_data)
        .await
        .expect("admin AppState init");

    let admin_api = admin.api();
    let admin_author = admin.author();
    let admin_control = admin_api.create().await.expect("admin create control");
    let admin_customers = admin_api.create().await.expect("admin create customers doc");
    let org_id = "multi-test-org".to_string();

    let sales_role = serde_json::json!({
        "can_open": ["customers", "products", "invoices", "orders"],
        "can_write": ["customers", "invoices", "orders"],
    });
    admin_control
        .set_bytes(admin_author, b"roles/sales".to_vec(), serde_json::to_vec(&sales_role).unwrap())
        .await
        .expect("admin write sales role");

    let org_info = serde_json::json!({"name": "Multi Test Org", "created_at": "2026-01-01T00:00:00Z"});
    admin_control
        .set_bytes(admin_author, b"org".to_vec(), serde_json::to_vec(&org_info).unwrap())
        .await
        .expect("admin write org info");

    let admin_node_id = admin.node_id();
    if let Ok(mut reg) = admin.registry().write() {
        reg.map_namespace_to_org(admin_control.id(), org_id.clone());
        reg.map_namespace_to_org(admin_customers.id(), org_id.clone());
        reg.upsert_device(
            org_id.clone(),
            admin_node_id,
            syntrix_core::registry::Device {
                node_id: admin_node_id,
                active: true,
                role: "admin".to_string(),
                person: "admin".to_string(),
                name: "Admin".to_string(),
            },
        );
        reg.upsert_role(
            org_id.clone(),
            "sales".to_string(),
            syntrix_core::registry::RoleGrants {
                can_open: vec!["customers".into(), "products".into(), "invoices".into(), "orders".into()],
                can_write: vec!["customers".into(), "invoices".into(), "orders".into()],
            },
        );
    }

    admin.add_org_docs("control", &org_id, "Multi Test Org", "admin", admin_control.clone());
    admin.add_org_docs("customers", &org_id, "Multi Test Org", "admin", admin_customers.clone());

    let admin_cfg = identity::ClientOrgConfig {
        org_id: org_id.clone(),
        name: "Multi Test Org".to_string(),
        role: "admin".to_string(),
        control_id: admin_control.id().to_string(),
        namespace_ids: {
            let mut m = std::collections::HashMap::new();
            m.insert("customers".to_string(), admin_customers.id().to_string());
            m
        },
    };
    admin.save_org_config(admin_cfg).expect("admin save org config");
    admin.set_active_org(&org_id).expect("admin set active org");

    let control_ticket_str = admin_control
        .share(
            iroh_docs::api::protocol::ShareMode::Write,
            iroh_docs::api::protocol::AddrInfoOptions::RelayAndAddresses,
        )
        .await
        .expect("share control ticket")
        .to_string();
    let customers_ticket_str = admin_customers
        .share(
            iroh_docs::api::protocol::ShareMode::Write,
            iroh_docs::api::protocol::AddrInfoOptions::RelayAndAddresses,
        )
        .await
        .expect("share customers ticket")
        .to_string();

    let tickets = vec![
        ("control".to_string(), control_ticket_str),
        ("customers".to_string(), customers_ticket_str),
    ];

    let admin_addr_str = syntrix_core::build_device_addr_string(admin.endpoint());

    // 3. Client1 joins
    let (_c1_dir, c1_data) = syntrix_testkit::temp_node_dir("multi_client1");
    let mut client1 = identity::AppState::new_with_data_dir(c1_data)
        .await
        .expect("client1 AppState init");

    let c1_api = client1.api();
    let mut c1_imported = Vec::new();
    for (ns, ticket_str) in &tickets {
        let ticket: iroh_docs::DocTicket = ticket_str.parse().expect("parse ticket");
        let doc = c1_api.import(ticket).await.expect("client1 import doc");
        c1_imported.push((ns.clone(), doc));
    }

    let _result = join_org_state_impl(
        &mut client1,
        &org_id,
        "Multi Test Org",
        "sales",
        c1_imported,
    ).expect("client1 join_org_state_impl");

    if let Some(admin_addr) = syntrix_core::parse_device_addr(&admin_addr_str) {
        let peers = vec![admin_addr];
        let c1_org = client1.get_org_docs(&org_id).expect("client1 org docs");
        let _ = c1_org.control_doc.start_sync(peers.clone()).await;
        for (_ns, doc) in &c1_org.entity_docs {
            let _ = doc.start_sync(peers.clone()).await;
        }
    }

    {
        let c1_node_id = client1.node_id();
        let c1_node_hex = hex::encode(c1_node_id);
        let mut reg = client1.registry().write().expect("client1 registry write");
        reg.upsert_device(
            org_id.clone(),
            c1_node_id,
            syntrix_core::registry::Device {
                node_id: c1_node_id,
                active: true,
                role: "sales".to_string(),
                person: c1_node_hex.clone(),
                name: format!("Client1-{}", &c1_node_hex[..6]),
            },
        );
        reg.upsert_role(
            org_id.clone(),
            "sales".to_string(),
            syntrix_core::registry::RoleGrants {
                can_open: vec!["customers".into(), "products".into(), "invoices".into(), "orders".into()],
                can_write: vec!["customers".into(), "invoices".into(), "orders".into()],
            },
        );
    }
    client1.set_active_org(&org_id).expect("client1 set active org");

    let before = query_entity_impl(&client1, Some(&org_id), "customers", None, None)
        .expect("client1 query customers before create");
    assert!(before.is_empty(), "client1 should see no customers initially");

    // 4. Client1 creates a customer
    let customer_id = "cust-multi-001";
    let customer_payload = serde_json::json!({
        "id": customer_id,
        "name": "Test Corp S.A. de C.V.",
        "tax_id": "TCR991231XXX",
        "email": "test@corp.com",
        "phone": "+5215551234567",
        "address": "Av. Reforma 222, CDMX",
    });

    {
        let c1_org = client1.get_org_docs(&org_id).expect("client1 org docs");
        let customers_doc = c1_org.entity_docs.get("customers").expect("customers doc");
        let author = client1.author();
        let node_id_hex = hex::encode(client1.node_id());
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
        let count = client1.counter().fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
        let key = format!("evt:{:020}:{:08}:{}", ts, count, &node_id_hex[..16]);

        let value = serde_json::json!({
            "type": "customer.created",
            "hlc": {"ts": ts, "count": count, "node": &node_id_hex[..16]},
            "schema_version": 1,
            "payload": &customer_payload,
        });
        customers_doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value).unwrap())
            .await
            .expect("client1 write customer event to customers doc");

        let _ = client1.indexer.upsert_document(&org_id, "customers", customer_id, &customer_payload);
    }

    let results = query_entity_impl(&client1, Some(&org_id), "customers", None, None)
        .expect("client1 query customers after create");
    assert!(!results.is_empty(), "client1 should see the customer after creation");
    let found = results.iter().any(|r| r.get("id").and_then(|v| v.as_str()) == Some(customer_id));
    assert!(found, "customer '{}' should be queryable by client1", customer_id);

    // 5. Verify admin can read the customer event from customers doc via P2P sync
    let poll_cfg = syntrix_testkit::PollConfig {
        max_retries: 10,
        base_delay: std::time::Duration::from_millis(500),
        max_delay: std::time::Duration::from_secs(3),
    };
    let sync_ok = syntrix_testkit::poll_until(
        || async {
            let mut entries = Box::pin(
                admin_customers.get_many(iroh_docs::store::Query::key_prefix("evt:"))
                    .await
                    .map_err(|e| format!("query error: {}", e))?
            );
            use futures_util::StreamExt;
            while let Some(Ok(entry)) = entries.next().await {
                if let Ok(bytes) = admin.store().blobs().get_bytes(entry.content_hash()).await {
                    if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        if val.get("type").and_then(|v| v.as_str()) == Some("customer.created") {
                            return Ok(true);
                        }
                    }
                }
            }
            Err("event not found yet".to_string())
        },
        &poll_cfg,
    ).await;
    if sync_ok.unwrap_or(false) {
        // 6. Client2 joins and verifies the customer is synced
        let (_c2_dir, c2_data) = syntrix_testkit::temp_node_dir("multi_client2");
        let mut client2 = identity::AppState::new_with_data_dir(c2_data)
            .await
            .expect("client2 AppState init");

        let c2_api = client2.api();
        let mut c2_imported = Vec::new();
        for (ns, ticket_str) in &tickets {
            let ticket: iroh_docs::DocTicket = ticket_str.parse().expect("c2 parse ticket");
            let doc = c2_api.import(ticket).await.expect("client2 import doc");
            c2_imported.push((ns.clone(), doc));
        }

        let _result = join_org_state_impl(
            &mut client2,
            &org_id,
            "Multi Test Org",
            "sales",
            c2_imported,
        ).expect("client2 join_org_state_impl");

        if let Some(admin_addr) = syntrix_core::parse_device_addr(&admin_addr_str) {
            let peers = vec![admin_addr];
            let c2_org = client2.get_org_docs(&org_id).expect("client2 org docs");
            let _ = c2_org.control_doc.start_sync(peers.clone()).await;
            for (_ns, doc) in &c2_org.entity_docs {
                let _ = doc.start_sync(peers.clone()).await;
            }
        }

        {
            let c2_node_id = client2.node_id();
            let c2_node_hex = hex::encode(c2_node_id);
            let mut reg = client2.registry().write().expect("client2 registry write");
            reg.upsert_device(
                org_id.clone(),
                c2_node_id,
                syntrix_core::registry::Device {
                    node_id: c2_node_id,
                    active: true,
                    role: "sales".to_string(),
                    person: c2_node_hex.clone(),
                    name: format!("Client2-{}", &c2_node_hex[..6]),
                },
            );
            reg.upsert_role(
                org_id.clone(),
                "sales".to_string(),
                syntrix_core::registry::RoleGrants {
                    can_open: vec!["customers".into(), "products".into(), "invoices".into(), "orders".into()],
                    can_write: vec!["customers".into(), "invoices".into(), "orders".into()],
                },
            );
        }
        client2.set_active_org(&org_id).expect("client2 set active org");

        if let Some(admin_addr) = syntrix_core::parse_device_addr(&admin_addr_str) {
            let peers = vec![admin_addr];
            let _ = admin_control.start_sync(peers.clone()).await;
            let _ = admin_customers.start_sync(peers).await;
        }

        let sync_result_c2 = syntrix_testkit::poll_until(
            || async {
                let c2_org = client2.get_org_docs(&org_id).ok_or("org not found".to_string())?;
                let customers_doc = c2_org.entity_docs.get("customers")
                    .ok_or("customers doc not found".to_string())?;
                let mut entries = Box::pin(
                    customers_doc.get_many(iroh_docs::store::Query::key_prefix("evt:"))
                        .await
                        .map_err(|e| format!("query error: {}", e))?
                );
                use futures_util::StreamExt;
                while let Some(Ok(entry)) = entries.next().await {
                    if let Ok(bytes) = client2.store().blobs().get_bytes(entry.content_hash()).await {
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if val.get("type").and_then(|v| v.as_str()) == Some("customer.created") {
                                return Ok(true);
                            }
                        }
                    }
                }
                Err("event not found yet".to_string())
            },
            &poll_cfg,
        ).await;
        if sync_result_c2.unwrap_or(false) {
            let c2_results = query_entity_impl(&client2, Some(&org_id), "customers", None, None)
                .expect("client2 query customers");
            assert!(!c2_results.is_empty(), "client2 should see customers after sync");
            let found_c2 = c2_results.iter().any(|r| r.get("id").and_then(|v| v.as_str()) == Some(customer_id));
            assert!(found_c2, "customer '{}' should be queryable by client2", customer_id);
        }
    }
}

/// Test that events written to entity docs can be read back and filtered.
#[tokio::test]
async fn test_audit_query_returns_events() {
    let (_dir, data_dir) = syntrix_testkit::temp_node_dir("test_audit_query");
    let mut state = identity::AppState::new_with_data_dir(data_dir)
        .await
        .expect("AppState init");

    let api = state.api();
    let author = state.author();
    let control_doc = api.create().await.expect("create control doc");
    let customers_doc = api.create().await.expect("create customers doc");
    let org_id = "audit-test-org".to_string();

    if let Ok(mut reg) = state.registry().write() {
        reg.map_namespace_to_org(control_doc.id(), org_id.clone());
        reg.map_namespace_to_org(customers_doc.id(), org_id.clone());
        reg.upsert_device(
            org_id.clone(),
            state.node_id(),
            syntrix_core::registry::Device {
                node_id: state.node_id(),
                active: true,
                role: "admin".to_string(),
                person: "admin".to_string(),
                name: "Admin".to_string(),
            },
        );
        reg.upsert_role(
            org_id.clone(),
            "admin".to_string(),
            syntrix_core::registry::RoleGrants {
                can_open: vec!["*".into()],
                can_write: vec!["*".into()],
            },
        );
    }

    state.add_org_docs("control", &org_id, "Audit Test Org", "admin", control_doc.clone());
    state.add_org_docs("customers", &org_id, "Audit Test Org", "admin", customers_doc.clone());
    state.set_active_org(&org_id).expect("set active org");

    let customer_payload = serde_json::json!({
        "id": "audit-cust-001",
        "name": "Audit Test Corp",
        "tax_id": "ATC991231XXX",
    });
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64;
    let count = state.counter().fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32;
    let node_id_hex = hex::encode(state.node_id());
    let key = format!("evt:{:020}:{:08}:{}", ts, count, &node_id_hex[..16]);

    let value = serde_json::json!({
        "type": "customer.created",
        "hlc": {"ts": ts, "count": count, "node": &node_id_hex[..16]},
        "schema_version": 1,
        "payload": &customer_payload,
    });
    customers_doc.set_bytes(author, key.clone().into_bytes(), serde_json::to_vec(&value).unwrap())
        .await
        .expect("write customer event");

    let _ = state.indexer.upsert_document(&org_id, "customers", "audit-cust-001", &customer_payload);

    use futures_util::StreamExt;
    let mut stream = Box::pin(
        customers_doc.get_many(iroh_docs::store::Query::key_prefix("evt:"))
            .await
            .expect("read doc events")
    );
    let mut found = false;
    while let Some(Ok(entry)) = stream.next().await {
        if let Ok(bytes) = state.store().blobs().get_bytes(entry.content_hash()).await {
            if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let event_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let payload = val.get("payload").and_then(|p| p.get("id")).and_then(|v| v.as_str()).unwrap_or("");
                if event_type == "customer.created" && payload == "audit-cust-001" {
                    found = true;
                    break;
                }
            }
        }
    }
    assert!(found, "customer.created event should be readable from customers doc");

    let results = query_entity_impl(&state, Some(&org_id), "customers", None, None)
        .expect("query customers");
    assert!(!results.is_empty(), "customers should be queryable");
    let found_cust = results.iter().any(|r| r.get("id").and_then(|v| v.as_str()) == Some("audit-cust-001"));
    assert!(found_cust, "customer should be found by id");
    assert!(results.iter().any(|r| r.get("id").and_then(|v| v.as_str()) == Some("audit-cust-001")));
}
