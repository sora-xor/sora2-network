.PHONY: default testlog localtestnet-run cargo-build-release cargo-build cargo-test-release cargo-test yarn-build

RUN := $(shell which nix-shell > /dev/null && echo 'eval `cat ./nix-env.sh`; nix-shell --run ' || echo 'sh -c ')
LAST_COMMIT := $(shell sh -c "git log | head -n 1 | awk '{ print \$$2 }'" 2> /dev/null)

default: cargo-build-release

testlog: .with_docker_jobs/localtestnet_testlog_from_commit_$(LAST_COMMIT).log
	less -R .with_docker_jobs/localtestnet_testlog_from_commit_$(LAST_COMMIT).log

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

.with_docker_jobs/localtestnet_testlog_from_commit_%.log:
	@mkdir -p .inside_docker_jobs/locked
	@rm -f .inside_docker_jobs/locked/socket
	@flock .inside_docker_jobs/locked -c "./scripts/docker_compose_up.sh $*" | tee $@.tmp
	@mv $@.tmp $@
	@exit

