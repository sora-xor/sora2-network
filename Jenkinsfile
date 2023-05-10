@Library('jenkins-library@feature/dops-2395/rust_library') _

def featureList    = 'private-net include-real-files reduced-pswap-reward-periods wip ready-to-test'
def palletListFile = 'pallet_list.txt'
def wasmReportFile = 'subwasm_report.json'
def sudoCheckStatus = 0

def pipeline = new org.rust.substratePipeline(steps: this,
      assignReviewers: true,
      disableSecretScanner: false,
      secretScannerExclusion: '.*Cargo.toml\$|.*pr.sh\$|.*Jenkinsfile\$',
      rustcVersion: 'nightly-2021-12-10',
      dockerImageTags: ['develop': 'dev', 'master': 'latest'],
      contractsPath: 'ethereum-bridge-contracts',
      contractsEnvFile: 'env.template',
      cargoDocImage: 'rust:1.62.0-slim-bullseye',
      githubPrCreator: 'ubuntu:jammy-20221020',
      envImageName: 'docker.soramitsu.co.jp/sora2/env:sub4',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      codeCoverage: true,
      staticScanner: true,
      substrate: true,
      cargoDoc: true,
      sendMessage: true,
      buildTestCmds: [
        'echo "with tag"',
        'cargo test  --release --features \"private-net runtime-benchmarks\"',
        'rm -rf target',
        "cargo build --release --features \'${featureList}\'",
        'mv ./target/release/framenode .',
        'mv ./target/release/relayer ./relayer.bin',
        'mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm',
        'wasm-opt -Os -o ./framenode_runtime.compact.wasm ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm',
        "subwasm --json info framenode_runtime.compact.wasm > ${wasmReportFile}",
        "subwasm metadata framenode_runtime.compact.wasm > ${palletListFile}",
        'set +e',
        'subwasm metadata -m Sudo target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm',
        'if [ \$(echo \$?) -eq \"${sudoCheckStatus}\" ]; then echo "sudo check is successful!"; else echo "sudo check is failed!"; exit 1; fi'
      ],
      buildTestCmdsWithoutTag: [
        'echo "without tag"',
        'rm -rf ~/.cargo/.package-cache',
        'rm Cargo.lock',
        'cargo fmt -- --check > /dev/null',
        'SKIP_WASM_BUILD=1 cargo check',
        'SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test',
        'SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test,wip',
        'cargo test',
        'cargo test --features \"private-net wip ready-to-test runtime-benchmarks\"'
      ]
      )
pipeline.runPipeline()
