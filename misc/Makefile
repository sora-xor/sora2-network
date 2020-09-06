.PHONY: default docker-test localtestnet-run cargo-build-release cargo-build cargo-test-release cargo-test yarn-build

RUN := $(shell which nix-shell > /dev/null && echo 'eval `cat ./nix-env.sh`; nix-shell --run ' || echo 'sh -c ')
LAST_COMMIT := $(shell sh -c "git log | head -n 1 | awk '{ print \$$2 }'" 2> /dev/null)

default: cargo-build-release

docker-test: $(LAST_COMMIT)

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

.with_docker_jobs/localtestnet_testlog_from_commit_%.log:
	@./scripts/docker_compose_up.sh $* $@ -ef

%:
	@./scripts/docker_compose_up.sh $@ .with_docker_jobs/localtestnet_testlog_from_commit_$@.log -ef


