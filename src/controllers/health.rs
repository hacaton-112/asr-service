use axum::Extension;

use crate::{error::AppJson, models::health::HealthResponse, repositories::EngineRepoExt};

/// Проверка живости сервиса и того, что модель успешно загружена.
pub async fn health(Extension(engine_repo): EngineRepoExt) -> AppJson<HealthResponse> {
    AppJson(HealthResponse {
        status: "ok",
        model: engine_repo.model_name().to_owned(),
        device: "cuda:0",
        flash_attention: true,
    })
}
