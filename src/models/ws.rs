use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientCommand {
    Stop,
    Ping,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerEvent {
    Ready {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "sampleRate")]
        sample_rate: usize,
        model: String,
    },
    Partial {
        transcript: String,
        #[serde(rename = "audioMs")]
        audio_ms: u64,
        #[serde(rename = "processingMs")]
        processing_ms: u64,
    },
    Final {
        transcript: String,
        #[serde(rename = "audioMs")]
        audio_ms: u64,
        #[serde(rename = "processingMs")]
        processing_ms: u64,
    },
    Pong,
    Error {
        message: String,
    },
}
