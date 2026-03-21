#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

mkdir -p "${tmpdir}/scripts"
cp "${repo_root}/scripts/check_readme_commands.sh" "${tmpdir}/scripts/"

cat > "${tmpdir}/README.md" <<'EOF'
# tmp

```bash
npm test
npm run present
node ./scripts/present.mjs
```
EOF

cat > "${tmpdir}/package.json" <<'EOF'
{
  "scripts": {
    "present": "echo ok"
  }
}
EOF

mkdir -p "${tmpdir}/scripts"
cat > "${tmpdir}/scripts/present.mjs" <<'EOF'
console.log('ok');
EOF

set +e
output="$(cd "${tmpdir}" && bash ./scripts/check_readme_commands.sh 2>&1)"
rc=$?
set -e

if [[ ${rc} -eq 0 ]]; then
  echo "[test-readme-commands] expected missing npm test script to fail" >&2
  echo "${output}" >&2
  exit 1
fi

if [[ "${output}" != *"npm script 'test' not found"* ]]; then
  echo "[test-readme-commands] expected missing npm test diagnostic, got:" >&2
  echo "${output}" >&2
  exit 1
fi

cat > "${tmpdir}/package.json" <<'EOF'
{
  "scripts": {
    "test": "echo ok",
    "present": "echo ok"
  }
}
EOF

(cd "${tmpdir}" && bash ./scripts/check_readme_commands.sh >/dev/null)
