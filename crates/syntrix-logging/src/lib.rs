use std::collections::{HashMap, VecDeque};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};

// Re-exported so the syntrix_span! macro can reference $crate::uuid::Uuid
pub use uuid;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub span_path: String,
    pub corr_id: String,
    pub message: String,
    pub fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub level: Option<String>,
    pub target: Option<String>,
    pub span_path: Option<String>,
    pub org: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub search: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummary {
    pub counts_by_level: HashMap<String, usize>,
    pub counts_by_target: HashMap<String, usize>,
    pub top_spans: Vec<String>,
    pub recent_errors: Vec<LogRecord>,
}

// ---------------------------------------------------------------------------
// Span context stored as span extension
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SpanContext {
    pub org: String,
    pub op: String,
    pub step: String,
    pub corr_id: String,
}

// ---------------------------------------------------------------------------
// Thread-local span stack for nested spans
// ---------------------------------------------------------------------------

thread_local! {
    pub static SPAN_STACK: std::cell::RefCell<Vec<SpanContext>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

// ---------------------------------------------------------------------------
// Span guard — pops from thread-local stack on drop
// ---------------------------------------------------------------------------

#[must_use]
pub struct SpanGuard {
    _private: (),
}

impl SpanGuard {
    pub fn new() -> Self {
        SpanGuard { _private: () }
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        SPAN_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

// ---------------------------------------------------------------------------
// Syntrix span macro
// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! syntrix_span {
    ($org:expr, $op:expr, $step:expr) => {{
        let __corr_id = $crate::uuid::Uuid::new_v4().to_string();
        let __ctx = $crate::SpanContext {
            org: $org.to_string(),
            op: $op.to_string(),
            step: $step.to_string(),
            corr_id: __corr_id.clone(),
        };
        $crate::SPAN_STACK.with(|stack| stack.borrow_mut().push(__ctx));
        let __guard = $crate::SpanGuard::new();
        let __span = tracing::span!(
            tracing::Level::INFO,
            "syntrix_op",
            org = %$org,
            op = %$op,
            step = %$step,
            corr_id = %__corr_id,
        );
        let __entered = __span.enter();
        (__guard, __entered)
    }};
}

// ---------------------------------------------------------------------------
// Canonical operation helpers
// ---------------------------------------------------------------------------

#[macro_export]
macro_rules! syntrix_create_org_span {
    ($org:expr) => { $crate::syntrix_span!($org, "create_org", "init") };
}

#[macro_export]
macro_rules! syntrix_send_invite_span {
    ($org:expr) => { $crate::syntrix_span!($org, "send_invite", "init") };
}

#[macro_export]
macro_rules! syntrix_join_org_span {
    ($org:expr) => { $crate::syntrix_span!($org, "join_org", "init") };
}

#[macro_export]
macro_rules! syntrix_commit_event_span {
    ($org:expr) => { $crate::syntrix_span!($org, "commit_event", "write") };
}

#[macro_export]
macro_rules! syntrix_sync_push_span {
    ($org:expr) => { $crate::syntrix_span!($org, "sync_push", "push") };
}

#[macro_export]
macro_rules! syntrix_sync_pull_span {
    ($org:expr) => { $crate::syntrix_span!($org, "sync_pull", "pull") };
}

#[macro_export]
macro_rules! syntrix_heartbeat_span {
    ($org:expr) => { $crate::syntrix_span!($org, "heartbeat", "beat") };
}

#[macro_export]
macro_rules! syntrix_subscribe_ingest_span {
    ($org:expr) => { $crate::syntrix_span!($org, "subscribe_ingest", "ingest") };
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

const SENSITIVE_FIELD_PREFIXES: &[&str] = &["secret_"];
const SENSITIVE_FIELD_NAMES: &[&str] = &["keypair", "doc_ticket", "ticket"];

fn redact_field_value(name: &str, _value: &str) -> String {
    if SENSITIVE_FIELD_NAMES.contains(&name) {
        return "[REDACTED]".to_string();
    }
    for prefix in SENSITIVE_FIELD_PREFIXES {
        if name.starts_with(prefix) {
            return "[REDACTED]".to_string();
        }
    }
    // Check for hex-encoded key material (64+ hex chars)
    if _value.len() >= 64 && _value.chars().all(|c| c.is_ascii_hexdigit()) {
        return "[REDACTED_KEY]".to_string();
    }
    _value.to_string()
}

// ---------------------------------------------------------------------------
// Event visitor for capturing field data
// ---------------------------------------------------------------------------

struct LogFieldVisitor<'a> {
    record: &'a mut LogRecord,
}

impl<'a> tracing::field::Visit for LogFieldVisitor<'a> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name();
        match name {
            "message" => self.record.message = value.to_string(),
            "org" | "op" | "step" => {
                self.record
                    .fields
                    .insert(name.to_string(), serde_json::Value::String(value.to_string()));
            }
            _ => {
                let val = redact_field_value(name, value);
                self.record
                    .fields
                    .insert(name.to_string(), serde_json::Value::String(val));
            }
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let debug_str = format!("{:?}", value);
        if name == "message" {
            self.record.message = debug_str;
        } else {
            let val = redact_field_value(name, &debug_str);
            self.record
                .fields
                .insert(name.to_string(), serde_json::Value::String(val));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record
            .fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
}

// ---------------------------------------------------------------------------
// Log Layer — shared inner state
// ---------------------------------------------------------------------------

struct LogLayerInner {
    ring: VecDeque<LogRecord>,
    ndjson_writer: Option<BufWriter<std::fs::File>>,
    tail_senders: Vec<std::sync::mpsc::Sender<LogRecord>>,
}

pub(crate) type LogInner = Arc<Mutex<LogLayerInner>>;

fn subscribe_tail_inner(inner: &LogInner) -> std::sync::mpsc::Receiver<LogRecord> {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Ok(mut guard) = inner.lock() {
        guard.tail_senders.push(tx);
    }
    rx
}

// ---------------------------------------------------------------------------
// Log Layer (consumed by the tracing registry)
// ---------------------------------------------------------------------------

pub struct LogLayer {
    inner: LogInner,
    max_ring: usize,
}

impl LogLayer {
    fn new(inner: LogInner, max_ring: usize) -> Self {
        LogLayer { inner, max_ring }
    }
}

fn format_level(level: &tracing::metadata::Level) -> String {
    match *level {
        tracing::Level::TRACE => "TRACE".to_string(),
        tracing::Level::DEBUG => "DEBUG".to_string(),
        tracing::Level::INFO => "INFO".to_string(),
        tracing::Level::WARN => "WARN".to_string(),
        tracing::Level::ERROR => "ERROR".to_string(),
    }
}

impl<S> Layer<S> for LogLayer
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("span must exist after on_new_span");
        let mut extensions = span.extensions_mut();
        let mut visitor = SpanFieldCollector::default();
        attrs.record(&mut visitor);
        if let (Some(org), Some(op), Some(step), Some(corr_id)) =
            (visitor.org, visitor.op, visitor.step, visitor.corr_id)
        {
            extensions.insert(SpanContext {
                org,
                op,
                step,
                corr_id,
            });
        }
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut record = LogRecord {
            ts: Utc::now().to_rfc3339(),
            level: format_level(event.metadata().level()),
            target: event.metadata().target().to_string(),
            span_path: String::new(),
            corr_id: String::new(),
            message: String::new(),
            fields: HashMap::new(),
        };

        // Capture event fields
        let mut visitor = LogFieldVisitor { record: &mut record };
        event.record(&mut visitor);

        // Read span context from current span stack
        let span = ctx.current_span();
        if let Some(id) = span.id() {
            if let Some(span_ref) = ctx.span(&id) {
                if let Some(sc) = span_ref.extensions().get::<SpanContext>() {
                    record.span_path = format!("org={}/op={}/step={}", sc.org, sc.op, sc.step);
                    record.corr_id = sc.corr_id.clone();
                }
            }
        }

        // Fallback: read from thread-local stack
        if record.span_path.is_empty() {
            SPAN_STACK.with(|stack| {
                if let Some(ctx) = stack.borrow().last() {
                    record.span_path =
                        format!("org={}/op={}/step={}", ctx.org, ctx.op, ctx.step);
                    record.corr_id = ctx.corr_id.clone();
                }
            });
        }

        let json = serde_json::to_string(&record).unwrap_or_default();

        if let Ok(mut inner) = self.inner.lock() {
            // Ring buffer
            inner.ring.push_back(record.clone());
            if inner.ring.len() > self.max_ring {
                inner.ring.pop_front();
            }

            // NDJSON file
            if let Some(ref mut writer) = inner.ndjson_writer {
                let _ = writeln!(writer, "{}", json);
            }

            // Tail subscribers
            let mut dead = Vec::new();
            for (i, sender) in inner.tail_senders.iter().enumerate() {
                if sender.send(record.clone()).is_err() {
                    dead.push(i);
                }
            }
            for i in dead.into_iter().rev() {
                inner.tail_senders.swap_remove(i);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Span field collector for on_new_span
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SpanFieldCollector {
    org: Option<String>,
    op: Option<String>,
    step: Option<String>,
    corr_id: Option<String>,
}

impl tracing::field::Visit for SpanFieldCollector {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "org" => self.org = Some(value.to_string()),
            "op" => self.op = Some(value.to_string()),
            "step" => self.step = Some(value.to_string()),
            "corr_id" => self.corr_id = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        match field.name() {
            "org" => self.org = Some(s),
            "op" => self.op = Some(s),
            "step" => self.step = Some(s),
            "corr_id" => self.corr_id = Some(s),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// LogHandle
// ---------------------------------------------------------------------------

pub struct LogHandle {
    inner: LogInner,
    data_dir: PathBuf,
    app_name: String,
}

impl LogHandle {
    pub fn subscribe_tail(&self) -> std::sync::mpsc::Receiver<LogRecord> {
        subscribe_tail_inner(&self.inner)
    }

    fn query_ring(
        &self,
        query: &LogQuery,
    ) -> Vec<LogRecord> {
        let inner = self.inner.lock().unwrap();
        let limit = query.limit.unwrap_or(500);
        let offset = query.offset.unwrap_or(0);

        let filtered: Vec<_> = inner
            .ring
            .iter()
            .filter(|r| matches_query(r, query))
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        filtered
    }

    fn scan_ndjson(
        &self,
        query: &LogQuery,
    ) -> Vec<LogRecord> {
        let log_dir = self.data_dir.join("logs");
        let pattern = format!("syntrix-{}.ndjson", self.app_name);
        let log_file = log_dir.join(&pattern);

        if !log_file.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&log_file) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let limit = query.limit.unwrap_or(500);
        let offset = query.offset.unwrap_or(0);

        content
            .lines()
            .filter_map(|line| serde_json::from_str::<LogRecord>(line).ok())
            .filter(|r| matches_query(r, query))
            .skip(offset)
            .take(limit)
            .collect()
    }
}

fn matches_query(record: &LogRecord, query: &LogQuery) -> bool {
    if let Some(ref level) = query.level {
        if !record.level.eq_ignore_ascii_case(level) {
            return false;
        }
    }
    if let Some(ref target) = query.target {
        if !record.target.contains(target) {
            return false;
        }
    }
    if let Some(ref span_path) = query.span_path {
        if !record.span_path.contains(span_path) {
            return false;
        }
    }
    if let Some(ref org) = query.org {
        let org_path = format!("org={}", org);
        if !record.span_path.contains(&org_path) {
            return false;
        }
    }
    if let Some(ref since) = query.since {
        if record.ts.as_str() < since.as_str() {
            return false;
        }
    }
    if let Some(ref until) = query.until {
        if record.ts.as_str() > until.as_str() {
            return false;
        }
    }
    if let Some(ref search) = query.search {
        let lower = search.to_lowercase();
        if !record.message.to_lowercase().contains(&lower)
            && !record.target.to_lowercase().contains(&lower)
            && !record.span_path.to_lowercase().contains(&lower)
        {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Query API (*_impl)
// ---------------------------------------------------------------------------

pub fn query_logs_impl(
    handle: &LogHandle,
    query: &LogQuery,
) -> Vec<LogRecord> {
    let has_time_window = query.since.is_some() || query.until.is_some();
    // If a time window is specified, scan NDJSON (historical).
    // For recent queries, the ring buffer is sufficient.
    if has_time_window {
        handle.scan_ndjson(query)
    } else {
        handle.query_ring(query)
    }
}

pub fn summarize_logs_impl(
    handle: &LogHandle,
    _window: Duration,
) -> LogSummary {
    let inner = handle.inner.lock().unwrap();
    let mut counts_by_level: HashMap<String, usize> = HashMap::new();
    let mut counts_by_target: HashMap<String, usize> = HashMap::new();
    let mut recent_errors: Vec<LogRecord> = Vec::new();

    for record in inner.ring.iter().rev() {
        *counts_by_level.entry(record.level.clone()).or_insert(0) += 1;
        *counts_by_target.entry(record.target.clone()).or_insert(0) += 1;

        if record.level == "ERROR" && recent_errors.len() < 20 {
            recent_errors.push(record.clone());
        }
    }

    // Top spans by frequency
    let mut span_counts: HashMap<String, usize> = HashMap::new();
    for record in inner.ring.iter() {
        if !record.span_path.is_empty() {
            *span_counts.entry(record.span_path.clone()).or_insert(0) += 1;
        }
    }
    let mut span_vec: Vec<(String, usize)> = span_counts.into_iter().collect();
    span_vec.sort_by(|a, b| b.1.cmp(&a.1));
    let top_spans: Vec<String> = span_vec.into_iter().take(10).map(|(s, _)| s).collect();

    LogSummary {
        counts_by_level,
        counts_by_target,
        top_spans,
        recent_errors,
    }
}

// ---------------------------------------------------------------------------
// Retention
// ---------------------------------------------------------------------------

fn cleanup_old_logs(data_dir: &PathBuf, app_name: &str) {
    let log_dir = data_dir.join("logs");
    if !log_dir.exists() {
        return;
    }

    let now = chrono::Utc::now();
    let max_age_app = chrono::Duration::days(7);
    let max_size_app: u64 = 200 * 1024 * 1024; // 200 MB for the whole dir
    let max_size_per_file: u64 = 50 * 1024 * 1024; // 50 MB per file

    let mut total_size: u64 = 0;
    let mut entries: Vec<_> = Vec::new();

    if let Ok(dir) = std::fs::read_dir(&log_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = path.metadata() {
                    total_size += metadata.len();
                    if let Ok(modified) = metadata.modified() {
                        let age = now.signed_duration_since(
                            chrono::DateTime::<chrono::Utc>::from(
                                std::time::SystemTime::from(modified),
                            ),
                        );
                        entries.push((path, age, metadata.len()));
                    }
                }
            }
        }
    }

    // Remove old files (>7 days)
    for (path, age, _size) in &entries {
        if *age > max_age_app {
                if *age > max_age_app {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    // If total dir size > 200 MB or single file > 50 MB, rotate the biggest files
    if total_size > max_size_app {
        let mut sorted: Vec<_> = entries
            .iter()
            .filter(|(p, _, _)| {
                let fname = p.file_name().unwrap_or_default().to_string_lossy();
                fname.contains(app_name)
            })
            .collect();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));
        let mut accum = 0u64;
        for (path, _, size) in sorted {
            accum += *size;
            if accum > max_size_app || *size > max_size_per_file {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

pub fn init_logging(app_name: &str, data_dir: PathBuf) -> LogHandle {
    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    // Cleanup old logs
    cleanup_old_logs(&data_dir, app_name);

    // Default ring size from env or 5000
    let max_ring: usize = std::env::var("SYNTRIX_LOG_RING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    // Shared inner state
    let inner = Arc::new(Mutex::new(LogLayerInner {
        ring: VecDeque::with_capacity(max_ring),
        ndjson_writer: None,
        tail_senders: Vec::new(),
    }));

    // Set up NDJSON file writer
    let ndjson_path = log_dir.join(format!("syntrix-{}.ndjson", app_name));
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ndjson_path)
    {
        if let Ok(mut guard) = inner.lock() {
            guard.ndjson_writer = Some(BufWriter::new(file));
        }
    }

    // Create the custom log layer (moved into registry, not shared via Arc)
    let log_layer = LogLayer::new(inner.clone(), max_ring);

    // Console layer: human-readable with EnvFilter
    let console_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        "syntrix=info".to_string()
    });
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_filter(EnvFilter::new(&console_filter));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(log_layer)
        .init();

    // Spawn periodic cleanup (only if a tokio runtime is active — init_logging may
    // be called before the Tauri/async runtime starts; cleanup runs on every launch
    // anyway, so skipping is safe during early init).
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        let cleanup_data_dir = data_dir.clone();
        let cleanup_app_name = app_name.to_string();
        runtime.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                cleanup_old_logs(&cleanup_data_dir, &cleanup_app_name);
            }
        });
    }

    LogHandle {
        inner,
        data_dir,
        app_name: app_name.to_string(),
    }
}
