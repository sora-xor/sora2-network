.PHONE: dev test staging

FEATURES = reduced-pswap-reward-periods coded-nets

default: dev test staging

dev:
	cargo run --release --features "$(FEATURES) dev-net" -- build-spec --chain dev-coded --raw --disable-default-bootnode > ./node/src/chain_spec/bytes/chain_spec_dev.json

test:
	cargo run --release --features "$(FEATURES) test-net" -- build-spec --chain test-coded --raw > ./node/src/chain_spec/bytes/chain_spec_test.json

staging:
	cargo run --release --features "$(FEATURES) stage-net" -- build-spec --chain staging-coded --raw > ./node/src/chain_spec/bytes/chain_spec_staging.json
