@Library('jenkins-library')

String agentLabel             = 'docker-build-agent'
String registry               = 'docker.soramitsu.co.jp'
String dockerBuildToolsUserId = 'bot-build-tools-ro'
String dockerRegistryRWUserId = 'bot-sora2-rw'
String cargoAuditImage        = registry + '/build-tools/cargo_audit'
String envImageName           = registry + '/sora2/env'
String rustcVersion           = 'nightly-2021-12-10'
String wasmReportFile         = 'subwasm_report.json'
String appImageName           = 'docker.soramitsu.co.jp/sora2/substrate'
String secretScannerExclusion = '.*Cargo.toml'
Boolean disableSecretScanner  = false
String featureList            = 'private-net include-real-files reduced-pswap-reward-periods'
Map pushTags                  = ['master': 'latest', 'develop': 'dev','substrate-4.0.0': 'sub4']

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
                    docker.withRegistry('https://' + registry, dockerBuildToolsUserId) {
                        secretScanner(disableSecretScanner, secretScannerExclusion)
                    }
                }
            }
        }
        stage('Audit') {
            steps {
                script {
                    docker.withRegistry( 'https://' + registry, dockerBuildToolsUserId) {
                        docker.image(cargoAuditImage + ':latest').inside(){
                            sh '''
                               cargo audit  > cargoAuditReport.txt || exit 0
                            '''
                            archiveArtifacts artifacts: "cargoAuditReport.txt"
                        }
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
                    docker.withRegistry('https://' + registry, dockerRegistryRWUserId) {
                        if (getPushVersion(pushTags)) {
                            docker.image(envImageName + ':sub4').inside() {
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
                                    mold --run cargo test  --release --features runtime-benchmarks
                                    mold --run cargo build --release --features \"${featureList}\"
                                    mv /app/target/release/framenode .
                                    wasm-opt -Os -o ./framenode_runtime.compact.wasm /app/target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                                    subwasm --json info framenode_runtime.compact.wasm > ${wasmReportFile}
                                """
                                archiveArtifacts artifacts:
                                    "framenode_runtime.compact.wasm, ${wasmReportFile}"
                            }
                        } else {
                            docker.image(envImageName + ':sub4').inside() {
                                sh '''
                                    cargo fmt -- --check > /dev/null
                                    mold --run cargo test 
                                    mold --run cargo test --features private-net
                                    mold --run cargo test --features runtime-benchmarks
                                '''
                            }
                        }
                    }
                }
            }
        }
        stage('Code Coverage') {
            when {
                expression { getPushVersion(pushTags) }
            }
            steps {
                script {
                    docker.withRegistry('https://' + registry, dockerRegistryRWUserId) {
                        docker.image(envImageName + ':sub4').inside() {
                            sh './housekeeping/coverage.sh'
                            cobertura coberturaReportFile: 'cobertura_report'
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
                    docker.withRegistry('https://' + registry, dockerRegistryRWUserId) {
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
        always {
            script{
                gitNotify('main-CI', currentBuild.result, currentBuild.result)
            }
        }
        cleanup { cleanWs() }
    }
}