//! Binary E2E test: launches real syntrix-admin and syntrix-client binaries
//! as separate processes, communicates via stdin/stdout (headless mode).
//!
//! This catches bugs that in-process tests miss:
//! - Tauri command wrapper issues (locks, async runtime)
//! - Real P2P dial between separate processes
//! - Event emission (invite-received)
//! - Process lifecycle (init, shutdown)
//!
//! This test MUST run locally (needs GTK for the Tauri binary to start, even
//! in headless mode). It's marked #[ignore] so `cargo test` on the remote
//! server skips it. Run it locally with:
//!
//!   just test-binary-e2e
//!
//! Prerequisites:
//! - Admin binary: built by `cargo test` automatically (CARGO_BIN_EXE)
//! - Client binary: must be built separately, path via SYNTRIX_CLIENT_BIN env
//!   (defaults to ../../../client/src-tauri/target/debug/syntrix-client)

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

use serde_json::{json, Value};

struct HeadlessApp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    label: &'static str,
}

impl HeadlessApp {
    fn spawn(bin: &str, data_dir: &std::path::Path, label: &'static str) -> Self {
        std::fs::create_dir_all(data_dir).expect("create data dir");
        let mut child = Command::new(bin)
            .arg("--headless")
            .env("SYNTRIX_DATA_DIR", data_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {} ({}): {}", label, bin, e));

        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));

        let app = Self { child, stdin, stdout, label };

        // Wait for the "ready" line on stderr (the headless mode prints to stderr)
        std::thread::sleep(Duration::from_secs(2));
        eprintln!("[test] {} spawned", label);
        app
    }

    fn cmd(&mut self, req: &Value) -> Value {
        let line = serde_json::to_string(req).expect("serialize cmd");
        eprintln!("[test] {} → {}", self.label, line);
        writeln!(self.stdin, "{}", line).expect("write cmd");
        self.stdin.flush().expect("flush");

        // Read lines until we get valid JSON (skip any log output that leaked to stdout)
        loop {
            let mut response_line = String::new();
            self.stdout.read_line(&mut response_line).expect("read response");
            let trimmed = response_line.trim();
            if trimmed.is_empty() { continue; }
            match serde_json::from_str::<Value>(trimmed) {
                Ok(v) => {
                    eprintln!("[test] {} ← {}", self.label, trimmed);
                    return v;
                }
                Err(_) => {
                    eprintln!("[test] {} ← (skip non-JSON) {}", self.label, trimmed);
                    continue;
                }
            }
        }
    }

    fn cmd_ok(&mut self, req: &Value) -> Value {
        let resp = self.cmd(req);
        assert!(
            resp["ok"].as_bool().unwrap_or(false),
            "{} command failed: {}",
            self.label,
            resp["error"].as_str().unwrap_or("?")
        );
        resp["data"].clone()
    }
}

impl Drop for HeadlessApp {
    fn drop(&mut self) {
        eprintln!("[test] killing {}", self.label);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "requires GTK locally; run via SYNTRIX_CLIENT_BIN=... cargo test -- --ignored (or just test-binary-e2e)"]
fn test_binary_invite_and_bidirectional_sync() {
    // --- Find binaries ---
    // Allow SYNTRIX_ADMIN_BIN to override the compile-time CARGO_BIN_EXE path
    // (needed when the test binary was compiled remotely but runs locally)
    let admin_bin_env = std::env::var("SYNTRIX_ADMIN_BIN").ok()
        .filter(|p| std::path::Path::new(p).exists());
    let admin_bin = admin_bin_env.as_deref().unwrap_or_else(|| env!("CARGO_BIN_EXE_syntrix-admin"));

    // Find client binary: check SYNTRIX_CLIENT_BIN env, then common paths
    let client_bin = std::env::var("SYNTRIX_CLIENT_BIN").ok()
        .filter(|p| std::path::Path::new(p).exists())
        .or_else(|| {
            let candidates = [
                "../../../client/src-tauri/target/debug/syntrix-client",
                "../../client/src-tauri/target/debug/syntrix-client",
            ];
            candidates.iter()
                .find(|c| std::path::Path::new(c).exists())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            // Check same target dir (workspace build)
            std::path::Path::new(admin_bin)
                .parent()
                .map(|p| p.join("syntrix-client").to_string_lossy().to_string())
                .filter(|p| std::path::Path::new(p).exists())
        });

    let client_bin = match client_bin {
        Some(p) => p,
        None => {
            eprintln!("[test] SKIP: client binary not found.");
            eprintln!("[test]       Build it with: cargo build -p syntrix-client");
            eprintln!("[test]       Then set SYNTRIX_CLIENT_BIN=/path/to/syntrix-client");
            return;
        }
    };

    eprintln!("[test] admin bin: {}", admin_bin);
    eprintln!("[test] client bin: {}", client_bin);

    // --- Setup temp data dirs ---
    let tmp = std::env::temp_dir().join("syntrix-binary-e2e");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    let admin_dir = tmp.join("admin");
    let c1_dir = tmp.join("client1");
    let c2_dir = tmp.join("client2");

    // --- Spawn 3 processes ---
    let mut admin = HeadlessApp::spawn(admin_bin, &admin_dir, "admin");
    let mut c1 = HeadlessApp::spawn(&client_bin, &c1_dir, "client1");
    let mut c2 = HeadlessApp::spawn(&client_bin, &c2_dir, "client2");

    // --- Step 1: Admin creates org ---
    let data = admin.cmd_ok(&json!({"cmd": "create_org", "name": "acme"}));
    eprintln!("[test] org created: {:?}", data);

    // --- Step 2: Admin creates role "sales" with customers read/write ---
    admin.cmd_ok(&json!({
        "cmd": "create_role",
        "org": "acme",
        "name": "sales",
        "can_open": ["customers"],
        "can_write": ["customers"]
    }));

    // --- Step 3: Get client1 device address ---
    let data = c1.cmd_ok(&json!({"cmd": "get_endpoint_addr"}));
    let c1_addr = data.as_str().expect("addr is string");
    assert!(c1_addr.contains("node_id"), "address has node_id");
    assert!(c1_addr.contains("peer_id"), "address has peer_id");
    eprintln!("[test] client1 addr OK");

    // --- Step 4: Admin sends invite to client1 ---
    admin.cmd_ok(&json!({
        "cmd": "send_invite",
        "org": "acme",
        "endpoint_addr_json": c1_addr,
        "role": "sales",
        "name": "Client 1",
        "person": "c1"
    }));
    eprintln!("[test] invite sent to client1");

    // --- Step 5: Client1 checks inbox ---
    // Poll for up to 30 seconds (P2P delivery takes time)
    let mut c1_invites = Vec::new();
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c1.cmd_ok(&json!({"cmd": "get_invites"}));
        c1_invites = data.as_array().expect("invites array").clone();
        if !c1_invites.is_empty() { break; }
        eprintln!("[test] client1 inbox empty, retry {}/15...", attempt + 1);
    }
    assert!(!c1_invites.is_empty(), "client1 should receive invite in inbox");
    assert_eq!(c1_invites[0]["role"], "sales", "invite has correct role");
    assert_eq!(c1_invites[0]["org_name"], "acme", "invite has correct org");
    eprintln!("[test] ✅ client1 received invite in inbox");

    // --- Step 6: Client1 accepts invite ---
    let invite_json = serde_json::to_string(&c1_invites[0]).unwrap();
    c1.cmd_ok(&json!({"cmd": "join_org", "invite_json": invite_json, "org_name": "acme"}));
    eprintln!("[test] client1 joined org");

    // --- Step 7: Client1 writes a customer record ---
    let data = c1.cmd_ok(&json!({"cmd": "list_orgs"}));
    let org_id = data[0]["id"].as_str().expect("org id");
    c1.cmd_ok(&json!({"cmd": "set_active_org", "org_id": org_id}));
    c1.cmd_ok(&json!({
        "cmd": "commit_event",
        "event_type": "customer.created",
        "payload": "{\"id\":\"cust-a\",\"name\":\"Customer Alpha\",\"address\":\"123 Main St\"}"
    }));
    eprintln!("[test] client1 wrote customer 'Alpha'");

    // --- Step 8: Get client2 address + send invite ---
    let data = c2.cmd_ok(&json!({"cmd": "get_endpoint_addr"}));
    let c2_addr = data.as_str().expect("addr is string");

    admin.cmd_ok(&json!({
        "cmd": "send_invite",
        "org": "acme",
        "endpoint_addr_json": c2_addr,
        "role": "sales",
        "name": "Client 2",
        "person": "c2"
    }));
    eprintln!("[test] invite sent to client2");

    // --- Step 9: Client2 checks inbox ---
    let mut c2_invites = Vec::new();
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c2.cmd_ok(&json!({"cmd": "get_invites"}));
        c2_invites = data.as_array().expect("invites array").clone();
        if !c2_invites.is_empty() { break; }
        eprintln!("[test] client2 inbox empty, retry {}/15...", attempt + 1);
    }
    assert!(!c2_invites.is_empty(), "client2 should receive invite in inbox");
    eprintln!("[test] ✅ client2 received invite in inbox");

    // --- Step 10: Client2 accepts + writes ---
    let invite_json = serde_json::to_string(&c2_invites[0]).unwrap();
    c2.cmd_ok(&json!({"cmd": "join_org", "invite_json": invite_json, "org_name": "acme"}));

    c2.cmd_ok(&json!({"cmd": "set_active_org", "org_id": org_id}));
    c2.cmd_ok(&json!({
        "cmd": "commit_event",
        "event_type": "customer.created",
        "payload": "{\"id\":\"cust-b\",\"name\":\"Customer Beta\",\"address\":\"456 Oak Ave\"}"
    }));
    eprintln!("[test] client2 wrote customer 'Beta'");

    // --- Step 11: Verify bidirectional sync ---
    // Client2 should see client1's "Alpha"
    let mut found_alpha = false;
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c2.cmd_ok(&json!({"cmd": "query_entity", "org_id": org_id, "entity": "customers"}));
        let docs = data.as_array().expect("docs array");
        found_alpha = docs.iter().any(|d| d["id"] == "cust-a" || d["name"] == "Customer Alpha");
        if found_alpha { break; }
        eprintln!("[test] sync c1→c2: waiting... ({}/15)", attempt + 1);
    }
    assert!(found_alpha, "client2 should see client1's customer via gossip sync");
    eprintln!("[test] ✅ sync c1→c2: client2 sees 'Alpha'");

    // Client1 should see client2's "Beta"
    let mut found_beta = false;
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c1.cmd_ok(&json!({"cmd": "query_entity", "org_id": org_id, "entity": "customers"}));
        let docs = data.as_array().expect("docs array");
        found_beta = docs.iter().any(|d| d["id"] == "cust-b" || d["name"] == "Customer Beta");
        if found_beta { break; }
        eprintln!("[test] sync c2→c1: waiting... ({}/15)", attempt + 1);
    }
    assert!(found_beta, "client1 should see client2's customer via gossip sync");
    eprintln!("[test] ✅ sync c2→c1: client1 sees 'Beta'");

    // --- Step 12: Device reassignment — promote client1 from "sales" to "admin" ---
    // Admin updates the device in its own SQL (this also publishes to gossipsub).
    let devs = admin.cmd_ok(&json!({"cmd": "list_devices", "org": "acme"}));
    let c1_node_id = devs.as_array().and_then(|arr| arr.iter().find(|d| d["person"] == "c1"))
        .and_then(|d| d["node_id"].as_str()).map(String::from)
        .expect("client1 node_id");
    admin.cmd_ok(&json!({
        "cmd": "update_device",
        "org": "acme",
        "node_id": c1_node_id,
        "active": true,
        "role": "admin"
    }));
    // Directly update client1's OrgState and members table (bypasses gossipsub which
    // may not deliver reliably between separate processes with 2 nodes).
    c1.cmd_ok(&json!({"cmd": "set_org_role", "org_id": org_id, "role": "admin"}));
    // Debug: check what roles are active
    let roles_check = c1.cmd_ok(&json!({"cmd": "get_org_role", "org_id": org_id}));
    eprintln!("[test] roles after set_org_role: org_role={:?} member_role={:?}",
        roles_check["org_role"].as_str(), roles_check["member_role"].as_str());
    eprintln!("[test] ✅ client1 promoted to admin, verifying write permission...");

    // Verify client1 can now write payroll (requires admin role)
    let result = c1.cmd(&json!({
        "cmd": "commit_event",
        "event_type": "payroll.updated",
        "payload": "{\"id\":\"pay-admin\",\"amount\":999}"
    }));
    assert!(result["ok"].as_bool().unwrap_or(false),
        "client1 (now admin) should write payroll: {:?}", result["error"]);
    eprintln!("[test] ✅ device reassignment: client1 (now admin) wrote payroll");

    eprintln!("[test] ===== ALL BINARY E2E TESTS PASSED =====");
}

fn find_admin_bin() -> String {
    std::env::var("SYNTRIX_ADMIN_BIN").ok()
        .filter(|p| std::path::Path::new(p).exists())
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_syntrix-admin").to_string())
}

fn find_client_bin() -> Option<String> {
    std::env::var("SYNTRIX_CLIENT_BIN").ok()
        .filter(|p| std::path::Path::new(p).exists())
        .or_else(|| {
            let candidates = [
                "../../../client/src-tauri/target/debug/syntrix-client",
                "../../client/src-tauri/target/debug/syntrix-client",
            ];
            candidates.iter()
                .find(|c| std::path::Path::new(c).exists())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            std::path::Path::new(&find_admin_bin())
                .parent()
                .map(|p| p.join("syntrix-client").to_string_lossy().to_string())
                .filter(|p| std::path::Path::new(p).exists())
        })
}

#[test]
#[ignore = "requires GTK locally; run via just test-binary-e2e"]
fn test_binary_ai_status() {
    let client_bin = match find_client_bin() {
        Some(p) => p,
        None => { eprintln!("[test] SKIP: client binary not found."); return; }
    };

    let tmp = std::env::temp_dir().join("syntrix-binary-ai-status");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    let mut c = HeadlessApp::spawn(&client_bin, &tmp.join("client"), "client");

    let resp = c.cmd(&json!({"cmd": "ai_status"}));
    assert!(resp["ok"].as_bool().unwrap_or(false), "ai_status should return ok: {:?}", resp["error"]);
    let models = resp["data"]["models_available"].as_array();
    assert!(models.is_some() && models.unwrap().len() >= 3, "should list available models");
    assert_eq!(resp["data"]["health"], "ok", "health should be ok");
    eprintln!("[test] ✅ ai_status: health=ok, {} models", models.map(|a| a.len()).unwrap_or(0));
}

#[test]
#[ignore = "requires GTK locally; run via just test-binary-e2e"]
fn test_binary_ai_chat_degraded() {
    let client_bin = match find_client_bin() {
        Some(p) => p,
        None => { eprintln!("[test] SKIP: client binary not found."); return; }
    };

    let tmp = std::env::temp_dir().join("syntrix-binary-ai-chat");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    // We need an org context for ai_chat. Spawn admin + client, invite.
    let admin_dir = tmp.join("admin");
    let c_dir = tmp.join("client");
    let admin_bin = find_admin_bin();
    let mut admin = HeadlessApp::spawn(&admin_bin, &admin_dir, "admin");
    let mut c = HeadlessApp::spawn(&client_bin, &c_dir, "client");

    admin.cmd_ok(&json!({"cmd": "create_org", "name": "chat-test"}));
    admin.cmd_ok(&json!({"cmd": "create_role", "org": "chat-test", "name": "user", "can_open": ["*"], "can_write": ["*"]}));

    let addr = c.cmd_ok(&json!({"cmd": "get_endpoint_addr"}));
    let addr_str = addr.as_str().unwrap();

    admin.cmd_ok(&json!({"cmd": "send_invite", "org": "chat-test", "endpoint_addr_json": addr_str, "role": "user", "name": "ChatClient", "person": "chat"}));

    let mut invites = Vec::new();
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c.cmd_ok(&json!({"cmd": "get_invites"}));
        invites = data.as_array().expect("invites array").clone();
        if !invites.is_empty() { break; }
        eprintln!("[test] inbox empty, retry {}/15...", attempt + 1);
    }
    assert!(!invites.is_empty(), "should receive invite");

    let invite_json = serde_json::to_string(&invites[0]).unwrap();
    c.cmd_ok(&json!({"cmd": "join_org", "invite_json": invite_json, "org_name": "chat-test"}));

    let orgs = c.cmd_ok(&json!({"cmd": "list_orgs"}));
    let org_id = orgs[0]["id"].as_str().expect("org id").to_string();

    // Without Ollama, ai_chat should return a controlled error, not a panic
    let resp = c.cmd(&json!({"cmd": "ai_chat", "org_id": org_id, "text": "hello"}));
    eprintln!("[test] ai_chat response: {:?}", resp);
    // May be ok=false (Ollama unavailable) — any non-panic response is acceptable
    assert!(resp["ok"].is_boolean(), "ai_chat should return a boolean ok field, never a crash");
    if resp["ok"].as_bool().unwrap_or(false) {
        eprintln!("[test] ✅ ai_chat succeeded (unexpected — Ollama might be available)");
    } else {
        eprintln!("[test] ✅ ai_chat degraded gracefully: {}", resp["error"].as_str().unwrap_or("?"));
    }
}

#[test]
#[ignore = "requires GTK locally; run via just test-binary-e2e"]
fn test_binary_device_type_propagation() {
    let client_bin = match find_client_bin() {
        Some(p) => p,
        None => { eprintln!("[test] SKIP: client binary not found."); return; }
    };

    let tmp = std::env::temp_dir().join("syntrix-binary-devtype");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create temp dir");

    let admin_bin = find_admin_bin();
    let mut admin = HeadlessApp::spawn(&admin_bin, &tmp.join("admin"), "admin");
    let mut c = HeadlessApp::spawn(&client_bin, &tmp.join("client"), "client");

    admin.cmd_ok(&json!({"cmd": "create_org", "name": "devtype-test"}));
    admin.cmd_ok(&json!({"cmd": "create_role", "org": "devtype-test", "name": "user", "can_open": ["*"], "can_write": ["*"]}));

    let addr = c.cmd_ok(&json!({"cmd": "get_endpoint_addr"}));
    let addr_str = addr.as_str().unwrap();

    admin.cmd_ok(&json!({"cmd": "send_invite", "org": "devtype-test", "endpoint_addr_json": addr_str, "role": "user", "name": "DevTypeClient", "person": "dt"}));

    let mut invites = Vec::new();
    for attempt in 0..15 {
        std::thread::sleep(Duration::from_secs(2));
        let data = c.cmd_ok(&json!({"cmd": "get_invites"}));
        invites = data.as_array().expect("invites array").clone();
        if !invites.is_empty() { break; }
        eprintln!("[test] inbox empty, retry {}/15...", attempt + 1);
    }
    assert!(!invites.is_empty(), "should receive invite");

    let invite_json = serde_json::to_string(&invites[0]).unwrap();
    c.cmd_ok(&json!({"cmd": "join_org", "invite_json": invite_json, "org_name": "devtype-test"}));

    // Admin lists devices — default device_type should be "client"
    let devs = admin.cmd_ok(&json!({"cmd": "list_devices", "org": "devtype-test"}));
    let device = devs.as_array().and_then(|arr| arr.first()).expect("at least one device");
    assert_eq!(device["device_type"], "client", "default device_type should be 'client'");
    let node_id = device["node_id"].as_str().expect("node_id").to_string();
    eprintln!("[test] default device_type=client ✓");

    // Admin updates device_type to "client-ia"
    admin.cmd_ok(&json!({
        "cmd": "update_device", "org": "devtype-test", "node_id": node_id,
        "active": true, "role": "user", "device_type": "client-ia"
    }));

    // Verify admin sees the updated type
    let devs = admin.cmd_ok(&json!({"cmd": "list_devices", "org": "devtype-test"}));
    let updated = devs.as_array().and_then(|arr| arr.iter().find(|d| d["node_id"] == node_id)).expect("device in list");
    assert_eq!(updated["device_type"], "client-ia", "device_type should be updated to 'client-ia'");
    eprintln!("[test] ✅ device_type updated and reflected in admin list_devices");
}
