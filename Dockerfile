# syntax=docker/dockerfile:1.5
# Multi-stage build for Flasharr

# Build arguments for versioning
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

# ── Chef stage: install cargo-chef once (cached layer) ──────────────────────
FROM rust:slim AS chef
RUN cargo install cargo-chef --locked
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build

# ── Planner stage: compute the dependency recipe ─────────────────────────────
FROM chef AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# ── Backend builder: cook deps (cached unless Cargo.toml/lock change) ─────────
FROM chef AS backend-builder

# Restore deps from recipe — this layer only re-runs when Cargo.toml/lock change
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --recipe-path recipe.json

# Now copy real source and do the final incremental compile (just flasharr crate)
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release && \
    cp /build/target/release/flasharr /build/flasharr-binary

# Frontend builder stage
FROM node:22-slim as frontend-builder

# Install build tools for native modules
RUN apt-get update && apt-get install -y --no-install-recommends \
    g++ \
    make \
    python3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies
COPY frontend/package*.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci

# Rebuild native modules for current platform (fixes lightningcss issue)
RUN npm rebuild lightningcss --platform=linux --arch=x64

# Build frontend
COPY frontend/ ./
RUN npm run build

# Final runtime stage
FROM debian:12-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    gosu \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user with UID/GID 911 (standard for media stacks)
RUN groupadd -g 911 flasharr && useradd -m -u 911 -g 911 flasharr

WORKDIR /app

# Copy backend binary from build stage
COPY --from=backend-builder /build/flasharr-binary /app/flasharr

# Copy frontend static files
COPY --from=frontend-builder /build/build /app/static

# Add version metadata (must be before LABEL)
ARG VERSION
ARG BUILD_DATE
ARG VCS_REF

LABEL org.opencontainers.image.title="Flasharr" \
    org.opencontainers.image.description="Multi-host download manager with *arr integration" \
    org.opencontainers.image.version="${VERSION}" \
    org.opencontainers.image.created="${BUILD_DATE}" \
    org.opencontainers.image.revision="${VCS_REF}" \
    org.opencontainers.image.vendor="Flasharr Team" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.url="https://github.com/duytran1406/flasharr" \
    org.opencontainers.image.source="https://github.com/duytran1406/flasharr" \
    org.opencontainers.image.documentation="https://github.com/duytran1406/flasharr/blob/main/README.md"

# Create version files for runtime access (before user switch)
RUN echo "${VERSION}" > /app/VERSION && \
    echo "Build Date: ${BUILD_DATE}" >> /app/BUILD_INFO && \
    echo "Git Commit: ${VCS_REF}" >> /app/BUILD_INFO

# Create appData directory structure with proper permissions
RUN mkdir -p /appData/config /appData/data /appData/downloads /appData/logs && \
    chown -R flasharr:flasharr /appData /app

# Copy entrypoint script
COPY docker-entrypoint.sh /app/docker-entrypoint.sh
RUN chmod +x /app/docker-entrypoint.sh

# Note: Container starts as root, entrypoint fixes volume permissions
# then drops to flasharr user via gosu

# Set environment variables
ENV FLASHARR_APPDATA_DIR=/appData \
    RUST_LOG=flasharr=info,tower_http=info

# Expose application port
EXPOSE 8484

# Health check - verify the application is responding
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8484/api/health || exit 1

# Entrypoint fixes volume permissions then drops to flasharr user
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["/app/flasharr"]