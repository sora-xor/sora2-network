FROM debian:bullseye-slim

# Set environment variables
ENV RUSTUP_HOME="/opt/rust"
ENV CARGO_HOME="/opt/rust"
ENV PATH="$PATH:$RUSTUP_HOME/bin"
ENV CARGO_BUILD_DEP_INFO_BASEDIR="."

# Install dependencies
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
        ca-certificates apt-transport-https gnupg \
        libssl-dev pkg-config \
        curl \
        git binaryen \
        software-properties-common \
        llvm clang && \
    rm -rf /var/lib/apt/lists/*

# Install docker
RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | apt-key add -
RUN add-apt-repository \
        "deb https://download.docker.com/linux/debian $(lsb_release -cs) stable" && \
    apt-get update && \
    apt-get install --no-install-recommends -y \
        docker-ce docker-ce-cli containerd.io && \
    rm -rf /var/lib/apt/lists/*

# Install rust
ENV RUST_VERSION=nightly-2021-07-26
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain ${RUST_VERSION} && \
    rustup default ${RUST_VERSION} && \
    rustup target add wasm32-unknown-unknown --toolchain ${RUST_VERSION}
RUN curl https://github.com/chevdor/subwasm/releases/download/v0.14.1/subwasm_linux_amd64_v0.14.1.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb

RUN cargo install sccache && \
    cargo install grcov && \
    rustup component add llvm-tools-preview

# Build project
ADD . /app
WORKDIR /app
ENV RUSTC_WRAPPER=sccache
ENV CARGO_INCREMENTAL=0
RUN rustc --version && RUSTFLAGS="$RUSTFLAGS -A warnings" cargo build --color=never --release --features "include-real-files" -- --color=never && \
    cargo test --release
WORKDIR /