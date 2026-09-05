use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request},
    Extension, Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, info_span};

use crate::{
    config::Config,
    repositories::{engine::EngineRepositoryImpl, session::InMemorySessionRepositoryImpl},
    router::router,
};

/// Собирает приложение: поднимает репозитории (модель Whisper, хранилище
/// сессий) и прокидывает их в роуты через Extension-слои. Это единственное
/// место, где HTTP-слой «узнаёт» о конкретных реализациях зависимостей.
pub async fn create_app(config: Arc<Config>) -> Router {
    let engine_repository = Arc::new(
        EngineRepositoryImpl::load(&config.model_path).expect("failed to load Whisper model"),
    );
    info!(model = %engine_repository.model_name(), "Whisper model is ready");

    let session_repository = Arc::new(InMemorySessionRepositoryImpl::new());

    router()
        .layer(
            TraceLayer::new_for_http()
                // Отдельный спан на запрос с уже подставленным matched path —
                // его добавляет axum как extension после роутинга.
                .make_span_with(|req: &Request| {
                    let method = req.method();
                    let uri = req.uri();
                    let matched_path = req
                        .extensions()
                        .get::<MatchedPath>()
                        .map(|matched_path| matched_path.as_str());

                    info_span!("request", %method, %uri, matched_path)
                })
                .on_failure(()),
        )
        .layer(Extension(engine_repository))
        .layer(Extension(session_repository))
        .layer(Extension(config))
        .layer(CorsLayer::permissive())
}
