//! SQL against the telemetry store (OpenObserve's `_search`), with the same endpoint and credentials as `init`.
//!
//! ```text
//! ARENA_OTLP_ENDPOINT=http://127.0.0.1:5080/api/default ARENA_OTLP_AUTH_FILE=~/projects/game-factory/ops/arena/observe.env \
//!   cargo run -p arena-telemetry --example query -- "SELECT body, count(*) AS n FROM arena_match GROUP BY body" [--hours 24] [--type logs|traces|metrics]
//! cargo run -p arena-telemetry --example query -- --streams        # every stream, its rows and its size on disk
//! ```

use arena_telemetry::Endpoint;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let endpoint =
        Endpoint::from_env()?.ok_or("set ARENA_OTLP_ENDPOINT (and ARENA_OTLP_AUTH_FILE)")?;
    let auth = endpoint.auth.clone().unwrap_or_default();
    if args.iter().any(|a| a == "--streams") {
        let mut total = 0.0;
        for kind in ["logs", "traces", "metrics"] {
            let url = format!("{}/streams?type={kind}", endpoint.url);
            let v: Value = ureq::get(&url)
                .header("Authorization", &auth)
                .call()
                .map_err(|e| format!("{url}: {e}"))?
                .body_mut()
                .read_json()
                .map_err(|e| e.to_string())?;
            for s in v["list"].as_array().cloned().unwrap_or_default() {
                let st = &s["stats"];
                let mb = st["storage_size"].as_f64().unwrap_or(0.0);
                total += st["compressed_size"].as_f64().unwrap_or(0.0);
                println!(
                    "{kind:<8} {:<40} rows {:>10}  {:>9.2} MB raw  {:>8.2} MB on disk",
                    s["name"].as_str().unwrap_or("?"),
                    st["doc_num"].as_i64().unwrap_or(0),
                    mb,
                    st["compressed_size"].as_f64().unwrap_or(0.0)
                );
            }
        }
        println!("total on disk: {total:.2} MB");
        return Ok(());
    }
    let sql = args
        .iter()
        .find(|a| {
            !a.starts_with("--")
                && Some(*a) != flag("--hours").as_ref()
                && Some(*a) != flag("--type").as_ref()
        })
        .ok_or("give an SQL query")?;
    let hours: f64 = flag("--hours")
        .map_or(Ok(24.0), |h| h.parse())
        .map_err(|_| "--hours is a number")?;
    let kind = flag("--type").unwrap_or_else(|| "logs".to_string());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_micros() as i64;
    let body = json!({"query": {"sql": sql, "start_time": now - (hours * 3.6e9) as i64, "end_time": now, "from": 0, "size": 1000}});
    let url = format!("{}/_search?type={kind}", endpoint.url);
    let v: Value = ureq::post(&url)
        .header("Authorization", &auth)
        .send_json(&body)
        .map_err(|e| format!("{url}: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    for hit in v["hits"].as_array().cloned().unwrap_or_default() {
        println!("{hit}");
    }
    eprintln!(
        "{} rows (of {} matched) in {} ms",
        v["hits"].as_array().map_or(0, Vec::len),
        v["total"],
        v["took"]
    );
    Ok(())
}
