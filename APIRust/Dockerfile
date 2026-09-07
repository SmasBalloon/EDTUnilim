# syntax=docker/dockerfile:1

# --- Compilation ------------------------------------------------------------
FROM rust:1-slim-trixie AS builder
WORKDIR /build

# Les dépendances sont compilées à part : tant que Cargo.toml ne bouge pas,
# cette couche est réutilisée d'un build à l'autre.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY data ./data
# `touch` force la recompilation du code applicatif après la couche factice.
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

# --- Exécution --------------------------------------------------------------
FROM debian:trixie-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app edt \
    && mkdir -p /app/cache && chown -R edt:edt /app

WORKDIR /app
COPY --from=builder /build/target/release/edt-unilim-api /usr/local/bin/edt-unilim-api

USER edt
ENV EDT_BIND=0.0.0.0:3000 \
    EDT_CACHE_DIR=/app/cache \
    EDT_LOG=info
EXPOSE 3000
VOLUME ["/app/cache"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://localhost:3000/health || exit 1

CMD ["edt-unilim-api"]
