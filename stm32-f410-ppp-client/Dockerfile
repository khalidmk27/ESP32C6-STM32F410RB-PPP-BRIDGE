
FROM mcr.microsoft.com/devcontainers/base:ubuntu

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    cmake \
    curl \
    git \
    libudev-dev \
    pkg-config \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    && rm -rf /var/lib/apt/lists/*

USER vscode

ENV RUSTUP_HOME=/home/vscode/.rustup
ENV CARGO_HOME=/home/vscode/.cargo
ENV PATH=/home/vscode/.cargo/bin:${PATH}

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal

RUN rustup component add rustfmt clippy \
    && rustup target add thumbv7em-none-eabihf

RUN cargo install flip-link --locked \
    && cargo install probe-rs-tools --locked

WORKDIR /workspaces
