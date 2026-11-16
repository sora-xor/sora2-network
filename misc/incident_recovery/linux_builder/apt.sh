#!/bin/sh
set -eu
# This file is copied verbatim into an owned context. Its only argument is fixed
# by the reviewed Dockerfile; no caller supplies a shell fragment.
export PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root DEBIAN_FRONTEND=noninteractive
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy NO_PROXY no_proxy
test "$(sed -n 's/^ID=//p' /etc/os-release)" = debian
test "$(sed -n 's/^VERSION_CODENAME=//p' /etc/os-release)" = bookworm
snapshot=$(sed -n 's/^  "debianSnapshot": "\([0-9]*T[0-9]*Z\)",$/\1/p' /preparation/recipe.json)
test -n "$snapshot"
rm -f /etc/apt/sources.list /etc/apt/sources.list.d/*
printf 'deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg check-valid-until=no] https://snapshot.debian.org/archive/debian/%s/ bookworm main\n' "$snapshot" > /etc/apt/sources.list
printf 'deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg check-valid-until=no] https://snapshot.debian.org/archive/debian-security/%s/ bookworm-security main\n' "$snapshot" >> /etc/apt/sources.list
printf 'Acquire::Check-Valid-Until "false";\nAcquire::Languages "none";\nAcquire::https::Verify-Peer "true";\n' > /etc/apt/apt.conf.d/99-sora-nexus-snapshot
apt-get update
case "$1" in
  runtime) apt-get install --no-install-recommends -y ca-certificates libc6 libgcc-s1 libstdc++6 libssl3 zlib1g ;;
  builder) apt-get install --no-install-recommends -y build-essential cmake pkg-config clang libclang-dev llvm-dev lld protobuf-compiler libssl-dev libudev-dev zlib1g-dev git curl python3 xz-utils ;;
  *) exit 2 ;;
esac
mkdir -p /preparation/apt-evidence
cp /var/lib/apt/lists/*InRelease /preparation/apt-evidence/
dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\n' > /preparation/apt-evidence/installed.tsv
find /preparation/apt-evidence -maxdepth 1 -type f ! -name SHA256SUMS -exec sha256sum '{}' + > /preparation/apt-evidence/SHA256SUMS
apt-get clean
rm -rf /var/lib/apt/lists/*
