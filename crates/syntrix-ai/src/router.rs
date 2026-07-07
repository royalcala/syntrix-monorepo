use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    SchemaLookup,
    SqlGenerate,
    ViewGenerate,
    FormGenerate,
    ToolSelect,
    RelationNavigate,
    SpanishAmbig,
    ErrorRecovery,
    MultiStep,
    ModuleGenerate,
}

impl TaskType {
    pub fn classify(query: &str) -> Self {
        let q = query.to_lowercase();
        if q.contains("schema")
            || q.contains("tablas")
            || q.contains("entidades")
            || q.contains("columnas")
        {
            return Self::SchemaLookup;
        }
        if q.contains("crear") && (q.contains("formulario") || q.contains("form")) {
            return Self::FormGenerate;
        }
        if q.contains("sql") || q.contains("consulta") || q.contains("select") {
            return Self::SqlGenerate;
        }
        if q.contains("relacion") || q.contains("join") || q.contains("hijo") || q.contains("child") {
            return Self::RelationNavigate;
        }
        if q.contains("modulo") || q.contains("template") || q.contains("plugin") {
            return Self::ModuleGenerate;
        }
        if q.contains("paso") || q.contains("multi") || q.contains("combinado") {
            return Self::MultiStep;
        }
        if q.contains("vista") || q.contains("tabla") || q.contains("listado") || q.starts_with("lista") || q.starts_with("dame") {
            return Self::ViewGenerate;
        }
        Self::ToolSelect
    }
}

#[derive(Debug, Clone)]
pub struct ModelSelection {
    pub primary: &'static str,
    pub fallback: &'static str,
    pub cloud: Option<&'static str>,
}

impl ModelSelection {
    pub fn all_models(&self) -> Vec<&str> {
        let mut models = vec![self.primary, self.fallback];
        if let Some(c) = self.cloud {
            models.push(c);
        }
        models
    }
}

pub const MODELS: &[(&str, &str)] = &[
    ("granite3.2:2b", "granite"),
    ("granite4.1:3b", "granite"),
    ("qwen2.5-coder:3b", "qwen"),
    ("deepseek-r1:1.5b", "deepseek"),
];

pub trait ModelRouter: Send + Sync {
    fn select(&self, task: TaskType) -> ModelSelection;
}

pub struct DefaultRouter;

impl ModelRouter for DefaultRouter {
    fn select(&self, task: TaskType) -> ModelSelection {
        match task {
        TaskType::SchemaLookup | TaskType::FormGenerate => ModelSelection {
            primary: "granite3.2:2b",
            fallback: "granite4.1:3b",
            cloud: None,
        },
        TaskType::SqlGenerate | TaskType::ToolSelect | TaskType::SpanishAmbig => ModelSelection {
            primary: "qwen2.5-coder:3b",
            fallback: "granite4.1:3b",
            cloud: Some("groq/llama3-70b"),
        },
        TaskType::ViewGenerate | TaskType::RelationNavigate | TaskType::MultiStep => ModelSelection {
            primary: "granite4.1:3b",
            fallback: "qwen2.5-coder:3b",
            cloud: Some("groq/llama3-70b"),
        },
        TaskType::ErrorRecovery => ModelSelection {
            primary: "qwen2.5-coder:3b",
            fallback: "deepseek-r1:1.5b",
            cloud: Some("deepseek/deepseek-chat"),
        },
        TaskType::ModuleGenerate => ModelSelection {
            primary: "granite4.1:3b",
            fallback: "deepseek-r1:1.5b",
            cloud: Some("deepseek/deepseek-chat"),
        },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_schema_lookup() {
        assert_eq!(TaskType::classify("qué tablas existen en el schema"), TaskType::SchemaLookup);
        assert_eq!(TaskType::classify("muéstrame las entidades"), TaskType::SchemaLookup);
    }

    #[test]
    fn test_classify_view_generate() {
        assert_eq!(TaskType::classify("muéstrame una tabla de facturas"), TaskType::ViewGenerate);
        assert_eq!(TaskType::classify("lista de clientes"), TaskType::ViewGenerate);
    }

    #[test]
    fn test_classify_form_generate() {
        assert_eq!(TaskType::classify("crear formulario para factura"), TaskType::FormGenerate);
        assert_eq!(TaskType::classify("crear form de cliente"), TaskType::FormGenerate);
    }

    #[test]
    fn test_classify_sql_generate() {
        assert_eq!(TaskType::classify("genera un SQL de consulta"), TaskType::SqlGenerate);
        assert_eq!(TaskType::classify("SELECT * FROM customers"), TaskType::SqlGenerate);
    }

    #[test]
    fn test_classify_fallback_to_tool_select() {
        assert_eq!(TaskType::classify("hola mundo"), TaskType::ToolSelect);
        assert_eq!(TaskType::classify("ayuda"), TaskType::ToolSelect);
    }

    #[test]
    fn test_default_router_returns_models() {
        let router = DefaultRouter;
        let sel = router.select(TaskType::SchemaLookup);
        assert_eq!(sel.primary, "granite3.2:2b");
        assert_eq!(sel.fallback, "granite4.1:3b");
        assert!(sel.cloud.is_none());
        let sel2 = router.select(TaskType::SqlGenerate);
        assert_eq!(sel2.primary, "qwen2.5-coder:3b");
    }
}
