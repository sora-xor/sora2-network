FROM python:3.11-alpine3.19

WORKDIR /app
COPY ./misc/runtime_upgrade/main.py \
    ./misc/runtime_upgrade/requirements.txt /app/

ENV PYTHONFAULTHANDLER=1
ENV PYTHONUNBUFFERED=1
ENV PYTHONHASHSEED=random
ENV PYTHONDONTWRITEBYTECODE 1
ENV PIP_NO_CACHE_DIR=off
ENV PIP_DISABLE_PIP_VERSION_CHECK=on
ENV PIP_DEFAULT_TIMEOUT=100

RUN pip3 install -r requirements.txt &&\
    adduser -DH appuser && \
    chown -R appuser:appuser /app

USER appuser

CMD ["python", "main.py"]