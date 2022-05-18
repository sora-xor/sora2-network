export RUST_LOG=info,relayer=debug 

cargo run --bin relayer --release -- \
    ethereum-relay \
    --base-path /tmp/relayer \
    --ethereum-url ws://localhost:8546 \
    --substrate-url ws://localhost:9944 \
    --substrate-key //Bob
