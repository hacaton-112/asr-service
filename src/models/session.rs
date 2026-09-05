use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Сессия живёт ограниченное время между созданием (`POST /v1/sessions`)
/// и подключением по WebSocket — так по сети не гуляют вечные sessionId.
pub const SESSION_TTL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct PendingSession {
    pub language: String,
    pub expires_at: Instant,
}

impl PendingSession {
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Instant::now()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "auto".to_owned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub ws_url: String,
    pub sample_rate: usize,
    pub expires_in_seconds: u64,
    pub model: String,
}
