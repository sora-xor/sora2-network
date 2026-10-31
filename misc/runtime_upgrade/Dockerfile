# Official image manifests verified on 2026-10-24.
FROM rust:1.99.1-slim-trixie@sha256:aab42391a986424e22d6a3ec14f37bc6dd3dacff4d457ca7cccd781e02297cc6 AS rust-toolchain
FROM python:3.13.17-slim-trixie@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962 AS wheel-builder

COPY --from=rust-toolchain /usr/local/cargo /usr/local/cargo
COPY --from=rust-toolchain /usr/local/rustup /usr/local/rustup
ENV PATH="/usr/local/cargo/bin:${PATH}" \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_TARGET_DIR=/build/target
RUN apt-get update && apt-get install -y --no-install-recommends build-essential pkg-config && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build/runtime_upgrade
COPY ./misc/runtime_upgrade/requirements.txt ./misc/runtime_upgrade/install_dependencies.py ./
COPY ./misc/runtime_upgrade/vendor ./vendor
RUN python install_dependencies.py --wheel-dir /wheels

FROM python:3.13.17-slim-trixie@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962
WORKDIR /app
ENV PYTHONFAULTHANDLER=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONHASHSEED=random \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=on \
    PIP_DEFAULT_TIMEOUT=100

COPY --from=wheel-builder /wheels /wheels
RUN python -m pip install --no-index --find-links=/wheels /wheels/*.whl && \
    python -m pip check && rm -rf /wheels && \
    useradd --create-home --shell /usr/sbin/nologin appuser
COPY ./misc/runtime_upgrade/main.py /app/main.py
COPY ./misc/runtime_upgrade/vendor /app/vendor
COPY ./framenode_runtime.compact.compressed.wasm /app/wasm.wasm
RUN chown -R appuser:appuser /app
USER appuser
CMD ["python", "main.py"]
