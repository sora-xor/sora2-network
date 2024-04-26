FROM gcc:12.3.0-bookworm

ARG DEBIAN_FRONTEND=noninteractive

ENV RUSTUP_HOME="/opt/rust"
ENV CARGO_HOME="/opt/rust"
ENV PATH="$PATH:$RUSTUP_HOME/bin"
ENV CARGO_BUILD_DEP_INFO_BASEDIR="."
ENV RUST_VERSION=nightly-2024-01-15
ENV TZ=Europe/Moscow
ENV LLVM_TOOLS_VER="14"
ENV DEBIAN_FRONTEND=noninteractive
ENV CC=gcc
ENV CXX=g++
ENV GRCOV_VERSION=0.8.19
ENV SUBWASM_VER=v0.20.0

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    binaryen cmake protobuf-compiler \
    software-properties-common musl-tools && \
    wget -O - https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain ${RUST_VERSION} && \
    rustup toolchain install ${RUST_VERSION} && \
    rustup target add wasm32-unknown-unknown --toolchain ${RUST_VERSION} && \
    wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh ${LLVM_TOOLS_VER} && \
    rm llvm.sh && \
    rustup update && \
    cargo install --locked --git https://github.com/chevdor/subwasm --tag ${SUBWASM_VER} && \
    cargo install grcov --version ${GRCOV_VERSION} && \
    rustup component add llvm-tools-preview && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    rm -rf "${CARGO_HOME}/git" && \
    ln -snf /usr/share/zoneinfo/${TZ} /etc/localtime && echo ${TZ} > /etc/timezone
