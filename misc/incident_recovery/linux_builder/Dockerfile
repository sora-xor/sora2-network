ARG DEBIAN_BASE
FROM ${DEBIAN_BASE} AS runtime
COPY tls-ca.pem /etc/ssl/certs/ca-certificates.crt
COPY recipe.json source-inventory.json /preparation/
COPY scripts/apt.sh /preparation/apt.sh
RUN /bin/sh /preparation/apt.sh runtime
ARG RECIPE_SHA256
LABEL org.sora-nexus.preparation.recipe=${RECIPE_SHA256}
LABEL org.sora-nexus.preparation.kind=runtime-base

FROM runtime AS builder
RUN /bin/sh /preparation/apt.sh builder
COPY rustup-init rust-manifest.toml /preparation/
COPY scripts/install_rust.py scripts/smoke.py /preparation/
RUN /usr/bin/python3 /preparation/install_rust.py
COPY source/ /source/
COPY scripts/fetch.sh /preparation/fetch.sh
RUN /bin/sh /preparation/fetch.sh
ENV CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
LABEL org.sora-nexus.preparation.kind=builder
