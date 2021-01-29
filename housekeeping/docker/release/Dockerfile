FROM debian:buster-slim
ENV POLKADOT_API_PATH=/usr/local/lib/node_modules/@polkadot
# Install dependencies
RUN apt-get update && \
    apt-get install --no-install-recommends -y \
        npm nano curl&& \
	apt-get autoremove -y && \
	apt-get clean && \
	find /var/lib/apt/lists/ -type f -not -name lock -delete
RUN npm install -g @polkadot/api-cli@0.22.1

RUN rm -rf $POLKADOT_API_PATH/api-cli
COPY ./housekeeping/docker/release/api-cli.tar.gz $POLKADOT_API_PATH/api-cli.tar.gz
RUN tar -xzf $POLKADOT_API_PATH/api-cli.tar.gz -C $POLKADOT_API_PATH/
RUN useradd substrate -u 10000

COPY ./housekeeping/framenode /usr/local/bin/framenode
COPY --chown=10000:10000 ./housekeeping/docker/release/parachain_registration.sh /opt/sora2/parachain_registration.sh
RUN chmod +x /opt/sora2/parachain_registration.sh

COPY --chown=10000:10000 ./misc/rococo-custom.json /chain.spec

USER substrate

ENTRYPOINT ["framenode"]