use std::{env, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use axum::Extension;

pub type ConfigExt = Extension<Arc<Config>>;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub public_ws_url: String,
    pub partial_interval: Duration,
    pub model_path: PathBuf,
}

impl Config {
    /// Читает конфигурацию из переменных окружения (обычно вместе с `.env`,
    /// который подхватывает `dotenvy` при старте).
    pub fn init() -> Result<Self> {
        let host = env::var("WHISPER_HOST").unwrap_or_else(|_| "0.0.0.0".to_owned());
        let port = env::var("WHISPER_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8787);
        let public_ws_url =
            env::var("WHISPER_PUBLIC_WS_URL").unwrap_or_else(|_| format!("ws://127.0.0.1:{port}"));
        let partial_interval = Duration::from_millis(
            env::var("WHISPER_PARTIAL_INTERVAL_MS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1_200)
                .max(500),
        );
        let model_path = resolve_model_path()?;

        Ok(Self {
            host,
            port,
            public_ws_url,
            partial_interval,
            model_path,
        })
    }
}

fn resolve_model_path() -> Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("WHISPER_MODEL_PATH") {
        candidates.push(PathBuf::from(path));
    }
    candidates.push(PathBuf::from("models/ggml-large-v3-turbo.bin"));
    if let Some(app_data) = env::var_os("APPDATA") {
        candidates.push(
            PathBuf::from(app_data)
                .join("com.rizo.trainer-client")
                .join("whisper-models")
                .join("ggml-large-v3-turbo.bin"),
        );
    }

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Whisper model not found; set WHISPER_MODEL_PATH to ggml-large-v3-turbo.bin"
            )
        })
}
