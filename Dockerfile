# Multi-stage build for Flasharr

# Build arguments for versioning
ARG VERSION=dev
ARG BUILD_DATE
ARG VCS_REF

FROM rust:slim as backend-builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy backend source
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src

# Build backend in release mode
RUN cargo build --release

# Frontend builder stage
FROM node:20-slim as frontend-builder

WORKDIR /build

# Copy frontend source
COPY frontend/package*.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# Final runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 flasharr

WORKDIR /app

# Copy backend binary
COPY --from=backend-builder /build/target/release/flasharr /app/flasharr

# Copy frontend static files
COPY --from=frontend-builder /build/build /app/static

# Create appData directory structure
RUN mkdir -p /appData/config /appData/data /appData/downloads /appData/logs && \
    chown -R flasharr:flasharr /appData /app

# Switch to app user
USER flasharr

# Set environment variables
ENV FLASHARR_APPDATA_DIR=/appData
ENV RUST_LOG=flasharr=info,tower_http=info

# Expose port
EXPOSE 8484

# Add version metadata
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

# Create version file for runtime access
RUN echo "${VERSION}" > /app/VERSION && \
    echo "Build Date: ${BUILD_DATE}" >> /app/BUILD_INFO && \
    echo "Git Commit: ${VCS_REF}" >> /app/BUILD_INFO

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/flasharr", "--version"] || exit 1

# Run the application
CMD ["/app/flasharr"]
