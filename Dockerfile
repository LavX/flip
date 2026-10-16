# syntax=docker/dockerfile:1

FROM node:26.12.0-trixie-slim AS frontend-build
WORKDIR /app/frontend

RUN npm install --global npm@12.2.0

COPY frontend/package.json frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci

COPY frontend/ ./
RUN npm run build

FROM rust:1.99.1-trixie AS rust-build
WORKDIR /app
ARG CARGO_BUILD_JOBS=1
ENV CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    mkdir -p src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY --from=frontend-build /app/frontend/dist ./frontend/dist
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    touch src/main.rs src/lib.rs && cargo build --release --locked && strip target/release/flip

FROM debian:trixie-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=rust-build /app/target/release/flip /usr/local/bin/flip

ARG OCI_REVISION=""
LABEL org.opencontainers.image.title="Flip" \
      org.opencontainers.image.revision="${OCI_REVISION}" \
      org.opencontainers.image.source="https://github.com/LavX/flip" \
      org.opencontainers.image.licenses="MIT"

ENV FLIP_BIND=0.0.0.0 \
    FLIP_PORT=8080
EXPOSE 8080
USER 65532:65532
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl --fail --silent --show-error --max-time 2 http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["/usr/local/bin/flip"]
