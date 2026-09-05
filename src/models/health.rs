use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
    pub model: String,
    pub device: &'static str,
    pub flash_attention: bool,
}
