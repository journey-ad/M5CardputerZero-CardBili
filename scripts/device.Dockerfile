FROM rust:1-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
    libopus-dev libssl-dev pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /work
