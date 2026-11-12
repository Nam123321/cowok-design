# syntax=docker/dockerfile:1.7-labs
# ========================================================
# cowok-design (UI/UX OS - FlowForge Harness OS)
# Multi-Stage Production OCI Container
# Stage 1: Build Rust Axum Daemon (harness-daemon)
# Stage 2: Build React 19 / Vite Web Frontend
# Stage 3: Minimal, Hardened Production Runtime
# ========================================================

# --------------------------------------------------------
# STAGE 1: Rust Harness Daemon Builder
# --------------------------------------------------------
FROM rust:1.85-slim-bookworm AS rust-builder

WORKDIR /usr/src/cowok-design

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust workspace definitions and manifests
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

# Build release binary for harness-daemon
RUN cargo build --release --bin harness-daemon \
    && strip target/release/harness-daemon

# --------------------------------------------------------
# STAGE 2: React 19 Frontend Builder
# --------------------------------------------------------
FROM node:22-bookworm-slim AS web-builder

WORKDIR /app

# Enable pnpm via corepack
RUN corepack enable && corepack prepare pnpm@9.15.4 --activate

# Copy package manifests and lockfiles
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY packages ./packages

# Install dependencies (frozen lockfile for reproducible builds)
RUN pnpm install --frozen-lockfile

# Copy source and configuration files
COPY tsconfig*.json vite.config.ts index.html ./
COPY src ./src
COPY public ./public

# Build frontend production bundle
RUN pnpm run build

# --------------------------------------------------------
# STAGE 3: Hardened Production Runtime
# --------------------------------------------------------
FROM node:22-bookworm-slim AS runner

WORKDIR /app

ENV NODE_ENV=production \
    UXUI_OS_DAEMON_BIN=/usr/local/bin/harness-daemon \
    UXUI_OS_WORKSPACE_ROOT=/app/workspace \
    PORT=4173 \
    HOST=0.0.0.0

# Install runtime utilities (curl for healthchecks, ca-certificates)
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && corepack enable && corepack prepare pnpm@9.15.4 --activate

# Copy compiled Rust daemon from stage 1
COPY --from=rust-builder /usr/src/cowok-design/target/release/harness-daemon /usr/local/bin/harness-daemon

# Copy production web assets and dependencies from stage 2
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml vite.config.ts tsconfig*.json ./
COPY packages ./packages
COPY src ./src
COPY --from=web-builder /app/dist ./dist
COPY --from=web-builder /app/node_modules ./node_modules

# Prepare persistent workspace directory with proper permissions
RUN mkdir -p /app/workspace /app/.cowok-design && chown -R node:node /app

USER node

EXPOSE 4173 3000

# Container healthcheck
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:4173/ || exit 1

# Start Vite preview server serving compiled SPA and proxying to loopback harness-daemon
CMD ["pnpm", "run", "preview", "--host", "0.0.0.0", "--port", "4173"]
