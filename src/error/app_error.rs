use axum::{
    extract::FromRequest,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// Обёртка над `axum::Json`: ошибки парсинга тела запроса (rejection)
/// тоже уходят клиенту в едином формате `{"message": ...}`, а не как
/// обычный текст, который отдаёт `axum::Json` по умолчанию.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct AppJson<T>(pub T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// Ошибка уровня приложения.
///
/// `Api` — ожидаемая ситуация (невалидный язык, неизвестная или истёкшая
/// сессия): код и текст безопасно показать клиенту напрямую.
/// `Internal` — всё остальное (сбой модели, паника драйвера и т.п.):
/// наружу уходит общий текст, а подробности — только в лог, чтобы не
/// раскрывать внутреннее устройство сервиса.
#[derive(Debug)]
pub enum AppError {
    Api(StatusCode, String),
    Internal(anyhow::Error),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::Api(StatusCode::BAD_REQUEST, message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::Api(StatusCode::NOT_FOUND, message.into())
    }

    pub fn gone(message: impl Into<String>) -> Self {
        Self::Api(StatusCode::GONE, message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            AppError::Api(status, message) => (status, message),
            AppError::Internal(error) => {
                tracing::error!(%error, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong".to_owned(),
                )
            }
        };

        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

// Позволяет использовать `?` в функциях, возвращающих `anyhow::Result`,
// автоматически превращая ошибку в `AppError::Internal`.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self::Internal(error.into())
    }
}
