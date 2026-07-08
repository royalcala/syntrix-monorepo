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
    fn select(&self, _task: TaskType) -> ModelSelection {
        ModelSelection {
            primary: "granite3.2:2b",
            fallback: "granite4.1:3b",
            cloud: Some("groq/llama3-70b"),
        }
    }
}

// ── Capability-Tiered Inference (Fase 2) ──────────────────────────

/// Device capability tier based on hardware specs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceTier {
    /// <4GB RAM, no GPU, <5 tok/s on 3B models
    Minimal,
    /// 4–8GB RAM, integrated GPU, 5–15 tok/s on 3B models
    Standard,
    /// 8–16GB RAM, dedicated GPU, 15–40 tok/s on 3B models
    High,
    /// >16GB RAM, GPU with >8GB VRAM, >40 tok/s on 3B models
    Workstation,
}

impl DeviceTier {
    /// Infer device tier from specs and measured tokens/sec on a 3B parameter model.
    pub fn from_specs(ram_gb: f64, has_gpu: bool, tok_per_sec: f64) -> Self {
        if tok_per_sec >= 40.0 && has_gpu {
            Self::Workstation
        } else if tok_per_sec >= 15.0 || ram_gb >= 12.0 {
            Self::High
        } else if tok_per_sec >= 5.0 || ram_gb >= 4.0 {
            Self::Standard
        } else {
            Self::Minimal
        }
    }

    /// Whether this tier can run a model of `model_size_b` (billions of parameters) locally
    /// at acceptable speed. Rough approximation: need RAM >= model_size * 2 + 2 GB.
    pub fn can_run_local(&self, model_size_b: f64) -> bool {
        let ram_gb = match self {
            Self::Workstation => 24.0,
            Self::High => 12.0,
            Self::Standard => 6.0,
            Self::Minimal => 3.0,
        };
        ram_gb >= model_size_b * 2.0 + 2.0
    }
}

/// Minimum device tier required for a task type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskTier {
    /// Pure text generation, tool selection, schema lookup
    Light,
    /// SQL generation, simple views
    Medium,
    /// Complex views, forms, multi-step, relation navigation
    Heavy,
    /// Module generation, error recovery with context analysis
    Intensive,
}

impl TaskTier {
    pub fn from_task_type(task: TaskType) -> Self {
        match task {
            TaskType::SchemaLookup | TaskType::ToolSelect | TaskType::SpanishAmbig => Self::Light,
            TaskType::SqlGenerate | TaskType::ViewGenerate => Self::Medium,
            TaskType::FormGenerate | TaskType::RelationNavigate | TaskType::MultiStep => Self::Heavy,
            TaskType::ModuleGenerate | TaskType::ErrorRecovery => Self::Intensive,
        }
    }
}

/// Cascade routing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CascadeDecision {
    /// Run locally on-device.
    Local,
    /// Delegate to a capable peer via request-response.
    Delegate,
    /// Send to cloud service (user opt-in required).
    Cloud,
    /// Cannot fulfill (no peer available, cloud not opted in).
    Unavailable,
}

impl CascadeDecision {
    /// Decide how to route based on device capability and task requirements.
    ///
    /// Returns `None` if decision can't be made (e.g. Delegate but no peer).
    pub fn resolve(device: DeviceTier, task: TaskTier, has_peer: bool, cloud_opt_in: bool) -> Self {
        let task_tier_as_device = match task {
            TaskTier::Light => DeviceTier::Minimal,
            TaskTier::Medium => DeviceTier::Standard,
            TaskTier::Heavy => DeviceTier::High,
            TaskTier::Intensive => DeviceTier::Workstation,
        };

        if device as u8 >= task_tier_as_device as u8 {
            return Self::Local;
        }

        if has_peer {
            return Self::Delegate;
        }

        if cloud_opt_in {
            return Self::Cloud;
        }

        Self::Unavailable
    }
}

/// Strip `<think>…</think>` tags from model output. Local models often include
/// chain-of-thought reasoning inside these tags which should not be shown to users.
pub fn strip_think_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_think = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::from("<");
            loop {
                match chars.next() {
                    Some('>') => {
                        tag.push('>');
                        break;
                    }
                    Some(c) => tag.push(c),
                    None => break,
                }
            }
            if tag.eq_ignore_ascii_case("<think>") {
                in_think = true;
                continue;
            }
            if tag.eq_ignore_ascii_case("</think>") {
                in_think = false;
                continue;
            }
            if !in_think {
                result.push_str(&tag);
            }
        } else if !in_think {
            result.push(c);
        }
    }
    result
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
        assert!(sel.cloud.is_some());
        let sel2 = router.select(TaskType::SqlGenerate);
        assert_eq!(sel2.primary, "granite3.2:2b");
    }

    // ── DeviceTier tests ──

    #[test]
    fn test_device_tier_minimal() {
        assert_eq!(DeviceTier::from_specs(2.0, false, 3.0), DeviceTier::Minimal);
    }

    #[test]
    fn test_device_tier_standard() {
        assert_eq!(DeviceTier::from_specs(6.0, false, 10.0), DeviceTier::Standard);
        assert_eq!(DeviceTier::from_specs(4.0, true, 4.0), DeviceTier::Standard);
    }

    #[test]
    fn test_device_tier_high() {
        assert_eq!(DeviceTier::from_specs(16.0, true, 20.0), DeviceTier::High);
        assert_eq!(DeviceTier::from_specs(12.0, false, 12.0), DeviceTier::High);
    }

    #[test]
    fn test_device_tier_workstation() {
        assert_eq!(DeviceTier::from_specs(32.0, true, 50.0), DeviceTier::Workstation);
    }

    #[test]
    fn test_can_run_local_by_tier() {
        assert!(DeviceTier::Workstation.can_run_local(7.0)); // 24 >= 7*2+2 = 16
        assert!(!DeviceTier::Minimal.can_run_local(7.0));    // 3 < 16
        assert!(DeviceTier::Standard.can_run_local(1.5));     // 6 >= 1.5*2+2 = 5
        assert!(!DeviceTier::Standard.can_run_local(7.0));   // 6 < 16
    }

    // ── TaskTier tests ──

    #[test]
    fn test_task_tier_light() {
        assert_eq!(TaskTier::from_task_type(TaskType::SchemaLookup), TaskTier::Light);
        assert_eq!(TaskTier::from_task_type(TaskType::ToolSelect), TaskTier::Light);
    }

    #[test]
    fn test_task_tier_medium() {
        assert_eq!(TaskTier::from_task_type(TaskType::SqlGenerate), TaskTier::Medium);
        assert_eq!(TaskTier::from_task_type(TaskType::ViewGenerate), TaskTier::Medium);
    }

    #[test]
    fn test_task_tier_heavy() {
        assert_eq!(TaskTier::from_task_type(TaskType::FormGenerate), TaskTier::Heavy);
        assert_eq!(TaskTier::from_task_type(TaskType::MultiStep), TaskTier::Heavy);
    }

    #[test]
    fn test_task_tier_intensive() {
        assert_eq!(TaskTier::from_task_type(TaskType::ModuleGenerate), TaskTier::Intensive);
    }

    // ── CascadeDecision tests ──

    #[test]
    fn test_cascade_local_when_device_capable() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::High, TaskTier::Medium, false, false),
            CascadeDecision::Local
        );
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Workstation, TaskTier::Intensive, false, false),
            CascadeDecision::Local
        );
    }

    #[test]
    fn test_cascade_delegate_weak_device_with_peer() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Minimal, TaskTier::Medium, true, false),
            CascadeDecision::Delegate
        );
    }

    #[test]
    fn test_cascade_cloud_when_no_peer_and_opted_in() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Standard, TaskTier::Intensive, false, true),
            CascadeDecision::Cloud
        );
    }

    #[test]
    fn test_cascade_unavailable_when_insufficient() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Minimal, TaskTier::Intensive, false, false),
            CascadeDecision::Unavailable
        );
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Standard, TaskTier::Heavy, false, false),
            CascadeDecision::Unavailable
        );
    }

    #[test]
    fn test_cascade_prefers_local_over_delegate() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Workstation, TaskTier::Light, true, true),
            CascadeDecision::Local
        );
    }

    #[test]
    fn test_cascade_prefers_delegate_over_cloud() {
        assert_eq!(
            CascadeDecision::resolve(DeviceTier::Minimal, TaskTier::Medium, true, true),
            CascadeDecision::Delegate
        );
    }

    // ── strip_think_tags tests ──

    #[test]
    fn test_strip_basic_think_block() {
        let input = "Hola<think>esto es reasoning</think>mundo";
        assert_eq!(strip_think_tags(input), "Holamundo");
    }

    #[test]
    fn test_strip_empty_think() {
        let input = "texto<think></think>fin";
        assert_eq!(strip_think_tags(input), "textofin");
    }

    #[test]
    fn test_strip_no_think_tags() {
        let input = "texto normal sin tags";
        assert_eq!(strip_think_tags(input), "texto normal sin tags");
    }

    #[test]
    fn test_strip_multiple_think_blocks() {
        let input = "a<think>1</think>b<think>2</think>c";
        assert_eq!(strip_think_tags(input), "abc");
    }

    #[test]
    fn test_strip_nested_like_html_inside_think() {
        let input = "<think><b>razonamiento</b></think>resultado";
        assert_eq!(strip_think_tags(input), "resultado");
    }

    #[test]
    fn test_strip_case_insensitive_think() {
        let input = "Hola<THINK>razon</THINK>mundo";
        assert_eq!(strip_think_tags(input), "Holamundo");
    }
}
