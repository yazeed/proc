# Build stage
FROM rust:latest as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (for caching)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies (minimal)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/proc /usr/local/bin/proc

# Set the entrypoint
ENTRYPOINT ["proc"]

# Default command (show help)
CMD ["--help"]

# Labels
LABEL org.opencontainers.image.title="proc"
LABEL org.opencontainers.image.description="Semantic CLI tool for process management. Target by port, PID, name or path."
LABEL org.opencontainers.image.source="https://github.com/yazeed/proc"
LABEL org.opencontainers.image.licenses="MIT"
