FROM python:3.11-slim

WORKDIR /app

COPY requirements.txt .

RUN pip install --no-cache-dir -r requirements.txt

COPY main.py .

COPY *.compact.compressed.wasm .

EXPOSE 9944

ENTRYPOINT  ["python", "main.py"]
