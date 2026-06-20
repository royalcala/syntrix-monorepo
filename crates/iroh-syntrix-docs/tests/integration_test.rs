//! Integration test: sync between two docs on the same iroh endpoint.
//!
//! This test proves the concept without network dependencies.
//! Two docs on the same endpoint share the same blob store and gossip,
//! so sync is instant and definitive.

use iroh::{Endpoint, endpoint::presets, SecretKey};
use iroh_docs::{
    api::Doc,
    engine::LiveEvent,
    api::protocol::{ShareMode, AddrInfoOptions},
};
use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use futures_util::StreamExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DeviceEntry { active: bool, role: String, person: String, name: String }

#[tokio::test]
async fn test_two_docs_same_endpoint() -> anyhow::Result<()> {
    let secret = SecretKey::generate();
    let ep = Endpoint::builder(presets::Minimal)
        .secret_key(secret)
        .bind()
        .await?;

    let store = iroh_blobs::store::mem::MemStore::new();
    let gossip = iroh_gossip::net::Gossip::builder().spawn(ep.clone());
    let docs = iroh_docs::protocol::Docs::memory()
        .spawn(ep.clone(), (*store).clone(), gossip.clone())
        .await?;
    let api = docs.api().clone();

    let _router = iroh::protocol::Router::builder(ep.clone())
        .accept(iroh_blobs::ALPN, iroh_blobs::BlobsProtocol::new(&store, None))
        .accept(iroh_gossip::ALPN, gossip)
        .accept(iroh_docs::ALPN, docs)
        .spawn();

    let author_a = api.author_create().await?;
    let author_b = api.author_create().await?;

    // ── Admin creates doc A, writes entry ──
    let doc_a: Doc = api.create().await?;
    doc_a.set_bytes(author_a, b"members/admin".to_vec(),
        serde_json::to_vec(&DeviceEntry {
            active: true, role: "admin".into(), person: "admin".into(), name: "Admin".into(),
        })?
    ).await?;

    // ── Share doc A to get its capability ──
    let ticket = doc_a.share(ShareMode::Write, AddrInfoOptions::Id).await?;

    // ── Import the same namespace as doc B (on same endpoint) ──
    let doc_b: Doc = api.import(ticket).await?;

    // Same endpoint → same store → entries are immediately visible
    let entry = doc_b.get_exact(author_a, b"members/admin", true).await?;
    assert!(entry.is_some(), "Doc B should see Doc A's entry on same endpoint");
    println!("✓ Entry visible on same endpoint");

    // ── Write from B and read from A ──
    doc_b.set_bytes(author_b, b"invoices/001".to_vec(),
        serde_json::to_vec(&serde_json::json!({"type":"invoice","total":150.0}))?
    ).await?;

    let invoice = doc_a.get_exact(author_b, b"invoices/001", true).await?;
    assert!(invoice.is_some(), "Doc A should see Doc B's entry on same endpoint");
    println!("✓ Cross-doc write/read works");

    // ── Query all entries from both docs ──
    let all_a: Vec<_> = doc_a.get_many(iroh_docs::store::Query::all()).await?.collect().await;
    let all_b: Vec<_> = doc_b.get_many(iroh_docs::store::Query::all()).await?.collect().await;
    assert!(all_a.iter().filter(|e| e.is_ok()).count() >= 1);
    println!("✓ Doc A has {} entries, Doc B has {} entries", 
        all_a.iter().filter(|e| e.is_ok()).count(),
        all_b.iter().filter(|e| e.is_ok()).count());

    // ── Revoke ──
    doc_a.set_bytes(author_a, b"members/admin".to_vec(),
        serde_json::to_vec(&DeviceEntry {
            active: false, role: "admin".into(), person: "admin".into(), name: "Admin".into(),
        })?
    ).await?;

    println!("✓ Full cycle on same endpoint: create → share → read → write → read → revoke");
    Ok(())
}
