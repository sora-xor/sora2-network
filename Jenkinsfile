@Library('jenkins-library')

String agentLabel = 'docker-build-agent'
String registry = 'docker.soramitsu.co.jp'
String dockerBuildToolsUserId = 'bot-build-tools-ro'
String dockerRegistryRWUserId = 'bot-sora2-rw'
String baseImageName = 'docker.soramitsu.co.jp/sora2/substrate-env:latest'
String srtoolImageName = 'paritytech/srtool:nightly-2021-03-15'
String rustcVersion = 'nightly-2021-03-11'
String srtoolReportFile = 'framenode_runtime_srtool_output.json'
String appImageName = 'docker.soramitsu.co.jp/sora2/substrate'
String secretScannerExclusion = '.*Cargo.toml'
Boolean disableSecretScanner = false
String featureList = 'private-net include-real-files reduced-pswap-reward-periods'
def pushTags = ['master': 'latest', 'develop': 'dev']

pipeline {
    options {
        buildDiscarder(logRotator(numToKeepStr: '20'))
        timestamps()
        disableConcurrentBuilds()
    }

    agent {
        label agentLabel
    }

    stages {
        stage('Secret scanner') {
            steps {
                script {
                    gitNotify('main-CI', 'PENDING', 'This commit is being built')
                    docker.withRegistry( 'https://' + registry, dockerBuildToolsUserId) {
                        secretScanner(disableSecretScanner, secretScannerExclusion)
                    }
                }
            }
        }
        stage('Build & Tests') {
            environment {
                PACKAGE = 'framenode-runtime'
                RUSTFLAGS = '-Dwarnings'
                RUNTIME_DIR = 'runtime'
                RUSTC_VERSION = "${rustcVersion}"
            }
            steps {
                script {
                    docker.withRegistry( 'https://' + registry, dockerRegistryRWUserId) {
                        docker.image(baseImageName).inside() {
                            if (getPushVersion(pushTags)) {
                                if (env.TAG_NAME =~ 'benchmarking.*') {
                                    featureList = 'runtime-benchmarks main-net-coded'
                                }
                                else if (env.TAG_NAME =~ 'stage.*') {
                                    featureList = 'private-net include-real-files'
                                }
                                else if (env.TAG_NAME =~ 'test.*') {
                                    featureList = 'private-net include-real-files reduced-pswap-reward-periods'
                                }
                                else if (env.TAG_NAME) {
                                    featureList = 'include-real-files'
                                }
                                sh """
                                    cargo build --release --features \"${featureList}\"
                                    cp target/release/framenode housekeeping/framenode
                                    cp target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm housekeeping/framenode_runtime.compact.wasm
                                    cargo test --release
                                """
                                archiveArtifacts artifacts: 'housekeeping/framenode_runtime.compact.wasm'
                            } else {
                                sh '''
                                    cargo fmt -- --check > /dev/null
                                    cargo check
                                    cargo test
                                    cargo check --features private-net
                                    cargo test --features private-net
                                    cargo check --features runtime-benchmarks
                                '''
                            }
                        }
                    }
                    docker.image(srtoolImageName).inside("-v ${env.WORKSPACE}:/build") { c ->
                        if (getPushVersion(pushTags)) {
                            sh "build --json | tee ${srtoolReportFile}"
                            archiveArtifacts artifacts: srtoolReportFile
                        }
                    }
                }
            }
        }
        stage('Code Coverage') {
            steps {
                script {
                    docker.withRegistry( 'https://' + registry, dockerRegistryRWUserId) {
                        docker.image(baseImageName).inside() {
                            sh './coverage.sh'
                            cobertura coberturaReportFile: 'target/debug/report'
                        }
                    }
                }
            }
        }
        stage('Push Image') {
            when {
                expression { getPushVersion(pushTags) }
            }
            steps {
                script {
                    sh "docker build -f housekeeping/docker/release/Dockerfile -t ${appImageName} ."
                    baseImageTag = "${getPushVersion(pushTags)}"
                    docker.withRegistry( 'https://' + registry, dockerRegistryRWUserId) {
                        sh """
                            docker tag ${appImageName} ${appImageName}:${baseImageTag}
                            docker push ${appImageName}:${baseImageTag}
                        """
                    }
                    docker.withRegistry('https://index.docker.io/v1/', 'docker-hub-credentials') {
                        sh """
                            docker tag ${appImageName} sora2/substrate:${baseImageTag}
                            docker push sora2/substrate:${baseImageTag}
                        """
                    }
                }
            }
        }
    }
    post {
        success {
            script { gitNotify('main-CI', 'SUCCESS', 'Success') }
        }
        failure {
            script { gitNotify('main-CI', 'FAILURE', 'Failure') }
        }
        aborted {
            script { gitNotify('main-CI', 'FAILURE', 'Aborted') }
        }
        cleanup { cleanWs() }
    }
}
