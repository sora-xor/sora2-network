FROM python:3.11-alpine3.19

WORKDIR /app

RUN adduser -DH appuser && \
    chown -R appuser:appuser /app

COPY ./misc/runtime_upgrade/main.py \
    ./framenode_runtime.compact.compressed.wasm \
    ./misc/runtime_upgrade/requirements.txt /app/

ENV PYTHONFAULTHANDLER=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONHASHSEED=random \
    PYTHONDONTWRITEBYTECODE=1 \
    PIP_NO_CACHE_DIR=off \
    PIP_DISABLE_PIP_VERSION_CHECK=on \
    PIP_DEFAULT_TIMEOUT=100

RUN mv ./framenode_runtime.compact.compressed.wasm wasm.wasm && \
    pip3 install -r requirements.txt

USER appuser

CMD ["python", "main.py"]
