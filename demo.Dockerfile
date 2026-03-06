# Stage 1: Build proc for Linux
FROM rust:latest AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

# Stage 2: VHS + demo environment
FROM ghcr.io/charmbracelet/vhs

RUN apt-get update --allow-releaseinfo-change && \
    apt-get install -y --no-install-recommends python3 nodejs iproute2 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/proc /usr/local/bin/proc
COPY demo.tape /vhs/demo.tape

# Entrypoint: start servers, wait, record
WORKDIR /vhs

ENTRYPOINT ["bash", "-c", "\
  mkdir -p /tmp/myapp && cd /tmp/myapp && \
  python3 -m http.server 3000 &>/dev/null & \
  node -e \"require('http').createServer((_,r)=>{r.end('ok')}).listen(8080)\" &>/dev/null & \
  sleep 2 && \
  cd /vhs && \
  vhs demo.tape && \
  find / -name 'demo.gif' 2>/dev/null && \
  cp /vhs/demo.gif /output/demo.gif \
"]
