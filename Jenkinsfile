@Library('jenkins-library@feature/dops-2395/rust_library') _

def pipeline = new org.rust.substratePipeline(steps: this,
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
      substrate: true
)
pipeline.runPipeline()
