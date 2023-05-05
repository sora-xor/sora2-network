#!/bin/bash
# Installing dependencies
echo 'Installing dependencies'
apt-get update
apt-get install curl -y
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
&& chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
&& echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
&& apt-get update \
&& apt-get install git -y \
&& apt-get install gh -y \
&& apt-get install jq -y

# git config
echo 'git config'
git config --global user.email ${GH_EMAIL}
git config --global user.name ${GH_USER}
git config --global github.token ${GH_TOKEN}
git config --global credential.helper 'cache --timeout 43200'
gh config set prompt disabled
GITHUB_TOKEN=${GH_TOKEN}

# Logining to github
echo 'Assigning teams to PR'
gh auth login --with-token <<< $GITHUB_TOKEN
git checkout origin/$BRANCH_NAME_TO_SWITCH
gh pr checkout $(echo $BRANCH_NAME | grep -Po "\\d+")

# Checking diffs
RESULT=$(git diff-tree --name-only origin/$CHANGE_TARGET $GIT_COMMIT)

# Parsing and creating lists of teams or reviewers
b=$(gh api -H "Accept: application/vnd.github+json"   /orgs/soramitsu/teams/devops-team/members | jq | grep -Po '(?<=login)\W*\K[^ ]*' | tr -d '"')
c=$(gh api -H "Accept: application/vnd.github+json"   /orgs/soramitsu/teams/devops-support/members | jq | grep -Po '(?<=login)\W*\K[^ ]*' | tr -d '"')
d=$(gh api -H "Accept: application/vnd.github+json"   /orgs/soramitsu/teams/polkaswap-team/members | jq | grep -Po '(?<=login)\W*\K[^ ]*' | tr -d '"')
devopsteam=$(echo ${b::-1} | tr -d ' ')
supportteam=$(echo ${c::-1} | tr -d ' ')
polkaswapteam=$(echo ${d::-1} | tr -d ' ')

changeslist=('liquidity-proxy common pallets Jenkinsfile housekeeping')

for i in "${changeslist[@]}";
do
if [[ "$RESULT" =~ 'Jenkinsfile' ]] || [[ "$RESULT" =~ 'housekeeping' ]]
then
gh pr edit $(echo $BRANCH_NAME | grep -Po "\\d+") \
--add-reviewer $(echo $devopsteam)
echo $devopsteam are assigned to reviewers!
fi
if [[ "$RESULT" =~ 'liquidity-proxy' ]] || [[ "$RESULT" =~ 'common' ]] || [[ "$RESULT" =~ 'pallets' ]]
then
gh pr edit $(echo $BRANCH_NAME | grep -Po "\\d+") \
--add-reviewer $(echo $polkaswapteam)
echo $polkaswapteam are assigned to reviewers!
fi
done