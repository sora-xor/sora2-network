FROM docker.io/library/debian:buster-slim

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    CC=clang-13 \
    CXX=clang-13

# Install dependencies
RUN apt-get -y update && \
    apt-get install --no-install-recommends -y \
    ca-certificates apt-transport-https gnupg \
    libssl-dev pkg-config \
    curl lsof xz-utils unzip protobuf-compiler \
    git binaryen \
    software-properties-common \
    llvm clang make cmake time rhash && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install clang
RUN curl -s https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - && \
    echo "deb http://apt.llvm.org/buster/ llvm-toolchain-buster-13 main" >> /etc/apt/sources.list.d/llvm-toochain-buster-13.list && \
    echo "deb-src http://apt.llvm.org/buster/ llvm-toolchain-buster-13 main" >> /etc/apt/sources.list.d/llvm-toochain-buster-13.list && \
    apt-get -y update && \
    apt-get install -y --no-install-recommends \
    clang-13 lldb-13 lld-13 libclang-13-dev && \
    update-alternatives --install /usr/bin/cc cc /usr/bin/clang-13 100 && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install docker
RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | apt-key add -
RUN add-apt-repository \
    "deb https://download.docker.com/linux/debian $(lsb_release -cs) stable" && \
    apt-get update && \
    apt-get install --no-install-recommends -y \
    docker-ce docker-ce-cli containerd.io && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install rust
ENV RUST_NIGHTLY_VERSION=nightly-2022-05-12
ENV GRCOV_VERSION=0.8.11
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain stable && \
    cargo install grcov --version ${GRCOV_VERSION} && \
    rustup component add llvm-tools-preview && \
    rustup toolchain install ${RUST_NIGHTLY_VERSION} && \
    ln -s /usr/local/rustup/toolchains/${RUST_NIGHTLY_VERSION}-x86_64-unknown-linux-gnu /usr/local/rustup/toolchains/nightly-x86_64-unknown-linux-gnu && \
    rustup target add wasm32-unknown-unknown && \
    rustup target add wasm32-unknown-unknown --toolchain nightly && \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

RUN curl https://github.com/chevdor/subwasm/releases/download/v0.17.0/subwasm_linux_amd64_v0.17.0.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb

# Build project
ADD . /app
WORKDIR /app
# ENV RUSTC_WRAPPER=sccache
ENV CARGO_INCREMENTAL=0
RUN cargo build --release --features "include-real-files" && \
    cargo test --release && \
    rm -rf $(find target -name deps -type d) $(find target -name build -type d) $(find target -name "*.d" -type f) "${CARGO_HOME}/registry" "${CARGO_HOME}/git"
WORKDIR /
