# ASR service

Axum WebSocket service for CUDA-accelerated Whisper transcription.

## Run

The service looks for `ggml-large-v3-turbo.bin` in this order:

1. `WHISPER_MODEL_PATH`;
2. `./models/ggml-large-v3-turbo.bin`;
3. the legacy trainer-client AppData model directory.

```powershell
$env:WHISPER_MODEL_PATH = "C:\path\to\ggml-large-v3-turbo.bin"
cargo run --release
```

HTTP endpoints:

- `GET /health`
- `POST /v1/sessions`
- `WS /v1/ws/{sessionId}`

WebSocket input is mono PCM16 little-endian at 16 kHz. Send `{"type":"stop"}` to finish. Output events are JSON with `ready`, `partial`, `final`, or `error` types.
