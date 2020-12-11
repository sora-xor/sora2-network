#!/usr/bin/env bash

test "$INSIDE_DOCKER" == "1" || exit 1
test "$PWD" == "/parachain"  || exit 1

if [ ! -f /usr/.prepare-tools.ready ]; then
	nix-env -i \
		which \
		gnugrep \
		nodejs \
		yarn \
		|| exit 1
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
		util-linux \
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

socket=.inside_docker_jobs/socket

if [ "$INSIDE_DOCKER_USE_LAST_COMMIT_BY_DEFAULT" == "1" -a "$INSIDE_DOCKER_USE_COMMIT" == "" ]; then
	pushd /repos/parachain
		INSIDE_DOCKER_USE_COMMIT=`git log | head -n 1 | awk '{ print $2 }'`
	popd
fi

if [ "$INSIDE_DOCKER_USE_COMMIT" != "" ]; then
	cd /parachain || exit 1
	git clone /repos/parachain || exit 1
	mv parachain/* . || exit 1
	mv parachain/.[a-hj-z]* . || exit 1
	git checkout "docker/build" || exit 1
	mv scripts scripts.docker || exit 1
	mv misc    misc.docker || exit 1
	git checkout "$INSIDE_DOCKER_USE_COMMIT" || exit 1
	if [ -d scripts ]; then
		mv scripts scripts.commit || exit 1
	fi
	if [ -d misc ]; then
		mv misc    misc.commit || exit 1
	fi
	mv scripts.docker scripts || exit 1
	mv misc.docker    misc || exit 1
	if [ "$INSIDE_DOCKER_RUN_COMMANDS" != "" ]; then
		bash -c "$INSIDE_DOCKER_RUN_COMMANDS"
		echo RUN COMMANDS IS FINISHED WITH CODE $?
	else
		bash ./scripts/localtestnet.sh
		echo RUN COMMANDS IS FINISHED WITH CODE $?
	fi
else
	if [ -e $socket ]; then
		socat - UNIX-CONNECT:$socket > ./.run_commands.sh
		bash ./.run_commands.sh
		echo RUN COMMANDS IS FINISHED WITH CODE $?
	else
		echo "# You can use command"
		echo "# ./scripts/docker_compose_up.sh --with-last-commit --run \"cargo build --release\""
	fi
fi




