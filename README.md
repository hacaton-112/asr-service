# ASR Service

Микросервис распознавания речи (Speech-to-Text) на Rust: принимает поток PCM16-аудио
по WebSocket и отдаёт промежуточные (partial) и финальные (final) расшифровки в реальном
времени. Инференс — [Whisper](https://github.com/ggml-org/whisper.cpp) (`ggml-large-v3-turbo`)
через [`whisper-rs`](https://github.com/tazz4843/whisper-rs) с ускорением на CUDA и Flash
Attention.

Сделан для хакатона как отдельный ASR-бэкенд: клиент (например, десктопное или веб-приложение)
открывает сессию, коннектится по WebSocket и стримит звук с микрофона, а сервис возвращает текст
почти без задержки.

## Как это работает

1. Клиент отправляет `POST /v1/sessions` — сервис создаёт одноразовую сессию (язык + TTL 60 c)
   и возвращает адрес WebSocket.
2. Клиент подключается по этому адресу и начинает слать бинарные PCM16-фреймы (моно, 16 кГц,
   little-endian).
3. Каждые `WHISPER_PARTIAL_INTERVAL_MS` (по умолчанию 1200 мс) сервис прогоняет накопленный буфер
   через Whisper и присылает событие `partial` с промежуточной расшифровкой.
4. Клиент отправляет `{"type":"stop"}` — сервис делает финальный прогон по всему буферу и присылает
   событие `final`, после чего соединение закрывается.

Тишина и слишком короткие фрагменты не гоняются через модель — это дорого и даёт бессмысленные
партиалы (см. `has_audible_signal` в [audio.rs](src/utils/audio.rs)).

На каждую WS-сессию заводится один `WhisperState` (KV-кэш + compute-буферы модели), который
переиспользуется на все partial/final внутри неё — см. `EngineRepository::create_stream` в
[repositories/engine.rs](src/repositories/engine.rs). Если пересоздавать состояние на каждый
вызов, CUDA/ggml-аллокатор не успевает отдавать освобождённую память обратно, и потребление
RAM/VRAM растёт линейно со временем работы сервиса.

## Архитектура

Слоистая структура в духе [этой статьи SoftwareMill](https://softwaremill.com/in-search-of-ideal-rust-microservice-template/):

```
(запрос) → router → controllers → services → repositories
```

```
src/
├── main.rs                — точка входа: конфиг, логирование, запуск сервера
├── config.rs               — Config: чтение настроек из окружения / .env
│
├── error/                  — единый формат ошибок
│   ├── mod.rs               — реэкспорт (AppError, AppJson)
│   └── app_error.rs         — сама реализация
│
├── utils/                  — чистые функции без состояния и зависимостей от остальных слоёв
│   ├── mod.rs               — реэкспорт (audio, language)
│   ├── audio.rs              — декод PCM16, детект тишины, длительность
│   └── language.rs           — валидация и нормализация языкового кода
│
├── app/                    — сборка приложения
│   ├── mod.rs               — реэкспорт (create_app)
│   └── builder.rs            — поднимает репозитории, прокидывает их в роуты через Extension-слои
│
├── router/                 — регистрация маршрутов
│   ├── mod.rs               — реэкспорт (router)
│   └── routes.rs             — сама регистрация
│
├── controllers/            — HTTP/WS-слой: разбор запроса, вызов services, формирование ответа
│   ├── health.rs             — GET /health
│   ├── sessions.rs           — POST /v1/sessions
│   └── ws.rs                 — GET /v1/ws/{sessionId} (апгрейд и цикл сокета)
│
├── services/                — бизнес-логика, не знает про HTTP/WebSocket
│   ├── session.rs             — создание сессии, проверка TTL
│   └── transcription.rs       — решение «стоит ли звать модель» + вызов движка
│
├── repositories/            — доступ к «внешним» ресурсам, каждый за своим трейтом
│   ├── engine.rs              — EngineRepository: обёртка над WhisperContext (GPU-модель)
│   └── session.rs             — SessionRepository: in-memory хранилище сессий
│
└── models/                  — структуры запросов/ответов и доменные типы
    ├── health.rs
    ├── session.rs
    └── ws.rs
```

Каждый `mod.rs` — это только `pub mod ...` / `pub use ...`, без логики; сама реализация всегда лежит
в соседнем файле того же каталога.

Зависимости между слоями идут только в одну сторону: `controllers` знают про `services`,
`services` — про трейты из `repositories` (не про конкретные реализации), а `repositories` ничего
не знают о вышестоящих слоях. Это позволяет тестировать `services` через мок-реализации трейтов
(`mockall`) без реальной GPU-модели — см. тесты в
[services/transcription.rs](src/services/transcription.rs) и
[services/session.rs](src/services/session.rs).

`AppError` различает два вида ошибок:

- **`Api(status, message)`** — ожидаемая ситуация (невалидный язык, неизвестная/истёкшая сессия):
  код и текст безопасно отдать клиенту как есть;
- **`Internal(anyhow::Error)`** — всё остальное: клиенту уходит общий текст, а подробности — только
  в лог (`tracing::error!`), чтобы не светить внутреннее устройство сервиса.

## Запуск

Сервису нужна модель `ggml-large-v3-turbo.bin` и видеокарта с CUDA. Путь к модели ищется в таком
порядке:

1. `WHISPER_MODEL_PATH`;
2. `./models/ggml-large-v3-turbo.bin`;
3. legacy-каталог модели в `%APPDATA%/com.rizo.trainer-client/whisper-models/`.

```powershell
copy .env.example .env
# указать путь к модели в .env, если она не в ./models
cargo run --release
```

Переменные окружения (см. [.env.example](.env.example)):

| Переменная                    | По умолчанию              | Описание                                   |
|--------------------------------|----------------------------|---------------------------------------------|
| `WHISPER_HOST`                  | `0.0.0.0`                  | Адрес, на котором слушает сервер            |
| `WHISPER_PORT`                  | `8787`                     | Порт                                        |
| `WHISPER_MODEL_PATH`            | —                           | Путь к `.bin`-файлу модели Whisper          |
| `WHISPER_PUBLIC_WS_URL`         | `ws://127.0.0.1:{port}`    | Базовый URL, который вернётся в `ws_url`    |
| `WHISPER_PARTIAL_INTERVAL_MS`   | `1200`                      | Как часто отдавать `partial` (мин. 500 мс) |
| `RUST_LOG`                      | `asr_service=info,tower_http=info` | Уровень логирования                 |

## API

### `GET /health`

```json
{ "status": "ok", "model": "ggml-large-v3-turbo.bin", "device": "cuda:0", "flashAttention": true }
```

### `POST /v1/sessions`

Запрос:

```json
{ "language": "ru" }
```

`language` — код ISO-639-1 (`ru`, `en`, ...) или `auto` (по умолчанию). Ответ:

```json
{
  "sessionId": "b3f1...",
  "wsUrl": "ws://127.0.0.1:8787/v1/ws/b3f1...",
  "sampleRate": 16000,
  "expiresInSeconds": 60,
  "model": "ggml-large-v3-turbo.bin"
}
```

Сессия одноразовая и живёт 60 секунд — если за это время не подключиться по WebSocket, она
считается истёкшей.

### `WS /v1/ws/{sessionId}`

Вход (от клиента):

- **бинарные фреймы** — PCM16 little-endian, моно, 16 кГц;
- **текстовые фреймы** — JSON-команды: `{"type":"stop"}` (завершить и получить финал),
  `{"type":"ping"}` (получить `pong`).

Выход (от сервера), все сообщения — JSON с полем `type`:

| `type`     | Когда приходит                          | Поля                                    |
|------------|-------------------------------------------|-------------------------------------------|
| `ready`    | сразу после апгрейда соединения           | `sessionId`, `sampleRate`, `model`        |
| `partial`  | каждые `WHISPER_PARTIAL_INTERVAL_MS`      | `transcript`, `audioMs`, `processingMs`   |
| `final`    | после `{"type":"stop"}`                   | `transcript`, `audioMs`, `processingMs`   |
| `pong`     | ответ на `{"type":"ping"}`                | —                                          |
| `error`    | невалидный фрейм / превышена длительность | `message`                                  |

Максимальная длительность одной сессии — 120 секунд аудио (`MAX_AUDIO_SECONDS` в
[audio.rs](src/utils/audio.rs)).

## Тесты

```powershell
cargo test
```

`services::session` и `services::transcription` покрыты юнит-тестами на мок-реализациях
репозиториев (`mockall`) — без реальной модели и GPU. `audio` и `language` — на чистых функциях.
