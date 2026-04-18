## Build stage: Rust backend
FROM rust:1.94-bookworm AS backend
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release -p pomodoro-daemon

## Build stage: Frontend
FROM node:22-bookworm-slim AS frontend
WORKDIR /build/gui
COPY gui/package.json gui/package-lock.json ./
RUN npm ci --ignore-scripts
COPY gui/ .
RUN npm run build

## Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libsqlite3-0 wget && rm -rf /var/lib/apt/lists/*
COPY --from=backend /build/target/release/pomodoro-daemon /usr/bin/pomodoro-daemon
COPY --from=frontend /build/gui/dist /usr/share/pomodoro/gui

ENV POMODORO_GUI_DIR=/usr/share/pomodoro/gui \
    POMODORO_DATA_DIR=/data \
    RUST_LOG=pomodoro_daemon=info

RUN useradd -r -s /bin/false appuser && mkdir -p /data && chown appuser:appuser /data

EXPOSE 9090
VOLUME /data
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD wget -qO- http://localhost:9090/api/health || exit 1
USER appuser
CMD ["pomodoro-daemon"]
