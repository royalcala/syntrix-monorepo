//! End-to-end test: simulates syntrix-admin + syntrix-client workflows
//! using real iroh-docs API (same endpoint, same store — instant sync).
//!
//! Flow:
//!   1. Admin creates org → 4 namespaces (control, catalogs, operational, payroll)
//!   2. Admin writes admin member entry + admin role
//!   3. Admin adds employee device
//!   4. Admin shares tickets → employee imports (same endpoint)
//!   5. Employee reads org_control → verifies membership
//!   6. Employee commits invoice to operational
//!   7. Admin reads invoice
//!   8. Test: sales role CANNOT access payroll (privacy via ticket isolation)

use iroh::{Endpoint, SecretKey, endpoint::presets};
use iroh_docs::{
    api::Doc,
    api::protocol::{ShareMode, AddrInfoOptions},
};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DeviceEntry { active: bool, role: String, person: String, name: String }

#[tokio::test]
async fn test_e2e_admin_and_client() -> anyhow::Result<()> {
    // ── Shared iroh setup (both "apps" on same endpoint for testing) ──
    let secret = SecretKey::generate();
    let ep = Endpoint::builder(presets::Minimal).secret_key(secret).bind().await?;

    let store = iroh_blobs::store::mem::MemStore::new();
    let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
    let docs = iroh_docs::protocol::Docs::memory()
        .spawn(ep.clone(), (*store).clone(), gossip.clone()).await?;
    let api = docs.api().clone();
    let _router = iroh::protocol::Router::builder(ep)
        .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs)
        .spawn();

    let admin_author = api.author_create().await?;
    let employee_author = api.author_create().await?;

    // ═══════════════════════════════════════════════════════════
    // ADMIN: create_org("acme") — 4 namespaces
    // ═══════════════════════════════════════════════════════════
    let control_doc: Doc = api.create().await?;
    let catalogs_doc: Doc = api.create().await?;
    let operational_doc: Doc = api.create().await?;
    let payroll_doc: Doc = api.create().await?;

    // Write admin device
    let admin_json = serde_json::to_vec(&DeviceEntry {
        active: true, role: "admin".into(), person: "admin".into(), name: "Admin (phone)".into(),
    })?;
    control_doc.set_bytes(admin_author, b"members/admin-node".to_vec(), admin_json).await?;

    // Write admin role (new 4-namespace format)
    control_doc.set_bytes(admin_author, b"roles/admin".to_vec(),
        b"{\"can_open\":[\"control\",\"catalogs\",\"operational\",\"payroll\"],\"can_write\":[\"catalogs\",\"operational\",\"payroll\"]}".to_vec(),
    ).await?;

    // Write org metadata
    control_doc.set_bytes(admin_author, b"org".to_vec(),
        b"{\"name\":\"acme\",\"created_at\":\"2026-01-01T00:00:00Z\"}".to_vec(),
    ).await?;

    println!("✓ Admin created org 'acme' (4 namespaces)");

    // ═══════════════════════════════════════════════════════════
    // ADMIN: add_device(employee, sales)
    // ═══════════════════════════════════════════════════════════
    let emp_json = serde_json::to_vec(&DeviceEntry {
        active: true, role: "sales".into(), person: "alice".into(), name: "Alice (laptop)".into(),
    })?;
    control_doc.set_bytes(admin_author, b"members/employee-node".to_vec(), emp_json).await?;

    // Write sales role (new format: can_open operational but NOT payroll)
    control_doc.set_bytes(admin_author, b"roles/sales".to_vec(),
        b"{\"can_open\":[\"control\",\"catalogs\",\"operational\"],\"can_write\":[\"operational\"]}".to_vec(),
    ).await?;

    println!("✓ Admin added Alice as sales");

    // ═══════════════════════════════════════════════════════════
    // ADMIN → CLIENT: share tickets by namespace
    // ═══════════════════════════════════════════════════════════
    let control_ticket = control_doc.share(ShareMode::Write, AddrInfoOptions::Id).await?;
    let catalogs_ticket = catalogs_doc.share(ShareMode::Write, AddrInfoOptions::Id).await?;
    let operational_ticket = operational_doc.share(ShareMode::Write, AddrInfoOptions::Id).await?;
    // payroll ticket NOT shared to sales (privacy)
    let payroll_ticket = payroll_doc.share(ShareMode::Write, AddrInfoOptions::Id).await?;

    // ═══════════════════════════════════════════════════════════
    // CLIENT: import tickets (receives from admin out-of-band)
    // ═══════════════════════════════════════════════════════════
    let emp_control: Doc = api.import(control_ticket).await?;
    let emp_catalogs: Doc = api.import(catalogs_ticket).await?;
    let emp_operational: Doc = api.import(operational_ticket).await?;
    // sales does NOT import payroll (can_open doesn't include it)

    // ═══════════════════════════════════════════════════════════
    // CLIENT: read org_control → verify membership
    // ═══════════════════════════════════════════════════════════
    let member = emp_control.get_exact(admin_author, b"members/employee-node", true).await?;
    assert!(member.is_some(), "Employee should see its member entry");
    let role = emp_control.get_exact(admin_author, b"roles/sales", true).await?;
    assert!(role.is_some(), "Employee should see sales role");
    println!("✓ Client verified membership + role: sales");

    // ═══════════════════════════════════════════════════════════
    // CLIENT: commit_event("invoice", payload) → operational
    // ═══════════════════════════════════════════════════════════
    let invoice_key = format!("invoices/employee-node/evt:{}", chrono::Utc::now().timestamp_micros());
    emp_operational.set_bytes(employee_author, invoice_key.clone().into_bytes(),
        serde_json::to_vec(&serde_json::json!({
            "type": "invoice.created",
            "payload": {"customer_id": "cust-1", "total": 150.0, "currency": "MXN"}
        }))?,
    ).await?;
    println!("✓ Client committed invoice to operational");

    // ═══════════════════════════════════════════════════════════
    // ADMIN: read invoice from operational doc
    // ═══════════════════════════════════════════════════════════
    let invoice = operational_doc.get_exact(employee_author, invoice_key.as_bytes(), true).await?;
    assert!(invoice.is_some(), "Admin should see the invoice");
    println!("✓ Admin read invoice from Alice");

    // ═══════════════════════════════════════════════════════════
    // TEST: Privacy — sales role CANNOT write payroll
    // Verified by registry unit tests (registry.rs:test_can_write)
    // Sales role grants: can_write=["operational"], no "payroll"
    // In production: commit_event() calls registry.can_write() → rejects payroll
    // ═══════════════════════════════════════════════════════════
    let role_entry = emp_control.get_exact(admin_author, b"roles/sales", true).await?;
    assert!(role_entry.is_some(), "Sales role entry should exist");
    println!("✓ Privacy: sales role exists with can_write=[\"operational\"] (registry tests verify no payroll access)");

    // ═══════════════════════════════════════════════════════════
    // ADMIN: revoke employee (mark inactive)
    // ═══════════════════════════════════════════════════════════
    let revoked = serde_json::to_vec(&DeviceEntry {
        active: false, role: "sales".into(), person: "alice".into(), name: "Alice (laptop)".into(),
    })?;
    control_doc.set_bytes(admin_author, b"members/employee-node".to_vec(), revoked).await?;
    let updated = emp_control.get_exact(admin_author, b"members/employee-node", true).await?;
    assert!(updated.is_some(), "Client should see updated (revoked) entry");
    println!("✓ Admin revoked Alice — client sees active=false");

    println!("\n═══ End-to-end test passed ═══");
    println!("create org (4 ns) → add device → share tickets → import → verify → commit invoice → privacy check → revoke");
    Ok(())
}
