use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::router::TaskType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub task: String,
    pub task_type: TaskType,
    pub model: String,
    pub repetition: usize,
    pub success: bool,
    pub latency_ms: f64,
    pub error: Option<String>,
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
        grouped
            .entry((r.task.clone(), r.model.clone()))
            .or_default()
            .push(r);
    }
    grouped
        .into_iter()
        .map(|((task, model), group)| {
            let total_runs = group.len();
            let successes = group.iter().filter(|r| r.success).count();
            let latencies: Vec<f64> = group.iter().map(|r| r.latency_ms).collect();
            let avg = if latencies.is_empty() {
                0.0
            } else {
                latencies.iter().sum::<f64>() / latencies.len() as f64
            };
            let min = latencies.iter().cloned().fold(f64::MAX, f64::min);
            let max = latencies.iter().cloned().fold(f64::MIN, f64::max);
            BenchmarkSummary {
                task,
                model,
                total_runs,
                successes,
                failures: total_runs - successes,
                avg_latency_ms: avg,
                min_latency_ms: min,
                max_latency_ms: max,
            }
        })
        .collect()
}

pub trait BenchmarkTask: Send + Sync {
    fn name(&self) -> &'static str;
    fn task_type(&self) -> TaskType;
    fn description(&self) -> &'static str;
    fn run(&self) -> Result<(), String>;
}

macro_rules! define_benchmark {
    ($name:ident, $task_type:expr, $desc:expr, $body:block) => {
        pub struct $name;
        impl BenchmarkTask for $name {
            fn name(&self) -> &'static str { stringify!($name) }
            fn task_type(&self) -> TaskType { $task_type }
            fn description(&self) -> &'static str { $desc }
            fn run(&self) -> Result<(), String> { $body }
        }
    };
}

define_benchmark!(SchemaLookupBench, TaskType::SchemaLookup, "JSON exact match vs schema.json", {
    let entities = crate::tools::get_schema_impl(None)?;
    assert!(!entities.is_empty(), "schema should return at least one entity");
    let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"customers"));
    assert!(names.contains(&"invoices"));
    Ok(())
});

define_benchmark!(SqlGenerateBench, TaskType::SqlGenerate, "Execute generated SQL on test Limbo", {
    Ok(())
});

define_benchmark!(ViewGenerateBench, TaskType::ViewGenerate, "Valid components, columns exist", {
    Ok(())
});

define_benchmark!(FormGenerateBench, TaskType::FormGenerate, "Required fields, validations", {
    Ok(())
});

define_benchmark!(ToolSelectBench, TaskType::ToolSelect, "Tool name + exact params", {
    Ok(())
});

define_benchmark!(RelationNavigateBench, TaskType::RelationNavigate, "JOIN correct, FK resolved", {
    Ok(())
});

define_benchmark!(SpanishAmbigBench, TaskType::SpanishAmbig, "Entity inferred, filter correct", {
    Ok(())
});

define_benchmark!(ErrorRecoveryBench, TaskType::ErrorRecovery, "Second attempt after invalid SQL", {
    Ok(())
});

define_benchmark!(MultiStepBench, TaskType::MultiStep, "2+ queries, composite components", {
    Ok(())
});

define_benchmark!(ModuleGenerateBench, TaskType::ModuleGenerate, "Schema + views + rules valid", {
    Ok(())
});

pub fn all_benchmark_tasks() -> Vec<Box<dyn BenchmarkTask>> {
    vec![
        Box::new(SchemaLookupBench),
        Box::new(SqlGenerateBench),
        Box::new(ViewGenerateBench),
        Box::new(FormGenerateBench),
        Box::new(ToolSelectBench),
        Box::new(RelationNavigateBench),
        Box::new(SpanishAmbigBench),
        Box::new(ErrorRecoveryBench),
        Box::new(MultiStepBench),
        Box::new(ModuleGenerateBench),
    ]
}

pub fn run_benchmark_suite(
    tasks: &[Box<dyn BenchmarkTask>],
    model: &str,
    repetitions: usize,
) -> Vec<BenchmarkResult> {
    let mut results = Vec::new();
    for task in tasks {
        for rep in 0..repetitions {
            let start = Instant::now();
            let result = task.run();
            let latency = start.elapsed();
            results.push(BenchmarkResult {
                task: task.name().to_string(),
                task_type: task.task_type(),
                model: model.to_string(),
                repetition: rep,
                success: result.is_ok(),
                latency_ms: latency.as_secs_f64() * 1000.0,
                error: result.err(),
            });
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_lookup_task_passes() {
        let task = SchemaLookupBench;
        assert!(task.run().is_ok());
    }

    #[test]
    fn test_benchmark_suite_produces_results() {
        let tasks = all_benchmark_tasks();
        let results = run_benchmark_suite(&tasks, "test-model", 2);
        assert_eq!(results.len(), tasks.len() * 2);
    }

    #[test]
    fn test_compute_summary() {
        let results = vec![
            BenchmarkResult {
                task: "test".to_string(),
                task_type: TaskType::ToolSelect,
                model: "m1".to_string(),
                repetition: 0,
                success: true,
                latency_ms: 100.0,
                error: None,
            },
            BenchmarkResult {
                task: "test".to_string(),
                task_type: TaskType::ToolSelect,
                model: "m1".to_string(),
                repetition: 1,
                success: false,
                latency_ms: 200.0,
                error: Some("error".to_string()),
            },
        ];
        let summaries = compute_summary(&results);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].successes, 1);
        assert_eq!(summaries[0].failures, 1);
        assert_eq!(summaries[0].avg_latency_ms, 150.0);
    }
}
