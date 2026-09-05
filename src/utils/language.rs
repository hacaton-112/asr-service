use crate::error::AppError;

/// Приводит локаль (`ru-RU`, `en_US`, ...) к языковому коду Whisper
/// (`ru`, `en`, ...) либо к `auto`. Отклоняет всё остальное ещё до
/// создания сессии, чтобы не тратить на это WebSocket-соединение.
pub fn normalize(language: &str) -> Result<String, AppError> {
    let normalized = language
        .split(['-', '_'])
        .next()
        .unwrap_or("auto")
        .trim()
        .to_lowercase();
    if normalized == "auto"
        || (normalized.len() == 2
            && normalized
                .chars()
                .all(|character| character.is_ascii_alphabetic()))
    {
        Ok(normalized)
    } else {
        Err(AppError::bad_request(
            "language must be an ISO-639-1 code or auto",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_locale_to_whisper_language() {
        assert_eq!(normalize("ru-RU").unwrap(), "ru");
        assert!(normalize("russian").is_err());
    }
}
