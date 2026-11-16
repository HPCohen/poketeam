use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    let mut urls: Vec<String> = std::env::args().skip(1).collect();
    if urls.is_empty() {
        urls.push("https://pokeapi.co/api/v2/pokemon/4/".into());
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("poketeam/0.1 (daily local catalog cache)")
        .build()?;
    let mut data = Vec::new();
    for url in urls {
        let started = Instant::now();
        eprintln!("GET {url} (total timeout 30s, connect timeout 10s)");
        let response = client.get(&url).send().await.with_context(|| {
            format!(
                "sending {url} after {:.3}s",
                started.elapsed().as_secs_f64()
            )
        })?;
        eprintln!(
            "Headers after {:.3}s: status={}, protocol={:?}, final_url={}, content_length={:?}",
            started.elapsed().as_secs_f64(),
            response.status(),
            response.version(),
            response.url(),
            response.content_length(),
        );
        let json: Value = response
            .error_for_status()
            .with_context(|| format!("HTTP error from {url}"))?
            .json()
            .await
            .with_context(|| {
                format!(
                    "reading JSON from {url} after {:.3}s",
                    started.elapsed().as_secs_f64()
                )
            })?;
        eprintln!(
            "Complete JSON after {:.3}s: {url}",
            started.elapsed().as_secs_f64()
        );
        println!("{}", serde_json::to_string_pretty(&json)?);
        data.push(json);
    }
    eprintln!("Finished: {} JSON responses retained in memory", data.len());
    Ok(())
}
