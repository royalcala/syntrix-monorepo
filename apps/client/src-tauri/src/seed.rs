use crate::identity::AppState;

fn random_pick<T: Clone>(list: &[T]) -> T {
    list[fastrand::usize(..list.len())].clone()
}

fn random_id() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("seed-{}-{}", ts, fastrand::u32(..9999))
}

pub fn seed_dev_data(state: &AppState) -> anyhow::Result<usize> {
    let mut count = 0;

    let first_names = ["Juan","María","Carlos","Ana","Luis","Rosa","Pedro","Elena","José","Laura","Miguel","Sofía","Diego","Carmen","Jorge"];
    let last_names = ["García","Martínez","López","Hernández","González","Rodríguez","Pérez","Sánchez","Ramírez","Torres"];
    let statuses_inv = ["draft","open","paid","cancelled"];
    let statuses_ord = ["pending","confirmed","shipped","delivered"];
    let skus = ["PROD-1001","PROD-2002","PROD-3003","PROD-4004","PROD-5005","PROD-6006","PROD-7007","PROD-8008"];
    let categories = ["Electrónicos","Oficina","Hogar","Herramientas","Limpieza","Alimentos"];

    let mut customer_ids: Vec<String> = vec![];

    // Seed customers (20)
    for _ in 0..20 {
        let id = random_id();
        let name = format!("{} {}", random_pick(&first_names), random_pick(&last_names));
        crate::events::commit_event(state, "customer.created", &serde_json::json!({
            "id": id, "name": name,
            "tax_id": format!("XAXX010101{}", fastrand::u32(100..999)),
            "email": format!("{}@{}.com", name.to_lowercase().replace(" ", "."), random_pick(&["email","correo","mail","empresa"])),
            "phone": format!("55{}{}", fastrand::u32(1000..9999), fastrand::u32(1000..9999)),
        }).to_string())?;
        customer_ids.push(id);
        count += 1;
    }

    // Seed products (30)
    let mut product_ids: Vec<String> = vec![];
    for i in 0..30 {
        let id = random_id();
        crate::events::commit_event(state, "product.created", &serde_json::json!({
            "id": id,
            "name": format!("{} {}", random_pick(&["Laptop","Monitor","Teclado","Mouse","Impresora","Escritorio","Silla","Archivador","Cable","Adaptador"]), i),
            "sku": random_pick(&skus),
            "price": fastrand::f64() * 5000.0 + 100.0,
            "unit": random_pick(&["pza","caja","kg","lt"]),
            "category": random_pick(&categories),
        }).to_string())?;
        product_ids.push(id);
        count += 1;
    }

    // Seed invoices (50)
    for _ in 0..50 {
        let id = format!("F-{}", fastrand::u32(1000..9999));
        let customer_id = random_pick(&customer_ids);
        let days_ago = fastrand::u32(0..90);
        let date = chrono::Utc::now() - chrono::Duration::days(days_ago as i64);
        crate::events::commit_event(state, "invoice.created", &serde_json::json!({
            "id": id,
            "customer_id": customer_id,
            "date": date.to_rfc3339(),
            "amount": (fastrand::f64() * 50000.0 + 500.0).round(),
            "status": random_pick(&statuses_inv),
            "items": [
                {"product_id": random_pick(&product_ids), "qty": fastrand::u32(1..10), "price": fastrand::f64() * 2000.0 + 50.0},
            ],
        }).to_string())?;
        count += 1;
    }

    // Seed orders (30)
    for _ in 0..30 {
        let id = format!("O-{}", fastrand::u32(1000..9999));
        let customer_id = random_pick(&customer_ids);
        let days_ago = fastrand::u32(0..60);
        let date = chrono::Utc::now() - chrono::Duration::days(days_ago as i64);
        crate::events::commit_event(state, "order.created", &serde_json::json!({
            "id": id,
            "customer_id": customer_id,
            "date": date.to_rfc3339(),
            "status": random_pick(&statuses_ord),
            "items": [
                {"product_id": random_pick(&product_ids), "qty": fastrand::u32(1..5)},
            ],
        }).to_string())?;
        count += 1;
    }

    Ok(count)
}
