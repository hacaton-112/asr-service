use std::{path::Path, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use mockall::automock;
use tokio::sync::Semaphore;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

/// Абстракция над движком распознавания речи. Вынесена в трейт, чтобы
/// слой services можно было тестировать с мок-реализацией, без реальной
/// модели и GPU.
#[automock]
pub trait EngineRepository: Send + Sync {
    /// Заводит поток декодирования на одну WS-сессию: под капотом это
    /// собственный `WhisperState` (KV-кэш + compute-буферы), который
    /// нужно создать один раз и переиспользовать на все partial/final
    /// в рамках сессии — иначе каждый вызов транскрибации заново
    /// аллоцирует и тут же освобождает эти буферы, а CUDA/ggml-аллокатор
    /// не спешит отдавать освобождённую память обратно, из-за чего
    /// потребление VRAM/RAM только растёт со временем.
    fn create_stream(&self) -> Result<Box<dyn TranscriptionStream>>;
}

/// Один поток распознавания — держит состояние декодера на протяжении
/// всей WS-сессии.
#[automock]
#[async_trait]
pub trait TranscriptionStream: Send {
    async fn transcribe(&mut self, samples: Vec<f32>, language: String) -> Result<String>;
}

pub struct EngineRepositoryImpl {
    context: Arc<WhisperContext>,
    model_name: String,
    // whisper.cpp не рассчитан на параллельный инференс на одном контексте,
    // поэтому семафор на одно разрешение сериализует вызовы модели между
    // всеми активными сессиями (каждая из них — свой WhisperState, но общий ctx).
    inference_gate: Arc<Semaphore>,
}

impl EngineRepositoryImpl {
    pub fn load(model_path: &Path) -> Result<Self> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(true).flash_attn(true).gpu_device(0);

        let context =
            WhisperContext::new_with_params(model_path.to_string_lossy().as_ref(), params)
                .map_err(|error| anyhow::anyhow!("failed to load Whisper model: {error}"))?;

        let model_name = model_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("whisper")
            .to_owned();

        Ok(Self {
            context: Arc::new(context),
            model_name,
            inference_gate: Arc::new(Semaphore::new(1)),
        })
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

impl EngineRepository for EngineRepositoryImpl {
    fn create_stream(&self) -> Result<Box<dyn TranscriptionStream>> {
        let state = self
            .context
            .create_state()
            .map_err(|error| anyhow::anyhow!("failed to create Whisper state: {error}"))?;

        Ok(Box::new(WhisperTranscriptionStream {
            state: Some(state),
            inference_gate: self.inference_gate.clone(),
        }))
    }
}

struct WhisperTranscriptionStream {
    // `Option`, чтобы можно было временно забрать состояние во blocking-задачу
    // (spawn_blocking требует владения) и вернуть обратно после инференса.
    state: Option<WhisperState>,
    inference_gate: Arc<Semaphore>,
}

#[async_trait]
impl TranscriptionStream for WhisperTranscriptionStream {
    async fn transcribe(&mut self, samples: Vec<f32>, language: String) -> Result<String> {
        let permit = self.inference_gate.clone().acquire_owned().await?;
        let mut state = self
            .state
            .take()
            .expect("whisper state is only absent while a transcribe call is in flight");

        let (state, transcript) = tokio::task::spawn_blocking(move || {
            let _permit = permit;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_n_threads(6);
            params.set_language(Some(&language));
            params.set_translate(false);
            params.set_no_context(true);
            params.set_no_timestamps(true);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            // Единственный детерминированный проход держит задержку партиалов предсказуемой.
            params.set_temperature_inc(0.0);

            state
                .full(params, &samples)
                .map_err(|error| anyhow::anyhow!("Whisper inference failed: {error}"))?;

            let transcript = state
                .as_iter()
                .map(|segment| segment.to_string())
                .collect::<String>()
                .trim()
                .to_owned();
            Ok::<_, anyhow::Error>((state, transcript))
        })
        .await??;

        self.state = Some(state);
        Ok(transcript)
    }
}
