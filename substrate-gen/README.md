# substrate-gen

The crate generates runtime metadata file.

```sh
SKIP_WASM_BUILD=1 cargo run -p substrate-gen --feature private-net,include-real-files,reduced-pswap-reward-periods,wip,ready-to-test
```

The file might be found in `/substrate-gen/bytes/metadata.scale`
