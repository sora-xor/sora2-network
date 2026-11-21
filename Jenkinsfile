@Library('jenkins-library') _

// Release publishing must not trigger the inherited test-network runtime upgrade.
class PublishOnlyRustPipeline extends org.rust.AppPipeline {
  @Override
  public void pushDockerImage() {
    if (steps.env.TAG_NAME in ['4.8.15', '4.8.16']) {
      steps.echo("SORA Nexus ${steps.env.TAG_NAME} images use the verified-artifact publication process.")
      return
    }
    super.pushDockerImage()
  }

  @Override
  public void runtimeUpgrade() {
    steps.echo('SORA Nexus runtime deployment is disabled; normal checks and artifact publication remain enabled.')
  }
}

def pipeline = new PublishOnlyRustPipeline(steps: this,
      initSubmodules: true,
      envImageName: 'docker.soramitsu.co.jp/sora2/env:latest',
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
