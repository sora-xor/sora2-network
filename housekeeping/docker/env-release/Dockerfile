FROM docker.io/paritytech/ci-linux:production

# Install dependencies
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates apt-transport-https gnupg \
    libssl-dev pkg-config \
    curl \
    git binaryen \
    software-properties-common \
    llvm clang make cmake time rhash && \
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
# ENV RUST_VERSION=nightly-2021-12-10
ENV GRCOV_VERSION=0.8.7
# RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain ${RUST_VERSION} && \
#     cargo install sccache && \
#     cargo install grcov --version ${GRCOV_VERSION} && \
#     rustup component add llvm-tools-preview && \
#     rustup target add wasm32-unknown-unknown
RUN cargo install grcov --version ${GRCOV_VERSION} && \
    rustup component add llvm-tools-preview

RUN curl https://github.com/chevdor/subwasm/releases/download/v0.17.0/subwasm_linux_amd64_v0.17.0.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb

# Build project
ADD . /app
WORKDIR /app
# ENV RUSTC_WRAPPER=sccache
ENV CARGO_INCREMENTAL=0
RUN cargo build --release --features "include-real-files" && \
    cargo test --release && \
    rm -rf $(find target -name deps -type d) $(find target -name build -type d)
WORKDIR /
