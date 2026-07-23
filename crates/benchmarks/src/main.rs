use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::{Duration, Instant},
};

#[derive(Serialize)]
/// Latency summary emitted as JSON for automation-friendly comparison.
struct Report {
    iterations: usize,
    minimum_us: u128,
    mean_us: u128,
    p50_us: u128,
    p95_us: u128,
    maximum_us: u128,
}

fn main() -> Result<()> {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(100);
    let request = json!({
        "jsonrpc": "2.0", "id": 1, "method": "ping", "params": {}
    })
    .to_string();
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        post(&request)?;
        samples.push(started.elapsed().as_micros());
    }
    samples.sort_unstable();
    let report = Report {
        iterations,
        minimum_us: samples[0],
        mean_us: samples.iter().sum::<u128>() / iterations as u128,
        p50_us: percentile(&samples, 50),
        p95_us: percentile(&samples, 95),
        maximum_us: *samples.last().unwrap(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

/// Returns a nearest-rank sample from an already sorted slice.
fn percentile(values: &[u128], percentile: usize) -> u128 {
    values[(values.len() - 1) * percentile / 100]
}

/// Sends one minimal HTTP/1.1 request to the local benchmark target.
fn post(body: &str) -> Result<()> {
    let mut stream =
        TcpStream::connect("127.0.0.1:8000").context("server is not listening on port 8000")?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write!(
        stream,
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:8000\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )?;
    stream.flush()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let status = String::from_utf8_lossy(&response);
    if !status.starts_with("HTTP/1.1 200") {
        anyhow::bail!(
            "unexpected response: {}",
            status.lines().next().unwrap_or("empty")
        );
    }
    Ok(())
}
