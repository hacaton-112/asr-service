use anyhow::Result;

use crate::{
    repositories::engine::TranscriptionStream,
    utils::audio::{self, SAMPLE_RATE},
};

const MIN_SAMPLES_FOR_INFERENCE: usize = SAMPLE_RATE / 10;

/// Прогоняет накопленный буфер через уже открытый поток распознавания
/// (один `WhisperState` на всю WS-сессию, см. `repositories::engine`).
///
/// Возвращает `None`, если сигнала недостаточно (тишина или слишком
/// короткий фрагмент) — в этом случае звать модель бессмысленно и дорого,
/// контроллер сам решает, что делать с пустым результатом (партиал
/// пропустить, финал — отдать пустую расшифровку).
pub async fn transcribe(
    stream: &mut dyn TranscriptionStream,
    samples: &[f32],
    language: &str,
) -> Result<Option<String>> {
    if samples.len() < MIN_SAMPLES_FOR_INFERENCE || !audio::has_audible_signal(samples) {
        return Ok(None);
    }

    let transcript = stream
        .transcribe(samples.to_vec(), language.to_owned())
        .await?;
    Ok(Some(transcript))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::engine::MockTranscriptionStream;

    #[tokio::test]
    async fn skips_inference_for_silence() {
        let mut mock_stream = MockTranscriptionStream::new();
        let samples = vec![0.0; SAMPLE_RATE];
        let result = transcribe(&mut mock_stream, &samples, "auto")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn returns_transcript_for_audible_signal() {
        let mut mock_stream = MockTranscriptionStream::new();
        mock_stream
            .expect_transcribe()
            .returning(|_, _| Ok("привет".to_owned()));

        let samples = (0..SAMPLE_RATE)
            .map(|index| if index % 2 == 0 { 0.05 } else { -0.05 })
            .collect::<Vec<_>>();
        let result = transcribe(&mut mock_stream, &samples, "ru").await.unwrap();
        assert_eq!(result.as_deref(), Some("привет"));
    }
}
