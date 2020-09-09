FROM debian:buster-slim

RUN useradd substrate -u 10000

COPY /opt/rust-target/release/parachain-collator /usr/local/bin/parachain-collator

USER substrate

ENTRYPOINT ["parachain-collator"]