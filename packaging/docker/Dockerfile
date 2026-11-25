# syntax=docker/dockerfile:1.7
# Story 12.3: Multi-Arch Hardened OCI Image (`linux/amd64`, `linux/arm64`)
# Rust Core Daemon + Universal MCP Gateway + Pooled 1-Chromium CDP (ADR-006)
# Release CI pins base images by @sha256:... digest via `docker buildx bake` (reproducible builds).

FROM --platform=$TARGETPLATFORM rust:1.85-slim-bookworm AS builder
ARG TARGETPLATFORM
ARG TARGETARCH
WORKDIR /build

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY packages/builtin-skills ./packages/builtin-skills
COPY packaging/plugins ./packaging/plugins

RUN cargo build --release --locked --bin cowok-design --manifest-path Cargo.toml \
    && strip /build/target/release/cowok-design

FROM --platform=$TARGETPLATFORM debian:bookworm-slim AS runtime
ARG TARGETPLATFORM
ARG TARGETARCH

LABEL org.opencontainers.image.title="cowok-ai/uiux-os" \
      org.opencontainers.image.description="Multi-Arch Design Harness Daemon, Universal MCP Gateway & Pooled 1-Chromium CDP for CI/CD" \
      org.opencontainers.image.source="https://github.com/cowok-ai/uiux-os" \
      org.opencontainers.image.vendor="cowok-ai" \
      io.cowok.story="12.3" \
      io.cowok.adr="ADR-006"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        chromium \
        curl \
        fonts-liberation \
        fonts-noto-core \
        libsqlite3-0 \
        libssl3 \
        tini \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN groupadd --gid 10001 --system cowok \
    && useradd --uid 10001 --gid 10001 --system --home-dir /tmp/cowok-runtime --shell /usr/sbin/nologin cowok \
    && mkdir -p /workspace /tmp/cowok-runtime/data /tmp/cowok-runtime/state /tmp/cowok-runtime/cache /tmp/cowok-runtime/chrome \
    && chown -R 10001:10001 /workspace /tmp/cowok-runtime \
    && chmod 0700 /tmp/cowok-runtime /tmp/cowok-runtime/data /tmp/cowok-runtime/state /tmp/cowok-runtime/cache /tmp/cowok-runtime/chrome

COPY --from=builder --chmod=0755 /build/target/release/cowok-design /usr/local/bin/cowok-design
RUN ln -sf /usr/local/bin/cowok-design /usr/local/bin/uiux-os \
    && ln -sf /usr/local/bin/cowok-design /usr/local/bin/harness-daemon

COPY --chmod=0755 packaging/docker/entrypoint.sh /usr/local/bin/cowok-entrypoint.sh

# Security note (P1-1): `COWOK_CHROMIUM_NO_SANDBOX=1` is safe ONLY inside this hardened
# non-root (`uid 10001`), `read_only: true`, `cap_drop: ["ALL"]`, `no-new-privileges:true`
# container boundary where Chromium's kernel user-namespace/SUID sandbox cannot initialize.
ENV COWOK_CDP_CHROME_BIN=/usr/bin/chromium \
    COWOK_CONTAINER_MODE=1 \
    COWOK_CHROMIUM_NO_SANDBOX=1 \
    UXUI_OS_WORKSPACE_ROOT=/workspace \
    UXUI_OS_DAEMON_PORT=4170 \
    XDG_DATA_HOME=/tmp/cowok-runtime/data \
    XDG_STATE_HOME=/tmp/cowok-runtime/state \
    XDG_CACHE_HOME=/tmp/cowok-runtime/cache \
    COWOK_MAX_MEMORY_MB=512 \
    HOME=/tmp/cowok-runtime

WORKDIR /workspace
USER 10001:10001

EXPOSE 4170

HEALTHCHECK --interval=15s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${UXUI_OS_DAEMON_PORT:-4170}/api/v1/health" >/dev/null || exit 1

ENTRYPOINT ["/usr/bin/tini", "-s", "--", "/usr/local/bin/cowok-entrypoint.sh"]
CMD ["up", "--port", "4170", "--no-open"]
