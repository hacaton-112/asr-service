use std::sync::Arc;

use crate::{
    error::AppError, models::session::PendingSession, repositories::session::SessionRepository,
    utils::language,
};

/// Создаёт новую сессию транскрибации с провалидированным языком.
pub async fn create<R: SessionRepository>(
    repo: Arc<R>,
    language: &str,
) -> Result<String, AppError> {
    let language = language::normalize(language)?;
    Ok(repo.create(language).await)
}

/// Забирает сессию по идентификатору и проверяет, что она ещё не истекла.
/// Сессия одноразовая, поэтому «не найдена» и «истекла» — разные ошибки
/// для клиента (404 vs 410).
pub async fn take<R: SessionRepository>(
    repo: Arc<R>,
    session_id: &str,
) -> Result<PendingSession, AppError> {
    let session = repo
        .take(session_id)
        .await
        .ok_or_else(|| AppError::not_found("unknown or already used session"))?;

    if session.is_expired() {
        return Err(AppError::gone("session expired"));
    }

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::session::MockSessionRepository;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn create_rejects_invalid_language() {
        let mock_repo = MockSessionRepository::new();
        let error = create(Arc::new(mock_repo), "russian").await.unwrap_err();
        assert!(matches!(error, AppError::Api(StatusCode::BAD_REQUEST, _)));
    }

    #[tokio::test]
    async fn take_maps_missing_session_to_not_found() {
        let mut mock_repo = MockSessionRepository::new();
        mock_repo.expect_take().returning(|_| None);
        let error = take(Arc::new(mock_repo), "unknown").await.unwrap_err();
        assert!(matches!(error, AppError::Api(StatusCode::NOT_FOUND, _)));
    }
}
