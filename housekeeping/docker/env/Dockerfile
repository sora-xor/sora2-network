FROM rust:latest AS coverage

# cache toolhcain for project build
RUN rustup install nightly-2023-03-21

WORKDIR ./sora2-network/

RUN cargo install grcov
RUN rustup component add llvm-tools-preview
