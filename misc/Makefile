.PHONY: cargo yarn

RUN := $(shell which nix-shell > /dev/null && echo 'eval `cat ./nix-env.sh`; nix-shell --run ' || echo 'sh -c ')

cargo:
	${RUN} "cargo build --release"

yarn:
	${RUN} "yarn"
	${RUN} "yarn build"

