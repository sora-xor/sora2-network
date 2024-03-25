@Library('jenkins-library@feature/dops-3101') _

def pipeline = new org.rust.AppPipeline(steps: this,
      initSubmodules: true,
      envImageName: 'docker.soramitsu.co.jp/sora2/env:env',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      codeCoverageCommand: './housekeeping/coverage.sh',
      cargoDoc: true,
      smartContractScanner: false,
      clippyLinter: false,
      cargoClippyTag: ':substrate',
      pushTags: ['master': 'dev'],
      cargoClippyCmds: ['housekeeping/clippy.sh'],
      buildTestCmds: 'housekeeping/build.sh',
      buildArtifacts: 'framenode_runtime.compact.compressed.wasm, subwasm_report.json, pallet_list.txt',
      pushToPublicRegistry: true,
      sonarProjectKey: 'sora:sora2-network',
      sonarProjectName: 'sora2-network',
      dojoProductType: 'sora'
)
pipeline.runPipeline()
