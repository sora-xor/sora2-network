FROM debian:bullseye-20240211-slim

ENV RUSTUP_HOME="/opt/rust"
ENV CARGO_HOME="/opt/rust"
ENV PATH="$PATH:$RUSTUP_HOME/bin"
ENV CARGO_BUILD_DEP_INFO_BASEDIR="."
ENV RUST_VERSION=nightly-2024-01-15
ENV TZ=Europe/Moscow
ENV CC=clang-14
ENV CXX=clang++-14
ENV SUBWASM_VER=v0.14.0
ENV GRCOV_VERSION=0.8.19
ENV LLVM_TOOLS_VER=14
ENV CCLANG_VER=14
ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates apt-transport-https gnupg \
    libssl-dev pkg-config \
    wget \
    git binaryen \
    make cmake libssl-dev \
    software-properties-common && \
    wget -O - https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain ${RUST_VERSION} && \
    rustup toolchain install ${RUST_VERSION} && \
    rustup target add wasm32-unknown-unknown --toolchain ${RUST_VERSION} && \
    wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh ${LLVM_TOOLS_VER} && \
    rm llvm.sh && \
    apt-get update && \
    apt-get install --no-install-recommends -y \
    clang-${CCLANG_VER} lldb-${CCLANG_VER} lld-${CCLANG_VER} libclang-${CCLANG_VER}-dev llvm-${LLVM_TOOLS_VER} protobuf-compiler && \
    update-alternatives --install /usr/bin/cc cc /usr/bin/${CC} 100 && \
    wget https://github.com/chevdor/subwasm/releases/download/${SUBWASM_VER}/subwasm_linux_amd64_${SUBWASM_VER}.deb -O subw.deb && \
    dpkg -i subw.deb && rm subw.deb && \
    rustup update && \
    cargo install grcov --version ${GRCOV_VERSION} && \
    rustup component add llvm-tools-preview && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    rm -rf "${CARGO_HOME}/git" && \
    ln -snf /usr/share/zoneinfo/${TZ} /etc/localtime && echo ${TZ} > /etc/timezone