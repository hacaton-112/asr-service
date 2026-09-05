use std::time::Instant;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path,
    },
    response::Response,
    Extension,
};
use futures_util::{SinkExt, StreamExt};
use tracing::{error, info, warn};

use crate::{
    config::ConfigExt,
    error::AppError,
    models::{
        session::PendingSession,
        ws::{ClientCommand, ServerEvent},
    },
    repositories::{
        engine::{EngineRepository, TranscriptionStream},
        EngineRepoExt, SessionRepoExt,
    },
    services,
    utils::audio::{self, MAX_AUDIO_SECONDS, SAMPLE_RATE},
};

/// Апгрейд HTTP-соединения до WebSocket. Сессия должна быть заранее создана
/// через `POST /v1/sessions` — так на потоковый эндпоинт не попасть со
/// случайным sessionId.
pub async fn websocket(
    Extension(session_repo): SessionRepoExt,
    Extension(engine_repo): EngineRepoExt,
    Extension(config): ConfigExt,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, AppError> {
    let session = services::session::take(session_repo, &session_id).await?;
    // Поток декодирования (WhisperState) заводится один раз на сессию и
    // переиспользуется на все partial/final — см. repositories::engine.
    let stream = engine_repo.create_stream()?;
    let model_name = engine_repo.model_name().to_owned();
    let partial_interval = config.partial_interval;

    Ok(ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            session_id,
            session,
            stream,
            model_name,
            partial_interval,
        )
    }))
}

async fn handle_socket(
    socket: WebSocket,
    session_id: String,
    session: PendingSession,
    mut stream: Box<dyn TranscriptionStream>,
    model_name: String,
    partial_interval: std::time::Duration,
) {
    let (mut sender, mut receiver) = socket.split();
    if send_event(
        &mut sender,
        &ServerEvent::Ready {
            session_id: session_id.clone(),
            sample_rate: SAMPLE_RATE,
            model: model_name,
        },
    )
    .await
    .is_err()
    {
        return;
    }

    info!(%session_id, language = %session.language, "stream connected");
    let mut samples = Vec::<f32>::new();
    let mut last_transcribed_samples = 0usize;
    let mut interval = tokio::time::interval(partial_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Binary(bytes))) => {
                        if bytes.len() % 2 != 0 {
                            let _ = send_event(&mut sender, &ServerEvent::Error {
                                message: "PCM16 frame has an odd byte length".to_owned(),
                            }).await;
                            continue;
                        }
                        let remaining = SAMPLE_RATE * MAX_AUDIO_SECONDS - samples.len();
                        audio::decode_pcm16(&bytes, &mut samples, remaining);
                        if samples.len() >= SAMPLE_RATE * MAX_AUDIO_SECONDS {
                            let _ = send_event(&mut sender, &ServerEvent::Error {
                                message: format!("maximum audio duration is {MAX_AUDIO_SECONDS} seconds"),
                            }).await;
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(ClientCommand::Stop) => {
                                if let Err(error) = transcribe_and_send(
                                    stream.as_mut(),
                                    &mut sender,
                                    &samples,
                                    &session.language,
                                    true,
                                ).await {
                                    error!(%session_id, %error, "final transcription failed");
                                }
                                break;
                            }
                            Ok(ClientCommand::Ping) => {
                                let _ = send_event(&mut sender, &ServerEvent::Pong).await;
                            }
                            Err(error) => {
                                let _ = send_event(&mut sender, &ServerEvent::Error {
                                    message: format!("invalid command: {error}"),
                                }).await;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        warn!(%session_id, %error, "websocket receive error");
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                let has_enough_audio = samples.len() >= SAMPLE_RATE;
                let has_new_audio = samples.len().saturating_sub(last_transcribed_samples) >= SAMPLE_RATE / 2;
                if has_enough_audio && has_new_audio {
                    if let Err(error) = transcribe_and_send(
                        stream.as_mut(),
                        &mut sender,
                        &samples,
                        &session.language,
                        false,
                    ).await {
                        error!(%session_id, %error, "partial transcription failed");
                        break;
                    }
                    last_transcribed_samples = samples.len();
                }
            }
        }
    }

    info!(%session_id, audio_ms = audio::duration_ms(samples.len()), "stream closed");
}

async fn transcribe_and_send<S>(
    stream: &mut dyn TranscriptionStream,
    sender: &mut S,
    samples: &[f32],
    language: &str,
    final_result: bool,
) -> anyhow::Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let started = Instant::now();
    let transcript = services::transcription::transcribe(stream, samples, language).await?;

    // `None` — сигнала недостаточно: партиал просто пропускаем, а на финал
    // отдаём пустую расшифровку, чтобы клиент всё равно получил закрывающее событие.
    let Some(transcript) = transcript else {
        if final_result {
            send_event(
                sender,
                &ServerEvent::Final {
                    transcript: String::new(),
                    audio_ms: audio::duration_ms(samples.len()),
                    processing_ms: 0,
                },
            )
            .await?;
        }
        return Ok(());
    };

    let processing_ms = started.elapsed().as_millis() as u64;
    let audio_ms = audio::duration_ms(samples.len());
    let event = if final_result {
        ServerEvent::Final {
            transcript,
            audio_ms,
            processing_ms,
        }
    } else {
        ServerEvent::Partial {
            transcript,
            audio_ms,
            processing_ms,
        }
    };
    send_event(sender, &event).await
}

async fn send_event<S>(sender: &mut S, event: &ServerEvent) -> anyhow::Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let json = serde_json::to_string(event)?;
    sender.send(Message::Text(json.into())).await?;
    Ok(())
}
