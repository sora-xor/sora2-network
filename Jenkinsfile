@Library('jenkins-library' ) _

String agentLabel = 'docker-build-agent'
String registry = "docker.soramitsu.co.jp"
String dockerBuildToolsUserId = 'bot-build-tools-ro'
String dockerRegistryRWUserId = 'bot-sora2-rw'
String baseImageName = "docker.soramitsu.co.jp/sora2/substrate-env:latest"
String appImageName = "docker.soramitsu.co.jp/sora2/substrate"
String secretScannerExclusion = '.*Cargo.toml'
Boolean disableSecretScanner = false
def pushTags=['master': 'latest', 'develop': 'dev', 'staging': 'stage']

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
        stage('Secret scanner'){
            steps {
                script {
                    gitNotify("main-CI", "PENDING", "This commit is being built")
                    docker.withRegistry( "https://" + registry, dockerBuildToolsUserId) {
                        secretScanner(disableSecretScanner, secretScannerExclusion)
                    }
                }
            }
        }
        stage('Build & Tests') {
            environment {
                RUSTFLAGS = "-Dwarnings"
            }
            steps{
                script {
                    docker.withRegistry( "https://" + registry, dockerRegistryRWUserId) {
                        docker.image(baseImageName).inside() {
                            sh "cd ${env.WORKSPACE} && cargo fmt -- --check > /dev/null && cargo build --release --features \"test-net reduced-pswap-reward-periods runtime-benchmarks\""
                            sh "cp /opt/rust-target/release/framenode ${env.WORKSPACE}/housekeeping/framenode"
                            sh "cargo test --release"
                        }
                    }
                }
            }
        }
        stage('Build image') {
            steps{
                script {
                    sh "docker build -f housekeeping/docker/release/Dockerfile -t ${appImageName} ."
                }
            }
        }
        stage('Push Image') {
            when {
                expression { getPushVersion(pushTags) }
            }
            steps{
                script {
                    baseImageTag = "${getPushVersion(pushTags)}"
                    docker.withRegistry( "https://" + registry, dockerRegistryRWUserId) {
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
            script { gitNotify("main-CI", "SUCCESS", "Success")}
        }
        failure {
            script { gitNotify("main-CI", "FAILURE", "Failure")}
        }
        aborted {
            script { gitNotify("main-CI", "FAILURE", "Aborted")}
        }
        cleanup { cleanWs() }
    }
}