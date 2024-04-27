FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && \
    apt-get install --no-install-recommends -y \
    ca-certificates nano curl libc6 libstdc++6 && \
    apt-get autoremove -y && \
    apt-get clean && \
    find /var/lib/apt/lists/ -type f -not -name lock -delete

RUN useradd substrate -u 10000
RUN mkdir /chain && \
    chown 10000:10000 /chain
USER substrate

COPY --chown=substrate:substrate ./framenode /usr/local/bin/framenode

ENTRYPOINT ["framenode"]
