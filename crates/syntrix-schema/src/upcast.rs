/// Upcaster trait: transforms a payload from an older schema version
/// to the current version. Upcasters are chained sequentially from
/// the event's `schema_version` to the current binary version.
///
/// Upcasters must be **deterministic pure functions** — no side effects,
/// no network access, no time-dependent output.
pub trait Upcaster: Send + Sync {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn upcast(&self, payload: serde_json::Value) -> serde_json::Value;
}

/// Collect all known upcasters. Add new upcasters here as schemas evolve.
pub fn collect_upcasters() -> Vec<Box<dyn Upcaster>> {
    vec![
        Box::new(CustomerV1ToV2),
    ]
}

/// Apply a chain of upcasters to bring a payload from an old schema version
/// to the current binary version.
pub fn apply_upcasters(
    mut payload: serde_json::Value,
    from_version: u32,
    to_version: u32,
    upcasters: &[Box<dyn Upcaster>],
) -> serde_json::Value {
    let mut current = from_version;
    while current < to_version {
        let mut applied = false;
        for up in upcasters {
            if up.from_version() == current && up.to_version() <= to_version {
                payload = up.upcast(payload);
                current = up.to_version();
                applied = true;
                break;
            }
        }
        if !applied {
            // No upcaster found for this version gap — stop here
            break;
        }
    }
    payload
}

// ---------------------------------------------------------------------------
// Example / default upcasters
// ---------------------------------------------------------------------------

/// Upcaster for customers: v1 -> v2 splits the `address` string field
/// into a structured `{ street, city, zip }` object.
pub struct CustomerV1ToV2;

impl Upcaster for CustomerV1ToV2 {
    fn from_version(&self) -> u32 { 1 }
    fn to_version(&self) -> u32 { 2 }

    fn upcast(&self, payload: serde_json::Value) -> serde_json::Value {
        let mut obj = match payload {
            serde_json::Value::Object(m) => m,
            _ => return payload,
        };

        // Convert flat address string to structured address
        if let Some(serde_json::Value::String(addr)) = obj.remove("address") {
            let parts: Vec<&str> = addr.split(',').collect();
            let street = parts.first().unwrap_or(&"").trim().to_string();
            let city = parts.get(1).unwrap_or(&"").trim().to_string();
            let zip = parts.get(2).unwrap_or(&"").trim().to_string();
            let structured = serde_json::json!({
                "street": street,
                "city": city,
                "zip": zip,
            });
            obj.insert("address".to_string(), structured);
        }

        serde_json::Value::Object(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_upcast_v1_to_v2() {
        let payload = serde_json::json!({
            "id": "cust_001",
            "name": "Juan Perez",
            "address": "Calle 123, Ciudad de Mexico, 06600"
        });

        let upcasters: Vec<Box<dyn Upcaster>> = vec![Box::new(CustomerV1ToV2)];
        let result = apply_upcasters(payload, 1, 2, &upcasters);

        assert_eq!(result["name"], "Juan Perez");
        assert_eq!(result["address"]["street"], "Calle 123");
        assert_eq!(result["address"]["city"], "Ciudad de Mexico");
        assert_eq!(result["address"]["zip"], "06600");
    }

    #[test]
    fn test_no_upcast_needed() {
        let payload = serde_json::json!({"id": "cust_001"});
        let upcasters: Vec<Box<dyn Upcaster>> = vec![];
        let result = apply_upcasters(payload, 2, 2, &upcasters);
        assert_eq!(result["id"], "cust_001");
    }
}
