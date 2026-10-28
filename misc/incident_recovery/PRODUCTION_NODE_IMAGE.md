# SORA Nexus Linux node and image build

`production_node_image.py` builds node **4.8.13**, exports Docker and OCI image
archives, and verifies the loaded image's binary and version. It does not start a
validator, publish an image, contact a registry, or establish recovery readiness.

This path can be prepared before the signed account mapping. It builds the node
with `--no-default-features` and `SKIP_WASM_BUILD=1`; it embeds no newly built
runtime Wasm. Operation requires an externally supplied **raw** chain
specification and the actual production recovery Wasm artifacts. Development
chain-spec generation that requires an embedded runtime is intentionally absent.
The CSV-bound Wasm build remains `production_build.py` and its separate evidence.

## Required local inputs

All paths must be absolute and canonical. Source and persistent target must be
separate existing directories. The output must be new and outside both.

- Exact root source inventory and its SHA-256, made by
  `production_build.py inventory --output PATH` from the reviewed checkout.
- The Docker CLI binary and its explicit SHA-256. The driver copies it into the
  output and uses an owned empty Docker configuration, ignoring user contexts,
  registry authentication, credential helpers and remote host environment.
- An explicitly approved local Unix Docker socket. Supply its canonical path,
  including canonical parents; aliases such as a symlinked `/var/run/docker.sock`
  are rejected. Device/inode/type are pinned for every build and cleanup command.
- Preloaded builder and runtime base **image IDs**, each of the exact form
  `sha256:<64 lowercase hexadecimal digits>`. Tags and registry references are
  refused. The driver never pulls or constructs the builder/base images.
- `linux/amd64` or `linux/arm64`, an existing persistent Cargo target directory,
  and an explicit integer source-date epoch.

The reviewed builder image must contain the exact dated toolchain in the source
`rust-toolchain.toml`, rustup proxies at `/opt/cargo/bin`, toolchains under
`/opt/rustup`, a complete offline Cargo registry/git cache under `/opt/cargo`, and
the native compiler/linker, libclang, protobuf, headers and other dependencies
needed by this checkout. Its Linux architecture must match the selected target.
The runtime base must contain the binary's dynamic loader/libraries. Missing
prerequisites cause failure; there is no package installation or network fallback.

The driver uses fixed argument arrays. Its CLI requires:

```text
python3 misc/incident_recovery/production_node_image.py
  --source-root CANONICAL_CHECKOUT
  --source-inventory INVENTORY_JSON
  --source-inventory-sha256 INVENTORY_SHA256
  --docker CANONICAL_DOCKER_BINARY
  --docker-sha256 DOCKER_SHA256
  --socket CANONICAL_APPROVED_LOCAL_SOCKET
  --builder-image sha256:PRELOADED_BUILDER_IMAGE_ID
  --base-image sha256:PRELOADED_RUNTIME_BASE_IMAGE_ID
  --platform linux/amd64
  --target EXISTING_PERSISTENT_TARGET
  --source-date-epoch APPROVED_INTEGER_EPOCH
  --output NEW_OUTPUT_DIRECTORY
```

These placeholders are an interface description, not an executable production
command. No image identities or approved socket are supplied by this document.

## Execution and evidence

The complete source is copied into the output and remains pinned, together with
the original checkout, Cargo.lock, toolchain, source inventory, driver and helper
files. A lock prevents cooperating executions from sharing the target at once.
The target stays warm across attempts; the driver never runs `cargo clean` and
does not force a job count.

Each container is created with an owned identity label, no network, no added
capabilities and no-new-privileges. Builds mount the source read-only and the
persistent target writable. The builder's image filesystem supplies its offline
dependency cache. No host home, credential directory or Docker socket is mounted
inside a container. The fixed Cargo command uses `--offline --locked --frozen`,
release mode, an explicit GNU Linux target and package/bin `framenode`.

The driver retains fresh Docker process receipts, stdout/stderr, container IDs
and inspections. It verifies the container exit code as well as the Docker CLI
exit, including when `start --attach` succeeds. It removes only the exact owned
containers after checking their identity label and image. Cleanup remains
available after a source guard fails, but fails closed if the pinned Docker
binary/configuration or socket changes. Cleanup failures remain visible in the
retained failure receipt. Image objects loaded into the local daemon are retained;
the driver does not delete unrelated images or mutate tags.

Commands have a 24-hour bound and capped output. Process termination is handled
by the existing owned-process runner; a failed/terminated attach is followed by
the independently checked owned-container removal. No validator or chain process
is launched: runtime smoke commands are exclusively `framenode --version`.

The Cargo JSON must identify the exact package, feature profile and executable.
The copied binary must be a matching ELF64 and report version 4.8.13 with the
source-inventory fingerprint and Linux target. A smoke run in the pinned base
checks the loader/library compatibility needed by that command.

The pinned base is exported locally, and every saved uncompressed layer is
checked against the pinned base configuration's diff ID. The driver adds only a
deterministic layer containing the exact node binary and constructs Docker and
OCI archives with fixed metadata. It loads the Docker archive locally, checks its
configuration-derived image ID, independently copies the image binary back out
and compares all bytes by hash, then runs the loaded image's version command.
The final image runs as user/group 10000 and contains no recovery policy or keys.

`BUILD-MANIFEST.json` binds the source, toolchain, actual Cargo artifact, node
digest, base/builder inspection receipts, image ID, OCI manifest digest, archive
checksums and fresh process lineage. All accepted files remain pinned through
completion. `failure.json` records incomplete attempts. Existing outputs are
never resumed or silently overwritten.

`productionReady`, `runtimeBindingVerified`, `consensusVerified`,
`operatorApprovalVerified`, `published`, and `sourceBuildReproducibilityVerified`
remain false. Deterministic archive assembly does not prove bit-for-bit
reproducibility of a native build or full application compatibility. Real
Linux/Docker execution, exact recovery rehearsals and release approval remain
separate gates. The unit tests use a fake process backend and never run Docker,
Cargo, a native recovery binary or Wasm.
