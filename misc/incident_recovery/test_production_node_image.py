"""Synthetic files and fake Docker process backend only; no Docker/Cargo run."""
import copy
import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import socket
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import production_node_image as image


class NodeImageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='node-image-unit-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.source = self.root / 'source'; self.source.mkdir()
        for name, raw in {'node/Cargo.toml': b'[package]\nname="framenode"\nversion="4.8.13"\n',
                          'rust-toolchain.toml': b'[toolchain]\nchannel="nightly-2026-10-23"\n',
                          'Cargo.lock': b'version = 4\n', 'Cargo.toml': b'[workspace]\nmembers=["node"]\n'}.items():
            p = self.source / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(raw)
        self.inventory = self.root / 'source-files.json'
        self.inventory.write_bytes(image.build.serialized(self.source_inventory(self.source)))
        self.docker = self.root / 'docker'; self.docker.write_bytes(b'fake Docker bytes never executed'); self.docker.chmod(0o555)
        self.target = self.root / 'target'; self.target.mkdir()
        self.socket_path = self.root / 'docker.sock'
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); self.sock.bind(str(self.socket_path))
        self.addCleanup(self.sock.close)
        self.base_raw, self.base_id = self.base_archive()
        self.builder_id = 'sha256:' + 'a' * 64
        self.args = SimpleNamespace(source_root=self.source, source_inventory=self.inventory,
            source_inventory_sha256=image.build.digest(self.inventory)['sha256'], docker=self.docker,
            docker_sha256=image.build.digest(self.docker)['sha256'], socket=self.socket_path,
            builder_image=self.builder_id, base_image=self.base_id, platform='linux/amd64', target=self.target,
            output=self.root/'result', source_date_epoch=1_792_972_800)
        self.containers = {}; self.removed = []; self.commands = []; self.exit_code = 0; self.mutate = None; self.malformed_create = False
        self.images = {self.base_id: {'Id': self.base_id, 'Os': 'linux', 'Architecture': 'amd64'},
                       self.builder_id: {'Id': self.builder_id, 'Os': 'linux', 'Architecture': 'amd64'}}

    def source_inventory(self, root):
        return {p.relative_to(root).as_posix(): image.build.digest(p) for p in sorted(root.rglob('*')) if p.is_file()}

    def base_archive(self):
        layer = io.BytesIO()
        with tarfile.open(fileobj=layer, mode='w') as tar:
            image.tar_add(tar, 'etc/os-release', io.BytesIO(b'synthetic base'), 14)
        raw = layer.getvalue(); sha = hashlib.sha256(raw).hexdigest()
        config = image.build.serialized({'architecture': 'amd64', 'os': 'linux',
            'rootfs': {'type': 'layers', 'diff_ids': ['sha256:' + sha]}, 'history': []})
        cid = hashlib.sha256(config).hexdigest()
        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode='w') as tar:
            image.tar_add(tar, 'base/layer.tar', io.BytesIO(raw), len(raw))
            image.tar_add(tar, cid + '.json', io.BytesIO(config), len(config))
            manifest = image.build.serialized([{'Config': cid + '.json', 'RepoTags': None, 'Layers': ['base/layer.tar']}])
            image.tar_add(tar, 'manifest.json', io.BytesIO(manifest), len(manifest))
        return archive.getvalue(), 'sha256:' + cid

    def node(self):
        raw = bytearray(64)
        raw[:7] = b'\x7fELF\x02\x01\x01'; raw[16:18] = (3).to_bytes(2, 'little'); raw[18:20] = (62).to_bytes(2, 'little')
        return bytes(raw) + b'synthetic executable, never run'

    def fake_process(self, command, cwd, output, guard, **kwargs):
        self.commands.append(command)
        self.assertEqual(command[1:5], ['--config', str(self.args.output/'docker-config'), '--host', 'unix://' + str(self.socket_path)])
        self.assertGreaterEqual(kwargs['timeout'], 3600)
        guard(); output.mkdir(parents=True)
        args = command[5:]; raw = b''
        if args[:2] == ['image', 'inspect']:
            raw = json.dumps([self.images[args[2]]]).encode()
        elif args[:2] == ['container', 'create']:
            self.assertEqual(args[args.index('--network')+1], 'none')
            self.assertEqual(args[args.index('--pull')+1], 'never')
            self.assertNotIn('--privileged', args)
            name = args[args.index('--name')+1]
            cid = hashlib.sha256(name.encode()).hexdigest()
            at = next(i for i, x in enumerate(args) if x.startswith('sha256:'))
            self.containers[cid] = {'Id': cid, 'Name': '/' + name, 'Image': args[at],
                'Config': {'Labels': {'org.sora-nexus.owned-build': args[args.index('--label')+1].split('=', 1)[1]}},
                'State': {'Running': False, 'ExitCode': self.exit_code}, 'args': args, 'tail': args[at+1:]}
            raw = b'malformed\n' if self.malformed_create else (cid + '\n').encode()
        elif args[:2] == ['container', 'start']:
            state = self.containers[args[-1]]
            tail = state['tail']; entry = state['args'][state['args'].index('--entrypoint')+1]
            if entry.endswith('/rustc'):
                raw = b'rustc synthetic\nhost: x86_64-unknown-linux-gnu\n'
            elif entry.endswith('/cargo') and tail[-1] == '-V':
                raw = b'cargo synthetic\n'
            elif entry.endswith('/cargo'):
                self.assertIn('--locked', tail); self.assertIn('--offline', tail); self.assertIn('--frozen', tail)
                self.assertIn('--no-default-features', tail)
                self.assertEqual(state['args'][state['args'].index('--workdir')+1], '/source')
                node = self.target/'x86_64-unknown-linux-gnu/release/framenode'; node.parent.mkdir(parents=True); node.write_bytes(self.node())
                raw = (json.dumps({'reason': 'compiler-artifact', 'target': {'name':'framenode','kind':['bin']},
                    'features': [], 'manifest_path': '/source/node/Cargo.toml', 'package_id': 'path+file:///source/node#framenode@4.8.13',
                    'executable':'/target/x86_64-unknown-linux-gnu/release/framenode','fresh':False})+'\n'+
                    json.dumps({'reason':'build-finished','success':True})+'\n').encode()
            else:
                raw = f'{Path(entry).name} 4.8.13-src-{self.args.source_inventory_sha256[:16]}-x86_64-unknown-linux-gnu\n'.encode()
        elif args[:2] == ['container', 'inspect']:
            matched = [value for key, value in self.containers.items() if key == args[2] or value['Name'] == '/' + args[2]]
            raw = json.dumps(matched).encode()
        elif args[:2] == ['container', 'cp']:
            Path(args[3]).write_bytes(self.node())
        elif args[:2] == ['container', 'rm']:
            self.removed.append(args[-1]); raw = (args[-1]+'\n').encode()
        elif args[:2] == ['image', 'save']:
            Path(args[3]).write_bytes(self.base_raw)
        elif args[:2] == ['image', 'load']:
            with tarfile.open(args[3]) as tar:
                self.assertTrue(args[3].endswith('/node.oci.tar'))
                index = json.load(tar.extractfile('index.json'))
                descriptor = index['manifests'][0]
                cid = descriptor['digest']
                manifest = json.load(tar.extractfile('blobs/sha256/' + cid[7:]))
                config = json.load(tar.extractfile('blobs/sha256/' + manifest['config']['digest'][7:]))
                self.images[cid] = {'Id': cid, 'Os': 'linux', 'Architecture': 'amd64',
                    'Config': config['config'], 'Descriptor': descriptor}
            raw = ('Loaded image ID: ' + cid).encode()
        else:
            self.fail('unexpected command: ' + repr(args))
        if self.mutate:
            self.mutate(output.name, args)
        guard()
        receipt = {'schema': 'synthetic-process-v1', 'command': command, 'pid': 123,
            'exitCode': 0, 'outputs': {}}
        for name, value in [('stdout.log', raw), ('stderr.log', b''), ('launch.json', {'synthetic': True}), ('started.json', {'pid':123})]:
            pin = image.build.write(output/name, value); kwargs['retain'](output/name, pin)
            if name in ('stdout.log','stderr.log'): receipt['outputs'][name] = pin
        pin = image.build.write(output/'execution.json', receipt); kwargs['retain'](output/'execution.json', pin)
        return raw, receipt

    def run_fake(self):
        with patch.object(image.build, 'source_inventory', side_effect=self.source_inventory), patch.object(image.build, 'process', side_effect=self.fake_process):
            return image.run(self.args)

    def test_complete_fake_corridor_is_offline_exact_and_not_ready(self):
        result = self.run_fake()
        self.assertEqual(result['status'], 'passed'); self.assertFalse(result['productionReady'])
        self.assertFalse(result['runtimeBindingVerified']); self.assertFalse(result['published'])
        self.assertEqual(set(self.containers), set(self.removed))
        self.assertTrue((self.args.output/'images/node.oci.tar').is_file())
        self.assertTrue((self.args.output/'images/node.docker.tar').is_file())
        self.assertFalse(any('push' in c or 'pull' in c or 'login' in c for c in self.commands))

    def test_version_probe_uses_release_executable_basename(self):
        self.run_fake()
        probes = [c for c in self.commands if c[5:7] == ['container', 'create'] and c[-1] == '--version']
        self.assertEqual(len(probes), 2)
        entries = [c[c.index('--entrypoint') + 1] for c in probes]
        self.assertEqual(entries, ['/framenode', '/usr/local/bin/framenode'])
        self.assertTrue(any(value.endswith('target=/framenode,readonly') for value in probes[0]))
        wrong_name = f'node 4.8.13-src-{self.args.source_inventory_sha256[:16]}-x86_64-unknown-linux-gnu\n'.encode()
        with self.assertRaisesRegex(ValueError, 'version/source/target differs'):
            image.version(wrong_name, self.args.source_inventory_sha256, 'x86_64-unknown-linux-gnu')

    def test_cargo_hardlink_is_copied_to_independent_release_file(self):
        def hardlink(stage, args):
            if stage == 'cargo-build-start':
                node = self.target/'x86_64-unknown-linux-gnu/release/framenode'
                deps = node.parent/'deps'; deps.mkdir()
                os.link(node, deps/'framenode-cargo-hash')
        self.mutate = hardlink
        result = self.run_fake()
        self.assertEqual(result['status'], 'passed')
        node = self.args.output/'bin/framenode'
        self.assertEqual(node.stat().st_nlink, 1)
        native = self.target/'x86_64-unknown-linux-gnu/release/framenode'
        self.assertEqual(native.stat().st_nlink, 2)
        self.assertNotEqual(node.stat().st_ino, native.stat().st_ino)
        with self.assertRaisesRegex(ValueError, 'file alias'):
            image.build.digest(native)

    def test_cargo_copy_rejects_symlink_wrong_path_and_source_mutation(self):
        native = self.target/'x86_64-unknown-linux-gnu/release/framenode'
        native.parent.mkdir(parents=True)
        original = self.root/'original'; original.write_bytes(self.node())
        native.symlink_to(original)
        with self.assertRaisesRegex(ValueError, 'aliased'):
            image.copy_cargo_artifact(native, self.root/'copy', self.target, 'x86_64-unknown-linux-gnu')
        native.unlink(); native.write_bytes(self.node())
        with self.assertRaisesRegex(ValueError, 'path differs'):
            image.copy_cargo_artifact(original, self.root/'copy', self.target, 'x86_64-unknown-linux-gnu')
        real_fsync = os.fsync
        def mutate(fd):
            real_fsync(fd)
            native.write_bytes(b'changed during copy')
        with patch.object(image.os, 'fsync', side_effect=mutate), self.assertRaisesRegex(ValueError, 'changed during copy'):
            image.copy_cargo_artifact(native, self.root/'copy', self.target, 'x86_64-unknown-linux-gnu')

    def test_rejects_mutable_images_and_non_socket_endpoints_before_commands(self):
        self.args.builder_image = 'builder:latest'
        with self.assertRaisesRegex(ValueError, 'immutable'): self.run_fake()
        self.assertEqual(self.commands, [])
        self.args.builder_image = self.builder_id; self.args.socket = self.docker
        with self.assertRaisesRegex(ValueError, 'Unix Docker socket'): self.run_fake()

    def test_rejects_symlink_socket_and_socket_replacement(self):
        link = self.root / 'linked.sock'; link.symlink_to(self.socket_path)
        self.args.socket = link
        with self.assertRaisesRegex(ValueError, 'without aliases'): self.run_fake()
        self.args.socket = self.socket_path
        replacement = None
        def mutate(stage, args):
            nonlocal replacement
            if stage == 'builder-image':
                self.socket_path.unlink()
                replacement = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                replacement.bind(str(self.socket_path))
                self.addCleanup(replacement.close)
        self.mutate = mutate
        with self.assertRaisesRegex(ValueError, 'socket identity changed'): self.run_fake()
        self.assertEqual(self.containers, {})

    def test_rejects_wrong_preloaded_architecture_without_container(self):
        self.images[self.builder_id]['Architecture'] = 'arm64'
        with self.assertRaisesRegex(ValueError, 'identity/platform'): self.run_fake()
        self.assertEqual(self.containers, {})

    def test_rejects_node_version_source_mismatch(self):
        (self.source/'node/Cargo.toml').write_bytes(b'[package]\nversion="4.8.12"\n')
        with self.assertRaises(ValueError): self.run_fake()
        self.assertEqual(self.commands, [])

    def test_container_nonzero_exit_fails_even_if_docker_cli_succeeds(self):
        self.exit_code = 9
        with self.assertRaisesRegex(ValueError, 'container process'): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))
        self.assertFalse((self.args.output/'BUILD-MANIFEST.json').exists())

    def test_malformed_create_stdout_still_removes_exact_owned_name(self):
        self.malformed_create = True
        with self.assertRaisesRegex(ValueError, 'created container identity'): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))
        self.assertFalse(any(c[5:7] == ['container','start'] for c in self.commands))

    def test_failed_create_after_daemon_creation_still_cleans_up(self):
        def mutate(stage, args):
            if stage == 'rustc-create':
                raise ValueError('synthetic CLI failure after creation')
        self.mutate = mutate
        with self.assertRaisesRegex(ValueError, 'CLI failure'): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))

    def test_source_mutation_still_cleans_up_owned_container(self):
        def mutate(stage, args):
            if stage == 'cargo-build-start':
                (self.source/'Cargo.lock').write_bytes(b'changed')
        self.mutate = mutate
        with self.assertRaises(ValueError): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))
        self.assertFalse((self.args.output/'BUILD-MANIFEST.json').exists())

    def test_prior_receipt_mutation_blocks_completion(self):
        def mutate(stage, args):
            if stage == 'image-version-start':
                p = self.args.output/'processes/cargo-build-start/execution.json'; p.write_bytes(p.read_bytes()+b' ')
        self.mutate = mutate
        with self.assertRaises(ValueError): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))

    def test_archive_assembly_is_deterministic_and_architecture_checked(self):
        base = self.root/'base.tar'; base.write_bytes(self.base_raw)
        node = self.root/'node'; node.write_bytes(self.node())
        a = image.archives(base, self.base_id, node, self.root/'a', 'amd64', '1'*64, 0)
        b = image.archives(base, self.base_id, node, self.root/'b', 'amd64', '1'*64, 0)
        self.assertEqual(a, b)
        with self.assertRaisesRegex(ValueError, 'platform'): image.archives(base,self.base_id,node,self.root/'c','arm64','1'*64,0)

    def test_base_archive_config_and_layer_tampering_are_rejected(self):
        base = self.root/'base.tar'; base.write_bytes(self.base_raw)
        node = self.root/'node'; node.write_bytes(self.node())
        with self.assertRaisesRegex(ValueError, 'configuration'): image.archives(base,'sha256:'+'b'*64,node,self.root/'a','amd64','1'*64,0)
        changed = bytearray(self.base_raw); at = changed.find(b'synthetic base'); self.assertGreater(at,0); changed[at] ^= 1; base.write_bytes(changed)
        with self.assertRaisesRegex(ValueError, 'rootfs'): image.archives(base,self.base_id,node,self.root/'b','amd64','1'*64,0)

    def test_loaded_image_binary_hash_is_checked_independently(self):
        def mutate(stage, args):
            if stage == 'image-extract-copy':
                (self.args.output/'image-node').write_bytes(b'wrong binary')
        self.mutate = mutate
        with self.assertRaisesRegex(ValueError, 'loaded image binary'): self.run_fake()
        self.assertEqual(set(self.containers), set(self.removed))

    def test_archive_mutation_after_assembly_cannot_be_rebound(self):
        original = image.archives
        def altered(*args):
            result = original(*args)
            path = args[3]/'node.oci.tar'; path.write_bytes(path.read_bytes()+b'x')
            return result
        with patch.object(image, 'archives', side_effect=altered):
            with self.assertRaisesRegex(ValueError, 'changed after assembly'): self.run_fake()
        self.assertFalse((self.args.output/'BUILD-MANIFEST.json').exists())

    def test_output_must_be_outside_source_and_persistent_target(self):
        self.args.output = self.source/'output'
        with self.assertRaisesRegex(ValueError, 'outside target/source'): self.run_fake()
        self.assertEqual(self.commands, [])
        self.args.output = self.target/'output'
        with self.assertRaisesRegex(ValueError, 'outside target/source'): self.run_fake()

    def test_elf_and_version_bind_target_and_source(self):
        image.elf(self.node(), 62)
        with self.assertRaises(ValueError): image.elf(self.node(),183)
        with self.assertRaises(ValueError): image.elf(b'not ELF',62)
        with self.assertRaises(ValueError): image.version(b'framenode 4.8.13-wrong\n','1'*64,'x86_64-unknown-linux-gnu')


    def oci_base_archive(self, *, transform=None, tar_transform=None, direct=False):
        with tarfile.open(fileobj=io.BytesIO(self.base_raw)) as old:
            legacy = json.load(old.extractfile('manifest.json'))[0]
            config = json.load(old.extractfile(legacy['Config']))
            layer = old.extractfile(legacy['Layers'][0]).read()
        payload = {'config': config, 'layer': gzip.compress(layer, mtime=123), 'diff': layer,
                   'layerType': 'application/vnd.oci.image.layer.v1.tar+gzip', 'duplicatePlatform': False}
        if transform:
            transform(payload)
        blobs = {}
        def blob(raw, media):
            sha = hashlib.sha256(raw).hexdigest(); blobs['blobs/sha256/' + sha] = raw
            return {'mediaType': media, 'digest': 'sha256:' + sha, 'size': len(raw)}
        cd = blob(image.build.serialized(payload['config']), 'application/vnd.oci.image.config.v1+json')
        ld = blob(payload['layer'], payload['layerType'])
        if payload.get('badLayerSize'): ld['size'] += 1
        if payload.get('badLayerDigest'): ld['digest'] = 'sha256:' + 'e' * 64
        manifest = {'schemaVersion': 2, 'mediaType': image.OCI_MANIFEST, 'config': cd, 'layers': [ld]}
        md = blob(image.build.serialized(manifest), image.OCI_MANIFEST)
        md['platform'] = {'os': 'linux', 'architecture': payload.get('architecture', 'amd64')}
        children = [md]
        # Unselected attestations remain references inside the authenticated index.
        children.append({'mediaType': image.OCI_MANIFEST, 'digest': 'sha256:' + 'b'*64, 'size': 42,
                         'platform': {'os': 'unknown', 'architecture': 'unknown'}})
        if payload['duplicatePlatform']:
            children.append(copy.deepcopy(md))
        if payload.get('secondPlatform'):
            second = copy.deepcopy(manifest); second['annotations'] = {'synthetic': 'second matching image'}
            second_d = blob(image.build.serialized(second), image.OCI_MANIFEST); second_d['platform'] = md['platform']
            children.append(second_d)
        root = md if direct else blob(image.build.serialized({'schemaVersion': 2, 'mediaType': image.OCI_INDEX,
                                                            'manifests': children}), image.OCI_INDEX)
        blobs['index.json'] = image.build.serialized({'schemaVersion': 2, 'manifests': [root]})
        blobs['oci-layout'] = image.build.serialized({'imageLayoutVersion': '1.0.0'})
        blobs['manifest.json'] = image.build.serialized([{'Config': 'blobs/sha256/' + cd['digest'][7:],
            'RepoTags': None, 'Layers': ['blobs/sha256/' + ld['digest'][7:]]}])
        if tar_transform:
            tar_transform(blobs, root, md, cd, ld)
        result = io.BytesIO()
        with tarfile.open(fileobj=result, mode='w') as tar:
            for name, raw in blobs.items():
                image.tar_add(tar, name, io.BytesIO(raw), len(raw))
        return result.getvalue(), root['digest']

    def assemble_oci(self, **kwargs):
        raw, identity = self.oci_base_archive(**kwargs)
        base = self.root/'base-oci.tar'; base.write_bytes(raw)
        node = self.root/'oci-node'; node.write_bytes(self.node())
        return image.archives(base, identity, node, self.root/'assembled', 'amd64', '1'*64, 0)

    def test_oci_index_graph_decompresses_and_preserves_content_identities(self):
        value = self.assemble_oci()
        self.assertEqual(value['baseIdentity']['format'], 'oci-descriptor-graph')
        self.assertNotEqual(value['baseIdentity']['inputImageId'], value['baseIdentity']['configId'])
        self.assertEqual(value['imageId'], value['ociManifestDigest'])
        self.assertEqual(value['imageId'], value['loadedImageReference'])
        self.assertNotEqual(value['imageId'], value['configId'])
        self.assertNotEqual(value['imageId'], value['ociIndexDigest'])
        with tarfile.open(self.root/'assembled/node.docker.tar') as tar:
            row = json.load(tar.extractfile('manifest.json'))[0]
            config = json.load(tar.extractfile(row['Config']))
            for name, diff in zip(row['Layers'], config['rootfs']['diff_ids']):
                self.assertEqual('sha256:' + hashlib.sha256(tar.extractfile(name).read()).hexdigest(), diff)
        legacy = self.root/'legacy.tar'; legacy.write_bytes(self.base_raw)
        legacy_value = image.archives(legacy, self.base_id, self.root/'oci-node', self.root/'legacy-result', 'amd64', '1'*64, 0)
        self.assertEqual(value['dockerArchive'], legacy_value['dockerArchive'])
        self.assertEqual(value['ociArchive'], legacy_value['ociArchive'])

    def test_direct_oci_manifest_is_authenticated(self):
        value = self.assemble_oci(direct=True)
        self.assertEqual(value['baseIdentity']['inputImageId'], value['baseIdentity']['selectedManifestDigest'])

    def test_oci_duplicate_or_ambiguous_platform_is_rejected(self):
        for field in ('duplicatePlatform', 'secondPlatform'):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temp:
                old = self.root; self.root = Path(temp).resolve()
                try:
                    with self.assertRaisesRegex(ValueError, 'duplicate|unique selected'):
                        self.assemble_oci(transform=lambda p: p.update({field: True}))
                finally: self.root = old

    def test_oci_config_platform_diffid_compression_and_blob_bounds_fail_closed(self):
        cases = [('config platform', lambda p: p['config'].update({'architecture': 'arm64'}), 'platform'),
                 ('wrong diff', lambda p: p['config']['rootfs'].update({'diff_ids':['sha256:'+'f'*64]}), 'diff ID'),
                 ('zstd', lambda p: p.update({'layerType':'application/vnd.oci.image.layer.v1.tar+zstd'}), 'descriptor'),
                 ('size', lambda p: p.update({'badLayerSize':True}), 'size/blob'),
                 ('missing blob', lambda p: p.update({'badLayerDigest':True}), 'size/blob')]
        for label, transform, error in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temp:
                old = self.root; self.root = Path(temp).resolve()
                try:
                    with self.assertRaisesRegex(ValueError, error): self.assemble_oci(transform=transform)
                finally: self.root = old

    def test_oci_digest_mutations_are_rejected_at_each_selected_graph_level(self):
        for field in ('root', 'manifest', 'config', 'layer'):
            def mutate(blobs, root, manifest, config, layer):
                selected = {'root':root,'manifest':manifest,'config':config,'layer':layer}[field]
                name = 'blobs/sha256/' + selected['digest'][7:]
                raw = bytearray(blobs[name]); raw[len(raw)//2] ^= 1; blobs[name] = bytes(raw)
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temp:
                old = self.root; self.root = Path(temp).resolve()
                try:
                    with self.assertRaises((ValueError, json.JSONDecodeError)):
                        self.assemble_oci(tar_transform=mutate)
                finally: self.root = old

    def test_oci_saved_manifest_cannot_reorder_or_rebind_graph(self):
        def change(blobs, root, manifest, config, layer):
            row = json.loads(blobs['manifest.json']); row[0]['Layers'] = ['blobs/sha256/' + config['digest'][7:]]
            blobs['manifest.json'] = image.build.serialized(row)
        with self.assertRaisesRegex(ValueError, 'layer order'): self.assemble_oci(tar_transform=change)

    def test_oci_expansion_bound_is_enforced_before_large_write(self):
        with patch.object(image, 'MAX_MEMBER', 4096):
            with self.assertRaisesRegex(ValueError, 'expanded layer byte bound'): self.assemble_oci()

    def test_oci_unknown_root_and_external_descriptor_fields_are_rejected(self):
        def change(blobs, root, manifest, config, layer):
            value = json.loads(blobs['index.json']); value['manifests'][0]['urls'] = ['https://untrusted.invalid/blob']
            blobs['index.json'] = image.build.serialized(value)
        with self.assertRaisesRegex(ValueError, 'descriptor'): self.assemble_oci(tar_transform=change)

    def test_loaded_result_uses_manifest_reference_and_rejects_config_id(self):
        self.base_raw, self.base_id = self.oci_base_archive()
        self.args.base_image = self.base_id
        self.images[self.base_id] = {'Id':self.base_id, 'Os':'linux', 'Architecture':'amd64'}
        result = self.run_fake()
        images = result['images']
        self.assertNotIn(images['configId'], self.images)
        self.assertIn(images['loadedImageReference'], self.images)
        generated = [c for c in self.commands if c[5:7] == ['container','create'] and '/usr/local/bin/framenode' in c]
        self.assertEqual(len(generated), 2)
        for command in generated: self.assertIn(images['ociManifestDigest'], command)

    def test_loaded_result_inspection_does_not_accept_different_digest(self):
        def changed(stage, args):
            if stage == 'image-load':
                for key, value in self.images.items():
                    if key not in (self.base_id, self.builder_id): value['Id'] = 'sha256:' + 'f' * 64
        self.mutate = changed
        with self.assertRaisesRegex(ValueError, 'identity/platform'): self.run_fake()


if __name__ == '__main__':
    unittest.main()
