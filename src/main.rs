mod app;
mod config;
mod controllers;
mod error;
mod models;
mod repositories;
mod router;
mod services;
mod utils;

use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "asr_service=info,tower_http=info".into()),
        )
        .init();

    let config = Arc::new(Config::init()?);
    info!(path = %config.model_path.display(), "loading Whisper model with CUDA and Flash Attention");

    let app = app::create_app(config.clone()).await;

    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port))
        .await
        .with_context(|| format!("failed to bind {}:{}", config.host, config.port))?;
    info!(address = %listener.local_addr()?, "Whisper service listening");
    axum::serve(listener, app).await?;
    Ok(())
}
