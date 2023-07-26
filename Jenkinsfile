@Library('jenkins-library@duty/fix_rust_push') _

def pipeline = new org.rust.AppPipeline(steps: this,
      initSubmodules: true,
      envImageName: 'docker.soramitsu.co.jp/sora2/env:sub4',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      codeCoverageCommand: './housekeeping/coverage.sh',
      pushTags: ['f33r0-patch-1':'f33r0-patch-1'],
      cargoDoc: true,
      smartContractScanner: true,
      buildTestCmds: ['housekeeping/build.sh'],
      buildArtifacts: 'framenode_runtime.compact.wasm, framenode_runtime.compact.compressed.wasm, subwasm_report.json, pallet_list.txt',
      pushToPublicRegistry: true
)
pipeline.runPipeline()
