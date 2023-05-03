@Library('jenkins-library@feature/dops-2395/rust_library') _

def pipeline = new org.rust.substratePipeline(steps: this,
      disableSecretScanner: true,
      secretScannerExclusion: '.*Cargo.toml\$|.*pr.sh\$|.*Jenkinsfile\$',
      palletListFile: 'pallet_list.txt',
      wasmReportFile: 'subwasm_report.json',
      rustcVersion: 'nightly-2021-12-10',
      featureList: 'private-net include-real-files reduced-pswap-reward-periods wip ready-to-test',
      dockerImageTags: ['develop': 'dev', 'master': 'latest'],
      contractsPath: 'ethereum-bridge-contracts',
      contractsEnvFile: 'env.template',
      cargoDocImage: 'rust:1.62.0-slim-bullseye',
      githubPrCreator: 'ubuntu:jammy-20221020',
      envImageName: 'docker.soramitsu.co.jp/sora2/env:sub4',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      substrate: true,
      // buildTestCmds: [
      //     if (dockerImageTag) {
      //         if (steps.env.TAG_NAME =~ 'benchamarking.*') {
      //           featureList = 'private-net runtime-benchmarks'
      //           sudoCheckStatus = 101
      //         }
      //         if (steps.env.TAG_NAME =~ 'stage.*') {
      //           featureList = 'private-net include-real-files ready-to-test'
      //           sudoCheckStatus = 0
      //         }
      //         if (steps.env.TAG_NAME =~ 'test.*') {
      //           featureList = 'private-net include-real-files reduced-pswap-reward-periods ready-to-test'
      //           sudoCheckStatus = 0
      //         }
      //         if (steps.env.TAG_NAME) {
      //           featureList = 'include-real-files'
      //           sudoCheckStatus = 101
      //         }
      //         steps.sh '''
      //           cargo test  --release --features \"private-net runtime-benchmarks\"
      //           rm -rf target
      //           cargo build --release --features \"${featureList}\"
      //           mv ./target/release/framenode .
      //           mv ./target/release/relayer ./relayer.bin
      //           mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
      //           wasm-opt -Os -o ./framenode_runtime.compact.wasm ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
      //           subwasm --json info framenode_runtime.compact.wasm > ${wasmReportFile}
      //           subwasm metadata framenode_runtime.compact.wasm > ${palletListFile}
      //           set +e
      //           subwasm metadata -m Sudo target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
      //           if [ \$(echo \$?) -eq \"${sudoCheckStatus}\" ]; then echo "sudo check is successful!"; else echo "sudo check is failed!";
      //         '''
      //         steps.archiveArtifacts "framenode_runtime.compact.wasm, framenode_runtime.compact.compressed.wasm, ${wasmReportFile}, ${palletListFile}"
      //       } else {
      //         steps.docker.image(envImageName).inside('-v /var/run/docker.sock:/var/run/docker.sock') {
      //           steps.sh '''
      //             rm -rf ~/.cargo/.package-cache
      //             rm Cargo.lock
      //             cargo fmt -- --check > /dev/null
      //             SKIP_WASM_BUILD=1 cargo check
      //             SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test
      //             SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test,wip
      //             cargo test
      //             cargo test --features \"private-net wip ready-to-test runtime-benchmarks\"
      //           '''
      //         }
      //       }
      // ]
      cmds: '''
        steps.sh '''
          if (dockerImageTag) {
              if (steps.env.TAG_NAME =~ 'benchamarking.*') {
                featureList = 'private-net runtime-benchmarks'
                sudoCheckStatus = 101
              }
              if (steps.env.TAG_NAME =~ 'stage.*') {
                featureList = 'private-net include-real-files ready-to-test'
                sudoCheckStatus = 0
              }
              if (steps.env.TAG_NAME =~ 'test.*') {
                featureList = 'private-net include-real-files reduced-pswap-reward-periods ready-to-test'
                sudoCheckStatus = 0
              }
              if (steps.env.TAG_NAME) {
                featureList = 'include-real-files'
                sudoCheckStatus = 101
              }
              steps.sh """
                cargo test  --release --features \"private-net runtime-benchmarks\"
                rm -rf target
                cargo build --release --features \"${featureList}\"
                mv ./target/release/framenode .
                mv ./target/release/relayer ./relayer.bin
                mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
                wasm-opt -Os -o ./framenode_runtime.compact.wasm ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                subwasm --json info framenode_runtime.compact.wasm > ${wasmReportFile}
                subwasm metadata framenode_runtime.compact.wasm > ${palletListFile}
                set +e
                subwasm metadata -m Sudo target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                if [ \$(echo \$?) -eq \"${sudoCheckStatus}\" ]; then echo "sudo check is successful!"; else echo "sudo check is failed!";
              """
              steps.archiveArtifacts "framenode_runtime.compact.wasm, framenode_runtime.compact.compressed.wasm, ${wasmReportFile}, ${palletListFile}"
            } else {
              steps.docker.image(envImageName).inside('-v /var/run/docker.sock:/var/run/docker.sock') {
                steps.sh """
                  rm -rf ~/.cargo/.package-cache
                  rm Cargo.lock
                  cargo fmt -- --check > /dev/null
                  SKIP_WASM_BUILD=1 cargo check
                  SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test
                  SKIP_WASM_BUILD=1 cargo check --features private-net,ready-to-test,wip
                  cargo test
                  cargo test --features \"private-net wip ready-to-test runtime-benchmarks\"
                """
              }
            } '''
      ''')
pipeline.runPipeline()
