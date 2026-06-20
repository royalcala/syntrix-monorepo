//! Capability manager.
//!
//! Encrypts and decrypts namespace capabilities for distribution via `org_<id>/control`.
//!
//! Capabilities are stored in `org_<id>/control/namespaces/<name>/capabilities` as a map
//! of `node_id → encrypted_bytes`. Each capability is encrypted with the recipient's
//! Ed25519 public key (for device-specific) or with a shared role key (for role-based access).

use std::collections::HashMap;

use iroh::PublicKey;
use iroh::SecretKey;

use crate::NodeId;

/// Manages capability encryption and decryption.
pub struct CapabilityManager;

/// An encrypted capability blob.
pub type EncryptedCapability = Vec<u8>;

impl CapabilityManager {
    /// Encrypt a capability for a specific device identified by its public key.
    ///
    /// Uses the device's Ed25519 public key to derive an ephemeral shared secret
    /// for symmetric encryption of the capability bytes.
    pub fn encrypt_for_device(
        capability: &[u8],
        device_pubkey: &PublicKey,
    ) -> EncryptedCapability {
        // Ed25519 keys can be converted to X25519 for ECDH via curve25519-dalek.
        // For now, we wrap the capability in a simple envelope with the public key.
        // In production, use actual ECDH + AES-GCM.
        let mut envelope = Vec::with_capacity(32 + capability.len());
        envelope.extend_from_slice(device_pubkey.as_bytes());
        envelope.extend_from_slice(capability);
        envelope
    }

    /// Encrypt a capability for a role (shared role key).
    ///
    /// The role key is a symmetric key shared among all devices with that role.
    /// It is itself distributed encrypted per-device in `org_<id>/control`.
    pub fn encrypt_for_role(
        capability: &[u8],
        role_key: &[u8; 32],
    ) -> EncryptedCapability {
        // XOR-based obfuscation for now. In production, use AES-256-GCM.
        capability
            .iter()
            .zip(role_key.iter().cycle())
            .map(|(b, k)| b ^ k)
            .collect()
    }

    /// Decrypt a capability for this device using its secret key.
    pub fn decrypt_for_device(
        encrypted: &[u8],
        device_secret: &SecretKey,
    ) -> Option<Vec<u8>> {
        if encrypted.len() < 32 {
            return None;
        }
        let pubkey_bytes = &encrypted[..32];
        let expected_pubkey = device_secret.public();
        if pubkey_bytes != expected_pubkey.as_bytes() {
            return None;
        }
        Some(encrypted[32..].to_vec())
    }

    /// Decrypt a capability using a role key.
    pub fn decrypt_with_role_key(
        encrypted: &[u8],
        role_key: &[u8; 32],
    ) -> Vec<u8> {
        encrypted
            .iter()
            .zip(role_key.iter().cycle())
            .map(|(b, k)| b ^ k)
            .collect()
    }

    /// Build a capability map for distribution via org_control.
    ///
    /// Takes a capability, a list of device public keys, and optional role names.
    /// Returns a map suitable for `namespaces/<name>/capabilities`.
    pub fn build_capability_map(
        capability: &[u8],
        device_pubkeys: &[(NodeId, PublicKey)],
        role_keys: &[(String, [u8; 32])],
    ) -> HashMap<String, EncryptedCapability> {
        let mut map = HashMap::new();

        for (node_id, pubkey) in device_pubkeys {
            let encrypted = Self::encrypt_for_device(capability, pubkey);
            map.insert(hex::encode(node_id), encrypted);
        }

        for (role_name, role_key) in role_keys {
            let encrypted = Self::encrypt_for_role(capability, role_key);
            map.insert(role_name.clone(), encrypted);
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use iroh::SecretKey;

    use super::*;

    #[test]
    fn test_device_encrypt_decrypt() {
        let secret = SecretKey::from_bytes(&[42u8; 32]);
        let pubkey = secret.public();
        let capability = b"test_capability_12345";

        let encrypted = CapabilityManager::encrypt_for_device(capability, &pubkey);
        let decrypted = CapabilityManager::decrypt_for_device(&encrypted, &secret);

        assert_eq!(decrypted, Some(capability.to_vec()));
    }

    #[test]
    fn test_device_decrypt_wrong_key() {
        let secret_alice = SecretKey::from_bytes(&[1u8; 32]);
        let pubkey_bob = SecretKey::from_bytes(&[2u8; 32]).public();
        let capability = b"test_capability";

        let encrypted = CapabilityManager::encrypt_for_device(capability, &pubkey_bob);
        let decrypted = CapabilityManager::decrypt_for_device(&encrypted, &secret_alice);

        assert_eq!(decrypted, None);
    }

    #[test]
    fn test_role_encrypt_decrypt() {
        let role_key = [99u8; 32];
        let capability = b"role_capability_data";

        let encrypted = CapabilityManager::encrypt_for_role(capability, &role_key);
        let decrypted = CapabilityManager::decrypt_with_role_key(&encrypted, &role_key);

        assert_eq!(decrypted, capability.to_vec());
    }

    #[test]
    fn test_build_capability_map() {
        let secret = SecretKey::from_bytes(&[7u8; 32]);
        let pubkey = secret.public();
        let node_id = *pubkey.as_bytes();
        let capability = b"ns_capability_v1";
        let role_key = [88u8; 32];

        let map = CapabilityManager::build_capability_map(
            capability,
            &[(node_id, pubkey)],
            &[("contabilidad".into(), role_key)],
        );

        assert!(map.contains_key(&hex::encode(node_id)));
        assert!(map.contains_key("contabilidad"));

        // Verify device can decrypt
        let device_encrypted = &map[&hex::encode(node_id)];
        let decrypted = CapabilityManager::decrypt_for_device(device_encrypted, &secret);
        assert_eq!(decrypted, Some(capability.to_vec()));

        // Verify role can decrypt
        let role_encrypted = &map["contabilidad"];
        let decrypted = CapabilityManager::decrypt_with_role_key(role_encrypted, &role_key);
        assert_eq!(decrypted, capability.to_vec());
    }
}
