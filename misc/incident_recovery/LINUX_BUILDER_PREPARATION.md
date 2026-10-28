# SORA Nexus Linux builder preparation

`prepare_linux_builder.py` supplies the previously missing preparation step for
`production_node_image.py`. It creates a reusable runtime base and a builder with
the source's exact dated nightly, `/opt/cargo`, `/opt/rustup`, Linux native tools
and an offline Cargo dependency cache. It does not build a node, execute a runtime,
publish images, start validators or declare production readiness.

The old `housekeeping/docker/env/Dockerfile` cannot satisfy this contract: it uses
`/opt/rust`, an obsolete nightly, floating package/tool updates and removes its
Git cache. The old release Dockerfile embeds a node binary. The preparation recipe
is separate and leaves both old files unchanged.

## Explicit inputs

Run only after the active source/replay guards permit local preparation. Supply:

- The reviewed source directory and exact `production_build.py inventory` file
  with its SHA-256.
- An approved immutable reference of the form
  `docker.io/library/debian@sha256:DIGEST`, for Debian Bookworm and the selected
  `linux/amd64` or `linux/arm64` platform. A tag is not accepted. The resolver does
  not discover or substitute a different base.
- A dated Debian snapshot, `YYYYMMDDTHHMMSSZ`. Apt uses HTTPS, the official base's
  Debian archive keyring and that immutable snapshot; only the expired-release
  timestamp check is disabled. Signature and package hash verification remain on.
- An explicit rustup version. The resolver downloads only that version's official
  Linux installer/checksum and the exact source-nightly manifest/checksum from
  `static.rust-lang.org`. It rejects redirects and ambient proxies. Rustup's
  automatic self-update is disabled. Installed channel metadata must match the
  pinned manifest, allowing only Rustup's omitted package commit field, added
  empty component arrays and explicit derived extension flags. Archive URLs and
  hashes, versions, targets, ordering and profiles remain exact; the offline
  rustc check also binds its reported full commit to the original public manifest.
- A canonical local **public PEM CA certificate bundle**, never private keys.
  Its exact bytes are copied and hashed. It bootstraps HTTPS in the minimal Debian
  base before the signed snapshot supplies Debian's certificate package.
- For preparation, canonical Docker and Buildx executable paths and SHA-256 pins,
  plus the explicitly approved canonical local Unix socket. Neither the current
  Docker context nor environment chooses the daemon.

The root task owns starting the approved local Colima profile after current
rehearsals finish. This tool neither starts Colima nor changes auto-activation,
contexts, providers or remote hosts.

## Two phases

First resolve public data. This performs explicit HTTPS downloads, but no Docker
or Cargo commands:

```text
python3 misc/incident_recovery/prepare_linux_builder.py resolve
  --source-root CANONICAL_CHECKOUT
  --source-inventory INVENTORY_JSON
  --source-inventory-sha256 INVENTORY_SHA256
  --platform linux/amd64
  --debian-base docker.io/library/debian@sha256:APPROVED_DIGEST
  --debian-snapshot APPROVED_TIMESTAMP
  --rustup-version APPROVED_RUSTUP_VERSION
  --ca-bundle CANONICAL_PUBLIC_CA_BUNDLE
  --output NEW_PUBLIC_INPUT_DIRECTORY
```

`recipe.json` binds exact payloads, origins, Cargo.lock, source inventory, platform
and nightly. `PUBLIC-RESOLUTION.json` retains the actual process ID, tool identity,
download method and result pins. This is provenance data, not a transferable
runtime or consensus certificate. Failed downloads preserve partial inputs.

Then execute local preparation against the pinned recipe:

```text
python3 misc/incident_recovery/prepare_linux_builder.py prepare
  --source-root CANONICAL_CHECKOUT
  --source-inventory INVENTORY_JSON
  --source-inventory-sha256 INVENTORY_SHA256
  --inputs PUBLIC_INPUT_DIRECTORY/recipe.json
  --inputs-sha256 RECIPE_SHA256
  --docker CANONICAL_DOCKER_BINARY
  --docker-sha256 DOCKER_SHA256
  --buildx CANONICAL_BUILDX_BINARY
  --buildx-sha256 BUILDX_SHA256
  --socket CANONICAL_APPROVED_LOCAL_SOCKET
  --output NEW_PREPARATION_OUTPUT
```

These are interface examples with placeholders, not executed commands. The
preparer copies both executable pins, installs Buildx only into its owned empty
configuration, selects the local default builder, and supplies an empty owned
home. Proxy variables, credential helpers, SSH agents and user contexts are not
inherited. No private directory or Docker socket is mounted in an image build.

The exact source, recipe and scripts form an owned build context. The immutable
official Debian base is pulled explicitly, then both fixed Dockerfile targets are
built locally. Docker's immutable build layers remain cacheable; dependency
fetching starts in the new image's clean `/opt/cargo`, avoiding an ambient shared
Git/registry configuration. The prepared image is the reusable offline dependency
cache. Cargo homes containing configuration or credentials are rejected. Public Git URLs are limited to the reviewed repositories in the source;
registry packages come from crates.io with Cargo.lock checksums. Only this
preparation phase has public network access. The release image driver stays
offline and accepts only resulting image IDs.

Each build, pull, inspect, smoke and cleanup command has a 24-hour bound and
retained actual process receipts/stdout/stderr. A fresh owned container runs with
`--network none`; it checks source bytes, the exact nightly/target, compiler,
libclang, CMake, protobuf, OpenSSL and `cargo metadata --offline --locked --frozen`.
The Docker CLI exit and the container exit are both checked. Cleanup targets only
the exact returned container ID after checking its generated name, image and
ownership label. A malformed-create cleanup instead uses the generated name,
with the same ownership checks. Build
cache and completed images are retained; unrelated images are never deleted.

`BUILDER-MANIFEST.json` binds actual image IDs, inspected architecture, source and
recipe identities, apt package versions/evidence hashes, the offline smoke and
process receipts. Supply `images.builder.imageId` and `images.runtime.imageId` to
`production_node_image.py`. Its later node build and runtime-base execution test
remain required: dependency metadata is not native link/build verification.

## Validation scope

The accompanying unit tests use synthetic inputs and mocked HTTP/Docker. No real
network request, Docker command, Cargo command, native node or Wasm execution is
performed by those tests. The staged recipe has not yet been run on Linux. Actual
Buildx compatibility, snapshot availability, installation and node compilation
must be checked by the root-owned local execution after current guards stop.
Deterministic inputs and retained receipts do not alone establish bit-for-bit
image reproducibility or release readiness. Every result keeps
`nativeBuildVerified`, `published` and `productionReady` false.
