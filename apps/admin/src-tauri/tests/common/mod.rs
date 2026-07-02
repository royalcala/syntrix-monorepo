use std::path::PathBuf;
use std::time::Duration;
use std::str::FromStr;

use syntrix_testkit::{poll_until, PollConfig};

use syntrix_admin_lib::identity::AppState as AdminState;
use syntrix_client_lib::identity::AppState as ClientState;

pub const POLL: PollConfig = PollConfig {
    max_retries: 60,
    base_delay: Duration::from_millis(200),
    max_delay: Duration::from_secs(5),
};

pub async fn spawn_admin(data_dir: PathBuf) -> AdminState {
    AdminState::new_with_data_dir(data_dir).await.unwrap()
}

pub async fn spawn_client(data_dir: PathBuf) -> ClientState {
    ClientState::new_with_data_dir(data_dir).await.unwrap().0
}

pub fn node_id_from_addr(addr_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(addr_json).unwrap();
    v["node_id"].as_str().unwrap().to_string()
}

pub async fn get_client_addr(client: &ClientState) -> String {
    let peer_id = client.p2p().local_peer_id();
    let addrs = client.p2p().listen_addrs().await;
    serde_json::json!({
        "node_id": hex::encode(client.node_id()),
        "peer_id": peer_id.to_base58(),
        "addrs": addrs,
    }).to_string()
}

pub async fn dial_client(admin: &AdminState, client_addr: &str) {
    let v: serde_json::Value = serde_json::from_str(client_addr).unwrap();
    if let Some(addrs) = v["addrs"].as_array() {
        for addr_val in addrs {
            if let Some(addr_str) = addr_val.as_str() {
                if let Ok(addr) = libp2p::Multiaddr::from_str(addr_str) {
                    let _ = admin.p2p().dial(addr);
                }
            }
        }
    }
}

pub async fn invite_one_client(
    admin: &mut AdminState,
    client: &mut ClientState,
    org_name: &str,
    role: &str,
) -> anyhow::Result<()> {
    let client_addr = get_client_addr(client).await;
    let node_id_hex = node_id_from_addr(&client_addr);

    syntrix_admin_lib::admin::add_device(
        admin,
        org_name,
        &node_id_hex,
        &format!("Device {}", &node_id_hex[..8]),
        &node_id_hex,
        role,
        &client_addr,
    )
    .await?;

    dial_client(admin, &client_addr).await;

    let peer_id = admin.p2p().local_peer_id();
    let addrs = admin.p2p().listen_addrs().await;
    let admin_addr = syntrix_core::build_device_addr_string(peer_id, &addrs);
    let org_state = admin.get_org(org_name).unwrap();
    let topic_id = org_state.topic_id.clone();

    let roles = admin.list_org_roles(org_name);
    let (can_open, can_write) = roles
        .iter()
        .find(|r| r.name == role)
        .map(|r| (r.can_open.clone(), r.can_write.clone()))
        .unwrap_or_else(|| {
            let g = syntrix_admin_lib::identity::default_role_grants(role);
            (g.can_open, g.can_write)
        });

    syntrix_admin_lib::admin::send_invite(
        admin,
        org_name,
        &client_addr,
        role,
        topic_id,
        admin_addr,
        can_open,
        can_write,
    )
    .await?;

    let invites = poll_until(
        || {
            let invites = syntrix_client_lib::get_invites_impl(client);
            async move {
                if invites.is_empty() {
                    Err("no invites yet")
                } else {
                    Ok(invites)
                }
            }
        },
        &POLL,
    )
    .await
    .map_err(|e| anyhow::anyhow!("no invites received by client for role {}: {}", role, e))?;

    let invite_json = serde_json::to_string(&invites[0])?;

    syntrix_client_lib::join_org_impl(client, &invite_json, Some(org_name))
        .await
        .map_err(|e| anyhow::anyhow!("join_org failed: {}", e))?;

    Ok(())
}

pub async fn invite_and_join(
    admin: &mut AdminState,
    client: &mut ClientState,
    org_name: &str,
    role: &str,
) -> anyhow::Result<String> {
    syntrix_admin_lib::admin::create_org(admin, org_name).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    invite_one_client(admin, client, org_name, role).await?;
    Ok(find_client_org_id(client, org_name))
}

pub async fn invite_and_join_two_clients(
    admin: &mut AdminState,
    client1: &mut ClientState,
    client2: &mut ClientState,
    org_name: &str,
    role: &str,
) -> anyhow::Result<String> {
    syntrix_admin_lib::admin::create_org(admin, org_name).await?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    invite_one_client(admin, client1, org_name, role).await?;
    invite_one_client(admin, client2, org_name, role).await?;
    let id = find_client_org_id(client1, org_name);
    Ok(id)
}

pub fn find_client_org_id(state: &ClientState, name: &str) -> String {
    syntrix_client_lib::list_orgs_impl(state)
        .into_iter()
        .find(|o| o.name == name)
        .map(|o| o.id)
        .expect("org not found in client state")
}

pub async fn wait_for_document(
    state: &ClientState,
    org_id: &str,
    entity: &str,
    doc_id: &str,
) -> anyhow::Result<serde_json::Value> {
    let idx = state.indexer();
    poll_until(
        || {
            let idx = idx.clone();
            let did = doc_id.to_string();
            async move {
                match idx.get_document(org_id, entity, &did).map_err(|e| e.to_string()) {
                    Ok(Some(doc)) => Ok(doc),
                    Ok(None) => Err("not found".to_string()),
                    Err(e) => Err(e),
                }
            }
        },
        &POLL,
    )
    .await
    .map_err(|e| anyhow::anyhow!("wait_for_document({} in {}:{}): {}", doc_id, org_id, entity, e))
}

pub fn count_events(state: &ClientState, org_id: &str) -> usize {
    state
        .indexer()
        .query_events_since(org_id, 0, 10000)
        .map(|e| e.len())
        .unwrap_or(0)
}

pub async fn wait_for_audit_entry(
    state: &AdminState,
    org_name: &str,
    doc_id: &str,
) -> anyhow::Result<syntrix_admin_lib::audit::AuditEntry> {
    poll_until(
        || {
            let did = doc_id.to_string();
            let o = org_name.to_string();
            async move {
                let entries = syntrix_admin_lib::audit::audit_query(
                    state,
                    &o,
                    &syntrix_admin_lib::audit::AuditFilter {
                        doc_id: Some(did.clone()),
                        ..Default::default()
                    },
                    10,
                    0,
                );
                match entries.into_iter().next() {
                    Some(entry) => Ok(entry),
                    None => Err("not found".to_string()),
                }
            }
        },
        &POLL,
    )
    .await
    .map_err(|e| anyhow::anyhow!("wait_for_audit_entry({}): {}", doc_id, e))
}

pub fn set_client_org(client: &mut ClientState, org_id: &str) {
    ClientState::set_active_org(client, org_id)
        .unwrap_or_else(|e| panic!("set_active_org({}): {}", org_id, e));
}
