FROM debian:bullseye-slim

ENV RUSTUP_HOME="/opt/rust"
ENV CARGO_HOME="/opt/rust"
ENV PATH="$PATH:$RUSTUP_HOME/bin"
ENV RUST_VERSION=nightly-2024-01-15
ENV TZ=Europe/Moscow
ENV CC=clang-13
ENV CXX=clang++-13
ENV SUBWASM_VER=v0.17.0
ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates apt-transport-https gnupg \
    libssl-dev pkg-config \
    curl \
    git binaryen \
    docker.io \
    make cmake libssl-dev \
    software-properties-common && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain ${RUST_VERSION} && \
    rustup toolchain install ${RUST_VERSION} && \
    rustup target add wasm32-unknown-unknown --toolchain ${RUST_VERSION} && \
    curl -s https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - && \
    echo "deb http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-13 main" >> /etc/apt/sources.list.d/llvm-toochain-bullseye-13.list && \
    echo "deb-src http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-13 main" >> /etc/apt/sources.list.d/llvm-toochain-bullseye-13.list && \
    apt-get update && \
    apt-get install --no-install-recommends -y \
    clang-13 lldb-13 lld-13 libclang-13-dev llvm-13 protobuf-compiler && \
    update-alternatives --install /usr/bin/cc cc /usr/bin/${CC} 100 && \
    curl https://github.com/chevdor/subwasm/releases/download/${SUBWASM_VER}/subwasm_linux_amd64_${SUBWASM_VER}.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb && \
    rustup update && \
    cargo install grcov && \
    rustup component add llvm-tools-preview && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git" && \
    ln -snf /usr/share/zoneinfo/${TZ} /etc/localtime && echo ${TZ} > /etc/timezone