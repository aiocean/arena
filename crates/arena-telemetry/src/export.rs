//! `init`: the subscriber a process runs under. Always a formatter to stderr (`RUST_LOG`, else the options' filter);
//! with a directory, a layer beside it that turns every exported event, finished span and metric event into a record
//! and hands it to a writer thread over a bounded channel. The tick only builds the record: when the channel is
//! full the record is dropped and counted (`Guard::dropped`, and a `telemetry.dropped` warning in the file), never
//! waited for. The writer appends JSON lines to `<dir>/<service>/<day>.jsonl` (`crate::files`) once a second, sums
//! the metric events into one line a series every `metrics_every`, and applies the retention at start and hourly.
//! The guard's drop writes what is left.
//!
//! A line (`crate::read` reads them back; `ops/telemetry/README.md` lists the fields):
//!
//! ```text
//! {"ts":"2026-11-18T10:00:00.123456Z","kind":"log","service":"arena-server","name":"match.start","level":"INFO",
//!  "target":"arena::match","trace_id":"…","span_id":"…","attrs":{"match_id":"…","seat":0,…}}
//! {"ts":…,"kind":"span","name":"match",…,"start":"…","end":"…","duration_ms":81234.5,"attrs":{…}}
//! {"ts":…,"kind":"metric","name":"arena_step_seconds","type":"histogram","interval_s":10.0,"count":600,"sum":0.03,
//!  "min":…,"max":…,"zero":0,"buckets":[[-104,12],…],"attrs":{}}
//! {"ts":…,"kind":"metric","name":"arena_casts_total","type":"counter","interval_s":10.0,"value":3,"attrs":{"skill":"…"}}
//! ```

use crate::files::{self, Retention};
use crate::hist::Hist;
use crate::time;
use crate::{target, under};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record as Values};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::filter::{EnvFilter, FilterExt, FilterFn};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::{LookupSpan, Registry};
use tracing_subscriber::util::SubscriberInitExt;

type Boxed = Box<dyn Layer<Registry> + Send + Sync>;

/// How a process's telemetry is set up. `Options::default()` reads the directory from `ARENA_TELEMETRY_DIR`.
#[derive(Clone, Debug)]
pub struct Options {
    /// The service's version, on the `telemetry.start` line; the caller's `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
    /// What the formatter prints when `RUST_LOG` is unset.
    pub filter: String,
    /// What of the service's own events and spans is written (an `EnvFilter`); the routed targets are written whole.
    pub export_filter: String,
    /// Where to write; `None` reads `ARENA_TELEMETRY_DIR` (`files::env_dir`). Neither: nothing is written.
    pub dir: Option<PathBuf>,
    /// How often the metric events are summed into lines.
    pub metrics_every: Duration,
    /// Log a panic as an error event (and keep the previous hook).
    pub panic_hook: bool,
    /// Print to stderr at all (off for a tool whose stdout and stderr are its output).
    pub stderr: bool,
    /// How long the files stay.
    pub retention: Retention,
    /// The records the channel holds before it drops them.
    pub capacity: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            version: env!("CARGO_PKG_VERSION"),
            filter: "info".to_string(),
            export_filter: "info".to_string(),
            dir: None,
            metrics_every: Duration::from_secs(10),
            panic_hook: true,
            stderr: true,
            retention: Retention::default(),
            capacity: 1 << 16,
        }
    }
}

impl Options {
    /// For a tool whose output is a table: only warnings on stderr, its events written.
    pub fn tool(version: &'static str) -> Options {
        Options {
            version,
            filter: "warn".to_string(),
            // the machinery's own chatter (tract's CPU notes, wasmtime's compiler) stays out of the files
            export_filter:
                "info,tract_linalg=warn,tract_core=warn,tract_onnx=warn,wasmtime=warn,cranelift=warn"
                    .to_string(),
            ..Options::default()
        }
    }
}

/// Writes what is left and stops the writer when dropped. Keep it alive for the whole of `main`.
#[must_use = "telemetry stops when the guard drops"]
pub struct Guard {
    tx: Option<SyncSender<Msg>>,
    writer: Option<JoinHandle<()>>,
    dropped: Arc<AtomicU64>,
    dir: Option<PathBuf>,
}

impl Guard {
    fn off() -> Guard {
        Guard {
            tx: None,
            writer: None,
            dropped: Arc::new(AtomicU64::new(0)),
            dir: None,
        }
    }

    /// Whether anything is written.
    pub fn exporting(&self) -> bool {
        self.tx.is_some()
    }

    /// The directory written to.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The records dropped so far because the channel was full.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Write everything sent so far, the metrics summed to now, and wait for it (at most five seconds).
    pub fn flush(&self) {
        if let Some(tx) = &self.tx {
            let (ack, done) = mpsc::channel();
            if tx.send(Msg::Flush(ack)).is_ok() {
                let _ = done.recv_timeout(Duration::from_secs(5));
            }
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(w) = self.writer.take()
            && w.join().is_err()
        {
            eprintln!("telemetry: the writer panicked");
        }
        let dropped = self.dropped();
        if dropped > 0 {
            eprintln!("telemetry: {dropped} records dropped (the channel was full)");
        }
    }
}

/// Whether an event comes from the machinery under the arena (the QUIC and TLS stacks): never written.
fn machinery(target: &str) -> bool {
    ["rustls", "quinn", "h2", "hyper", "tower", "web_transport"]
        .iter()
        .any(|p| target.starts_with(p))
}

/// Set up telemetry for service `service` (`arena-server`, `arena-ladder`, …) and install it as the global
/// subscriber. A second call in one process installs nothing and writes nothing. A directory that cannot be made is
/// reported on stderr and the process runs unexported.
pub fn init(service: &str, options: Options) -> Guard {
    let dir = options.dir.clone().or_else(files::env_dir);
    let mut layers: Vec<Boxed> = Vec::new();
    if options.stderr {
        layers.push(stderr_layer(&options.filter));
    }
    let mut guard = Guard::off();
    if let Some(dir) = &dir {
        match std::fs::create_dir_all(dir) {
            Ok(()) => {
                let (layer, g) = layer(service, &options, dir);
                layers.push(layer);
                guard = g;
            }
            Err(e) => eprintln!("telemetry: {}: {e}; not exporting", dir.display()),
        }
    }
    if tracing_subscriber::registry()
        .with(layers)
        .try_init()
        .is_err()
    {
        // a subscriber is already installed (a test, a second init): this guard writes nothing
        return Guard::off();
    }
    if options.panic_hook {
        let before = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let at = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default();
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            tracing::error!(name: "panic", reason = %message, location = %at, "panic");
            before(info);
        }));
    }
    if guard.exporting() {
        tracing::info!(
            dir = %guard.dir().map(Path::display).map(|d| d.to_string()).unwrap_or_default(),
            version = options.version,
            git = crate::GIT_HASH,
            host = %host(),
            pid = std::process::id(),
            "telemetry.start"
        );
    }
    guard
}

fn host() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// The formatter: `RUST_LOG`, else `filter`; the per-event duel stream and the metric events stay off unless asked.
fn stderr_layer(filter: &str) -> Boxed {
    let mut spec = std::env::var("RUST_LOG")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| filter.to_string());
    for quiet in [target::DUEL, target::METRICS] {
        if !spec.contains(quiet) {
            spec.push_str(&format!(",{quiet}=off"));
        }
    }
    let env = EnvFilter::try_new(&spec).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(env)
        .boxed()
}

/// The file layer and its guard, the writer started, not installed (`init` installs it; a test runs it under
/// `tracing::subscriber::with_default`). What it takes: the routed targets and the metric events whole (info and
/// above), the rest through `export_filter`.
pub fn layer(service: &str, options: &Options, dir: &Path) -> (Boxed, Guard) {
    let (tx, rx) = mpsc::sync_channel(options.capacity.max(1));
    let dropped = Arc::new(AtomicU64::new(0));
    // one service a process: its name lives as long as the process
    let service: &'static str = Box::leak(service.to_string().into_boxed_str());
    let writer = Writer {
        dir: dir.to_path_buf(),
        service,
        retention: options.retention,
        metrics_every: options.metrics_every,
        dropped: Arc::clone(&dropped),
        reported: 0,
        pending: BTreeMap::new(),
        pending_bytes: 0,
        series: HashMap::new(),
        interval_start: Instant::now(),
    };
    let spawned = std::thread::Builder::new()
        .name("arena-telemetry".into())
        .spawn(move || writer.run(&rx));
    let file = FileLayer {
        service,
        sink: Sink {
            tx: tx.clone(),
            dropped: Arc::clone(&dropped),
        },
    };
    let routed = FilterFn::new(|m: &Metadata<'_>| {
        (crate::route_of(m.target()).is_some() || under(m.target(), target::METRICS))
            && *m.level() <= Level::INFO
    });
    let rest =
        EnvFilter::try_new(&options.export_filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let own = FilterFn::new(|m: &Metadata<'_>| {
        !machinery(m.target())
            && crate::route_of(m.target()).is_none()
            && !under(m.target(), target::METRICS)
    });
    let boxed = file.with_filter(routed.or(own.and(rest))).boxed();
    match spawned {
        Ok(handle) => (
            boxed,
            Guard {
                tx: Some(tx),
                writer: Some(handle),
                dropped,
                dir: Some(dir.to_path_buf()),
            },
        ),
        Err(e) => {
            eprintln!("telemetry: the writer did not start: {e}; not exporting");
            (
                Box::new(tracing_subscriber::layer::Identity::new()),
                Guard::off(),
            )
        }
    }
}

// ---- what crosses the channel ----

/// A field's value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Val {
    I(i64),
    U(u64),
    F(f64),
    B(bool),
    S(String),
}

impl Val {
    fn write(&self, out: &mut String) {
        match self {
            Val::I(v) => {
                let _ = write!(out, "{v}");
            }
            Val::U(v) => {
                let _ = write!(out, "{v}");
            }
            Val::F(v) => number(out, *v),
            Val::B(v) => out.push_str(if *v { "true" } else { "false" }),
            Val::S(s) => string(out, s),
        }
    }

    fn as_f64(&self) -> Option<f64> {
        match self {
            Val::I(v) => Some(*v as f64),
            Val::U(v) => Some(*v as f64),
            Val::F(v) => Some(*v),
            _ => None,
        }
    }
}

type Attrs = Vec<(&'static str, Val)>;

fn set(attrs: &mut Attrs, key: &'static str, v: Val) {
    match attrs.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = v,
        None => attrs.push((key, v)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Log,
    Span,
}

/// A log record or a finished span.
#[derive(Debug)]
struct Record {
    kind: Kind,
    ts_us: i64,
    service: &'static str,
    name: String,
    level: &'static str,
    target: &'static str,
    trace_id: Option<u128>,
    span_id: Option<u64>,
    parent_id: Option<u64>,
    /// A span's start (its end is `ts_us`).
    start_us: Option<i64>,
    attrs: Attrs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum MetricType {
    Counter,
    Histogram,
}

/// One measurement of a metric event.
#[derive(Debug)]
struct Metric {
    service: &'static str,
    name: &'static str,
    kind: MetricType,
    value: f64,
    attrs: Attrs,
}

#[derive(Debug)]
enum Msg {
    Record(Record),
    Metric(Metric),
    Flush(mpsc::Sender<()>),
    Stop,
}

/// The layer's end of the channel: a full channel drops the record and counts it.
struct Sink {
    tx: SyncSender<Msg>,
    dropped: Arc<AtomicU64>,
}

impl Sink {
    fn send(&self, m: Msg) {
        match self.tx.try_send(m) {
            Ok(()) | Err(TrySendError::Disconnected(_)) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

// ---- the layer ----

struct FileLayer {
    service: &'static str,
    sink: Sink,
}

/// What a span keeps until it closes.
struct SpanData {
    trace_id: u128,
    span_id: u64,
    parent_id: Option<u64>,
    start_us: i64,
    attrs: Attrs,
}

/// A fresh random id (splitmix64 over a process-wide counter seeded from the clock and the pid).
fn next_id() -> u64 {
    static SEED: OnceLock<u64> = OnceLock::new();
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let seed =
        *SEED.get_or_init(|| (time::now_us() as u64) ^ (u64::from(std::process::id()) << 40));
    let mut z = seed.wrapping_add(
        COUNT
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15),
    );
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)).max(1)
}

struct Fields<'a> {
    attrs: &'a mut Attrs,
    message: Option<String>,
}

impl Visit for Fields<'_> {
    fn record_f64(&mut self, f: &Field, v: f64) {
        set(self.attrs, f.name(), Val::F(v));
    }
    fn record_i64(&mut self, f: &Field, v: i64) {
        set(self.attrs, f.name(), Val::I(v));
    }
    fn record_u64(&mut self, f: &Field, v: u64) {
        set(self.attrs, f.name(), Val::U(v));
    }
    fn record_bool(&mut self, f: &Field, v: bool) {
        set(self.attrs, f.name(), Val::B(v));
    }
    fn record_str(&mut self, f: &Field, v: &str) {
        if f.name() == "message" {
            self.message = Some(v.to_string());
        } else {
            set(self.attrs, f.name(), Val::S(v.to_string()));
        }
    }
    fn record_error(&mut self, f: &Field, v: &(dyn std::error::Error + 'static)) {
        set(self.attrs, f.name(), Val::S(v.to_string()));
    }
    fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
        if f.name() == "message" {
            self.message = Some(format!("{v:?}"));
        } else {
            set(self.attrs, f.name(), Val::S(format!("{v:?}")));
        }
    }
}

/// A metric event's fields: its measurements (`monotonic_counter.*`, `counter.*`, `histogram.*`) and the rest.
#[derive(Default)]
struct MetricFields {
    measures: Vec<(&'static str, MetricType, f64)>,
    attrs: Attrs,
}

impl MetricFields {
    fn put(&mut self, name: &'static str, v: Val) {
        for (prefix, kind) in [
            ("monotonic_counter.", MetricType::Counter),
            ("counter.", MetricType::Counter),
            ("histogram.", MetricType::Histogram),
        ] {
            if let Some(metric) = name.strip_prefix(prefix) {
                if let Some(x) = v.as_f64() {
                    self.measures.push((metric, kind, x));
                }
                return;
            }
        }
        if name != "message" {
            set(&mut self.attrs, name, v);
        }
    }
}

impl Visit for MetricFields {
    fn record_f64(&mut self, f: &Field, v: f64) {
        self.put(f.name(), Val::F(v));
    }
    fn record_i64(&mut self, f: &Field, v: i64) {
        self.put(f.name(), Val::I(v));
    }
    fn record_u64(&mut self, f: &Field, v: u64) {
        self.put(f.name(), Val::U(v));
    }
    fn record_bool(&mut self, f: &Field, v: bool) {
        self.put(f.name(), Val::B(v));
    }
    fn record_str(&mut self, f: &Field, v: &str) {
        self.put(f.name(), Val::S(v.to_string()));
    }
    fn record_debug(&mut self, f: &Field, v: &dyn std::fmt::Debug) {
        self.put(f.name(), Val::S(format!("{v:?}")));
    }
}

fn level_name(l: &Level) -> &'static str {
    match *l {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    }
}

impl FileLayer {
    /// The service a target's records are filed under: its route's (the bot host's are `arena-bot`'s), else ours.
    fn service_of(&self, target: &str) -> &'static str {
        crate::route_of(target)
            .and_then(|r| r.service)
            .unwrap_or(self.service)
    }
}

impl<S> Layer<S> for FileLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let parent = span.parent().and_then(|p| {
            p.extensions()
                .get::<SpanData>()
                .map(|d| (d.trace_id, d.span_id))
        });
        let mut data = SpanData {
            trace_id: parent.map_or_else(
                || (u128::from(next_id()) << 64) | u128::from(next_id()),
                |p| p.0,
            ),
            span_id: next_id(),
            parent_id: parent.map(|p| p.1),
            start_us: time::now_us(),
            attrs: Vec::new(),
        };
        attrs.record(&mut Fields {
            attrs: &mut data.attrs,
            message: None,
        });
        span.extensions_mut().insert(data);
    }

    fn on_record(&self, id: &Id, values: &Values<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        if let Some(data) = ext.get_mut::<SpanData>() {
            values.record(&mut Fields {
                attrs: &mut data.attrs,
                message: None,
            });
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();
        if under(meta.target(), target::METRICS) {
            let mut f = MetricFields::default();
            event.record(&mut f);
            let n = f.measures.len();
            for (i, (name, kind, value)) in f.measures.into_iter().enumerate() {
                let attrs = if i + 1 == n {
                    std::mem::take(&mut f.attrs)
                } else {
                    f.attrs.clone()
                };
                self.sink.send(Msg::Metric(Metric {
                    service: self.service,
                    name,
                    kind,
                    value,
                    attrs,
                }));
            }
            return;
        }
        let mut attrs = Vec::new();
        let mut f = Fields {
            attrs: &mut attrs,
            message: None,
        };
        event.record(&mut f);
        let name = f.message.take().unwrap_or_else(|| meta.name().to_string());
        let ids = ctx.event_span(event).and_then(|s| {
            s.extensions()
                .get::<SpanData>()
                .map(|d| (d.trace_id, d.span_id))
        });
        self.sink.send(Msg::Record(Record {
            kind: Kind::Log,
            ts_us: time::now_us(),
            service: self.service_of(meta.target()),
            name,
            level: level_name(meta.level()),
            target: meta.target(),
            trace_id: ids.map(|i| i.0),
            span_id: ids.map(|i| i.1),
            parent_id: None,
            start_us: None,
            attrs,
        }));
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let Some(data) = span.extensions_mut().remove::<SpanData>() else {
            return;
        };
        let meta = span.metadata();
        self.sink.send(Msg::Record(Record {
            kind: Kind::Span,
            ts_us: time::now_us(),
            service: self.service_of(meta.target()),
            name: meta.name().to_string(),
            level: level_name(meta.level()),
            target: meta.target(),
            trace_id: Some(data.trace_id),
            span_id: Some(data.span_id),
            parent_id: data.parent_id,
            start_us: Some(data.start_us),
            attrs: data.attrs,
        }));
    }
}

// ---- the lines ----

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A number as JSON: a finite float as itself (an integral one with no fraction), NaN and the infinities as null.
fn number(out: &mut String, v: f64) {
    if v.is_finite() {
        let _ = write!(out, "{v}");
    } else {
        out.push_str("null");
    }
}

fn key(out: &mut String, k: &str) {
    out.push(',');
    string(out, k);
    out.push(':');
}

/// The head every line begins with: `{"ts":…,"kind":…,"service":…,"name":…`, `ts` first and always 27 characters,
/// so a reader finds a line's time without parsing it.
fn head(out: &mut String, ts_us: i64, kind: &str, service: &str, name: &str) {
    out.push_str("{\"ts\":\"");
    out.push_str(&time::format_ts(ts_us));
    out.push('"');
    key(out, "kind");
    string(out, kind);
    key(out, "service");
    string(out, service);
    key(out, "name");
    string(out, name);
}

fn attrs(out: &mut String, attrs: &Attrs) {
    key(out, "attrs");
    out.push('{');
    for (i, (k, v)) in attrs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(out, k);
        out.push(':');
        v.write(out);
    }
    out.push('}');
}

fn record_line(r: &Record) -> String {
    let mut out = String::with_capacity(256);
    let kind = match r.kind {
        Kind::Log => "log",
        Kind::Span => "span",
    };
    head(&mut out, r.ts_us, kind, r.service, &r.name);
    key(&mut out, "level");
    string(&mut out, r.level);
    key(&mut out, "target");
    string(&mut out, r.target);
    if let Some(t) = r.trace_id {
        key(&mut out, "trace_id");
        string(&mut out, &format!("{t:032x}"));
    }
    if let Some(s) = r.span_id {
        key(&mut out, "span_id");
        string(&mut out, &format!("{s:016x}"));
    }
    if let Some(p) = r.parent_id {
        key(&mut out, "parent_id");
        string(&mut out, &format!("{p:016x}"));
    }
    if let Some(start) = r.start_us {
        key(&mut out, "start");
        string(&mut out, &time::format_ts(start));
        key(&mut out, "end");
        string(&mut out, &time::format_ts(r.ts_us));
        key(&mut out, "duration_ms");
        number(&mut out, (r.ts_us - start) as f64 / 1e3);
    }
    attrs(&mut out, &r.attrs);
    out.push('}');
    out
}

/// One metric series over an interval.
struct Series {
    service: &'static str,
    name: &'static str,
    kind: MetricType,
    attrs: Attrs,
    sum: f64,
    hist: Hist,
}

fn series_line(s: &Series, ts_us: i64, interval_s: f64) -> String {
    let mut out = String::with_capacity(256);
    head(&mut out, ts_us, "metric", s.service, s.name);
    key(&mut out, "type");
    match s.kind {
        MetricType::Counter => {
            string(&mut out, "counter");
            key(&mut out, "interval_s");
            number(&mut out, interval_s);
            key(&mut out, "value");
            number(&mut out, s.sum);
        }
        MetricType::Histogram => {
            let h = &s.hist;
            string(&mut out, "histogram");
            key(&mut out, "interval_s");
            number(&mut out, interval_s);
            key(&mut out, "count");
            let _ = write!(out, "{}", h.count);
            key(&mut out, "sum");
            number(&mut out, h.sum);
            key(&mut out, "min");
            number(&mut out, h.min);
            key(&mut out, "max");
            number(&mut out, h.max);
            key(&mut out, "zero");
            let _ = write!(out, "{}", h.zero);
            key(&mut out, "buckets");
            out.push('[');
            for (i, (b, n)) in h.buckets.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "[{b},{n}]");
            }
            out.push(']');
        }
    }
    attrs(&mut out, &s.attrs);
    out.push('}');
    out
}

/// A series' key: its service, name, type and attributes as written.
fn series_key(m: &Metric) -> String {
    let mut k = String::new();
    k.push_str(m.service);
    k.push('\0');
    k.push_str(m.name);
    k.push('\0');
    k.push(if m.kind == MetricType::Counter {
        'c'
    } else {
        'h'
    });
    attrs(&mut k, &m.attrs);
    k
}

// ---- the writer thread ----

/// Lines are written at least this often.
const WRITE_EVERY: Duration = Duration::from_secs(1);
/// Or sooner, when this much waits.
const WRITE_BYTES: usize = 1 << 20;
/// The retention is applied this often (and at start).
const RETAIN_EVERY: Duration = Duration::from_secs(3600);

struct Writer {
    dir: PathBuf,
    service: &'static str,
    retention: Retention,
    metrics_every: Duration,
    dropped: Arc<AtomicU64>,
    /// The drops already written as `telemetry.dropped`.
    reported: u64,
    /// The lines waiting, by service and day.
    pending: BTreeMap<(&'static str, String), String>,
    pending_bytes: usize,
    series: HashMap<String, Series>,
    interval_start: Instant,
}

impl Writer {
    fn run(mut self, rx: &Receiver<Msg>) {
        self.retain();
        let mut next_write = Instant::now() + WRITE_EVERY;
        let mut next_metrics = Instant::now() + self.metrics_every;
        let mut next_retain = Instant::now() + RETAIN_EVERY;
        loop {
            let now = Instant::now();
            let wait = next_write
                .min(next_metrics)
                .min(next_retain)
                .saturating_duration_since(now);
            match rx.recv_timeout(wait) {
                Ok(Msg::Record(r)) => self.push(r.service, r.ts_us, record_line(&r)),
                Ok(Msg::Metric(m)) => self.measure(m),
                Ok(Msg::Flush(ack)) => {
                    self.drain(rx);
                    self.metrics();
                    self.write();
                    let _ = ack.send(());
                }
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => {
                    self.drain(rx);
                    self.metrics();
                    self.write();
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            let now = Instant::now();
            if now >= next_metrics {
                self.metrics();
                next_metrics = now + self.metrics_every;
            }
            if now >= next_write || self.pending_bytes >= WRITE_BYTES {
                self.write();
                next_write = now + WRITE_EVERY;
            }
            if now >= next_retain {
                self.write();
                self.retain();
                next_retain = now + RETAIN_EVERY;
            }
        }
    }

    /// What is already in the channel, taken without waiting (a flush writes everything sent before it).
    fn drain(&mut self, rx: &Receiver<Msg>) {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Record(r) => self.push(r.service, r.ts_us, record_line(&r)),
                Msg::Metric(m) => self.measure(m),
                Msg::Flush(ack) => {
                    let _ = ack.send(());
                }
                Msg::Stop => {}
            }
        }
    }

    fn push(&mut self, service: &'static str, ts_us: i64, line: String) {
        let buf = self.pending.entry((service, time::day(ts_us))).or_default();
        buf.push_str(&line);
        buf.push('\n');
        self.pending_bytes += line.len() + 1;
    }

    fn measure(&mut self, m: Metric) {
        let k = series_key(&m);
        let s = self.series.entry(k).or_insert_with(|| Series {
            service: m.service,
            name: m.name,
            kind: m.kind,
            attrs: m.attrs,
            sum: 0.0,
            hist: Hist::default(),
        });
        match s.kind {
            MetricType::Counter => s.sum += m.value,
            MetricType::Histogram => s.hist.add(m.value),
        }
    }

    /// Each series of the interval as a line, and the drops since the last report as a warning.
    fn metrics(&mut self) {
        let now = time::now_us();
        let interval_s = self.interval_start.elapsed().as_secs_f64();
        self.interval_start = Instant::now();
        let mut series: Vec<Series> = self.series.drain().map(|(_, s)| s).collect();
        series.sort_by(|a, b| (a.service, a.name).cmp(&(b.service, b.name)));
        for s in series {
            let line = series_line(&s, now, interval_s);
            self.push(s.service, now, line);
        }
        let dropped = self.dropped.load(Ordering::Relaxed);
        if dropped > self.reported {
            let r = Record {
                kind: Kind::Log,
                ts_us: now,
                service: self.service,
                name: "telemetry.dropped".to_string(),
                level: "WARN",
                target: "arena_telemetry",
                trace_id: None,
                span_id: None,
                parent_id: None,
                start_us: None,
                attrs: vec![
                    ("dropped", Val::U(dropped - self.reported)),
                    ("dropped_total", Val::U(dropped)),
                ],
            };
            self.reported = dropped;
            self.push(r.service, now, record_line(&r));
        }
    }

    /// Append what waits, one write a file of whole lines (so two processes appending to one file never split a
    /// line), the file opened and closed each time (so a file the retention deleted is made again).
    fn write(&mut self) {
        for ((service, day), buf) in std::mem::take(&mut self.pending) {
            let path = files::path(&self.dir, service, &day);
            let res = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)
                })
                .and_then(|mut f| f.write_all(buf.as_bytes()));
            if let Err(e) = res {
                eprintln!("telemetry: {}: {e}", path.display());
            }
        }
        self.pending_bytes = 0;
    }

    fn retain(&self) {
        if let Err(e) = self.retention.apply(&self.dir, time::now_us()) {
            eprintln!("telemetry: retention in {}: {e}", self.dir.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_channel_drops_and_counts() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let sink = Sink {
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
        };
        for _ in 0..3 {
            sink.send(Msg::Stop);
        }
        assert_eq!(sink.dropped.load(Ordering::Relaxed), 2);
        // a writer gone is not a drop
        let (tx, rx) = mpsc::sync_channel(1);
        drop(rx);
        let sink = Sink {
            tx,
            dropped: Arc::new(AtomicU64::new(0)),
        };
        sink.send(Msg::Stop);
        assert_eq!(sink.dropped.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_line_is_json_with_its_ts_first() -> Result<(), String> {
        let r = Record {
            kind: Kind::Span,
            ts_us: 1_794_942_538_123_456,
            service: "arena-server",
            name: "match \"x\"\n".into(),
            level: "INFO",
            target: "arena::match",
            trace_id: Some(5),
            span_id: Some(6),
            parent_id: None,
            start_us: Some(1_794_942_538_000_000),
            attrs: vec![
                ("seed", Val::U(7)),
                ("nan", Val::F(f64::NAN)),
                ("ok", Val::B(true)),
                ("x", Val::F(1.5)),
            ],
        };
        let line = record_line(&r);
        assert!(
            line.starts_with("{\"ts\":\"2026-11-17T19:08:58.123456Z\""),
            "{line}"
        );
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        assert_eq!(v["name"], "match \"x\"\n");
        assert_eq!(v["duration_ms"], 123.456);
        assert_eq!(v["attrs"]["seed"], 7);
        assert!(v["attrs"]["nan"].is_null());
        assert_eq!(v["trace_id"].as_str().map(str::len), Some(32));
        Ok(())
    }
}
