@Library('jenkins-library@feature/dops-2395/rust_library') _

def pipeline = new org.rust.substratePipeline(steps: this,
      assignReviewers: true,
      disableSecretScanner: false,
      secretScannerExclusion: '.*Cargo.toml\$|.*pr.sh\$',
      rustcVersion: 'nightly-2021-12-10',
      dockerImageTags: ['develop': 'dev', 'feature/dops-2387/fix_ci_build': 'benchmarking.1', 'master': 'latest'],
      contractsPath: 'ethereum-bridge-contracts',
      contractsEnvFile: 'env.template',
      envImageName: 'docker.soramitsu.co.jp/sora2/env:sub4',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      codeCoverage: true,
      substrate: true,
      cargoDoc: true,
      prStatusNotif: true,
      buildTestCmds: [
        'housekeeping/build.sh'
      ]
      )
pipeline.runPipeline()
