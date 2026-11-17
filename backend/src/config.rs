use anyhow::{Context, Result, ensure};
use std::{env, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub listen: String,
    pub public_origin: String,
    pub upstream: Url,
    pub allowed_hosts: Vec<String>,
    pub allow_insecure_upstream: bool,
    pub secure_cookie: bool,
    pub scan_interval: Duration,
    pub demo_update_token: Option<String>,
}
impl Config {
    pub fn from_env() -> Result<Self> {
        let public_origin =
            env::var("PUBLIC_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());
        let origin = Url::parse(&public_origin).context("PUBLIC_ORIGIN must be an absolute URL")?;
        ensure!(
            origin.origin().ascii_serialization() == public_origin,
            "PUBLIC_ORIGIN must contain only scheme, host and optional port"
        );
        ensure!(
            matches!(origin.scheme(), "http" | "https"),
            "invalid public origin scheme"
        );
        let upstream = Url::parse(
            &env::var("UPSTREAM_URL").unwrap_or_else(|_| "https://pokeapi.co/api/v2/".into()),
        )?;
        let allow_insecure_upstream = env::var("ALLOW_INSECURE_UPSTREAM").as_deref() == Ok("true");
        ensure!(
            upstream.scheme() == "https" || allow_insecure_upstream,
            "HTTP upstream requires ALLOW_INSECURE_UPSTREAM=true (local mocks only)"
        );
        ensure!(
            upstream.path().ends_with('/'),
            "UPSTREAM_URL must end with /"
        );
        let allowed_hosts = env::var("UPSTREAM_ALLOWED_HOSTS")
            .unwrap_or_else(|_| "pokeapi.co,raw.githubusercontent.com".into())
            .split(',')
            .map(|s| s.trim().to_owned())
            .collect();
        let interval: u64 = env::var("SCAN_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "86400".into())
            .parse()?;
        ensure!(interval > 0, "scan interval must be positive");
        let demo_update_token = env::var("DEMO_UPDATE_TOKEN")
            .ok()
            .filter(|token| !token.is_empty());
        if let Some(token) = &demo_update_token {
            ensure!(
                token.len() >= 32 && token.bytes().all(|byte| byte.is_ascii_graphic()),
                "DEMO_UPDATE_TOKEN must contain at least 32 printable non-space ASCII characters"
            );
        }
        Ok(Self {
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            listen: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into()),
            secure_cookie: origin.scheme() == "https",
            public_origin,
            upstream,
            allowed_hosts,
            allow_insecure_upstream,
            scan_interval: Duration::from_secs(interval),
            demo_update_token,
        })
    }
}
