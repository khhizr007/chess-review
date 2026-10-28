# syntax=docker/dockerfile:1.7
# One image: the Rust server serves the API and the built frontend, and runs Stockfish.
#   docker build -t chess-review .
#   docker run --rm -p 8080:8080 chess-review

# --- Frontend -----------------------------------------------------------------
FROM node:24-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY frontend/ ./
RUN npm run build

# --- Backend ------------------------------------------------------------------
FROM rust:1-slim-trixie AS backend
WORKDIR /app/backend
COPY backend/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/backend/target \
    cargo build --release --locked -p review-server \
 && cp target/release/review-server /usr/local/bin/review-server

# --- Stockfish (GPL-3.0, official release binary) -----------------------------
FROM debian:trixie-slim AS stockfish
# x86-64-avx2 suits most cloud CPUs; use x86-64 for old CPUs. Arm hosts are not covered.
ARG STOCKFISH_VERSION=sf_18
ARG STOCKFISH_ARCH=x86-64-avx2
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /tmp/sf.tar \
      "https://github.com/official-stockfish/Stockfish/releases/download/${STOCKFISH_VERSION}/stockfish-ubuntu-${STOCKFISH_ARCH}.tar" \
 && tar -xf /tmp/sf.tar -C /tmp \
 && install -m 0755 "/tmp/stockfish/stockfish-ubuntu-${STOCKFISH_ARCH}" /usr/local/bin/stockfish \
 && rm -rf /tmp/sf.tar /tmp/stockfish

# --- Runtime ------------------------------------------------------------------
FROM debian:trixie-slim
RUN useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin app
WORKDIR /app
COPY --from=stockfish /usr/local/bin/stockfish /usr/local/bin/stockfish
COPY --from=backend /usr/local/bin/review-server /usr/local/bin/review-server
COPY --from=frontend /app/frontend/dist /app/static
COPY backend/data/openings /app/data/openings

ENV BIND_ADDR=0.0.0.0:8080 \
    STOCKFISH_PATH=/usr/local/bin/stockfish \
    STATIC_DIR=/app/static \
    OPENINGS_DIR=/app/data/openings \
    RUST_LOG=info,tower_http=warn

USER app
EXPOSE 8080
CMD ["review-server"]
