@Library('jenkins-library')

String agentLabel             = 'docker-build-agent'
String registry               = 'docker.soramitsu.co.jp'
String dockerBuildToolsUserId = 'bot-build-tools-ro'
String dockerRegistryRWUserId = 'bot-sora2-rw'
String envImageName           = 'docker.soramitsu.co.jp/sora2/env'
String srtoolImageName        = 'paritytech/srtool:nightly-2021-03-15'
String rustcVersion           = '1.54'
String srtoolReportFile       = 'framenode_runtime_srtool_output.json'
String appImageName           = 'docker.soramitsu.co.jp/sora2/substrate'
String secretScannerExclusion = '.*Cargo.toml'
Boolean disableSecretScanner  = false
String featureList            = 'private-net include-real-files reduced-pswap-reward-periods'
Map pushTags                  = ['substrate-4.0.0': 'sub4']

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
        stage('Build & Tests') {
            environment {
                PACKAGE       = 'framenode-runtime'
                RUSTFLAGS     = '-Dwarnings'
                RUNTIME_DIR   = 'runtime'
                RUSTC_VERSION = "${rustcVersion}"
            }
            steps {
                script {
                    docker.withRegistry('https://' + registry, dockerRegistryRWUserId) {
                        if (getPushVersion(pushTags)) {
                            docker.image(envImageName + ':latest').inside() {
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
                                sh """#!/bin/bash
                                    time cargo build --release --features \"${featureList}\" --target-dir /app/target/
                                    time cargo test  --release --target-dir /app/target/
                                    sccache -s
                                    time mv /app/target/release/framenode .
                                    time wasm-opt -Os -o ./framenode_runtime.compact.wasm /app/target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                                """
                                archiveArtifacts artifacts:
                                    'framenode_runtime.compact.wasm'
                            }
                        } else {
                            docker.image(envImageName + ':dev').inside() {
                                sh '''#!/bin/bash
                                    time cargo fmt -- --check > /dev/null
                                    time cargo check --target-dir /app/target/
                                    time cargo test  --target-dir /app/target/
                                    time cargo check --features private-net        --target-dir /app/target/
                                    time cargo test  --features private-net        --target-dir /app/target/
                                    time cargo check --features runtime-benchmarks --target-dir /app/target/
                                    sccache -s
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
            when {
                expression { getPushVersion(pushTags) }
            }
            steps {
                script {
                    docker.withRegistry('https://' + registry, dockerRegistryRWUserId) {
                        docker.image(envImageName + ':latest').inside() {
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
