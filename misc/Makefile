.PHONY: cargo-build-release yarn-build localtestnet

RUN := $(shell which nix-shell > /dev/null && echo 'eval `cat ./nix-env.sh`; nix-shell --run ' || echo 'sh -c ')

localtestnet:
	./scripts/localtestnet.sh

cargo-build-release:
	${RUN} "cargo build --release"

yarn-build:
	${RUN} "yarn"
	${RUN} "yarn build"

