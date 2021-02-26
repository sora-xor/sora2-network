.PHONY: cargo-build-release-error-head cargo-test-release-error-head default docker-build-image docker-env-test docker-localtestnet docker-localtestnet-debug docker-localtestnet docker-build-release docker-test-release docker-build-debug docker-test-debug cargo-build-release cargo-build-debug cargo-test-release cargo-test-debug yarn-build

RUN := $(shell which nix-shell > /dev/null 2>&1 && echo 'nix-shell --run ' || echo 'sh -c ')
LAST_COMMIT := $(shell sh -c "git log | head -n 1 | awk '{ print \$$2 }'" 2> /dev/null)

default: cargo-build-release

cargo-test-release-error-head:
	make cargo-test-release  2>&1 | awk '/(error\[)|(^error: [^f])/{ start = 1 } { if (start) { print $0 } }' | head

cargo-build-release-error-head:
	make cargo-build-release  2>&1 | awk '/(error\[)|(^error: [^f])/{ start = 1 } { if (start) { print $0 } }' | head

docker-build-image:
	docker build -t soraneo-develop-nix .

docker-env-test:
	@./scripts/docker_compose_up.sh --with-last-commit --run "echo \$$PROTOC"

docker-localtestnet: $(LAST_COMMIT)

docker-localtestnet-debug:
	echo "Unimplemented"

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
	${RUN} "SKIP_WASM_BUILD=1 cargo test --release"

cargo-test-debug:
	${RUN} "SKIP_WASM_BUILD=1 cargo test"

cargo-check-release:
	${RUN} "SKIP_WASM_BUILD=1 cargo check --release"

yarn-build:
	${RUN} "yarn"
	${RUN} "yarn build"

.with_docker_jobs/localtestnet_testlog_from_commit_%.log:
	@./scripts/docker_compose_up.sh --commit $* --logfile $@ -- -e

%:
	@./scripts/docker_compose_up.sh --commit $@ --logfile .with_docker_jobs/localtestnet_testlog_from_commit_$@.log -- -e


