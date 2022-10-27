#!/bin/bash
set +e
apt-get update && apt-get --no-install-recommends install git -y && apt-get install clang -y
rustup default nightly-2021-03-11
rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-11
rustup component add rustfmt
git config --global user.email ${GH_EMAIL}
git config --global user.name ${GH_USER}
git config --global github.token ${GH_TOKEN}
cd /home/
git clone https://${GH_USER}:${GH_TOKEN}@${GH_REPOSITORY}
cd sora2-substrate
git checkout ${GH_BRANCH}
rm -rf docs
git pull origin master
cargo doc --no-deps || exit 0
cargo fmt
mkdir docs
echo "<meta http-equiv=\"refresh\" content=\"0; url=assets\">" > target/doc/index.html
mv target/doc/* docs/
git add docs
git commit -sm 'Publish doc'
git push origin ${GH_BRANCH}