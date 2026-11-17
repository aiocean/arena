//! `init`: the subscriber a process runs under. Always a formatter to stderr (`RUST_LOG`, else the options' filter);
//! with an endpoint, an OpenTelemetry pipeline beside it: a log exporter per routed target (`ROUTES`) and one for the
//! service's own events, a tracer for the spans, a meter for the metric events, each batched on its own thread.

use crate::config::Endpoint;
use crate::{ROUTES, Route, service_stream, target, under};
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{
    LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig, WithHttpConfig,
};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{
    Aggregation, Instrument, PeriodicReader, SdkMeterProvider, Stream, Temporality,
};
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{Level, Metadata};
use tracing_subscriber::filter::{EnvFilter, FilterFn};
use tracing_subscriber::layer::{Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt;

type Boxed = Box<dyn Layer<Registry> + Send + Sync>;

/// The bucket bounds of every `*_fuel` histogram (a bot's fuel a decision; the cap is 2e7).
const FUEL: [f64; 9] = [1e3, 1e4, 1e5, 3e5, 1e6, 3e6, 1e7, 2e7, 1e8];

/// The bucket bounds of every `*_seconds` histogram.
const SECONDS: [f64; 14] = [
    1e-5, 2.5e-5, 5e-5, 1e-4, 2.5e-4, 5e-4, 1e-3, 2.5e-3, 5e-3, 1e-2, 2.5e-2, 5e-2, 0.1, 1.0,
];

/// How a process's telemetry is set up. `Options::default()` reads the endpoint from the environment.
#[derive(Clone, Debug)]
pub struct Options {
    /// The service's version (`service.version`); the caller's `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
    /// What the formatter prints when `RUST_LOG` is unset.
    pub filter: String,
    /// What of the service's own events is exported (an `EnvFilter`); the routed targets are exported whole.
    pub export_filter: String,
    /// Where to export; `None` reads `ARENA_OTLP_ENDPOINT` (`Endpoint::from_env`).
    pub endpoint: Option<Endpoint>,
    /// How often metrics are sent.
    pub metrics_every: Duration,
    /// Log a panic as an error event (and keep the previous hook).
    pub panic_hook: bool,
    /// Print to stderr at all (off for a tool whose stdout and stderr are its output).
    pub stderr: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            version: env!("CARGO_PKG_VERSION"),
            filter: "info".to_string(),
            export_filter: "info".to_string(),
            endpoint: None,
            metrics_every: Duration::from_secs(10),
            panic_hook: true,
            stderr: true,
        }
    }
}

impl Options {
    /// For a tool whose output is a table: only warnings on stderr, its events exported.
    pub fn tool(version: &'static str) -> Options {
        Options {
            version,
            filter: "warn".to_string(),
            // the machinery's own chatter (tract's CPU notes, wasmtime's compiler) stays out of the stream
            export_filter: "info,tract_linalg=warn,tract_core=warn,tract_onnx=warn,wasmtime=warn,cranelift=warn"
                .to_string(),
            ..Options::default()
        }
    }
}

/// Flushes and shuts the exporters when dropped. Keep it alive for the whole of `main`.
#[must_use = "telemetry stops when the guard drops"]
pub struct Guard {
    logs: Vec<SdkLoggerProvider>,
    traces: Option<SdkTracerProvider>,
    metrics: Option<SdkMeterProvider>,
    endpoint: Option<String>,
}

impl Guard {
    /// Whether anything is exported.
    pub fn exporting(&self) -> bool {
        !self.logs.is_empty()
    }

    /// Where it exports, without credentials.
    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    /// Send what is batched now (it is sent on drop anyway).
    pub fn flush(&self) {
        for l in &self.logs {
            let _ = l.force_flush();
        }
        if let Some(t) = &self.traces {
            let _ = t.force_flush();
        }
        if let Some(m) = &self.metrics {
            let _ = m.force_flush();
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // spans first, so their logs and metrics follow them out
        if let Some(t) = self.traces.take()
            && let Err(e) = t.shutdown()
        {
            eprintln!("telemetry: traces: {e}");
        }
        if let Some(m) = self.metrics.take()
            && let Err(e) = m.shutdown()
        {
            eprintln!("telemetry: metrics: {e}");
        }
        for l in self.logs.drain(..) {
            if let Err(e) = l.shutdown() {
                eprintln!("telemetry: logs: {e}");
            }
        }
    }
}

/// Whether an event comes from the exporting machinery itself: never exported, or it would feed itself.
fn machinery(target: &str) -> bool {
    [
        "opentelemetry",
        "reqwest",
        "hyper",
        "hyper_util",
        "h2",
        "tower",
        "rustls",
    ]
    .iter()
    .any(|p| target.starts_with(p))
}

/// Set up telemetry for service `service` (`arena-server`, `arena-ladder`, …) and install it as the global
/// subscriber. A second call in one process installs nothing and exports nothing. A bad endpoint is reported on
/// stderr and the process runs unexported.
pub fn init(service: &str, options: Options) -> Guard {
    let endpoint = match options.endpoint.clone() {
        Some(e) => Some(e),
        None => Endpoint::from_env().unwrap_or_else(|e| {
            eprintln!("telemetry: {e}; not exporting");
            None
        }),
    };
    let mut layers: Vec<Boxed> = Vec::new();
    if options.stderr {
        layers.push(stderr_layer(&options.filter));
    }
    let mut guard = Guard {
        logs: Vec::new(),
        traces: None,
        metrics: None,
        endpoint: endpoint.as_ref().map(|e| e.url.clone()),
    };
    if let Some(endpoint) = &endpoint {
        match exporting(service, &options, endpoint, &mut guard) {
            Ok(more) => layers.extend(more),
            Err(e) => {
                eprintln!("telemetry: {e}; not exporting");
                guard = Guard {
                    logs: Vec::new(),
                    traces: None,
                    metrics: None,
                    endpoint: None,
                };
            }
        }
    }
    if tracing_subscriber::registry()
        .with(layers)
        .try_init()
        .is_err()
    {
        // a subscriber is already installed (a test, a second init): this guard exports nothing
        return Guard {
            logs: Vec::new(),
            traces: None,
            metrics: None,
            endpoint: None,
        };
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
            endpoint = guard.endpoint().unwrap_or(""),
            git = crate::GIT_HASH,
            "telemetry exporting"
        );
    }
    guard
}

/// The formatter: `RUST_LOG`, else `filter`; the per-event duel stream and the metric events stay off unless asked.
fn stderr_layer(filter: &str) -> Boxed {
    let spec = std::env::var("RUST_LOG")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| filter.to_string());
    let mut spec = spec;
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

fn resource(service: &str, version: &str) -> Resource {
    Resource::builder()
        .with_service_name(service.to_string())
        .with_attributes([
            KeyValue::new("service.version", version.to_string()),
            KeyValue::new("vcs.revision", crate::GIT_HASH),
            KeyValue::new(
                "host.name",
                std::env::var("HOSTNAME")
                    .ok()
                    .or_else(|| {
                        std::process::Command::new("hostname")
                            .output()
                            .ok()
                            .and_then(|o| String::from_utf8(o.stdout).ok())
                    })
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default(),
            ),
        ])
        .build()
}

fn headers(endpoint: &Endpoint, stream: &str) -> HashMap<String, String> {
    let mut h = HashMap::new();
    if let Some(a) = &endpoint.auth {
        h.insert("Authorization".to_string(), a.clone());
    }
    // OpenObserve files OTLP logs and traces under this stream
    h.insert("stream-name".to_string(), stream.to_string());
    h
}

fn log_provider(
    endpoint: &Endpoint,
    stream: &str,
    resource: Resource,
) -> Result<SdkLoggerProvider, String> {
    let exporter = LogExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.signal("logs"))
        .with_headers(headers(endpoint, stream))
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("the log exporter: {e}"))?;
    Ok(SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build())
}

/// The exporting layers: a log bridge per route and one for the rest, the spans, the metric events.
fn exporting(
    service: &str,
    options: &Options,
    endpoint: &Endpoint,
    guard: &mut Guard,
) -> Result<Vec<Boxed>, String> {
    let mut layers: Vec<Boxed> = Vec::new();
    let own = resource(service, options.version);

    for route in ROUTES {
        let Route {
            target: prefix,
            stream,
            service: filed,
        } = route;
        let provider = log_provider(
            endpoint,
            stream,
            resource(filed.unwrap_or(service), options.version),
        )?;
        let only = FilterFn::new(move |m: &Metadata<'_>| {
            m.is_event() && under(m.target(), prefix) && *m.level() <= Level::INFO
        });
        layers.push(
            OpenTelemetryTracingBridge::new(&provider)
                .with_filter(only)
                .boxed(),
        );
        guard.logs.push(provider);
    }

    let provider = log_provider(endpoint, &service_stream(service), own.clone())?;
    let rest =
        EnvFilter::try_new(&options.export_filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let unrouted = FilterFn::new(|m: &Metadata<'_>| {
        !machinery(m.target())
            && !under(m.target(), target::METRICS)
            && crate::route_of(m.target()).is_none()
    });
    layers.push(
        OpenTelemetryTracingBridge::new(&provider)
            .with_filter(unrouted)
            .with_filter(rest)
            .boxed(),
    );
    guard.logs.push(provider);

    let spans = SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.signal("traces"))
        .with_headers(headers(endpoint, &service_stream(service)))
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("the span exporter: {e}"))?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(own.clone())
        .with_batch_exporter(spans)
        .build();
    let tracer = tracer_provider.tracer("arena");
    // spans only: an event inside a match span would be a span event, thousands to a match
    let spans_only = FilterFn::new(|m: &Metadata<'_>| {
        m.is_span() && !machinery(m.target()) && *m.level() <= Level::INFO
    });
    layers.push(
        tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(spans_only)
            .boxed(),
    );
    guard.traces = Some(tracer_provider);

    // delta: each point is its interval's own counts (and a histogram's min and max its interval's), so the store's
    // SQL sums points over any range and a restart starts no new cumulative series to stitch
    let metrics = MetricExporter::builder()
        .with_temporality(Temporality::Delta)
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint.signal("metrics"))
        .with_headers(headers(endpoint, &service_stream(service)))
        .with_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("the metric exporter: {e}"))?;
    let reader = PeriodicReader::builder(metrics)
        .with_interval(options.metrics_every)
        .build();
    let meter_provider = SdkMeterProvider::builder()
        .with_resource(own)
        .with_reader(reader)
        // a `*_seconds` histogram times a step, a think or a publish (microseconds to a second); a `*_fuel` one
        // counts a bot's fuel (a thousand to the cap and past it)
        .with_view(|i: &Instrument| {
            let bounds: &[f64] = if i.name().ends_with("_seconds") {
                &SECONDS
            } else if i.name().ends_with("_fuel") {
                &FUEL
            } else {
                return None;
            };
            Stream::builder()
                .with_aggregation(Aggregation::ExplicitBucketHistogram {
                    boundaries: bounds.to_vec(),
                    record_min_max: true,
                })
                .build()
                .ok()
        })
        .build();
    let only_metrics = FilterFn::new(|m: &Metadata<'_>| under(m.target(), target::METRICS));
    layers.push(
        tracing_opentelemetry::MetricsLayer::new(meter_provider.clone())
            .with_filter(only_metrics)
            .boxed(),
    );
    guard.metrics = Some(meter_provider);
    Ok(layers)
}
