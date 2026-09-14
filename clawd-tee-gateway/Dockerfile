# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:1.94-bookworm AS builder

WORKDIR /build

# Cache dependencies separately from source
COPY Cargo.toml Cargo.toml
RUN mkdir src && echo 'fn main(){}' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

COPY src/ src/
RUN touch src/main.rs && cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────
# Use a minimal Debian base — must be Debian/Ubuntu to get the NVIDIA and Intel
# TDX runtime libraries injected by the host's TEE driver.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/clawd-tee-gateway /usr/local/bin/clawd-tee-gateway

# Non-root user (TEE requirement: no privilege escalation inside enclave).
# Render mounts runtime secret files readable by group 1000 for Docker services.
RUN groupadd -g 1000 render-secrets \
    && useradd -r -s /sbin/nologin -G render-secrets clawd
USER clawd

EXPOSE 8080

# Honor Railway-injected PORT (default 8080). Compose still maps 8080:8080.
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD sh -c 'curl -fsS "http://127.0.0.1:${PORT:-8080}/health" || exit 1'

ENTRYPOINT ["/usr/local/bin/clawd-tee-gateway"]
