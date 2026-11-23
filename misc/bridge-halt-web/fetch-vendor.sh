#!/bin/sh
# Re-download the pinned polkadot.js browser bundles from the npm registry.
# Each tarball must match the SHA-512 integrity pinned below (the registry's
# published value when these versions were vendored), and each extracted bundle
# must match vendor/SHA256SUMS. Needs curl, openssl, python3 and tar.
# To upgrade: change versions and integrities, run with --update, test the page
# against a dev node, and commit vendor/ together with the new SHA256SUMS.
set -eu

cd "$(dirname "$0")"

PACKAGES='@polkadot/util 14.0.3 bundle-polkadot-util.js sha512-mg1NR7ixHlNiz2zbvdcdy1OXZmca2tVA4DpewGpY/qFkW/gq9HdDrHLu7g0k90QnunDcFW4emb7NB60sGJQ0bw==
@polkadot/util-crypto 14.0.3 bundle-polkadot-util-crypto.js sha512-V00BI6XnZLCkrAmV8uN0eSB6fy48CkxdDZT29cgSMSwHPtY6oKUNgd1ST07PGCL5x8XflwjoA7CTlhdbp1Y9gw==
@polkadot/keyring 14.0.3 bundle-polkadot-keyring.js sha512-ozp1dQwaHCjgX/fpTTORmHjxdUNQnyiTVJszpzUaUpvtH/IGZhSU/mSHXMqNETS/g57vQa7NatIDcWfyR9abyA==
@polkadot/types 16.5.4 bundle-polkadot-types.js sha512-8Oo1QWaL0DkIc/n2wKBIozPWug/0b2dPVhL+XrXHxJX7rIqS0x8sXDRbM9r166sI0nTqJiUho7pRIkt2PR/DMQ==
@polkadot/api 16.5.4 bundle-polkadot-api.js sha512-mX1fwtXCBAHXEyZLSnSrMDGP+jfU2rr7GfDVQBz0cBY1nmY8N34RqPWGrZWj8o4DxVu1DQ91sGncOmlBwEl0Qg=='

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

printf '%s\n' "$PACKAGES" | while read -r name version file pinned; do
  encoded=$(printf '%s' "$name" | sed 's|/|%2f|')
  curl -fsSL "https://registry.npmjs.org/$encoded/$version" -o "$work/meta.json"
  published=$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["dist"]["integrity"])' "$work/meta.json")
  if [ "$published" != "$pinned" ]; then
    echo "registry integrity for $name@$version differs from the pinned value" >&2
    exit 1
  fi
  tarball=$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["dist"]["tarball"])' "$work/meta.json")
  rm -rf "$work/package" && curl -fsSL "$tarball" -o "$work/package.tgz"
  actual="sha512-$(openssl dgst -sha512 -binary "$work/package.tgz" | openssl base64 -A)"
  if [ "$actual" != "$pinned" ]; then
    echo "tarball for $name@$version does not match the pinned integrity" >&2
    exit 1
  fi
  tar -xzf "$work/package.tgz" -C "$work" "package/$file"
  mkdir -p vendor
  mv "$work/package/$file" "vendor/$file"
  echo "verified $name@$version"
done

cd vendor
if [ "${1:-}" = "--update" ]; then
  shasum -a 256 bundle-polkadot-util.js bundle-polkadot-util-crypto.js \
    bundle-polkadot-keyring.js bundle-polkadot-types.js bundle-polkadot-api.js >SHA256SUMS
  echo "wrote vendor/SHA256SUMS"
else
  shasum -a 256 -c SHA256SUMS
fi
