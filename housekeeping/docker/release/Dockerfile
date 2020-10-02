FROM debian:buster-slim

# Install dependencies
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
        npm && \
    npm install -g yarn && \
    yarn global add @polkadot/api-cli && \
# apt cleanup
	apt-get autoremove -y && \
	apt-get clean && \
	find /var/lib/apt/lists/ -type f -not -name lock -delete

RUN useradd substrate -u 10000

COPY ./housekeeping/parachain-collator /usr/local/bin/parachain-collator

USER substrate

ENTRYPOINT ["parachain-collator"]