# Prototype packaging for the official A30 offline profile. Not yet built or
# benchmarked on the BTC A30; native Linux experiments use a separate GTX1650 host.
FROM rust:1.99.1-bookworm AS rust-toolchain
FROM nvidia/cuda:12.4.1-devel-ubuntu22.04 AS builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git cmake ninja-build build-essential pkg-config python3 libclang-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo
COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup
ENV PATH=/usr/local/cargo/bin:$PATH CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup
WORKDIR /src
RUN git init llama && cd llama && git remote add origin https://github.com/ggml-org/llama.cpp \
    && git fetch --depth 1 origin 8642541b255bfb5dc39cd351a178be09013a55d6 \
    && git checkout --detach 8642541b255bfb5dc39cd351a178be09013a55d6 \
    && cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=OFF \
       -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=80 -DBUILD_SHARED_LIBS=OFF \
       -DGGML_BACKEND_DL=OFF -DLLAMA_CURL=OFF -DLLAMA_OPENSSL=OFF \
       -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_APP=OFF \
       -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_SUBPROCESS=OFF \
       -DLLAMA_BUILD_UI=OFF -DLLAMA_USE_PREBUILT_UI=OFF \
    && cmake --build build --target llama-server --parallel 4 \
    && install -Dm755 build/bin/llama-server /opt/llama/bin/llama-server \
    && install -Dm644 LICENSE /opt/notices/llama.cpp-LICENSE
RUN git init whisper && cd whisper && git remote add origin https://github.com/ggml-org/whisper.cpp \
    && git fetch --depth 1 origin d1be6fde11ac6e0407606b4e42fe72d34add8037 \
    && git checkout --detach d1be6fde11ac6e0407606b4e42fe72d34add8037 \
    && cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=OFF \
       -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=80 -DBUILD_SHARED_LIBS=OFF \
       -DGGML_BACKEND_DL=OFF -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=ON \
    && cmake --build build --target whisper-server --parallel 4 \
    && install -Dm755 build/bin/whisper-server /opt/whisper/bin/whisper-server \
    && install -Dm644 LICENSE /opt/notices/whisper.cpp-LICENSE
WORKDIR /src/nora
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY .cargo .cargo
COPY crates crates
ENV CARGO_BUILD_JOBS=2
RUN cargo build --locked --release -p nora-lifemate --features whisper

FROM nvidia/cuda:12.4.1-runtime-ubuntu22.04 AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends python3 libgomp1 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home nora \
    && mkdir -p /app/bin /app/result /models \
    && chown -R 10001:10001 /app
COPY --from=builder /opt/llama /opt/llama
COPY --from=builder /opt/whisper /opt/whisper
COPY --from=builder /opt/notices /app/licenses/native
COPY --from=builder /src/nora/target/release/nora-lifemate /app/bin/nora-lifemate
COPY LICENSE THIRD_PARTY_NOTICES.md /app/licenses/
COPY assets/manifests/lifemate-offline.v1.json /app/asset-manifest.json
COPY crates/nora-lifemate/python/bridge.py /app/bridge.py
COPY crates/nora-lifemate/python/container_entry.py /app/container_entry.py
# Models are provided by scripts/prepare_lifemate_assets.py. This candidate
# recipe still needs the exact Cargo/native/model/CUDA distribution notices
# reviewed before image redistribution; the copied files are not a clearance.
# No competition data, .env file or credentials enter the image.
COPY models/Qwen3.5-4B-Q4_K_M.gguf /models/Qwen3.5-4B-Q4_K_M.gguf
COPY models/ggml-large-v3-turbo-q5_0.bin /models/ggml-large-v3-turbo-q5_0.bin
WORKDIR /app
USER 10001:10001
ENTRYPOINT ["python3", "-I", "-B", "/app/container_entry.py"]
