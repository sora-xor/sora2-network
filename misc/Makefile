.PHONY: default docker-env-test docker-localtestnet docker-localtestnet docker-build-release docker-test-release docker-build-debug docker-test-debug cargo-build-release cargo-build-debug cargo-test-release cargo-test-debug yarn-build

RUN := $(shell which nix-shell > /dev/null && echo 'eval "`cat ./nix-env.sh`"; nix-shell --run ' || echo 'sh -c ')
LAST_COMMIT := $(shell sh -c "git log | head -n 1 | awk '{ print \$$2 }'" 2> /dev/null)

default: cargo-build-release

docker-env-test:
	@./scripts/docker_compose_up.sh --with-last-commit --run "echo \$$PROTOC"

docker-localtestnet: $(LAST_COMMIT)

docker-build-release:
	@./scripts/docker_compose_up.sh --with-last-commit --run "cargo build --release"

docker-build-debug:
	@./scripts/docker_compose_up.sh --with-last-commit --run "cargo build"

docker-test-release:
	@./scripts/docker_compose_up.sh --with-last-commit --run "cargo test --release"

docker-test-debug:
	@./scripts/docker_compose_up.sh --with-last-commit --run "cargo test"

cargo-build-release:
	${RUN} "cargo build --release"

cargo-build-debug:
	${RUN} "cargo build"

cargo-test-release:
	${RUN} "cargo test --release"

cargo-test-debug:
	${RUN} "cargo test"

yarn-build:
	${RUN} "yarn"
	${RUN} "yarn build"

.with_docker_jobs/localtestnet_testlog_from_commit_%.log:
	@./scripts/docker_compose_up.sh $* $@ -ef

%:
	@./scripts/docker_compose_up.sh --commit $@ --logfile .with_docker_jobs/localtestnet_testlog_from_commit_$@.log -- -ef


