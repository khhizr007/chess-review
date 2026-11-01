# syntax=docker/dockerfile:1.7
# One image: the Rust server serves the API and the built frontend, and runs Stockfish and the
# Maia-2 sidecar. Set MAIA_COMMAND= (empty) at run time to turn Maia off.
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

# --- Sound pack (optional, from a separate repository) --------------------------
# Set SOUNDS_REPO=owner/name to pull a raw sound pack (default/*.mp3 and assets-ds/*.mp3, as in
# khhizr007/chess_sounds) from GitHub; tools/sounds/install-sound-pack.sh renames it to the
# <kind>.mp3 files the frontend loads. A private repository needs a token as the build secret
# `github_token` (read-only "Contents" access is enough); it is mounted for this step only and
# never stored in the image. Without SOUNDS_REPO the stage is empty and the app uses its
# synthesised sounds.
FROM debian:trixie-slim AS sounds
ARG SOUNDS_REPO=
ARG SOUNDS_REF=master
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*
COPY tools/sounds/install-sound-pack.sh /work/tools/sounds/install-sound-pack.sh
RUN --mount=type=secret,id=github_token,required=false \
    set -eu; mkdir -p /sounds; \
    if [ -n "${SOUNDS_REPO}" ]; then \
      auth=""; \
      if [ -s /run/secrets/github_token ]; then auth="Authorization: Bearer $(cat /run/secrets/github_token)"; fi; \
      mkdir /tmp/repo; \
      curl -fsSL ${auth:+-H "$auth"} -H "Accept: application/vnd.github+json" \
        "https://api.github.com/repos/${SOUNDS_REPO}/tarball/${SOUNDS_REF}" \
        | tar -xz -C /tmp/repo --strip-components=1; \
      bash /work/tools/sounds/install-sound-pack.sh /tmp/repo; \
      cp /work/frontend/public/sounds/*.mp3 /sounds/; \
      rm -rf /tmp/repo; \
      echo "sound pack:"; ls /sounds; \
    fi

# --- Maia-2 (MIT): human-move model for the coach, with its weights -------------
# maia2 0.10.0 pins torch 2.8.0 and needs Python 3.12 or 3.13. CPU-only torch keeps it small.
FROM python:3.12-slim-trixie AS maia
ARG TORCH_INDEX=https://download.pytorch.org/whl/cpu
RUN python -m venv /opt/maia \
 && /opt/maia/bin/pip install --no-cache-dir torch==2.8.0 --index-url "${TORCH_INDEX}" \
 && /opt/maia/bin/pip install --no-cache-dir maia2==0.10.0 --extra-index-url "${TORCH_INDEX}"
# Fetch the weights at build time (maia2's own published files, 280 MB), so start-up needs no network.
RUN mkdir -p /opt/maia-models \
 && /opt/maia/bin/python -c "import gdown; \
gdown.download('https://drive.google.com/uc?id=1gbC1-c7c0EQOPPAVpGWubezeEW8grVwc', '/opt/maia-models/rapid_model.pt', quiet=True); \
gdown.download('https://drive.google.com/uc?id=1GQTskYMVMubNwZH2Bi6AmevI15CS6gk0', '/opt/maia-models/config.yaml', quiet=True)" \
 && test -s /opt/maia-models/rapid_model.pt && test -s /opt/maia-models/config.yaml \
 && chmod -R a+rX /opt/maia-models

# --- Runtime ------------------------------------------------------------------
# Python base, because the Maia venv links to this image's /usr/local/bin/python3.12.
FROM python:3.12-slim-trixie
RUN useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin app
WORKDIR /app
COPY --from=maia /opt/maia /opt/maia
COPY --from=maia /opt/maia-models /opt/maia-models
COPY tools/maia/maia_sidecar.py /app/maia/maia_sidecar.py
COPY --from=stockfish /usr/local/bin/stockfish /usr/local/bin/stockfish
COPY --from=backend /usr/local/bin/review-server /usr/local/bin/review-server
COPY --from=frontend /app/frontend/dist /app/static
COPY --from=sounds /sounds/ /app/static/sounds/
COPY backend/data/openings /app/data/openings
# Finished reviews, one file each, deleted a day after they finish (REVIEW_TTL_SECS).
RUN mkdir -p /app/data/reviews && chown app:app /app/data/reviews
VOLUME /app/data/reviews

ENV BIND_ADDR=0.0.0.0:8080 \
    STOCKFISH_PATH=/usr/local/bin/stockfish \
    STATIC_DIR=/app/static \
    OPENINGS_DIR=/app/data/openings \
    REVIEW_STORE_DIR=/app/data/reviews \
    MAIA_COMMAND="/opt/maia/bin/python /app/maia/maia_sidecar.py --type rapid --models-dir /opt/maia-models --threads 2" \
    RUST_LOG=info,tower_http=warn

USER app
EXPOSE 8080
CMD ["review-server"]
