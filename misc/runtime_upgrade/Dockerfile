FROM python:3.11-slim

WORKDIR /app

RUN adduser --system --no-create-home nonroot

COPY requirements.txt .

RUN pip install --no-cache-dir -r requirements.txt

COPY main.py .

USER nonroot

ENTRYPOINT  ["python", "main.py"]
