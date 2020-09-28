FROM debian:buster-slim

RUN useradd substrate -u 10000

COPY ./housekeeping/parachain-collator /usr/local/bin/parachain-collator

USER substrate

ENTRYPOINT ["parachain-collator"]