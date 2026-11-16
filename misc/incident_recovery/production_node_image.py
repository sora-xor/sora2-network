#!/usr/bin/env python3
"""Offline, local Docker build corridor for the SORA Nexus Linux node.

Build evidence only: no validator, registry, credentials, network, chain admission
or embedded recovery runtime. The caller must preload explicitly pinned images.
"""
from __future__ import annotations
import argparse
import datetime
import fcntl
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import tarfile
import tomllib
from uuid import uuid4

import production_build as build

SCHEMA = 'sora-nexus-linux-node-image-build-v1'
ARCH = {'linux/amd64': ('amd64', 'x86_64-unknown-linux-gnu', 62),
        'linux/arm64': ('arm64', 'aarch64-unknown-linux-gnu', 183)}
MAX_ARCHIVE = 64 * 1024**3
MAX_MEMBER = 16 * 1024**3
TIMEOUT = 24 * 3600


def require(ok, message):
    build.require(ok, 'Linux node image: ' + message)


def socket_identity(path):
    require(path.is_absolute() and path.resolve(strict=True) == path and not path.is_symlink(), 'canonical Unix socket without aliases required')
    status = path.lstat()
    require(str(path).endswith('.sock') and stat.S_ISSOCK(status.st_mode), 'local Unix Docker socket required')
    return (status.st_dev, status.st_ino, stat.S_IFMT(status.st_mode))


def image_id(value):
    require(isinstance(value, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', value), 'preloaded immutable image ID required')
    return value


def directory(path):
    require(path.is_dir() and not path.is_symlink() and path.resolve(strict=True) == path, 'directory alias')
    require(',' not in str(path) and '\n' not in str(path), 'mount path delimiters')
    return path


def inspect_image(raw, expected, architecture):
    value = build.parse(raw)
    require(isinstance(value, list) and len(value) == 1, 'image inspection shape')
    item = value[0]
    require(item.get('Id') == expected and item.get('Os') == 'linux' and item.get('Architecture') == architecture,
            'preloaded image identity/platform differs')
    return item


def elf(raw, machine):
    require(len(raw) >= 64 and raw[:6] == b'\x7fELF\x02\x01' and raw[6] == 1
            and int.from_bytes(raw[16:18], 'little') in (2, 3)
            and int.from_bytes(raw[18:20], 'little') == machine, 'node is not the expected 64-bit Linux ELF')


def copy_cargo_artifact(source, destination, target_root, target):
    """Detach Cargo's hard-linked output without relaxing release-file guards.

    Only the exact executable reported by the completed Cargo invocation is
    eligible. The caller holds the target lane lock. Read through one no-follow
    descriptor, reject concurrent mutation/replacement, and pin the independent
    copy; later builds may legitimately replace the warm target's artifact.
    """
    require(source == target_root / target / 'release/framenode'
            and source.resolve(strict=True) == source and not source.is_symlink(),
            'Cargo executable path differs or is aliased')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with os.fdopen(os.open(source, os.O_RDONLY | os.O_NOFOLLOW), 'rb') as original:
        before = os.fstat(original.fileno())
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= MAX_MEMBER,
                'Cargo executable must be a bounded regular file')
        def identity(value):
            return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        require(identity(source.lstat()) == identity(before), 'Cargo executable replaced before copy')
        sha = hashlib.sha256(); blake = hashlib.blake2b(digest_size=32); size = 0
        with destination.open('xb') as copied:
            while chunk := original.read(1024 * 1024):
                size += len(chunk)
                require(size <= before.st_size, 'Cargo executable grew during copy')
                sha.update(chunk); blake.update(chunk); copied.write(chunk)
            copied.flush(); os.fsync(copied.fileno())
        require(size == before.st_size and identity(os.fstat(original.fileno())) == identity(before)
                and identity(source.lstat()) == identity(before), 'Cargo executable changed during copy')
    expected = {'sha256': sha.hexdigest(), 'blake2_256': blake.hexdigest(), 'bytes': size}
    require(build.digest(destination) == expected, 'independent Cargo executable copy differs')
    destination.chmod(0o555)
    return expected


def version(raw, fingerprint, target):
    expected = f'framenode 4.8.13-src-{fingerprint[:16]}-{target}\n'.encode()
    require(raw in (expected, expected + b'\n'), 'node version/source/target differs')


def member_name(name):
    require(isinstance(name, str) and '\\' not in name and '\0' not in name, 'unsafe archive member')
    p = PurePosixPath(name)
    require(name and not p.is_absolute() and '..' not in p.parts and str(p) == name, 'unsafe archive member')
    return name


def sha_stream(stream, output=None):
    digest = hashlib.sha256()
    while chunk := stream.read(1024 * 1024):
        digest.update(chunk)
        if output is not None:
            output.write(chunk)
    return digest.hexdigest()


def tar_add(archive, name, source, size, *, mode=0o444):
    info = tarfile.TarInfo(member_name(name))
    info.size = size
    info.mode = mode
    info.mtime = 0
    info.uid = info.gid = 0
    info.uname = info.gname = ''
    archive.addfile(info, source)


# Accepted local save formats. Unknown compression/external descriptors fail closed.
OCI_INDEX = 'application/vnd.oci.image.index.v1+json'
OCI_MANIFEST = 'application/vnd.oci.image.manifest.v1+json'
INDEX_TYPES = {OCI_INDEX, 'application/vnd.docker.distribution.manifest.list.v2+json'}
MANIFEST_TYPES = {OCI_MANIFEST, 'application/vnd.docker.distribution.manifest.v2+json'}
CONFIG_TYPES = {'application/vnd.oci.image.config.v1+json', 'application/vnd.docker.container.image.v1+json'}
LAYER_TYPES = {'application/vnd.oci.image.layer.v1.tar': False,
               'application/vnd.oci.image.layer.v1.tar+gzip': True,
               'application/vnd.docker.image.rootfs.diff.tar': False,
               'application/vnd.docker.image.rootfs.diff.tar.gzip': True}
MAX_METADATA = 16 * 1024**2
MAX_GRAPH = 128
MAX_GRAPH_DEPTH = 4


def saved_base_graph(small, index, base_id, architecture):
    """Authenticate the selected base graph; the layout root is only a lookup.

    The trusted identity is base_id, independently obtained by exact image inspect.
    Non-selected platform descriptors (including attestations) cannot become layers.
    No referenced URLs/data, unknown compression, or ambiguous selection is admitted.
    """
    image_id(base_id)
    manifest = build.parse(small('manifest.json'))
    require(isinstance(manifest, list) and len(manifest) == 1 and isinstance(manifest[0], dict), 'one saved base image required')
    row = manifest[0]
    require(set(row) == {'Config', 'RepoTags', 'Layers'} and isinstance(row['Layers'], list)
            and len(row['Layers']) <= 256 and len(set(row['Layers'])) == len(row['Layers']), 'saved image manifest shape')
    config_bytes = small(row['Config'])
    config_id = 'sha256:' + hashlib.sha256(config_bytes).hexdigest()
    graph = []
    descriptors = None
    selected_manifest = None
    if config_id != base_id:
        require('index.json' in index and 'oci-layout' in index, 'saved base configuration differs from pinned ID without OCI graph')
        require(build.parse(small('oci-layout')) == {'imageLayoutVersion': '1.0.0'}, 'unsupported OCI layout')
        layout = build.parse(small('index.json'))
        require(isinstance(layout, dict) and type(layout.get('schemaVersion')) is int and layout['schemaVersion'] == 2
                and layout.get('mediaType', OCI_INDEX) == OCI_INDEX
                and set(layout) <= {'schemaVersion', 'mediaType', 'manifests', 'annotations'}
                and isinstance(layout.get('manifests'), list) and len(layout['manifests']) == 1,
                'one pinned OCI layout root required')
        require(layout['manifests'][0].get('digest') == base_id, 'OCI root does not match pinned base ID')
        seen = set()
        selected = []
        def descriptor(d, allowed, maximum):
            require(isinstance(d, dict) and {'mediaType', 'digest', 'size'} <= set(d)
                    and set(d) <= {'mediaType', 'digest', 'size', 'platform', 'annotations'}
                    and d['mediaType'] in allowed and type(d['size']) is int and 0 < d['size'] <= maximum,
                    'unsupported or oversized OCI descriptor')
            image_id(d['digest'])
            name = 'blobs/sha256/' + d['digest'][7:]
            require(name in index and index[name].isfile() and index[name].size == d['size'], 'OCI descriptor size/blob differs')
            return name
        def document(d, allowed):
            name = descriptor(d, allowed, MAX_METADATA)
            raw = small(name)
            require('sha256:' + hashlib.sha256(raw).hexdigest() == d['digest'], 'OCI descriptor digest differs')
            graph.append({'digest': d['digest'], 'size': d['size'], 'mediaType': d['mediaType']})
            value = build.parse(raw)
            require(isinstance(value, dict), 'OCI document object required')
            return value
        def visit(d, depth):
            require(depth <= MAX_GRAPH_DEPTH and len(seen) < MAX_GRAPH, 'OCI graph bound')
            require(isinstance(d, dict) and isinstance(d.get('digest'), str) and d['digest'] not in seen, 'duplicate/cyclic OCI graph descriptor')
            seen.add(d['digest'])
            # Descriptors must be structurally valid even when their platform is skipped.
            require(d.get('mediaType') in INDEX_TYPES | MANIFEST_TYPES, 'unsupported OCI graph media type')
            platform = d.get('platform')
            if platform is not None:
                require(isinstance(platform, dict) and set(platform) <= {'os', 'architecture', 'variant'}
                        and isinstance(platform.get('os'), str) and isinstance(platform.get('architecture'), str), 'OCI platform shape')
                if platform['os'] != 'linux' or platform['architecture'] != architecture:
                    # Only a descriptor in the authenticated parent is ignored; no blob is used.
                    require(type(d.get('size')) is int and 0 < d['size'] <= MAX_METADATA, 'unselected OCI descriptor size')
                    image_id(d['digest'])
                    require({'mediaType', 'digest', 'size'} <= set(d)
                            and set(d) <= {'mediaType', 'digest', 'size', 'platform', 'annotations'}, 'unselected OCI descriptor shape')
                    return
                require('variant' not in platform or (architecture == 'arm64' and platform['variant'] == 'v8'), 'unsupported selected OCI platform variant')
            value = document(d, INDEX_TYPES | MANIFEST_TYPES)
            require(type(value.get('schemaVersion')) is int and value['schemaVersion'] == 2
                    and value.get('mediaType') == d['mediaType'], 'OCI document schema/media type differs')
            if d['mediaType'] in INDEX_TYPES:
                require(set(value) <= {'schemaVersion', 'mediaType', 'manifests', 'annotations'}
                        and isinstance(value.get('manifests'), list) and 0 < len(value['manifests']) <= MAX_GRAPH,
                        'OCI index shape/bound')
                for child in value['manifests']:
                    visit(child, depth + 1)
            else:
                require(set(value) <= {'schemaVersion', 'mediaType', 'config', 'layers', 'annotations'}
                        and isinstance(value.get('layers'), list) and len(value['layers']) <= 256, 'OCI image manifest shape')
                candidate_config = document(value.get('config'), CONFIG_TYPES)
                if candidate_config.get('os') != 'linux' or candidate_config.get('architecture') != architecture:
                    require(platform is None, 'OCI descriptor/config platform differs')
                    return
                selected.append((d['digest'], value))
        visit(layout['manifests'][0], 0)
        require(len(selected) == 1, 'unique selected OCI platform image required')
        selected_manifest, value = selected[0]
        require(value['config']['digest'] == config_id
                and row['Config'] == 'blobs/sha256/' + config_id[7:], 'saved Docker/OCI configuration differs')
        descriptors = value['layers']
        require(len(descriptors) == len(row['Layers']), 'saved Docker/OCI layer count differs')
        for name, d in zip(row['Layers'], descriptors):
            require(descriptor(d, LAYER_TYPES, MAX_MEMBER) == name, 'saved Docker/OCI layer order differs')
    config = build.parse(config_bytes)
    require(isinstance(config, dict) and config.get('os') == 'linux' and config.get('architecture') == architecture, 'saved base platform differs')
    rootfs = config.get('rootfs', {})
    require(isinstance(rootfs, dict) and rootfs.get('type') == 'layers' and isinstance(rootfs.get('diff_ids'), list)
            and len(rootfs['diff_ids']) == len(row['Layers']) <= 256, 'base rootfs differs')
    require(isinstance(config.get('history', []), list), 'base history shape')
    layers = []
    for position, (name, diff_id) in enumerate(zip(row['Layers'], rootfs['diff_ids'])):
        member_name(name); image_id(diff_id)
        require(name in index and index[name].isfile(), 'saved layer missing')
        d = descriptors[position] if descriptors is not None else None
        layers.append({'name': name, 'diffId': diff_id, 'gzip': LAYER_TYPES[d['mediaType']] if d else False,
                       'blobDigest': d['digest'] if d else diff_id, 'bytes': index[name].size})
    return config, layers, {'inputImageId': base_id, 'configId': config_id,
                           'selectedManifestDigest': selected_manifest, 'authenticatedDocuments': graph,
                           'format': 'oci-descriptor-graph' if descriptors is not None else 'docker-config-id'}


def normalized_layer(base, member, info, destination, remaining):
    """Authenticate compressed bytes first, then bound and authenticate plaintext.

    Layer tar entries are never extracted. This copies one opaque layer stream to
    a fresh file, so compressed blob hashes are never confused with rootfs diff IDs.
    """
    with base.extractfile(member) as source:
        require('sha256:' + sha_stream(source) == info['blobDigest'], 'saved layer blob digest differs from pinned rootfs/descriptor')
    sha = hashlib.sha256(); blake = hashlib.blake2b(digest_size=32); count = 0
    with base.extractfile(member) as source, destination.open('xb') as sink:
        stream = gzip.GzipFile(fileobj=source, mode='rb') if info['gzip'] else source
        try:
            while chunk := stream.read(1024 * 1024):
                count += len(chunk)
                require(count <= MAX_MEMBER and count <= remaining, 'expanded layer byte bound')
                sha.update(chunk); blake.update(chunk); sink.write(chunk)
        finally:
            if info['gzip']:
                stream.close()
        sink.flush(); os.fsync(sink.fileno())
    require('sha256:' + sha.hexdigest() == info['diffId'], 'saved layer differs from pinned rootfs diff ID')
    expected = {'sha256': sha.hexdigest(), 'blake2_256': blake.hexdigest(), 'bytes': count}
    require(build.digest(destination) == expected, 'normalized layer changed')
    return expected


def archives(base_archive, base_id, binary, output, architecture, fingerprint, epoch):
    """Create deterministic Docker and OCI archives from exact verified layers.

    This does not run the image and cannot itself establish execution readiness.
    Layer paths are never extracted; base layer content is authenticated against
    the pinned image configuration's uncompressed diff IDs.
    """
    require(base_archive.stat().st_size <= MAX_ARCHIVE, 'base archive byte bound')
    base_pin = build.digest(base_archive)
    node_pin = build.digest(binary)
    output.mkdir(parents=True, exist_ok=False)
    layout = output / 'oci'
    blobs = layout / 'blobs/sha256'
    blobs.mkdir(parents=True)
    layer = output / 'node-layer.tar'
    with tarfile.open(layer, 'w', format=tarfile.USTAR_FORMAT) as tar:
        with binary.open('rb') as source:
            tar_add(tar, 'usr/local/bin/framenode', source, binary.stat().st_size, mode=0o555)
    node_layer_pin = build.digest(layer)
    new_diff = node_layer_pin['sha256']
    created_pins = {layer: node_layer_pin}
    with tarfile.open(base_archive, 'r:') as base:
        index = {}
        member_bytes = 0
        for member in base:
            member_bytes += member.size
            require(len(index) < 65536 and member_bytes <= MAX_ARCHIVE, 'base archive bounds')
            name = member_name(member.name.rstrip('/'))
            require(name not in index and (member.isfile() or member.isdir()) and 0 <= member.size <= MAX_MEMBER,
                    'duplicate/linked/oversized base archive member')
            index[name] = member
        def small(name):
            member_name(name)
            require(name in index and index[name].isfile() and index[name].size <= MAX_METADATA, 'base metadata missing/oversized')
            with base.extractfile(index[name]) as stream:
                return stream.read()
        config, layer_infos, base_identity = saved_base_graph(small, index, base_id, architecture)
        rootfs = config['rootfs']
        layers = []
        expanded = 0
        normalized = output / 'base-layers'
        normalized.mkdir()
        for position, info in enumerate(layer_infos):
            path = normalized / f'{position:03d}.tar'
            pin = normalized_layer(base, index[info['name']], info, path, MAX_ARCHIVE - expanded)
            expanded += pin['bytes']
            layers.append((path, info['diffId'], pin))
            created_pins[path] = pin
        created = datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc).isoformat().replace('+00:00', 'Z')
        labels = {'org.opencontainers.image.title': 'SORA Nexus node', 'org.opencontainers.image.version': '4.8.13',
                  'org.sora-nexus.source-sha256': fingerprint, 'org.sora-nexus.node-sha256': node_pin['sha256']}
        final_config = {'architecture': architecture, 'os': 'linux', 'created': created,
            'config': {'User': '10000:10000', 'Env': ['PATH=/usr/local/bin:/usr/bin:/bin'],
                       'Entrypoint': ['/usr/local/bin/framenode'], 'WorkingDir': '/', 'Labels': labels},
            'rootfs': {'type': 'layers', 'diff_ids': rootfs['diff_ids'] + ['sha256:' + new_diff]},
            'history': config.get('history', []) + [{'created': created, 'created_by': 'SORA Nexus verified node layer'}]}
        config_raw = build.serialized(final_config)
        config_sha = hashlib.sha256(config_raw).hexdigest()
        created_pins[blobs / config_sha] = build.write(blobs / config_sha, config_raw)
        descriptors = []
        docker_path = output / 'node.docker.tar'
        with tarfile.open(docker_path, 'w', format=tarfile.USTAR_FORMAT) as docker:
            names = []
            for position, (path, diff_id, layer_pin) in enumerate(layers + [(layer, 'sha256:' + new_diff, node_layer_pin)]):
                require(build.digest(path) == layer_pin, 'normalized layer changed before packaging')
                size = layer_pin['bytes']
                normalized = f'layers/{position:03d}-{diff_id[7:]}/layer.tar'
                names.append(normalized)
                with path.open('rb') as stream:
                    tar_add(docker, normalized, stream, size)
                compressed = output / f'compressed-{position:03d}.gz'
                with compressed.open('xb') as sink, gzip.GzipFile(filename='', mode='wb', fileobj=sink, mtime=0) as zipper:
                    with path.open('rb') as source:
                        require('sha256:' + sha_stream(source, zipper) == diff_id, 'layer changed while compressing')
                require(build.digest(path) == layer_pin, 'normalized layer changed during packaging')
                pin = build.digest(compressed)
                compressed.rename(blobs / pin['sha256'])
                created_pins[blobs / pin['sha256']] = pin
                descriptors.append({'mediaType': 'application/vnd.oci.image.layer.v1.tar+gzip',
                                    'digest': 'sha256:' + pin['sha256'], 'size': pin['bytes']})
            tar_add(docker, config_sha + '.json', io.BytesIO(config_raw), len(config_raw))
            dm = build.serialized([{'Config': config_sha + '.json', 'RepoTags': None, 'Layers': names}])
            tar_add(docker, 'manifest.json', io.BytesIO(dm), len(dm))
        om = build.serialized({'schemaVersion': 2, 'mediaType': 'application/vnd.oci.image.manifest.v1+json',
            'config': {'mediaType': 'application/vnd.oci.image.config.v1+json', 'digest': 'sha256:' + config_sha, 'size': len(config_raw)},
            'layers': descriptors})
        om_sha = hashlib.sha256(om).hexdigest()
        created_pins[docker_path] = build.digest(docker_path)
        created_pins[blobs / om_sha] = build.write(blobs / om_sha, om)
        created_pins[layout / 'oci-layout'] = build.write(layout / 'oci-layout', {'imageLayoutVersion': '1.0.0'})
        index_pin = build.write(layout / 'index.json', {'schemaVersion': 2, 'manifests': [{'mediaType': 'application/vnd.oci.image.manifest.v1+json',
            'digest': 'sha256:' + om_sha, 'size': len(om), 'platform': {'os': 'linux', 'architecture': architecture}}]})
        created_pins[layout / 'index.json'] = index_pin
        for path, expected in created_pins.items():
            require(build.digest(path) == expected, 'image component changed before OCI packaging')
        with tarfile.open(output / 'node.oci.tar', 'w', format=tarfile.USTAR_FORMAT) as tar:
            for path in sorted(layout.rglob('*')):
                if path.is_file():
                    with path.open('rb') as source:
                        tar_add(tar, path.relative_to(layout).as_posix(), source, path.stat().st_size)
    require(build.digest(base_archive) == base_pin and build.digest(binary) == node_pin, 'archive/node input changed during assembly')
    created_pins[output / 'node.oci.tar'] = build.digest(output / 'node.oci.tar')
    require({p for p in output.rglob('*') if p.is_file()} == set(created_pins), 'unexpected image assembly output')
    for path, expected in created_pins.items():
        require(build.digest(path) == expected, 'image component changed after packaging')
    return {'imageId': 'sha256:' + om_sha, 'loadedImageReference': 'sha256:' + om_sha,
            'configId': 'sha256:' + config_sha, 'ociManifestDigest': 'sha256:' + om_sha,
            'ociIndexDigest': 'sha256:' + index_pin['sha256'], 'baseIdentity': base_identity,
            'loadedArchiveFormat': 'oci-layout', 'dockerArchiveLoadVerified': False,
            'node': node_pin, 'labels': labels, 'dockerArchive': created_pins[docker_path],
            'ociArchive': created_pins[output / 'node.oci.tar'], 'created': created,
            'files': {p.relative_to(output).as_posix(): expected for p, expected in created_pins.items()}}


def run(args):
    require(args.platform in ARCH, 'unsupported platform')
    architecture, target, machine = ARCH[args.platform]
    image_id(args.builder_image); image_id(args.base_image)
    directory(args.source_root); directory(args.target)
    require(args.target != args.source_root and not args.target.is_relative_to(args.source_root) and not args.source_root.is_relative_to(args.target), 'persistent target must be outside source tree')
    require(not args.output.exists() and not args.output.is_relative_to(args.target) and not args.output.is_relative_to(args.source_root), 'fresh output outside target/source required')
    socket_pin = socket_identity(args.socket)
    require(0 <= args.source_date_epoch <= 4_102_444_800, 'source date epoch bound')
    source_files = build.verify_inventory(args.source_inventory, args.source_inventory_sha256, args.source_root)
    require(len(source_files) <= 65536 and sum(p['bytes'] for p in source_files.values()) <= MAX_ARCHIVE
            and all(p['bytes'] <= build.MAX_FILE for p in source_files.values()), 'source export bounds')
    require(args.output.resolve() == args.output, 'output parent alias')
    fingerprint = build.digest(args.source_inventory)['sha256']
    require(tomllib.loads((args.source_root / 'node/Cargo.toml').read_text())['package']['version'] == '4.8.13', 'node source version differs')
    toolchain = tomllib.loads((args.source_root / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    require(re.fullmatch(r'nightly-\d{4}-\d{2}-\d{2}', toolchain), 'explicit dated toolchain required')
    args.output.mkdir(parents=True)
    pins, stages, containers = {}, [], []
    def retain(path, pin):
        require(str(path) not in pins or pins[str(path)] == pin, 'evidence rebinding')
        pins[str(path)] = pin
    def write(path, value):
        pin = build.write(path, value); retain(path, pin); return pin
    def copy(source, destination, expected):
        require(build.digest(source) == expected, 'source changed before copy')
        destination.parent.mkdir(parents=True, exist_ok=True)
        with source.open('rb') as original, destination.open('xb') as copied:
            shutil.copyfileobj(original, copied, 1024 * 1024)
        require(build.digest(destination) == expected and build.digest(source) == expected, 'copy changed source bytes')
        destination.chmod(source.stat().st_mode & 0o777)
        retain(source, expected); retain(destination, expected)
    exported = args.output / 'source'
    for name, expected in source_files.items():
        copy(build.release.local_file(args.source_root, name), exported / name, expected)
    copy(args.source_inventory, args.output / 'source-files.json', build.digest(args.source_inventory))
    helpers = {Path(__file__).resolve(), Path(build.__file__).resolve(), Path(build.release.__file__).resolve(),
               Path(build.scale.__file__).resolve(), Path(build.bootstrap.__file__).resolve(), Path(build.build_helpers.__file__).resolve()}
    for index, path in enumerate(sorted(helpers)):
        copy(path, args.output / 'helpers' / f'{index:02d}-{path.name}', build.digest(path))
    docker_pin = build.digest(args.docker)
    require(docker_pin['sha256'] == args.docker_sha256, 'Docker executable pin differs')
    docker = args.output / 'docker'
    copy(args.docker, docker, docker_pin); docker.chmod(0o555)
    config = args.output / 'docker-config'
    write(config / 'config.json', {})
    prefix = [str(docker), '--config', str(config), '--host', 'unix://' + str(args.socket)]
    def guard():
        require(socket_identity(args.socket) == socket_pin, 'Docker socket identity changed')
        build.guard_files(pins)
        require(build.source_inventory(args.source_root) == source_files, 'root source set changed')
        require({p.relative_to(exported).as_posix() for p in exported.rglob('*') if p.is_file()} == set(source_files), 'exported source set changed')
        require(set(config.iterdir()) == {config / 'config.json'}, 'Docker config gained extra files')
    def cleanup_guard():
        require(socket_identity(args.socket) == socket_pin, 'cleanup Docker socket identity changed')
        require(build.digest(docker) == docker_pin, 'cleanup Docker executable changed')
        require(build.digest(config / 'config.json') == pins[str(config / 'config.json')], 'cleanup Docker config changed')
    def command(name, arguments, *, cleanup=False):
        raw, receipt = build.process(prefix + arguments, args.output, args.output / 'processes' / name,
            cleanup_guard if cleanup else guard, timeout=TIMEOUT, maximum=256 * 1024**2, retain=retain)
        stages.append({'name': name, 'receipt': build.digest(args.output / 'processes' / name / 'execution.json')})
        return raw
    def container(name, image, entrypoint, arguments, mounts=(), environment=(), workdir='/', extract=None):
        token = uuid4().hex
        label = 'org.sora-nexus.owned-build=' + token
        argv = ['container', 'create', '--name', 'sora-nexus-build-' + token, '--label', label,
            '--platform', args.platform, '--network', 'none', '--pull', 'never', '--cap-drop', 'ALL',
            '--security-opt', 'no-new-privileges', '--workdir', workdir, '--entrypoint', entrypoint]
        for mount in mounts:
            argv += ['--mount', mount]
        for value in environment:
            argv += ['--env', value]
        owned_name = 'sora-nexus-build-' + token
        owned = {'name': name, 'ownedName': owned_name, 'id': None, 'image': image, 'ownershipLabel': token, 'removed': False}
        containers.append(owned)
        def inspected(suffix):
            detail = build.parse(command(name + suffix, ['container', 'inspect', owned_name], cleanup=True))
            require(isinstance(detail, list) and len(detail) == 1
                and isinstance(detail[0].get('Id'), str) and re.fullmatch(r'[0-9a-f]{64}', detail[0]['Id'])
                and detail[0].get('Name') == '/' + owned_name and detail[0].get('Image') == image
                and detail[0].get('Config', {}).get('Labels', {}).get('org.sora-nexus.owned-build') == token,
                'owned container cleanup identity differs')
            return detail[0]
        succeeded = False
        try:
            raw = command(name + '-create', argv + [image] + arguments)
            cid = raw.decode().strip()
            require(re.fullmatch(r'[0-9a-f]{64}', cid), 'created container identity')
            registered = inspected('-register')
            owned['id'] = registered['Id']
            require(cid == registered['Id'], 'created container stdout differs from owned name')
            if extract is None:
                result = command(name + '-start', ['container', 'start', '--attach', cid])
            else:
                require(not extract.exists(), 'fresh image extraction required')
                result = command(name + '-copy', ['container', 'cp', cid + ':/usr/local/bin/framenode', str(extract)])
            succeeded = True
            return result
        finally:
            # The daemon may have created the container before the CLI failed or
            # returned malformed stdout. The unique name/token were known before
            # launch; only that exact inspected owner/image may be removed.
            detail = inspected('-inspect')
            require(owned['id'] is None or detail['Id'] == owned['id'], 'owned container was replaced')
            owned['id'] = detail['Id']
            command(name + '-remove', ['container', 'rm', '--force', detail['Id']], cleanup=True)
            owned['removed'] = True
            if succeeded:
                require(detail.get('State', {}).get('Running') is False and detail.get('State', {}).get('ExitCode') == 0, 'container process did not exit successfully')
    lock = args.target / '.sora-nexus-linux-build.lock'
    require(not lock.is_symlink(), 'target lock alias')
    with os.fdopen(os.open(lock, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600), 'a+b') as lane:
        require(stat.S_ISREG(os.fstat(lane.fileno()).st_mode) and os.fstat(lane.fileno()).st_nlink == 1, 'target lock must be an independent regular file')
        try:
            fcntl.flock(lane, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError('Persistent build lane is already in use') from error
        try:
            for name, image in [('builder', args.builder_image), ('base', args.base_image)]:
                inspect_image(command(name + '-image', ['image', 'inspect', image]), image, architecture)
            environment = ['CARGO_HOME=/opt/cargo', 'RUSTUP_HOME=/opt/rustup', 'RUSTUP_TOOLCHAIN=' + toolchain,
                'CARGO_NET_OFFLINE=true', 'CARGO_TARGET_DIR=/target', 'SKIP_WASM_BUILD=1',
                'GIT_COMMIT=src-' + fingerprint[:16], 'SOURCE_DATE_EPOCH=' + str(args.source_date_epoch)]
            rust = container('rustc', args.builder_image, '/opt/cargo/bin/rustc', ['+' + toolchain, '-vV'], environment=environment)
            require(re.search(rb'^host: ' + target.encode() + rb'$', rust, re.M) is not None, 'builder Rust host differs')
            cargo = container('cargo-version', args.builder_image, '/opt/cargo/bin/cargo', ['+' + toolchain, '-V'], environment=environment)
            require(cargo.startswith(b'cargo '), 'Cargo version output missing')
            mounts = [f'type=bind,source={exported},target=/source,readonly', f'type=bind,source={args.target},target=/target']
            raw = container('cargo-build', args.builder_image, '/opt/cargo/bin/cargo', ['+' + toolchain,
                'build', '--manifest-path', '/source/Cargo.toml', '--offline', '--locked', '--frozen',
                '--release', '--target', target, '--package', 'framenode', '--bin', 'framenode',
                '--no-default-features', '--message-format=json-render-diagnostics'], mounts, environment, '/source')
            messages = [build.parse(line) for line in raw.splitlines() if line.strip()]
            candidates = [row for row in messages if row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'framenode'
                          and row.get('target', {}).get('kind') == ['bin'] and row.get('features') == []
                          and row.get('manifest_path') == '/source/node/Cargo.toml'
                          and row.get('package_id') == 'path+file:///source/node#framenode@4.8.13'
                          and row.get('executable') == '/target/' + target + '/release/framenode']
            require(len(candidates) == 1 and any(row.get('reason') == 'build-finished' and row.get('success') is True for row in messages), 'Cargo did not report exact completed node artifact')
            native = args.target / target / 'release/framenode'
            node = args.output / 'bin/framenode'
            native_pin = copy_cargo_artifact(native, node, args.target, target)
            retain(node, native_pin)
            with node.open('rb') as stream:
                elf(stream.read(64), machine)
            version(container('node-version', args.base_image, '/framenode', ['--version'],
                [f'type=bind,source={node},target=/framenode,readonly']), fingerprint, target)
            base_archive = args.output / 'base.docker.tar'
            command('base-save', ['image', 'save', '--output', str(base_archive), args.base_image])
            retain(base_archive, build.digest(base_archive))
            images = archives(base_archive, args.base_image, node, args.output / 'images', architecture, fingerprint, args.source_date_epoch)
            for name, expected in images['files'].items():
                path = args.output / 'images' / name
                require(build.digest(path) == expected, 'image output changed after assembly')
                retain(path, expected)
            guard()
            command('image-load', ['image', 'load', '--input', str(args.output / 'images/node.oci.tar')])
            inspection = inspect_image(command('result-image', ['image', 'inspect', images['imageId']]), images['imageId'], architecture)
            require(inspection.get('Config', {}).get('Labels') == images['labels'], 'loaded image labels differ')
            extracted = args.output / 'image-node'
            container('image-extract', images['imageId'], '/usr/local/bin/framenode', [], extract=extracted)
            require(build.digest(extracted) == images['node'], 'loaded image binary differs from built node')
            retain(extracted, images['node'])
            version(container('image-version', images['imageId'], '/usr/local/bin/framenode', ['--version']), fingerprint, target)
            result = {'schema': SCHEMA, 'status': 'passed', 'platform': args.platform, 'nodeVersion': '4.8.13',
                'sourceFingerprint': fingerprint, 'sourceFiles': build.digest(args.output / 'source-files.json'),
                'toolchain': toolchain, 'cargoArtifact': candidates[0], 'images': images, 'processes': stages,
                'containers': containers, 'runtimeEmbedding': 'none; external raw recovery chain specification and production Wasm required',
                'freshBuildProcessVerified': True, 'linuxElfAndVersionVerified': True, 'loadedImageVersionVerified': True, 'loadedImageBinarySha256Verified': True,
                'sourceBuildReproducibilityVerified': False, 'runtimeBindingVerified': False, 'consensusVerified': False,
                'operatorApprovalVerified': False, 'published': False, 'productionReady': False}
            guard()
            require({str(p) for p in args.output.rglob('*') if p.is_file()} == {n for n in pins if Path(n).is_relative_to(args.output)}, 'unbound build output file')
            write(args.output / 'BUILD-MANIFEST.json', result)
            return result
        except BaseException as error:
            build.write(args.output / 'failure.json', {'schema': SCHEMA, 'status': 'failed', 'error': str(error),
                'completedProcesses': stages, 'ownedContainers': containers, 'productionReady': False})
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['source-root', 'source-inventory', 'docker', 'socket', 'target', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    for name in ['source-inventory-sha256', 'docker-sha256', 'builder-image', 'base-image']:
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--platform', choices=ARCH, required=True)
    parser.add_argument('--source-date-epoch', type=int, required=True)
    args = parser.parse_args()
    require(all(getattr(args, name).is_absolute() for name in ['source_root', 'source_inventory', 'docker', 'socket', 'target', 'output']), 'all paths must be absolute')
    result = run(args)
    print(json.dumps({'manifest': str(args.output / 'BUILD-MANIFEST.json'), 'imageId': result['images']['imageId'], 'productionReady': False}))


if __name__ == '__main__':
    main()
