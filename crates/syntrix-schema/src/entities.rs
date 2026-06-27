use crate::schema::*;

/// Convenience constructors for common field patterns.
macro_rules! field {
    (string: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: false,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (text: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: false,
            searchable: true,
            sort_key: false,
            relation: None,
        }
    };
    (indexed: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: true,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (searchable: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: true,
            searchable: true,
            sort_key: false,
            relation: None,
        }
    };
    (sort_key: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: true,
            searchable: false,
            sort_key: true,
            relation: None,
        }
    };
    (number: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Number,
            indexed: false,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (number_indexed: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Number,
            indexed: true,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (date: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Date,
            indexed: false,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (date_indexed: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Date,
            indexed: true,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (bool: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Boolean,
            indexed: false,
            searchable: false,
            sort_key: false,
            relation: None,
        }
    };
    (relation: $name:expr, target: $target:expr, field: $field:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::Relation,
            indexed: true,
            searchable: false,
            sort_key: false,
            relation: Some(RelationDef {
                target: $target.to_string(),
                field: $field.to_string(),
            }),
        }
    };
    (sort_key_indexed: $name:expr) => {
        FieldSchema {
            name: $name.to_string(),
            field_type: FieldType::String,
            indexed: true,
            searchable: false,
            sort_key: true,
            relation: None,
        }
    };
}

macro_rules! entity {
    ($name:expr, $version:expr, $namespace:expr, [$($field:expr),* $(,)?], [$($index:expr),* $(,)?]) => {
        EntitySchema {
            name: $name.to_string(),
            version: $version,
            namespace: $namespace,
            fields: vec![$($field),*],
            indexes: vec![$($index),*],
        }
    };
}

macro_rules! index {
    ($name:expr, [$($field:expr),*]) => {
        IndexDef {
            name: $name.to_string(),
            fields: vec![$($field.to_string()),*],
        }
    };
}

// ---------------------------------------------------------------------------
// Entity definitions
// ---------------------------------------------------------------------------

pub fn customers_schema() -> EntitySchema {
    entity!("customers", 1, Namespace::Catalogs, [
        field!(string: "id"),
        field!(searchable: "name"),
        field!(text: "email"),
        field!(text: "phone"),
        field!(text: "rfc"),
        field!(text: "address"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_status", ["status"]),
    ])
}

pub fn suppliers_schema() -> EntitySchema {
    entity!("suppliers", 1, Namespace::Catalogs, [
        field!(string: "id"),
        field!(searchable: "name"),
        field!(text: "email"),
        field!(text: "phone"),
        field!(text: "rfc"),
        field!(text: "address"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_status", ["status"]),
    ])
}

pub fn products_schema() -> EntitySchema {
    entity!("products", 1, Namespace::Catalogs, [
        field!(string: "id"),
        field!(searchable: "name"),
        field!(text: "description"),
        field!(number_indexed: "price"),
        field!(number_indexed: "cost"),
        field!(indexed: "sku"),
        field!(indexed: "category"),
        field!(number_indexed: "stock"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_category", ["category"]),
        index!("by_status", ["status"]),
    ])
}

pub fn invoices_schema() -> EntitySchema {
    entity!("invoices", 1, Namespace::Operational, [
        field!(string: "id"),
        field!(sort_key_indexed: "folio"),
        field!(relation: "customer_id", target: "customers", field: "id"),
        field!(date_indexed: "date"),
        field!(date: "due_date"),
        field!(number_indexed: "total"),
        field!(number_indexed: "subtotal"),
        field!(number: "tax"),
        field!(indexed: "status"),
        field!(bool: "paid"),
        field!(text: "notes"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_status", ["status"]),
        index!("by_status_date", ["status", "date"]),
    ])
}

pub fn orders_schema() -> EntitySchema {
    entity!("orders", 1, Namespace::Operational, [
        field!(string: "id"),
        field!(sort_key_indexed: "folio"),
        field!(relation: "customer_id", target: "customers", field: "id"),
        field!(date_indexed: "date"),
        field!(number_indexed: "total"),
        field!(indexed: "status"),
        field!(text: "notes"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_status", ["status"]),
        index!("by_status_date", ["status", "date"]),
    ])
}

pub fn payroll_schema() -> EntitySchema {
    entity!("payroll", 1, Namespace::Payroll, [
        field!(string: "id"),
        field!(searchable: "name"),
        field!(indexed: "department"),
        field!(number_indexed: "salary"),
        field!(indexed: "role"),
        field!(text: "payment_method"),
        field!(bool: "active"),
        field!(date_indexed: "created_at"),
        field!(date_indexed: "updated_at"),
    ], [
        index!("by_department", ["department"]),
        index!("by_role", ["role"]),
        index!("by_active", ["active"]),
    ])
}

/// Returns all entity schemas as a Vec.
pub fn all_schemas() -> Vec<EntitySchema> {
    vec![
        customers_schema(),
        suppliers_schema(),
        products_schema(),
        invoices_schema(),
        orders_schema(),
        payroll_schema(),
    ]
}

/// Build a SchemaRegistry populated with all entities.
pub fn build_registry() -> SchemaRegistry {
    let mut reg = SchemaRegistry::new();
    for s in all_schemas() {
        reg.register(s);
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customers_schema() {
        let s = customers_schema();
        assert_eq!(s.name, "customers");
        assert_eq!(s.version, 1);
        assert!(s.field("name").unwrap().searchable);
        assert!(s.field("name").unwrap().indexed);
        assert!(s.field("id").is_some());
        assert!(!s.field("id").unwrap().indexed);
    }

    #[test]
    fn test_registry() {
        let reg = build_registry();
        assert!(reg.get("customers").is_some());
        assert!(reg.get("invoices").is_some());
        assert_eq!(reg.all().len(), 6);
    }

    #[test]
    fn test_indexed_fields() {
        let inv = invoices_schema();
        let indexed = inv.indexed_fields();
        assert!(indexed.iter().any(|f| f.name == "status"));
        assert!(indexed.iter().any(|f| f.name == "customer_id"));
        assert!(!indexed.iter().any(|f| f.name == "notes"));
    }

    #[test]
    fn test_relation_metadata() {
        let inv = invoices_schema();
        let rel = inv.field("customer_id").unwrap();
        assert!(rel.relation.is_some());
        let rel_def = rel.relation.as_ref().unwrap();
        assert_eq!(rel_def.target, "customers");
        assert_eq!(rel_def.field, "id");
    }
}
