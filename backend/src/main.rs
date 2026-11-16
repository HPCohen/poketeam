use anyhow::{Result, bail};
use poketeam::{
    AppState, SystemClock,
    config::Config,
    http::{self, ApiDoc},
    sync::{self, PokeApi},
};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use utoipa::OpenApi;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "poketeam=info,tower_http=info".into()),
        )
        .init();
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if command == "openapi" {
        println!("{}", ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }
    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;
    let state = AppState {
        pool,
        config,
        clock: Arc::new(SystemClock),
    };
    match command.as_str() {
        "migrate" => sqlx::migrate!().run(&state.pool).await?,
        "serve" => {
            let listener = tokio::net::TcpListener::bind(&state.config.listen).await?;
            tracing::info!(address=%state.config.listen,"API listening");
            axum::serve(listener, http::router(state.clone()))
                .with_graceful_shutdown(http::shutdown_signal())
                .await?;
        }
        "sync" => {
            let outcome = sync::run(&state, &PokeApi::new(state.config.clone())?).await?;
            println!("{}", serde_json::to_string(&outcome)?);
        }
        "worker" => sync::worker(state.clone(), &PokeApi::new(state.config.clone())?).await?,
        _ => bail!("Usage: poketeam [serve|worker|sync|migrate|openapi]"),
    }
    state.pool.close().await;
    Ok(())
}
