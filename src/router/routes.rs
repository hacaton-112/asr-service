use axum::{
    routing::{get, post},
    Router,
};

use crate::controllers::{health, sessions, ws};

/// Сборка маршрутов без привязки к конкретным зависимостям — их прокидывает
/// `app::create_app` через Extension-слои поверх этого роутера.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/sessions", post(sessions::create_session))
        .route("/v1/ws/{session_id}", get(ws::websocket))
}
