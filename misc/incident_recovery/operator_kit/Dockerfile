# SORA Nexus recovery node for chain sora-nexus-recovery-28191817.
#
# Compiles framenode from source at the release commit pinned in sora-recovery,
# then packages it with this kit's chain spec, work bundle and feeder. From the
# kit directory:
#
#   docker build -t sora-nexus-recovery:28191817 .
#
# The image keeps the old release image's conventions: user substrate (uid 10000)
# and the data directory /chain.

FROM debian:bookworm AS build

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    build-essential ca-certificates clang cmake curl git libclang-dev libssl-dev \
    pkg-config protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rust CARGO_HOME=/opt/rust PATH=/opt/rust/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --no-modify-path --profile minimal --default-toolchain none

COPY sora-recovery /kit/sora-recovery

# The cached checkout keeps its file times, so rebuilding this image recompiles
# nothing that has not changed.
RUN --mount=type=cache,id=sora-recovery-cargo-registry,target=/opt/rust/registry \
    --mount=type=cache,id=sora-recovery-cargo-git,target=/opt/rust/git \
    --mount=type=cache,id=sora-recovery-source,target=/src \
    /kit/sora-recovery build --source /src --output /kit/framenode

FROM debian:bookworm-slim

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates curl libssl3 libstdc++6 python3 zstd && \
    rm -rf /var/lib/apt/lists/*

RUN useradd substrate -u 10000 && \
    mkdir /chain && \
    chown 10000:10000 /chain

COPY --from=build /kit/framenode /usr/local/bin/framenode
COPY sora-recovery sora-nexus-recovery-28191817.json README.md SHA256SUMS /opt/sora-recovery/
COPY sora-nexus-recovery-work-bundle /opt/sora-recovery/sora-nexus-recovery-work-bundle

ENV SORA_RECOVERY_NODE=/usr/local/bin/framenode SORA_BASE_PATH=/chain

USER substrate
EXPOSE 30333 9615

ENTRYPOINT ["/opt/sora-recovery/sora-recovery"]
CMD ["run"]
