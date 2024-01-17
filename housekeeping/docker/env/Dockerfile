FROM debian:bullseye-slim

ENV RUSTUP_HOME="/opt/rust"
ENV CARGO_HOME="/opt/rust"
ENV PATH="$PATH:$RUSTUP_HOME/bin"
ENV RUST_VERSION=nightly-2024-01-15
ENV TZ=Europe/Moscow
ENV CC=clang-14
ENV CXX=clang++-14
ENV SUBWASM_VER=v0.14.0
ENV GRCOV_VERSION=0.8.19
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
    echo "deb http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-14 main" >> /etc/apt/sources.list.d/llvm-toochain-bullseye-14.list && \
    echo "deb-src http://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-14 main" >> /etc/apt/sources.list.d/llvm-toochain-bullseye-14.list && \
    apt-get update && \
    apt-get install --no-install-recommends -y \
    clang-14 lldb-14 lld-14 libclang-14-dev llvm-14 protobuf-compiler && \
    update-alternatives --install /usr/bin/cc cc /usr/bin/${CC} 100 && \
    curl https://github.com/chevdor/subwasm/releases/download/${SUBWASM_VER}/subwasm_linux_amd64_${SUBWASM_VER}.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb && \
    rustup update && \
    cargo install grcov --version ${GRCOV_VERSION} && \
    rustup component add llvm-tools-preview && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git" && \
    ln -snf /usr/share/zoneinfo/${TZ} /etc/localtime && echo ${TZ} > /etc/timezone