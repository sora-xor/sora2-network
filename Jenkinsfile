@Library('jenkins-library@feature/dops-2395/rust_library') _

def featureList    = 'private-net include-real-files reduced-pswap-reward-periods wip ready-to-test'
def palletListFile = 'pallet_list.txt'
def wasmReportFile = 'subwasm_report.json'
def sudoCheckStatus = 0

def pipeline = new org.rust.substratePipeline(steps: this,
      assignReviewers: true,
      disableSecretScanner: false,
      secretScannerExclusion: '.*Cargo.toml\$|.*pr.sh\$',
      rustcVersion: 'nightly-2021-12-10',
      dockerImageTags: ['develop': 'dev', 'PR-437': 'benchmarking', 'master': 'latest'],
      contractsPath: 'ethereum-bridge-contracts',
      contractsEnvFile: 'env.template',
      prStatusNotif: true,
      envImageName: 'docker.soramitsu.co.jp/sora2/env:sub4',
      appImageName: 'docker.soramitsu.co.jp/sora2/substrate',
      codeCoverage: true,
      staticScanner: true,
      substrate: true,
      cargoDoc: true,
      buildTestCmds: [
        'housekeeping/build.sh'
      ]
      )
pipeline.runPipeline()
