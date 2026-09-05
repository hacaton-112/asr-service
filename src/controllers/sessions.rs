use axum::{Extension, Json};

use crate::{
    config::ConfigExt,
    error::{AppError, AppJson},
    models::session::{CreateSessionRequest, CreateSessionResponse, SESSION_TTL},
    repositories::{EngineRepoExt, SessionRepoExt},
    services,
    utils::audio::SAMPLE_RATE,
};

/// Создаёт одноразовую сессию транскрибации и возвращает адрес WebSocket,
/// по которому нужно подключиться, пока сессия не истекла.
pub async fn create_session(
    Extension(config): ConfigExt,
    Extension(session_repo): SessionRepoExt,
    Extension(engine_repo): EngineRepoExt,
    Json(request): Json<CreateSessionRequest>,
) -> Result<AppJson<CreateSessionResponse>, AppError> {
    let session_id = services::session::create(session_repo, &request.language).await?;

    Ok(AppJson(CreateSessionResponse {
        ws_url: format!(
            "{}/v1/ws/{session_id}",
            config.public_ws_url.trim_end_matches('/')
        ),
        session_id,
        sample_rate: SAMPLE_RATE,
        expires_in_seconds: SESSION_TTL.as_secs(),
        model: engine_repo.model_name().to_owned(),
    }))
}
