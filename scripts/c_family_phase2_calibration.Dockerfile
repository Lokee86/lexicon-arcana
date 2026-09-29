FROM rust:1.90-slim-bookworm@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9 AS rust

FROM ubuntu:24.04@sha256:008173c23f95b170204355c12626cb5a965d779a7e1283b09e9cffbb1bf33ca3

COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
ENV PATH="/usr/local/cargo/bin:${PATH}"
ENV RUSTUP_HOME="/usr/local/rustup"
ENV CARGO_HOME="/usr/local/cargo"

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        clang-18=1:18.1.3-1ubuntu1 \
        llvm-18-dev=1:18.1.3-1ubuntu1 \
        libclang-18-dev=1:18.1.3-1ubuntu1 \
        cmake=3.28.3-1build7 \
        ninja-build=1.11.1-2 \
        python3 \
        python3-psutil=5.9.8-2build2 \
        build-essential \
        git \
        pkg-config \
        ca-certificates \
        zlib1g-dev=1:1.3.dfsg-3.1ubuntu2.2 \
        libzstd-dev=1.5.5+dfsg2-2build1.1 \
        libffi-dev \
        libedit-dev \
        libxml2-dev \
        libtinfo-dev \
    && rm -rf /var/lib/apt/lists/*

ENV LLVM_DIR="/usr/lib/llvm-18/lib/cmake/llvm"
ENV Clang_DIR="/usr/lib/llvm-18/lib/cmake/clang"
