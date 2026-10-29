#!/usr/bin/env python3
"""Explicit public preparation for the offline SORA Nexus Linux builder.

No caller command, registry credentials, ambient Docker context or proxy is used.
Public resolution/preparation is separate from the offline release build.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import ssl
import tomllib
import urllib.request
from uuid import uuid4

import production_build as build
import production_node_image as image

HERE = Path(__file__).resolve().parent
RECIPES = HERE / 'linux_builder'
SCHEMA = 'sora-nexus-linux-builder-public-inputs-v1'
TIMEOUT = 24 * 3600
MAX_DOWNLOAD = 128 * 1024**2
GIT_REPOSITORIES = {'https://github.com/paritytech/polkadot-sdk.git',
    'https://github.com/sora-xor/ethabi.git', 'https://github.com/sora-xor/ethash.git',
    'https://github.com/sora-xor/jsonrpc.git', 'https://github.com/sora-xor/sora2-common.git',
    'https://github.com/open-web3-stack/open-runtime-module-library.git',
    'https://github.com/sora-xor/fixnum', 'https://github.com/w3f/fflonk'}
need = build.require


def digest(path):
    return build.digest(path)


def source_policy(root):
    toolchain = tomllib.loads((root / 'rust-toolchain.toml').read_text())['toolchain']
    need(re.fullmatch(r'nightly-\d{4}-\d{2}-\d{2}', toolchain['channel']), 'dated source nightly required')
    need(set(toolchain['targets']) == {'wasm32-unknown-unknown', 'wasm32v1-none'}
        and toolchain['components'] == ['rust-src'], 'unreviewed source toolchain components')
    lock = tomllib.loads((root / 'Cargo.lock').read_text())
    sources = sorted({row['source'] for row in lock['package'] if 'source' in row})
    for source in sources:
        if source == 'registry+https://github.com/rust-lang/crates.io-index':
            continue
        match = re.fullmatch(r'git\+(https://github.com/[^?#]+)(?:\?[^#]+)?#([0-9a-f]{40})', source)
        need(match is not None and match[1] in GIT_REPOSITORIES, 'unapproved or unpinned Cargo source')
    for name in build.source_names(root):
        if name.endswith('Cargo.toml'):
            value = tomllib.loads((root / name).read_text())
            def visit(item):
                if isinstance(item, dict):
                    if 'git' in item:
                        need(item['git'] in GIT_REPOSITORIES, 'unapproved manifest Git source')
                    if 'registry' in item:
                        need(item['registry'] == 'crates-io', 'private/alternate registry forbidden')
                    for child in item.values(): visit(child)
                elif isinstance(item, list):
                    for child in item: visit(child)
            visit(value)
    config = tomllib.loads((root / '.cargo/config.toml').read_text())
    need(set(config) <= {'env', 'profile'} and set(config.get('env', {})) <= {'WASM_BUILD_RUSTFLAGS'},
        'Cargo source/proxy/credential configuration is not admitted')
    return {'toolchain': toolchain['channel'], 'cargoLock': digest(root / 'Cargo.lock'), 'cargoSources': sources}


def ca_normalization(path):
    """Extract only verified public CA certificates; annotations are not trust data."""
    data = build.read(path, maximum=8 * 1024**2)
    text = data.decode('ascii')
    need('PRIVATE KEY' not in text and all(char in '\n\r\t' or 32 <= ord(char) <= 126 for char in text),
         'public PEM certificate bundle required')
    matches = list(re.finditer(r'-----BEGIN CERTIFICATE-----\s+[A-Za-z0-9+/=\s]+-----END CERTIFICATE-----', text))
    need(0 < len(matches) <= 4096, 'bounded public CA certificate bundle required')
    previous, normalized, entries = 0, [], []
    for match in matches:
        certificate = match[0]
        need(len(certificate) <= 128 * 1024, 'public certificate size bound')
        der = ssl.PEM_cert_to_DER_cert(certificate)
        canonical = ssl.DER_cert_to_PEM_cert(der)
        # PEM/base64 decoding alone accepts arbitrary bytes. OpenSSL must parse
        # exactly one CA certificate and return those complete DER bytes.
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(cadata=canonical)
        need(context.get_ca_certs(binary_form=True) == [der], 'one exact X.509 CA certificate required')
        prefix = text[previous:match.start()]
        lines = [line for line in prefix.splitlines() if line.strip() and not line.lstrip().startswith('#')]
        annotated = bool(lines)
        need('-----' not in prefix, 'unknown or malformed PEM content')
        if annotated:
            # macOS/OpenBSD bundles carry a textual certificate rendering. Its
            # middle is discarded, not interpreted; fingerprints bind the frame
            # to the following DER and prevent accidental certificate mismatch.
            fingerprint = lambda value: ':'.join(value[i:i + 2] for i in range(0, len(value), 2)).upper()
            need(len(lines) >= 5 and lines[0].startswith('=== ') and len(lines[0]) > 4
                and lines[1] == 'Certificate:' and lines[2] == '    Data:'
                and lines[-2] == 'SHA1 Fingerprint=' + fingerprint(hashlib.sha1(der).hexdigest())
                and lines[-1] == 'SHA256 Fingerprint=' + fingerprint(hashlib.sha256(der).hexdigest()),
                'unrecognized or contradictory public certificate annotation')
        entries.append({'sourceStart': match.start(), 'sourceEnd': match.end(),
            'derSha256': hashlib.sha256(der).hexdigest(), 'pemSha256': hashlib.sha256(canonical.encode()).hexdigest(),
            'annotationSha256': hashlib.sha256(prefix.encode()).hexdigest(), 'annotated': annotated})
        normalized.append(canonical)
        previous = match.end()
    tail = text[previous:]
    need('-----' not in tail and all(not line.strip() or line.lstrip().startswith('#') for line in tail.splitlines()),
         'trailing non-certificate content')
    result = ''.join(normalized).encode('ascii')
    return result, {'schema': 'sora-nexus-public-ca-normalization-v1',
        'sourcePath': str(path), 'source': build.pin_bytes(data), 'normalized': build.pin_bytes(result),
        'method': 'ordered exact X.509 CA DER to certificate-only canonical PEM; annotations discarded',
        'certificates': entries, 'sourceTailSha256': hashlib.sha256(tail.encode()).hexdigest(),
        'certificateCount': len(entries), 'productionReady': False}


def ca_bundle(path):
    return ca_normalization(path)[0]


def buildkit_seed_state(path, relative):
    """Admit only BuildKit's generated entropy for the public Docker registry.

    Upstream: moby/buildkit/session/auth/authprovider/tokenseed.go.
    This runs only inside the fresh owned Docker configuration; ambient
    credentials, configured builders and other registry entries stay forbidden.
    """
    if relative == Path('.token_seed.lock'):
        need(path.stat().st_size == 0, 'unexpected token seed lock contents')
        return True
    if relative != Path('.token_seed'):
        return False
    raw = build.read(path, maximum=4096)
    value = build.parse(raw)
    need(isinstance(value, dict) and set(value) == {'registry-1.docker.io'},
         'unexpected token seed registry')
    entry = value['registry-1.docker.io']
    need(isinstance(entry, dict) and set(entry) == {'Seed'} and isinstance(entry['Seed'], str),
         'unexpected token seed schema')
    seed = base64.b64decode(entry['Seed'], validate=True)
    need(len(seed) == 16 and base64.b64encode(seed).decode('ascii') == entry['Seed'],
         'unexpected token seed encoding')
    need(path.stat().st_mode & 0o777 == 0o600, 'token seed permissions differ')
    return True


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        raise ValueError('public download redirect is not approved')


def fetch(url, ca):
    need(re.fullmatch(r'https://static\.rust-lang\.org/(?:dist|rustup/archive)/[A-Za-z0-9./_-]+', url),
        'public download origin/path is not approved')
    context = ssl.create_default_context(cafile=str(ca))
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
        urllib.request.HTTPSHandler(context=context), NoRedirect())
    request = urllib.request.Request(url, headers={'User-Agent': 'sora-nexus-builder-preparation/1'})
    with opener.open(request, timeout=TIMEOUT) as response:
        need(response.status == 200, 'public download failed')
        data = response.read(MAX_DOWNLOAD + 1)
    need(0 < len(data) <= MAX_DOWNLOAD, 'public download size bound')
    return data


def resolve(args):
    started = build.now()
    need(not args.output.exists() and not args.output.is_relative_to(args.source_root), 'fresh output outside source required')
    source = build.verify_inventory(args.source_inventory, args.source_inventory_sha256, args.source_root)
    policy = source_policy(args.source_root)
    need(re.fullmatch(r'docker.io/library/debian@sha256:[0-9a-f]{64}', args.debian_base), 'pinned official Debian image digest required')
    need(re.fullmatch(r'\d{8}T\d{6}Z', args.debian_snapshot), 'dated Debian snapshot required')
    need(re.fullmatch(r'[1-9][0-9]*\.[0-9]+\.[0-9]+', args.rustup_version), 'explicit rustup release required')
    target = image.ARCH[args.platform][1]
    ca, ca_provenance = ca_normalization(args.ca_bundle)
    args.output.mkdir(parents=True)
    build.write(args.output / 'tls-ca.pem', ca)
    need(digest(args.ca_bundle) == ca_provenance['source'], 'public CA source changed during normalization')
    urls = {'rustup-init': f'https://static.rust-lang.org/rustup/archive/{args.rustup_version}/{target}/rustup-init',
        'rust-manifest.toml': f'https://static.rust-lang.org/dist/{policy["toolchain"][8:]}/channel-rust-nightly.toml'}
    files = {'tls-ca.pem': {'origin': 'explicit public trust bundle', **digest(args.output / 'tls-ca.pem')}}
    try:
        for name, url in urls.items():
            checksum = fetch(url + '.sha256', args.output / 'tls-ca.pem')
            match = re.fullmatch(rb'([0-9a-f]{64})(?:[ \t]+[^\r\n]+)?\r?\n?', checksum)
            need(match is not None, 'official checksum format differs')
            data = fetch(url, args.output / 'tls-ca.pem')
            need(hashlib.sha256(data).hexdigest() == match[1].decode(), 'official download checksum differs')
            build.write(args.output / name, data)
            build.write(args.output / (name + '.sha256'), checksum)
            files[name] = {'origin': url, **digest(args.output / name)}
            files[name + '.sha256'] = {'origin': url + '.sha256', **digest(args.output / (name + '.sha256'))}
    except BaseException as error:
        build.write(args.output / 'failure.json', {'pid': os.getpid(), 'startedAt': started,
            'finishedAt': build.now(), 'error': str(error), 'retainedFiles': files, 'caNormalization': ca_provenance, 'productionReady': False})
        raise
    manifest = tomllib.loads((args.output / 'rust-manifest.toml').read_text())
    need(manifest['date'] == policy['toolchain'][8:] and manifest['manifest-version'] == '2', 'wrong dated Rust manifest')
    for package in ('rustc', 'cargo', 'rust-std'):
        need(manifest['pkg'][package]['target'][target]['available'] is True, 'required native Rust package unavailable')
    need(build.source_inventory(args.source_root) == source, 'source changed during public resolution')
    need(digest(args.ca_bundle) == ca_provenance['source'] and digest(args.output / 'tls-ca.pem') == ca_provenance['normalized'],
         'public CA source/normalized input changed during resolution')
    recipe = {'schema': SCHEMA, **policy, 'platform': args.platform, 'rustTarget': target,
        'rustupVersion': args.rustup_version, 'debianBase': args.debian_base, 'debianSnapshot': args.debian_snapshot,
        'sourceInventorySha256': args.source_inventory_sha256, 'files': files, 'productionReady': False}
    build.write(args.output / 'recipe.json', recipe)
    build.write(args.output / 'PUBLIC-RESOLUTION.json', {'schema': 'sora-nexus-public-builder-resolution-v1',
        'pid': os.getpid(), 'startedAt': started, 'finishedAt': build.now(), 'fixedTool': digest(Path(__file__).resolve()),
        'method': 'explicit checksum-verified HTTPS; no redirects or ambient proxies',
        'recipe': digest(args.output / 'recipe.json'), 'sourceInventory': digest(args.source_inventory),
        'files': files, 'caNormalization': ca_provenance, 'productionReady': False})
    return recipe


def recipe_input(path, expected, root, inventory_sha):
    value = build.parse(build.read(path, expected))
    need(set(value) == {'schema', 'toolchain', 'cargoLock', 'cargoSources', 'platform', 'rustTarget',
        'rustupVersion', 'debianBase', 'debianSnapshot', 'sourceInventorySha256', 'files', 'productionReady'}, 'unknown recipe fields')
    need(value['schema'] == SCHEMA and value['productionReady'] is False, 'invalid preparation recipe')
    need(re.fullmatch(r'docker.io/library/debian@sha256:[0-9a-f]{64}', value['debianBase'])
        and re.fullmatch(r'\d{8}T\d{6}Z', value['debianSnapshot']), 'unapproved Debian input')
    need(value['platform'] in image.ARCH and value['rustTarget'] == image.ARCH[value['platform']][1], 'platform/target differs')
    need(re.fullmatch(r'[1-9][0-9]*\.[0-9]+\.[0-9]+', value['rustupVersion']), 'explicit rustup release required')
    need({key:value[key] for key in ('toolchain', 'cargoLock', 'cargoSources')} == source_policy(root)
        and value['sourceInventorySha256'] == inventory_sha, 'recipe source/toolchain differs')
    need(set(value['files']) == {'tls-ca.pem', 'rustup-init', 'rustup-init.sha256', 'rust-manifest.toml', 'rust-manifest.toml.sha256'}, 'unexpected public payload graph')
    for name, row in value['files'].items():
        need(set(row) == {'origin', 'sha256', 'blake2_256', 'bytes'}, 'payload pin fields differ')
        build.read(build.local(path.parent, name), {k:row[k] for k in ('sha256', 'blake2_256', 'bytes')}, maximum=MAX_DOWNLOAD)
    expected_origins = {'rustup-init': f'https://static.rust-lang.org/rustup/archive/{value["rustupVersion"]}/{value["rustTarget"]}/rustup-init',
        'rust-manifest.toml': f'https://static.rust-lang.org/dist/{value["toolchain"][8:]}/channel-rust-nightly.toml'}
    for name, url in expected_origins.items():
        need(value['files'][name]['origin'] == url and value['files'][name + '.sha256']['origin'] == url + '.sha256',
            'unapproved public payload origin')
        checksum = build.read(path.parent / (name + '.sha256'), maximum=4096).split()[0].decode()
        need(checksum == value['files'][name]['sha256'], 'official checksum/recipe mismatch')
    need(value['files']['tls-ca.pem']['origin'] == 'explicit public trust bundle', 'unexpected CA origin')
    manifest = tomllib.loads((path.parent / 'rust-manifest.toml').read_text())
    need(manifest.get('date') == value['toolchain'][8:] and manifest.get('manifest-version') == '2', 'wrong Rust manifest identity')
    def check_urls(item):
        if isinstance(item, dict):
            for key, child in item.items():
                if key.endswith('url'):
                    need(isinstance(child, str) and re.fullmatch(r'https://static\.rust-lang\.org/dist/[A-Za-z0-9./_-]+', child), 'unapproved Rust package origin')
                else: check_urls(child)
        elif isinstance(item, list):
            for child in item: check_urls(child)
    check_urls(manifest)
    image.elf((path.parent / 'rustup-init').read_bytes(), image.ARCH[value['platform']][2])
    need(ca_bundle(path.parent / 'tls-ca.pem') == build.read(path.parent / 'tls-ca.pem'),
         'TLS payload must contain canonical certificate-only bytes')
    return value


def prepare(args):
    image.directory(args.source_root)
    need(not args.output.exists() and args.output.resolve() == args.output
        and not args.output.is_relative_to(args.source_root), 'fresh isolated preparation output required')
    source = build.verify_inventory(args.source_inventory, args.source_inventory_sha256, args.source_root)
    value = recipe_input(args.inputs, args.inputs_sha256, args.source_root, args.source_inventory_sha256)
    socket = image.socket_identity(args.socket)
    architecture = image.ARCH[value['platform']][0]
    args.output.mkdir(parents=True)
    pins = {}
    def retain(path, pin):
        need(str(path) not in pins or pins[str(path)] == pin, 'preparation evidence rebinding')
        pins[str(path)] = pin
    def copy(source_path, destination, expected=None):
        pin = digest(source_path)
        if expected is not None:
            need(pin == expected if isinstance(expected, dict) else pin['sha256'] == expected, 'validated input changed before copy')
        retain(source_path, pin)
        build.write(destination, build.read(source_path, pin, maximum=2 * 1024**3))
        need(digest(destination) == pin, 'preparation copy differs'); retain(destination, pin)
    cli = args.output / 'docker'
    plugin = args.output / 'docker-config/cli-plugins/docker-buildx'
    for original, expected, destination in [(args.docker, args.docker_sha256, cli), (args.buildx, args.buildx_sha256, plugin)]:
        build.read(original, expected, maximum=256 * 1024**2)
        copy(original, destination, expected); destination.chmod(0o555)
    config = args.output / 'docker-config/config.json'
    build.write(config, {}); retain(config, digest(config))
    copy(args.inputs, args.output / 'context/recipe.json', args.inputs_sha256)
    resolution = build.parse(build.read(args.inputs.parent / 'PUBLIC-RESOLUTION.json'))
    need(resolution.get('schema') == 'sora-nexus-public-builder-resolution-v1'
        and resolution.get('recipe') == digest(args.inputs) and resolution.get('files') == value['files']
        and resolution.get('sourceInventory') == digest(args.source_inventory)
        and resolution.get('fixedTool') == digest(Path(__file__).resolve()), 'public resolution diagnostic differs')
    copy(args.inputs.parent / 'PUBLIC-RESOLUTION.json', args.output / 'PUBLIC-RESOLUTION.json')
    copy(args.source_inventory, args.output / 'source-inventory.json', args.source_inventory_sha256)
    copy(args.source_inventory, args.output / 'context/source-inventory.json', args.source_inventory_sha256)
    for name, row in value['files'].items():
        copy(args.inputs.parent / name, args.output / 'context' / name, {key:row[key] for key in ('sha256', 'blake2_256', 'bytes')})
    for path in sorted(RECIPES.iterdir()):
        need(path.is_file(), 'unexpected recipe directory')
        copy(path, args.output / 'context' / ('Dockerfile' if path.name == 'Dockerfile' else 'scripts/' + path.name))
    for name, pin in source.items():
        need(digest(args.source_root / name) == pin, 'source changed before context export')
        copy(args.source_root / name, args.output / 'context/source' / name, pin)
    for name, pin in build.helper_pins().items():
        retain(Path(name), pin)
    for helper in (Path(__file__).resolve(), Path(image.__file__).resolve()): retain(helper, digest(helper))
    (args.output / 'empty-home').mkdir()
    environment = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C', 'DOCKER_CONFIG': str(config.parent),
        'HOME': str(args.output / 'empty-home'), 'DOCKER_HOST': 'unix://' + str(args.socket),
        'DOCKER_BUILDKIT': '1', 'BUILDX_BUILDER': 'default', 'BUILDX_CONFIG': str(config.parent / 'buildx'),
        'BUILDX_GIT_INFO': '0', 'BUILDX_GIT_LABELS': '0', 'BUILDX_GIT_CHECK_DIRTY': '0'}
    prefix = [str(cli), '--config', str(config.parent), '--host', 'unix://' + str(args.socket)]
    stages = []; sequence = 0
    control = {str(path): pins[str(path)] for path in (cli, plugin, config)}
    def local_guard():
        build.guard_files(control)
        need(image.socket_identity(args.socket) == socket, 'approved local socket changed')
        entries = list(config.parent.rglob('*'))
        need(len(entries) <= 4096, 'owned Buildx state bound')
        for path in entries:
            relative = path.relative_to(config.parent)
            need(not path.is_symlink() and path.resolve(strict=True) == path, 'owned Docker state alias')
            if path.is_file() and str(path) not in control:
                if buildkit_seed_state(path, relative):
                    continue
                need(relative.parts[0] == 'buildx' and 'instances' not in relative.parts
                    and not any(part in ('contexts', 'credentials', 'credentials.json', 'config.json') for part in relative.parts)
                    and path.stat().st_size <= 16 * 1024**2, 'unexpected Docker credential/context/builder configuration')
    def guard():
        local_guard(); build.guard_files(pins)
        need(build.source_inventory(args.source_root) == source, 'source set changed during preparation')
    def run(label, arguments, cleanup=False):
        nonlocal sequence
        sequence += 1
        folder = args.output / 'processes' / f'{sequence:03d}-{label}'
        stdout, receipt = build.process(prefix + arguments, args.output, folder,
            local_guard if cleanup else guard, env=environment, timeout=TIMEOUT,
            maximum=256 * 1024**2, retain=retain)
        need(receipt['exitCode'] == 0, 'Docker preparation command failed')
        stages.append({'label': label, 'receipt': digest(folder / 'execution.json')})
        return stdout
    def inspect(image_id):
        data = build.parse(run('inspect-image', ['image', 'inspect', image_id]))
        return image.inspect_image(json.dumps(data).encode(), image_id, architecture)
    active_name = None; active_id = None
    try:
        run('docker-version', ['version', '--format', '{{json .}}'])
        run('buildx-version', ['buildx', 'version'])
        run('pull-pinned-debian', ['pull', '--platform', value['platform'], value['debianBase']])
        base = build.parse(run('inspect-pinned-debian', ['image', 'inspect', value['debianBase']]))
        need(isinstance(base, list) and len(base) == 1, 'Debian image inspection shape')
        base_id = base[0]['Id']; image.image_id(base_id); image.inspect_image(json.dumps(base).encode(), base_id, architecture)
        need(any(item.endswith('@' + value['debianBase'].split('@')[1]) for item in base[0].get('RepoDigests', [])), 'Debian digest admission differs')
        images = {}
        for stage in ('runtime', 'builder'):
            iid = args.output / (stage + '.iid')
            need(not os.path.lexists(iid), 'fresh image ID output required')
            run('build-' + stage, ['buildx', 'build', '--builder', 'default', '--load', '--progress=plain',
                '--iidfile', str(iid),
                '--pull=false', '--network=default', '--platform', value['platform'], '--target', stage,
                '--build-arg', 'DEBIAN_BASE=' + value['debianBase'], '--build-arg', 'RECIPE_SHA256=' + args.inputs_sha256,
                '--file', str(args.output / 'context/Dockerfile'), str(args.output / 'context')])
            raw = build.read(iid, maximum=128)
            need(re.fullmatch(rb'sha256:[0-9a-f]{64}\n?', raw), 'build image ID file malformed')
            iid_pin = build.pin_bytes(raw)
            need(digest(iid) == iid_pin, 'build image ID changed during admission')
            retain(iid, iid_pin)
            image_id = raw.decode().strip(); image.image_id(image_id)
            inspected = inspect(image_id)
            need(inspected['Config']['Labels'].get('org.sora-nexus.preparation.recipe') == args.inputs_sha256,
                'prepared image recipe label differs')
            images[stage] = {'imageId': image_id, 'inspection': inspected}
        active_name = 'sora-nexus-builder-smoke-' + uuid4().hex
        owner = uuid4().hex
        raw = run('create-offline-smoke', ['create', '--name', active_name,
            '--label', 'org.sora-nexus.preparation.owner=' + owner, '--network', 'none', '--cap-drop', 'ALL',
            '--security-opt', 'no-new-privileges', '--entrypoint', '/usr/bin/python3', images['builder']['imageId'], '/preparation/smoke.py'])
        need(re.fullmatch(rb'[0-9a-f]{64}\n?', raw), 'owned smoke container ID absent')
        active_id = raw.decode().strip()
        smoke_raw = run('offline-smoke', ['start', '--attach', active_id])
        observed = build.parse(run('inspect-offline-smoke', ['container', 'inspect', active_id]))
        need(len(observed) == 1 and observed[0]['Id'] == active_id and observed[0]['Name'] == '/' + active_name
            and observed[0]['Image'] == images['builder']['imageId']
            and observed[0]['Config']['Labels'].get('org.sora-nexus.preparation.owner') == owner
            and observed[0]['State']['ExitCode'] == 0, 'offline smoke container failed or changed')
        smoke = build.parse(smoke_raw)
        need(smoke.get('schema') == 'sora-nexus-linux-builder-offline-smoke-v1'
            and smoke.get('toolchain') == value['toolchain'] and smoke.get('rustTarget') == value['rustTarget']
            and smoke.get('dependencyMetadataVerified') is True and smoke.get('nativeBuildVerified') is False
            and smoke.get('sourceInventoryVerified') is True and smoke.get('sourceInventorySha256') == args.source_inventory_sha256
            and smoke.get('recipeSha256') == args.inputs_sha256
            and smoke.get('productionReady') is False, 'offline smoke contract differs')
        need(set(smoke.get('checks', {})) == {'rustc', 'cargo', 'cc', 'cxx', 'cmake', 'protoc', 'openssl', 'clang', 'metadata'},
            'offline smoke checks incomplete')
        run('remove-owned-smoke', ['rm', '-f', active_id], cleanup=True); active_name = None; active_id = None
        for path in config.parent.rglob('*'):
            if path.is_file() and str(path) not in pins: retain(path, digest(path))
        guard()
        result = {'schema': 'sora-nexus-linux-builder-preparation-v1', 'status': 'passed',
            'recipe': pins[str(args.inputs)], 'sourceInventory': pins[str(args.source_inventory)],
            'debianBase': {'reference': value['debianBase'], 'imageId': base_id}, 'images': images,
            'offlineSmoke': smoke, 'processes': stages, 'files': {name: pin for name, pin in pins.items() if Path(name).is_relative_to(args.output)},
            'nativeBuildVerified': False, 'published': False, 'productionReady': False}
        guard()
        build.write(args.output / 'BUILDER-MANIFEST.json', result)
        return result
    except BaseException as error:
        if active_name is not None:
            try:
                found = build.parse(run('cleanup-inspect', ['container', 'inspect', active_id or active_name], cleanup=True))
                need(len(found) == 1 and found[0]['Name'] == '/' + active_name
                    and (active_id is None or found[0]['Id'] == active_id)
                    and found[0]['Config']['Labels'].get('org.sora-nexus.preparation.owner') == owner
                    and found[0]['Image'] == images['builder']['imageId'], 'cleanup identity differs')
                run('cleanup-owned-smoke', ['rm', '-f', active_id or active_name], cleanup=True)
            except BaseException as cleanup_error:
                build.write(args.output / 'cleanup-failure.json', {'error': str(cleanup_error), 'productionReady': False})
        build.write(args.output / 'failure.json', {'error': str(error), 'productionReady': False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    for name in ('resolve', 'prepare'):
        item = sub.add_parser(name)
        for flag in ('source-root', 'source-inventory', 'output'):
            item.add_argument('--' + flag, type=Path, required=True)
        item.add_argument('--source-inventory-sha256', required=True)
        if name == 'resolve':
            item.add_argument('--platform', choices=sorted(image.ARCH), required=True)
            for flag in ('debian-base', 'debian-snapshot', 'rustup-version'):
                item.add_argument('--' + flag, required=True)
            item.add_argument('--ca-bundle', type=Path, required=True)
        else:
            for flag in ('inputs', 'docker', 'buildx', 'socket'):
                item.add_argument('--' + flag, type=Path, required=True)
            for flag in ('inputs-sha256', 'docker-sha256', 'buildx-sha256'):
                item.add_argument('--' + flag, required=True)
    args = parser.parse_args()
    need(all(value.is_absolute() for value in vars(args).values() if isinstance(value, Path)), 'absolute input paths required')
    result = resolve(args) if args.action == 'resolve' else prepare(args)
    print(json.dumps({'schema': result['schema'], 'output': str(args.output), 'productionReady': False}))


if __name__ == '__main__':
    main()
