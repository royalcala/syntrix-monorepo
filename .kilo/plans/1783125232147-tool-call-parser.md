# Tool-Call Parser para Modelos Locales

## El Problema

Cuando el `ai_chat_impl` (agent loop) envía un mensaje a Ollama con herramientas definidas, el modelo **debería** responder con un campo estructurado `tool_calls` en el JSON:

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": null,
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": { "name": "query_entity", "arguments": "{\"sql\": \"SELECT...\"}" }
      }]
    }
  }]
}
```

Pero los modelos pequeños (granite3.2:2b, qwen2.5-coder:3b) **no generan ese campo estructurado**. En vez de eso, ponen la llamada a herramienta como **texto plano** en el campo `content`:

```json
{
  "content": "Primero necesito consultar la base de datos. Usaría query_entity con SQL: SELECT * FROM invoices WHERE date LIKE '2024-05%'"
}
```

O peor aún, a veces ponen JSON dentro del texto:

```json
{
  "content": "{\"function\": \"query_entity\", \"arguments\": {\"sql\": \"SELECT * FROM invoices WHERE date LIKE '2024-05%'\"}}"
}
```

## La Solución: Parser de Texto

En vez de depender del campo `tool_calls` estructurado (que los modelos locales no soportan), el `ai_chat_impl` debe **parsear el texto de respuesta** para detectar:

1. **JSON explícito**: Si el contenido empieza con `{` o `[`, intentar parsearlo como llamada a tool
2. **Keywords + regex**: Buscar patrones como `query_entity(...)`, `search_entity(...)` o `SQL: SELECT...` en el texto
3. **Fallback a LLM**: Enviar el texto de vuelta al modelo pidiendo que extraiga la llamada en formato JSON

### Ejemplo de parsing

```rust
fn parse_tool_call(text: &str, tools: &[ToolDefinition]) -> Option<ToolCall> {
    // 1. Intentar parsear como JSON directo
    if let Ok(json) = serde_json::from_str::<Value>(text) {
        if let Some(name) = json.get("function").and_then(|v| v.as_str()) {
            if let Some(args) = json.get("arguments") {
                return Some(ToolCall { name: name.into(), args: args.clone(), .. });
            }
        }
    }
    
    // 2. Buscar patrones tipo: query_entity(sql: "...")
    for tool in tools {
        let pattern = format!(r#"{}\((.*?)\)"#, tool.name);
        if let Some(captures) = regex::Captures::new(&pattern, text) {
            // Extraer argumentos
            return Some(ToolCall { name: tool.name.clone(), args: parsed_args, .. });
        }
    }
    
    // 3. Buscar SQL inline
    if text.contains("SELECT") || text.contains("WITH") {
        return Some(ToolCall {
            name: "query_entity".into(),
            args: json!({"sql": extract_sql(text)}),
            .. 
        });
    }
    
    None
}
```

## Por qué esto funciona

Ambos modelos (granite3.2:2b y qwen2.5-coder:3b) **sí entienden qué tool usar** — el benchmark mostró que generan las llamadas correctas, solo que en texto plano. El parser:

- Aprovecha el conocimiento que el modelo YA tiene
- No requiere GPU, fine-tuning, API key, ni cloud
- Es determinístico (sabemos exactamente qué patrones buscar)
- Se puede mejorar incrementalmente (agregar más patrones)

## Cuándo NO funcionaría

- Si el modelo da una respuesta puramente explicativa sin mencionar la tool
- Si el texto es muy ambiguo (poco probable con nuestros prompts)

En esos casos, el fallback es: enviar el texto de vuelta al modelo en un segundo turno pidiendo específicamente el formato JSON.

## Implementación

1. Agregar `regex` como dependencia a `crates/syntrix-ai/`
2. Crear `crates/syntrix-ai/src/parser.rs` con `parse_tool_call(text, tools) -> Option<ToolCall>`
3. Modificar `ai_chat_impl` para que si `provider_response.tool_calls` está vacío, intente el parser
4. Si el parser falla, enviar mensaje adicional al modelo: "Tu respuesta debe incluir SOLO JSON con nombre de herramienta y argumentos"

## Prioridad

**Pendiente** — después de Tasks 10-17.
