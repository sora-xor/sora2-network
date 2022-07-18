FROM debian:bookworm-slim

# Set environment variables
ENV RUSTUP_HOME="/opt/rustup"
ENV CARGO_HOME="/opt/cargo"
ENV PATH="$PATH:$CARGO_HOME/bin"
ENV CARGO_BUILD_DEP_INFO_BASEDIR="."
ENV CC=clang-13
ENV CXX=clang++-13

# Install dependencies
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates apt-transport-https gnupg \
    libssl-dev pkg-config \
    curl \
    git binaryen \
    make cmake libssl-dev zlib1g-dev \
    file bsdmainutils mold clang lldb lld libclang-dev llvm \
    software-properties-common && \
    apt-get autoremove -y && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# RUN curl -s https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - && \
#     echo "deb http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-13 main" >> /etc/apt/sources.list.d/llvm-toochain-13.list && \
#     echo "deb-src http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-13 main" >> /etc/apt/sources.list.d/llvm-toochain-13.list && \
#     apt-get -y update && \
#     apt-get install -y --no-install-recommends \
#     clang-13 lldb-13 lld-13 libclang-13-dev llvm-13 && \
#     update-alternatives --install /usr/bin/cc cc /usr/bin/clang-13 100 && \
#     apt-get autoremove -y && \
#     apt-get clean && \
#     rm -rf /var/lib/apt/lists/*

# Install docker
# RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | apt-key add -
# RUN echo "deb https://download.docker.com/linux/debian bullseye stable" >> /etc/apt/sources.list.d/docker-ce.list && \
#     apt-get update && \
#     apt-get install --no-install-recommends -y \
#     docker-ce docker-ce-cli containerd.io && \
#     apt-get autoremove -y && \
#     apt-get clean && \
#     rm -rf /var/lib/apt/lists/*

# Install rust
ENV RUST_NIGHTLY_VERSION=nightly-2022-05-12
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain stable && \
    rustup toolchain install ${RUST_NIGHTLY_VERSION} && \
    rustup target add wasm32-unknown-unknown && \
    rustup target add wasm32-unknown-unknown --toolchain ${RUST_NIGHTLY_VERSION} && \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

RUN curl https://github.com/chevdor/subwasm/releases/download/v0.17.0/subwasm_linux_amd64_v0.17.0.deb -sSL -o subw.deb && \
    dpkg -i subw.deb && rm subw.deb

RUN cargo install grcov && \
    rustup component add llvm-tools-preview && \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

# RUN git clone https://github.com/rui314/mold.git && \
#     git -C "./mold" checkout v1.3.1 && \
#     make --directory="./mold" -j && \
#     make --directory="./mold" install && \
#     rm -rf "./mold"
