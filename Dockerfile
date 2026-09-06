# syntax=docker/dockerfile:1

# whisper-rs (feature "cuda") компилирует whisper.cpp с CUDA-ядрами через
# cmake — для этого нужен полный CUDA toolkit (nvcc, cuBLAS dev), поэтому
# билд-стадия использует "devel"-образ, а не "runtime".
FROM nvidia/cuda:12.4.1-devel-ubuntu22.04 AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
        curl build-essential cmake pkg-config ca-certificates \
        clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app

# Компиляция whisper.cpp/CUDA внутри whisper-rs-sys — самая долгая часть
# сборки и не меняется, пока не меняется Cargo.toml/Cargo.lock. Собираем
# зависимости отдельным слоем на фиктивном main.rs, чтобы не пересобирать
# их при каждом изменении src/*.rs.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime-образ CUDA уже несёт libcudart/libcublas — этого достаточно для
# запуска собранного бинарника, полный devel-toolchain здесь не нужен.
FROM nvidia/cuda:12.4.1-runtime-ubuntu22.04 AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/asr-service ./asr-service

EXPOSE 8787
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=5 \
    CMD curl -f http://localhost:8787/health || exit 1

ENTRYPOINT ["./asr-service"]
