use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub task: String,
    pub model: String,
    pub mode: String,
    pub repetition: usize,
    pub success: bool,
    pub latency_ms: f64,
    pub validation_msg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub task: String,
    pub model: String,
    pub total_runs: usize,
    pub successes: usize,
    pub failures: usize,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
}

pub fn compute_summary(results: &[BenchmarkResult]) -> Vec<BenchmarkSummary> {
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<(String, String), Vec<&BenchmarkResult>> = BTreeMap::new();
    for r in results {
        grouped.entry((r.task.clone(), r.model.clone())).or_default().push(r);
    }
    grouped
        .into_iter()
        .map(|((task, model), group)| {
            let total = group.len();
            let successes = group.iter().filter(|r| r.success).count();
            let latencies: Vec<f64> = group.iter().map(|r| r.latency_ms).collect();
            let avg = if latencies.is_empty() { 0.0 } else { latencies.iter().sum::<f64>() / latencies.len() as f64 };
            let min = latencies.iter().cloned().fold(f64::MAX, f64::min);
            let max = latencies.iter().cloned().fold(f64::MIN, f64::max);
            BenchmarkSummary { task, model, total_runs: total, successes, failures: total - successes, avg_latency_ms: avg, min_latency_ms: min, max_latency_ms: max }
        })
        .collect()
}

/// Represents a tool the model can call
#[derive(Clone, Serialize)]
struct ToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Sends a chat request to an OpenAI-compatible API and returns the full response JSON
fn call_ollama(url: &str, model: &str, system: &str, prompt: &str, tools: &[ToolDef]) -> Result<serde_json::Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("client: {e}"))?;

    let mut body = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt}
        ]
    });

    if !tools.is_empty() {
        body["tools"] = serde_json::Value::Array(tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            })
        }).collect());
    }

    let api_url = format!("{}/v1/chat/completions", url.trim_end_matches('/'));
    let resp = client.post(&api_url)
        .json(&body).send().map_err(|e| format!("request: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    resp.json().map_err(|e| format!("parse: {e}"))
}

fn make_query_tool() -> ToolDef {
    ToolDef {
        name: "query_entity".into(),
        description: "Ejecuta SQL SELECT contra la base de datos. Solo lecturas.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "sql": {"type": "string", "description": "SQL SELECT a ejecutar"},
            },
            "required": ["sql"]
        }),
    }
}

fn make_search_tool() -> ToolDef {
    ToolDef {
        name: "search_entity".into(),
        description: "Busca texto completo en entidades.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "entities": {"type": "array", "items": {"type": "string"}},
                "limit": {"type": "integer"}
            },
            "required": ["query"]
        }),
    }
}

fn make_schema_tool() -> ToolDef {
    ToolDef {
        name: "get_schema".into(),
        description: "Obtiene metadata de entidades, columnas y relaciones.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "entity": {"type": "string"}
            }
        }),
    }
}

fn make_save_view_tool() -> ToolDef {
    ToolDef {
        name: "save_view".into(),
        description: "Guarda una vista para reuso futuro.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "view": {"type": "object"}
            },
            "required": ["view"]
        }),
    }
}

fn all_tools() -> Vec<ToolDef> {
    vec![make_query_tool(), make_search_tool(), make_schema_tool(), make_save_view_tool()]
}

fn all_tools_except_save() -> Vec<ToolDef> {
    vec![make_query_tool(), make_search_tool(), make_schema_tool()]
}

// ── Tool-calling test cases ──

struct ToolTestCase {
    name: &'static str,
    system: &'static str,
    prompt: &'static str,
    tools: Vec<ToolDef>,
    expected_tool: Option<&'static str>, // None = expect text response, no tool call
    validate_args: fn(&serde_json::Value) -> Result<(), String>,
}

fn get_tool_calls(response: &serde_json::Value) -> Vec<(String, serde_json::Value)> {
    let msg = &response["choices"][0]["message"];
    msg["tool_calls"].as_array()
        .map(|calls| {
            calls.iter().filter_map(|tc| {
                let name = tc["function"]["name"].as_str()?.to_string();
                let args: serde_json::Value = serde_json::from_str(
                    tc["function"]["arguments"].as_str().unwrap_or("{}")
                ).unwrap_or(serde_json::json!({}));
                Some((name, args))
            }).collect()
        })
        .unwrap_or_default()
}

fn get_text(response: &serde_json::Value) -> String {
    response["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
}

fn run_tool_test(url: &str, model: &str, tc: &ToolTestCase) -> (bool, String, f64) {
    let start = Instant::now();
    let result = call_ollama(url, model, tc.system, tc.prompt, &tc.tools);
    let latency = start.elapsed().as_secs_f64() * 1000.0;

    match result {
        Ok(json) => {
            let tool_calls = get_tool_calls(&json);
            let text = get_text(&json);

            if let Some(expected) = tc.expected_tool {
                // Expecting a tool call
                if tool_calls.is_empty() {
                    (false, format!("Esperaba tool call '{expected}' pero no hubo. Texto: {text:.120}"), latency)
                } else {
                    let (actual_name, actual_args) = &tool_calls[0];
                    if actual_name != expected {
                        (false, format!("Tool esperado '{expected}', obtuvo '{actual_name}'. Args: {actual_args}"), latency)
                    } else {
                        match (tc.validate_args)(actual_args) {
                            Ok(()) => (true, format!("✓ tool={actual_name} args OK"), latency),
                            Err(e) => (false, format!("tool={actual_name} args inválidos: {e}"), latency),
                        }
                    }
                }
            } else {
                // Expecting text response, no tool call
                if !tool_calls.is_empty() {
                    let (name, args) = &tool_calls[0];
                    (false, format!("No esperaba tool call pero llamó '{name}': {args}"), latency)
                } else if text.trim().is_empty() {
                    (false, "Respuesta vacía".to_string(), latency)
                } else {
                    (true, format!("✓ texto: {:.80}", text), latency)
                }
            }
        }
        Err(e) => (false, format!("Error: {e}"), latency),
    }
}

// Define test cases

fn schema_lookup_tc() -> ToolTestCase {
    ToolTestCase {
        name: "schema_lookup",
        system: "Eres un asistente de base de datos. Usa get_schema para conocer las tablas disponibles antes de responder.",
        prompt: "¿Qué columnas tiene la tabla invoices?",
        tools: vec![make_schema_tool()],
        expected_tool: Some("get_schema"),
        validate_args: |args| {
            let entity = args.get("entity").and_then(|v| v.as_str()).unwrap_or("");
            if entity == "invoices" || entity.is_empty() { Ok(()) }
            else { Err(format!("entity esperado 'invoices', obtuvo '{entity}'")) }
        },
    }
}

fn sql_generate_tc() -> ToolTestCase {
    ToolTestCase {
        name: "sql_generate",
        system: "Eres un asistente de base de datos. Tienes las tablas: customers, invoices, products, orders. Las facturas tienen columna 'date' en formato TEXT 'YYYY-MM-DD'. Usa query_entity para ejecutar SQL.",
        prompt: "Genera y ejecuta SQL para listar todas las facturas de mayo de 2024",
        tools: all_tools_except_save(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("invoices") && lower.contains("date") && lower.contains("2024-05") {
                Ok(())
            } else {
                Err(format!("SQL no filtra invoices por fecha mayo 2024: {sql:.100}"))
            }
        },
    }
}

fn search_product_tc() -> ToolTestCase {
    ToolTestCase {
        name: "search_product",
        system: "Eres un asistente de base de datos. Usa search_entity para búsqueda de texto completo, query_entity para SQL.",
        prompt: "Busca productos que contengan 'laptop'",
        tools: all_tools_except_save(),
        expected_tool: Some("search_entity"),
        validate_args: |args| {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.to_lowercase().contains("laptop") { Ok(()) }
            else { Err(format!("query no contiene 'laptop': {query}")) }
        },
    }
}

fn relation_query_tc() -> ToolTestCase {
    ToolTestCase {
        name: "relation_query",
        system: "Eres un asistente de base de datos. Las facturas tienen customer_id que referencia a customers. Usa query_entity.",
        prompt: "El cliente ACME (customer_id='C001') quiere ver todas sus facturas. Genera el SQL.",
        tools: all_tools_except_save(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("invoices") && (lower.contains("c001") || lower.contains("acme")) {
                Ok(())
            } else {
                Err(format!("SQL no referencia invoices del cliente C001: {sql:.100}"))
            }
        },
    }
}

fn view_generate_tc() -> ToolTestCase {
    ToolTestCase {
        name: "view_generate",
        system: "Eres un asistente de base de datos. Usa query_entity para validar SQL y save_view para guardar la vista.",
        prompt: "Crea una vista llamada 'facturas-pendientes' que muestre facturas con status 'pendiente'. Columnas: customer_id, amount, date. Primero ejecuta el SQL para validar.",
        tools: all_tools(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("invoices") && lower.contains("pendiente") || lower.contains("status") {
                Ok(())
            } else {
                Err(format!("SQL no filtra facturas pendientes: {sql:.100}"))
            }
        },
    }
}

fn spanish_ambig_tc() -> ToolTestCase {
    ToolTestCase {
        name: "spanish_ambig",
        system: "Eres un asistente de base de datos. El usuario puede decir frases ambiguas. 'los que deben' se refiere a facturas impagas/pendientes.",
        prompt: "Muéstrame 'los que deben'",
        tools: all_tools_except_save(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("invoices") && (lower.contains("pendiente") || lower.contains("impaga") || lower.contains("status")) {
                Ok(())
            } else {
                Err(format!("SQL no infiere facturas pendientes: {sql:.100}"))
            }
        },
    }
}

fn error_recovery_tc() -> ToolTestCase {
    ToolTestCase {
        name: "error_recovery",
        system: "Eres un asistente de base de datos que recibe SQL inválido. Debes corregirlo y ejecutar la versión correcta.",
        prompt: "Corrige y ejecuta este SQL: 'SELEC * FROM customer WHERE name = 'Juan''",
        tools: all_tools_except_save(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("select") && !lower.contains("selec") && lower.contains("customers") {
                Ok(())
            } else {
                Err(format!("SQL no corregido: {sql:.100}"))
            }
        },
    }
}

fn multi_step_tc() -> ToolTestCase {
    ToolTestCase {
        name: "multi_step",
        system: "Eres un asistente de base de datos. Puedes hacer múltiples consultas. Responde con la primera query primero.",
        prompt: "Compara las ventas totales de facturas de mayo vs abril 2024. Genera el SQL para mayo.",
        tools: all_tools_except_save(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("invoices") && (lower.contains("2024-05") || lower.contains("may")) {
                Ok(())
            } else {
                Err(format!("SQL no cubre mayo: {sql:.100}"))
            }
        },
    }
}

fn module_generate_tc() -> ToolTestCase {
    ToolTestCase {
        name: "module_generate",
        system: "Eres un asistente de base de datos. Usa save_view para guardar vistas.",
        prompt: "Crea un módulo de gestión de gastos. Primero genera y ejecuta el SQL para crear una tabla de gastos con: monto, categoría, fecha, descripción.",
        tools: all_tools(),
        expected_tool: Some("query_entity"),
        validate_args: |args| {
            let sql = args.get("sql").and_then(|v| v.as_str()).unwrap_or("");
            let lower = sql.to_lowercase();
            if lower.contains("gastos") || lower.contains("monto") || lower.contains("categor") {
                Ok(())
            } else {
                Err(format!("SQL no crea tabla de gastos: {sql:.100}"))
            }
        },
    }
}

fn all_test_cases() -> Vec<ToolTestCase> {
    vec![
        schema_lookup_tc(),
        sql_generate_tc(),
        search_product_tc(),
        relation_query_tc(),
        view_generate_tc(),
        spanish_ambig_tc(),
        error_recovery_tc(),
        multi_step_tc(),
        module_generate_tc(),
    ]
}

pub fn run_benchmark(url: &str, model: &str, repetitions: usize) -> Vec<BenchmarkResult> {
    let cases = all_test_cases();
    let mut results = Vec::new();

    for tc in &cases {
        for rep in 0..repetitions {
            let (success, msg, latency) = run_tool_test(url, model, tc);
            results.push(BenchmarkResult {
                task: tc.name.to_string(),
                model: model.to_string(),
                mode: "tool-calling".into(),
                repetition: rep,
                success,
                latency_ms: latency,
                validation_msg: msg,
            });
        }
    }
    results
}

pub fn print_results(results: &[BenchmarkResult], summary: &[BenchmarkSummary]) {
    println!("\n═══ TOOL-CALLING BENCHMARK ═══\n");
    for s in summary {
        let pct = if s.total_runs > 0 { (s.successes as f64 / s.total_runs as f64) * 100.0 } else { 0.0 };
        println!(
            "  {:<25} {:<20} {:>3}/{:>3} ({:>5.1}%)  avg {:>8.1}ms  min {:>8.1}ms  max {:>8.1}ms",
            s.task, s.model, s.successes, s.total_runs, pct, s.avg_latency_ms, s.min_latency_ms, s.max_latency_ms,
        );
    }

    let total_ok: usize = summary.iter().map(|s| s.successes).sum();
    let total_all: usize = summary.iter().map(|s| s.total_runs).sum();
    let overall_pct = if total_all > 0 { (total_ok as f64 / total_all as f64) * 100.0 } else { 0.0 };
    println!("\n  TOTAL: {}/{} ({:.1}%)", total_ok, total_all, overall_pct);

    let failures: Vec<&BenchmarkResult> = results.iter().filter(|r| !r.success).collect();
    if !failures.is_empty() {
        println!("─── FAILURES (first 20) ───");
        for f in failures.iter().take(20) {
            println!("  {:<25} rep {}: {}", f.task, f.repetition, f.validation_msg);
        }
    }
}

// ── Integration test ──

#[test]
#[ignore]
fn bench_local_models() {
    let ollama_url = std::env::var("SYNTRIX_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = std::env::var("SYNTRIX_BENCH_MODEL").unwrap_or_else(|_| "granite3.2:2b".to_string());
    let reps: usize = std::env::var("SYNTRIX_BENCH_REPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3);

    eprintln!("Benchmark: model={model}, url={ollama_url}, reps={reps}");
    let results = run_benchmark(&ollama_url, &model, reps);
    let summary = compute_summary(&results);
    print_results(&results, &summary);

    let total_ok: usize = summary.iter().map(|s| s.successes).sum();
    let total_all: usize = summary.iter().map(|s| s.total_runs).sum();
    let pct = if total_all > 0 { total_ok as f64 / total_all as f64 * 100.0 } else { 0.0 };
    eprintln!("Accuracy: {:.1}%", pct);
}

#[test]
#[ignore]
fn bench_deepseek_cloud() {
    let api_key = match std::env::var("DEEPSEEK_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")) {
        Ok(k) => k,
        Err(_) => {
            eprintln!("SKIP: Set DEEPSEEK_API_KEY or OPENAI_API_KEY to test cloud fallback");
            return;
        }
    };

    let (url, model) = if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        ("https://api.deepseek.com/v1".to_string(), "deepseek-chat".to_string())
    } else {
        ("https://api.openai.com/v1".to_string(), "gpt-4o-mini".to_string())
    };

    // For cloud, we use a custom client with auth header
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build().unwrap();

    let cases = all_test_cases();
    let mut results = Vec::new();
    let reps: usize = std::env::var("SYNTRIX_BENCH_REPS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);

    eprintln!("Benchmark cloud: model={model}, url={url}, reps={reps}");

    for tc in &cases {
        for rep in 0..reps {
            let start = Instant::now();

            let mut body = serde_json::json!({
                "model": model,
                "stream": false,
                "messages": [
                    {"role": "system", "content": tc.system},
                    {"role": "user", "content": tc.prompt}
                ]
            });

            let tool_defs: Vec<serde_json::Value> = tc.tools.iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
                })
            }).collect();
            if !tool_defs.is_empty() {
                body["tools"] = serde_json::Value::Array(tool_defs);
            }

            let http_result = client.post(url.clone())
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&body)
                .send();

            let latency = start.elapsed().as_secs_f64() * 1000.0;

            let (success, msg) = match http_result {
                Ok(resp) => {
                    if !resp.status().is_success() {
                        let status = resp.status();
                        let text = resp.text().unwrap_or_default();
                        (false, format!("HTTP {status}: {text:.100}"))
                    } else {
                        match resp.json::<serde_json::Value>() {
                            Ok(json) => {
                                let tool_calls = get_tool_calls(&json);
                                let text = get_text(&json);
                                if let Some(expected) = tc.expected_tool {
                                    if tool_calls.is_empty() {
                                        (false, format!("Esperaba tool '{expected}', no hubo. Texto: {text:.100}"))
                                    } else {
                                        let (name, args) = &tool_calls[0];
                                        if name != expected {
                                            (false, format!("Esperaba '{expected}', obtuvo '{name}'"))
                                        } else {
                                            match (tc.validate_args)(args) {
                                                Ok(()) => (true, format!("✓ {name} args OK")),
                                                Err(e) => (false, format!("{name} args: {e}")),
                                            }
                                        }
                                    }
                                } else {
                                    (true, format!("✓ texto: {text:.80}"))
                                }
                            }
                            Err(e) => (false, format!("parse: {e}")),
                        }
                    }
                }
                Err(e) => (false, format!("Error: {e}")),
            };

            results.push(BenchmarkResult {
                task: tc.name.to_string(),
                model: model.to_string(),
                mode: "tool-calling".into(),
                repetition: rep,
                success,
                latency_ms: latency,
                validation_msg: msg,
            });
        }
    }

    let summary = compute_summary(&results);
    print_results(&results, &summary);

    let total_ok: usize = summary.iter().map(|s| s.successes).sum();
    let total_all: usize = summary.iter().map(|s| s.total_runs).sum();
    let pct = if total_all > 0 { total_ok as f64 / total_all as f64 * 100.0 } else { 0.0 };
    eprintln!("Cloud accuracy: {:.1}%", pct);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_calls_parses_valid_response() {
        let json = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "get_schema",
                            "arguments": r#"{"entity": "invoices"}"#
                        }
                    }]
                }
            }]
        });
        let calls = get_tool_calls(&json);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "get_schema");
        assert_eq!(calls[0].1["entity"], "invoices");
    }

    #[test]
    fn test_get_tool_calls_empty_when_none() {
        let json = serde_json::json!({
            "choices": [{"message": {"role": "assistant", "content": "ok"}}]
        });
        assert!(get_tool_calls(&json).is_empty());
    }

    #[test]
    fn test_all_test_cases_have_valid_structure() {
        let cases = all_test_cases();
        assert_eq!(cases.len(), 9);
        for tc in &cases {
            assert!(!tc.name.is_empty());
            assert!(!tc.prompt.is_empty());
            assert!(tc.expected_tool.is_some());
        }
    }
}
