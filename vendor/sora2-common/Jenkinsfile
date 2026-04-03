@Library('jenkins-library') _

def pipeline = new org.rust.AppPipeline(steps: this,
      envImageName: 'docker.soramitsu.co.jp/sora2/env:env',
      appImageName: 'docker.soramitsu.co.jp/sora2/parachain',
      buildTestCmds: 'housekeeping/tests.sh',
      disableCodeCoverage: true,
      sonarProjectKey: 'sora:sora2-common',
      sonarProjectName: 'sora2-common',
      dojoProductType: 'sora',
      clippyLinter: false
)
pipeline.runPipeline()
