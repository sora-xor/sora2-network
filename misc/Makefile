.PHONY: localtestnet-run cargo-build-release cargo-build cargo-test-release cargo-test yarn-build

RUN := $(shell which nix-shell > /dev/null && echo 'eval `cat ./nix-env.sh`; nix-shell --run ' || echo 'sh -c ')

localtestnet-run:
	./scripts/localtestnet.sh

cargo-build-release:
	${RUN} "cargo build --release"

cargo-build:
	${RUN} "cargo build"

cargo-test-release:
	${RUN} "cargo test --release"

cargo-test:
	${RUN} "cargo test"

yarn-build:
	${RUN} "yarn"
	${RUN} "yarn build"

