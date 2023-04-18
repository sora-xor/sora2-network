@Library('jenkins-library')

String agentLabel             = 'docker-build-agent'
String registry               = 'docker.soramitsu.co.jp'
String dockerBuildToolsUserId = 'bot-build-tools-ro'
String dockerRegistryRWUserId = 'bot-sora2-rw'
String cargoAuditImage        = registry + '/build-tools/cargo_audit'
String envImageName           = registry + '/sora2/env:sub4'
String rustcVersion           = 'nightly-2021-12-10'
String wasmReportFile         = 'subwasm_report.json'
String palletListFile         = 'pallet_list.txt'
String appImageName           = 'docker.soramitsu.co.jp/sora2/substrate'
String secretScannerExclusion = '.*Cargo.toml\$|.*pr.sh\$'
Boolean disableSecretScanner  = false
int sudoCheckStatus           = 0
String featureList            = 'private-net include-real-files reduced-pswap-reward-periods wip ready-to-test'
Map pushTags                  = ['master': 'latest', 'develop': 'dev']

String contractsPath          = 'ethereum-bridge-contracts'
String contractsEnvFile       = 'env.template'
String solcVersion            = '0.8.14'
String nodeVersion            = '14.16.1'
String gitHubUser             = 'sorabot'
String gitHubRepo             = 'github.com/sora-xor/sora2-network.git'
String gitHubBranch           = 'doc'
String gitHubEmail            = 'admin@soramitsu.co.jp'
String cargoDocImage          = 'rust:1.62.0-slim-bullseye'
String githubPrCreator        = 'ubuntu:jammy-20221020'
String checkChangesToRegexp   = '(Jenkinsfile|housekeeping|liquidity-proxy|common|pallets|)'
Boolean hasChanges(String regexp) {
    echo "Comparing current changes with origin/${env.CHANGE_TARGET}"
    return !env.CHANGE_TARGET || sh(
        returnStatus: true,
        returnStdout: true,
        script: "(git diff-tree --name-only origin/${env.CHANGE_TARGET} ${env.GIT_COMMIT} | egrep '${regexp}')"
    ) == 0
}
Boolean prStatusNotif = true
String telegramChatId    = 'telegram-deploy-chat-id'
String telegramChatIdPswap = 'telegramChatIdPswap'

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
                                rm -rf ~/.cargo/.package-cache
                                rm Cargo.lock
                                cargo audit  > cargoAuditReport.txt || exit 0
                            '''
                            archiveArtifacts artifacts: "cargoAuditReport.txt"
                        }
                    }
                }
            }
        }
        stage('Init submodule') {
            environment {
                GIT_SSH_COMMAND = "ssh -o UserKnownHostsFile=/dev/null StrictHostKeyChecking=no"
            }
            steps {
                script {
                    sshagent(['soramitsu-bot-ssh']) {
                        sh """
                        git submodule update --init --recursive
                        """
                    }
                }
            }
        }
        stage('Solidity Static Scanner') {
            steps {
                script {
                    docker.withRegistry('https://' + registry, dockerBuildToolsUserId) {
                        slither(contractsPath, contractsEnvFile, solcVersion, nodeVersion)
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
                            docker.image(envImageName).inside() {
                                if (env.TAG_NAME =~ 'benchmarking.*') {
                                    featureList = 'private-net runtime-benchmarks main-net-coded'
                                    sudoCheckStatus = 101
                                }
                                else if (env.TAG_NAME =~ 'stage.*') {
                                    featureList = 'private-net include-real-files ready-to-test'
                                    sudoCheckStatus = 0
                                }
                                else if (env.TAG_NAME =~ 'test.*') {
                                    featureList = 'private-net include-real-files reduced-pswap-reward-periods ready-to-test'
                                    sudoCheckStatus = 0
                                }
                                else if (env.TAG_NAME) {
                                    featureList = 'include-real-files'
                                    sudoCheckStatus = 101
                                }
                                sh """
                                    cargo test  --release --features \"private-net runtime-benchmarks\"
                                    rm -rf target
                                    cargo build --release --features \"${featureList}\"
                                    mv ./target/release/framenode .
                                    mv ./target/release/relayer ./relayer.bin
                                    mv ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm ./framenode_runtime.compact.compressed.wasm
                                    wasm-opt -Os -o ./framenode_runtime.compact.wasm ./target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                                    subwasm --json info framenode_runtime.compact.wasm > ${wasmReportFile}
                                    subwasm metadata framenode_runtime.compact.wasm > ${palletListFile}
                                    set +e
                                    subwasm metadata -m Sudo target/release/wbuild/framenode-runtime/framenode_runtime.compact.wasm
                                    if [ \$(echo \$?) -eq \"${sudoCheckStatus}\" ]; then echo "sudo check is successful!"; else echo "sudo check is failed!"; exit 1; fi
                                """
                                archiveArtifacts artifacts:
                                    "framenode_runtime.compact.wasm, framenode_runtime.compact.compressed.wasm, ${wasmReportFile}, ${palletListFile}"
                            }
                        } else {
                            docker.image(envImageName).inside() {
                                sh '''
                                    rm -rf ~/.cargo/.package-cache
                                    rm Cargo.lock
                                    cargo fmt -- --check > /dev/null
                                    cargo test
                                    cargo test --features \"private-net wip ready-to-test\"
                                    cargo test --features \"private-net wip ready-to-test runtime-benchmarks\"
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
                        docker.image(envImageName).inside() {
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
        stage('Build docs & publish') {
            when {
                expression { return (env.GIT_BRANCH == "master" || env.TAG_NAME) }
            }
            environment {
                GH_USER = "${gitHubUser}"
                GH_TOKEN = credentials('sorabot-github-token')
                GH_REPOSITORY = "${gitHubRepo}"
                GH_BRANCH = "${gitHubBranch}"
                GH_EMAIL  = "${gitHubEmail}"
            }
            steps {
                script {
                    docker.image("${cargoDocImage}").inside() {
                             sh './housekeeping/docs.sh'
                    }
                }
            }
        }
        stage('Assign reviewers to PR') {
            when { 
                allOf {
                expression { hasChanges(checkChangesToRegexp) }
                expression { env.BRANCH_NAME.startsWith('PR-') }
                }
            }
            environment {
                GH_USER = "${gitHubUser}"
                GH_TOKEN = credentials('sorabot-github-token')
                GH_REPOSITORY = "${gitHubRepo}"
                GH_EMAIL  = "${gitHubEmail}"
                BRANCH_NAME_PR = "${env.BRANCH_NAME.startsWith('PR-')}"
                BRANCH_NAME = "${env.BRANCH_NAME}"
                BRANCH_NAME_TO_SWITCH = "${env.GIT_BRANCH}"
                CHANGE_TARGET = "${env.CHANGE_TARGET}"
                GIT_COMMIT = "${env.GIT_COMMIT}"
                GIT_AUTHOR = "${env.GIT_AUTHOR_NAME}"
            }
            steps {
                script {
                    docker.image("${githubPrCreator}").inside() {
                        sh './housekeeping/pr.sh'
                        RESULT=sh (
                            script : 'git diff-tree --name-only origin/$CHANGE_TARGET $GIT_COMMIT',
                            returnStdout: true
                        ).trim()
                        
                    }
                }
            }
        }
        stage ('Send Notification about PR') {
            when { 
                allOf {
                expression { prStatusNotif }
                expression { env.BRANCH_NAME.startsWith('PR-') }
                }
            }
            environment {
                TELEGRAM_CHAT_ID = credentials("${telegramChatId}")
                TELEGRAM_CHAT_ID_PSWAP = credentials("${telegramChatIdPswap}")
                RESULT = "${RESULT}"
            }
            steps {
                pushNotiTelegram(
                    prStatusNotif: prStatusNotif,
                    telegramChatId: "${TELEGRAM_CHAT_ID}",
                    telegramChatIdPswap: "${TELEGRAM_CHAT_ID_PSWAP}"
                )
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
