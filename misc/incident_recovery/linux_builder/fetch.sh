#!/bin/sh
set -eu
export PATH=/opt/cargo/bin:/usr/bin:/bin HOME=/empty-home CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup
export LANG=C LC_ALL=C CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse CARGO_NET_GIT_FETCH_WITH_CLI=true
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy NO_PROXY no_proxy
for file in config config.toml credentials credentials.toml; do
  test ! -e "/opt/cargo/$file" || { printf 'Unexpected ambient Cargo configuration\n' >&2; exit 2; }
done
test ! -e /empty-home/.gitconfig
test ! -e /empty-home/.cargo/config
test ! -e /empty-home/.cargo/config.toml
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0
export GIT_CONFIG_COUNT=4 GIT_CONFIG_KEY_0=credential.helper GIT_CONFIG_VALUE_0=
export GIT_CONFIG_KEY_1=protocol.allow GIT_CONFIG_VALUE_1=never
export GIT_CONFIG_KEY_2=protocol.https.allow GIT_CONFIG_VALUE_2=always
export GIT_CONFIG_KEY_3=http.proxy GIT_CONFIG_VALUE_3=
cd /source
target=$(/usr/bin/python3 -c 'import json; print(json.load(open("/preparation/recipe.json"))["rustTarget"])')
toolchain=$(/usr/bin/python3 -c 'import json; print(json.load(open("/preparation/recipe.json"))["toolchain"])')
/opt/cargo/bin/cargo "+$toolchain" fetch --locked --target "$target"
