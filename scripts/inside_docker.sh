#!/bin/sh

test "$INSIDE_DOCKER" == "1" || exit 1
test "$PWD" == "/parachain"  || exit 1

if [ ! -f /usr/.prepare-tools.ready ]; then
	nix-env -i which gnugrep || exit 1
	nix-env -iA nixos.nodejs || exit 1
	nix-env -i yarn || exit 1
	touch /usr/.prepare-tools.ready || exit 1
fi

if [ ! -f /usr/.polkadot-js-api.ready ]; then
	yarn --global-folder /usr global add @polkadot/api-cli || exit 1
	/usr/node_modules/.bin/polkadot-js-api --version | grep -qE '[0-9]+\.[0-9]+\.[0-9]+' || exit 1
	touch /usr/.polkadot-js-api.ready || exit 1
fi

if [ ! -f /usr/.developer-tools.ready ]; then
	nix-env -i \
		findutils \
		gnused \
		gawk \
		gnumake \
		git \
		glibc \
		zlib \
		rustup \
		|| exit 1
	rustup update nightly || exit 1
	rustup target add wasm32-unknown-unknown --toolchain nightly || exit 1
	rustup update stable || exit 1
	for x in /nix/store/*-zlib-*/lib/libz.so.1
	do
		cp $x /root/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/
		rustc --version > /dev/null 2>&1 && break
	done
	rustc --version | grep -E "rustc [0-9]+\.[0-9]+\.[0-9]+\-nightly" || exit 1
	cargo --version | grep -E "cargo [0-9]+\.[0-9]+\.[0-9]+\-nightly" || exit 1
	touch /usr/.developer-tools.ready || exit 1
fi

if [ ! -f /usr/.container-tools.ready ]; then
	nix-env -i \
		socat \
		gnutar \
		wget \
		|| exit 1
	touch /usr/.container-tools.ready || exit 1
fi

if [ ! -f /usr/.configs.ready ]; then
#####################################

cat <<EOF > /root/.gitconfig
[advice]
        detachedHead = false
EOF

#####################################
fi

export GIT_SSL_CAINFO=/repos/parachain/misc/ca-certificates.crt
export SSL_CERT_FILE=/repos/parachain/misc/ca-certificates.crt

socat - UNIX-CONNECT:.inside_docker_jobs/locked/socket | exec bash
