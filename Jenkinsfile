@Library('jenkins-library@feature/dops-2395/rust_library') _

def pipeline = new org.rust.substratePipeline(steps: this,
      secretScannerExclusion: '.*Cargo.toml\$|.*pr.sh\$'
)
pipeline.runPipeline()