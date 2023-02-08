#!/bin/bash
set +e
apt-get update && apt-get --no-install-recommends install git clang make cmake -y
rustup default nightly-2022-05-12
rustup target add wasm32-unknown-unknown --toolchain nightly-2022-05-12
rustup component add rustfmt
git config --global user.email ${GH_EMAIL}
git config --global user.name ${GH_USER}
git config --global github.token ${GH_TOKEN}
cd /home/
git clone https://${GH_USER}:${GH_TOKEN}@${GH_REPOSITORY}
cd sora2-substrate
git checkout master
rm -rf docs
git fetch
git pull origin master
cargo doc --no-deps --workspace --exclude relayer
cargo fmt
mkdir docs
echo "<meta http-equiv=\"refresh\" content=\"0; url=assets\">" > target/doc/index.html
mv target/doc/* docs/
git add docs
git commit -sm 'Publish doc'
git push --force origin master
