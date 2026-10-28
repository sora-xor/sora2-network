"""Synthetic public inputs and mocked Docker/HTTP only; no real service/build."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import prepare_linux_builder as p


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.source = self.root / 'source'; self.source.mkdir()
        files = {
            'Cargo.toml': '[workspace]\nmembers = ["node"]\n',
            'Cargo.lock': 'version = 4\n[[package]]\nname="synthetic"\nversion="1.0.0"\nsource="registry+https://github.com/rust-lang/crates.io-index"\nchecksum="' + '1'*64 + '"\n',
            'node/Cargo.toml': '[package]\nname="framenode"\nversion="4.8.13"\n',
            '.cargo/config.toml': '[profile.release]\nlto="thin"\n',
            'rust-toolchain.toml': '[toolchain]\nchannel="nightly-2026-10-23"\ncomponents=["rust-src"]\ntargets=["wasm32-unknown-unknown","wasm32v1-none"]\n',
        }
        for name, text in files.items():
            path = self.source / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text(text)
        self.names = sorted(files)
        self.inventory = self.root / 'inventory.json'
        self.inventory.write_text(json.dumps(self.source_pins(), sort_keys=True))
        self.ca = self.root / 'public-ca.pem'
        self.ca.write_text('# Synthetic public certificate bytes; never used for TLS.\n-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n')
        elf = bytearray(64); elf[:7] = b'\x7fELF\x02\x01\x01'; elf[16:18] = (3).to_bytes(2,'little'); elf[18:20] = (62).to_bytes(2,'little')
        self.rustup = bytes(elf)
        self.manifest = ('manifest-version = "2"\ndate = "2026-10-23"\n' + ''.join(
            '[pkg.'+name+'.target.x86_64-unknown-linux-gnu]\navailable=true\nurl="https://static.rust-lang.org/dist/2026-10-23/'+name+'.tar.gz"\n'
            for name in ('rustc','cargo','rust-std'))).encode()
        self.resolve_args = SimpleNamespace(source_root=self.source, source_inventory=self.inventory,
            source_inventory_sha256=p.digest(self.inventory)['sha256'], output=self.root/'inputs',
            platform='linux/amd64', debian_base='docker.io/library/debian@sha256:'+'d'*64,
            debian_snapshot='20261023T000000Z', rustup_version='1.28.2', ca_bundle=self.ca)
        self.calls = []; self.mutation = None; self.container_exit = 0; self.owner = None
        self.docker = self.root/'docker'; self.docker.write_bytes(b'fake docker, never executed')
        self.buildx = self.root/'buildx'; self.buildx.write_bytes(b'fake buildx, never executed')
        self.socket = self.root/'docker.sock'; self.socket.write_bytes(b'fake Unix socket identity in tests')
        self.base_id = 'sha256:'+'c'*64; self.runtime_id = 'sha256:'+'a'*64; self.builder_id = 'sha256:'+'b'*64

    def source_pins(self):
        return {name:p.digest(self.source/name) for name in self.names}

    def fake_fetch(self, url, _ca):
        data = self.manifest if 'channel-rust-nightly.toml' in url else self.rustup
        return hashlib.sha256(data).hexdigest().encode()+b'\n' if url.endswith('.sha256') else data

    def resolve(self):
        with patch.object(p.build,'source_names',return_value=self.names), patch.object(p.build,'source_inventory',side_effect=lambda _root:self.source_pins()), patch.object(p,'fetch',side_effect=self.fake_fetch):
            return p.resolve(self.resolve_args)

    def arguments(self):
        self.resolve()
        recipe=self.resolve_args.output/'recipe.json'
        self.args=SimpleNamespace(source_root=self.source,source_inventory=self.inventory,
            source_inventory_sha256=p.digest(self.inventory)['sha256'], inputs=recipe,
            inputs_sha256=p.digest(recipe)['sha256'], docker=self.docker,docker_sha256=p.digest(self.docker)['sha256'],
            buildx=self.buildx,buildx_sha256=p.digest(self.buildx)['sha256'],socket=self.socket,output=self.root/'prepared')
        return self.args

    def inspected(self, identity):
        return {'Id':identity,'Os':'linux','Architecture':'amd64','RepoDigests':[self.resolve_args.debian_base],
            'Config':{'Labels':{'org.sora-nexus.preparation.recipe':self.args.inputs_sha256}}}

    def fake_process(self, command, cwd, output, guard, env, timeout, maximum, retain):
        self.assertEqual(timeout,86400)
        self.assertEqual(command[1:5],['--config',str(self.args.output/'docker-config'),'--host','unix://'+str(self.socket)])
        self.assertNotIn('HTTP_PROXY',env);self.assertNotIn('DOCKER_CONTEXT',env)
        self.assertEqual(env['HOME'],str(self.args.output/'empty-home'))
        guard();output.mkdir(parents=True)
        args=command[5:];self.calls.append(args)
        if args[:2]==['image','inspect']:
            identity=self.base_id if args[2].startswith('docker.io/') else args[2]
            result=json.dumps([self.inspected(identity)]).encode()
        elif args[:2]==['buildx','build']:
            self.assertIn('--load',args);self.assertIn('--pull=false',args);self.assertIn('--network=default',args)
            stage=args[args.index('--target')+1]
            result=(self.builder_id if stage=='builder' else self.runtime_id).encode()+b'\n'
        elif args[0]=='create':
            self.assertEqual(args[args.index('--network')+1],'none')
            self.owner=args[args.index('--label')+1].split('=',1)[1]
            self.created_name=args[args.index('--name')+1]
            result=b'e'*64+b'\n'
        elif args[:2]==['start','--attach']:
            self.assertEqual(args[2], 'e'*64)
            result=json.dumps({'schema':'sora-nexus-linux-builder-offline-smoke-v1','toolchain':'nightly-2026-10-23',
                'rustTarget':'x86_64-unknown-linux-gnu','dependencyMetadataVerified':True,'sourceInventoryVerified':True,
                'sourceInventorySha256':self.args.source_inventory_sha256,'recipeSha256':self.args.inputs_sha256,
                'nativeBuildVerified':False,'productionReady':False,
                'checks':{name:{} for name in ('rustc','cargo','cc','cxx','cmake','protoc','openssl','clang','metadata')}}).encode()
        elif args[:2]==['container','inspect']:
            result=json.dumps([{'Id':'e'*64,'Name':'/'+self.created_name,'Image':self.builder_id,'Config':{'Labels':{'org.sora-nexus.preparation.owner':self.owner}},'State':{'ExitCode':self.container_exit}}]).encode()
        else:result=b'public synthetic Docker result\n'
        if self.mutation is not None:result=self.mutation(args,result)
        receipt={'pid':12345,'exitCode':0,'syntheticOnly':True}
        for name,data in [('stdout.log',result),('stderr.log',b''),('execution.json',json.dumps(receipt).encode())]:
            file=output/name;file.write_bytes(data);retain(file,p.digest(file))
        guard()
        return result,receipt

    def run_fake(self):
        with patch.object(p.build,'source_names',return_value=self.names), patch.object(p.build,'source_inventory',side_effect=lambda _root:self.source_pins()), patch.object(p.build,'helper_pins',return_value={}), patch.object(p.image,'socket_identity',return_value=(1,2,3)), patch.object(p.build,'process',side_effect=self.fake_process):
            return p.prepare(self.args)

    def test_complete_public_resolution_and_mock_preparation(self):
        self.arguments(); result=self.run_fake()
        self.assertEqual(result['images']['builder']['imageId'],self.builder_id)
        self.assertEqual(result['images']['runtime']['imageId'],self.runtime_id)
        self.assertFalse(result['productionReady']);self.assertFalse(result['nativeBuildVerified'])
        self.assertTrue(any(row[0]=='rm' for row in self.calls))
        self.assertFalse(any('login' in row or 'push' in row for row in self.calls))

    def test_public_download_rejects_unapproved_origin_without_network(self):
        for url in ('http://static.rust-lang.org/dist/a','https://example.invalid/rustup-init','https://static.rust-lang.org/../secret'):
            with self.assertRaises(ValueError):p.fetch(url,self.ca)
        with self.assertRaises(ValueError):p.NoRedirect().redirect_request(None,None,None,None,None,None)

    def test_official_checksum_mismatch_stops_resolution(self):
        def changed(url,ca):return b'0'*64+b'\n' if url.endswith('.sha256') else self.fake_fetch(url,ca)
        with patch.object(p.build,'source_names',return_value=self.names),patch.object(p.build,'source_inventory',side_effect=lambda _root:self.source_pins()),patch.object(p,'fetch',side_effect=changed):
            with self.assertRaisesRegex(ValueError,'checksum differs'):p.resolve(self.resolve_args)

    def test_unpinned_base_or_snapshot_rejected(self):
        for field,value in [('debian_base','debian:bookworm'),('debian_snapshot','latest')]:
            args=copy.copy(self.resolve_args);setattr(args,field,value)
            with patch.object(p.build,'source_names',return_value=self.names),patch.object(p.build,'source_inventory',side_effect=lambda _root:self.source_pins()),patch.object(p,'fetch',side_effect=AssertionError('must not download')):
                with self.assertRaises(ValueError):p.resolve(args)

    def test_unapproved_manifest_git_and_cargo_configuration_fail_closed(self):
        with patch.object(p.build,'source_names',return_value=self.names):
            (self.source/'node/Cargo.toml').write_text('[dependencies]\nbad={git="https://example.invalid/project"}\n')
            with self.assertRaisesRegex(ValueError,'manifest Git'):p.source_policy(self.source)
            (self.source/'node/Cargo.toml').write_text('[package]\nname="framenode"\nversion="4.8.13"\n')
            (self.source/'.cargo/config.toml').write_text('[http]\nproxy="https://example.invalid"\n')
            with self.assertRaisesRegex(ValueError,'credential configuration'):p.source_policy(self.source)

    def test_recipe_payload_or_origin_mutation_rejected_before_docker(self):
        self.arguments();value=json.loads(self.args.inputs.read_bytes())
        value['files']['rustup-init']['origin']='https://example.invalid/rustup-init'
        self.args.inputs.write_text(json.dumps(value));self.args.inputs_sha256=p.digest(self.args.inputs)['sha256']
        with self.assertRaisesRegex(ValueError,'payload origin'):self.run_fake()
        self.assertEqual(self.calls,[])

    def test_wrong_linux_architecture_rejected(self):
        self.arguments()
        previous=self.inspected
        self.inspected=lambda identity:{**previous(identity),'Architecture':'arm64'}
        with self.assertRaisesRegex(ValueError,'platform differs'):self.run_fake()
        self.assertFalse(any(row[:2]==['buildx','build'] for row in self.calls))

    def test_nonzero_container_exit_fails_despite_docker_success(self):
        self.arguments();self.container_exit=1
        with self.assertRaisesRegex(ValueError,'smoke container failed'):self.run_fake()
        self.assertTrue(any(row[0]=='rm' for row in self.calls))

    def test_smoke_container_id_or_name_rebinding_rejected(self):
        for field, replacement in [('Id', 'f'*64), ('Name', '/unowned-name')]:
            with self.subTest(field=field):
                case=PreparationTests(); case.setUp()
                try:
                    case.arguments()
                    def mutate(args, result):
                        if args[:2] == ['container', 'inspect']:
                            value=json.loads(result); value[0][field]=replacement
                            return json.dumps(value).encode()
                        return result
                    case.mutation=mutate
                    with case.assertRaisesRegex(ValueError, 'smoke container failed or changed'):
                        case.run_fake()
                    case.assertFalse(any(row[0] == 'rm' for row in case.calls))
                    case.assertTrue((case.args.output/'cleanup-failure.json').exists())
                finally:
                    case.doCleanups()

    def test_create_malformed_stdout_still_cleans_exact_owned_container(self):
        self.arguments();self.mutation=lambda args,result:b'wrong' if args[0]=='create' else result
        with self.assertRaisesRegex(ValueError,'container ID'):self.run_fake()
        self.assertTrue(any(row[0]=='rm' for row in self.calls))

    def test_source_drift_after_create_still_cleans_owned_container(self):
        self.arguments()
        def mutate(args,result):
            if args[0]=='start':(self.source/'Cargo.toml').write_text('changed')
            return result
        self.mutation=mutate
        with self.assertRaises(ValueError):self.run_fake()
        self.assertTrue(any(row[0]=='rm' for row in self.calls))

    def test_unknown_docker_credential_state_is_rejected(self):
        self.arguments()
        def mutate(args,result):
            if args[:2]==['buildx','version']:
                (self.args.output/'docker-config/credentials.json').write_text('{}')
            return result
        self.mutation=mutate
        with self.assertRaisesRegex(ValueError,'credential/context'):self.run_fake()

    def test_previous_receipt_mutation_rejects_completion(self):
        self.arguments()
        def mutate(args,result):
            if args[:2]==['image','inspect']:
                prior=self.args.output/'processes/001-docker-version/stdout.log'
                prior.write_text('changed')
            return result
        self.mutation=mutate
        with self.assertRaisesRegex(ValueError,'bound file changed'):self.run_fake()

    def test_validated_recipe_changed_before_copy_is_rejected(self):
        self.arguments()
        original = p.recipe_input
        def mutate(*args):
            result = original(*args)
            self.args.inputs.write_bytes(self.args.inputs.read_bytes() + b' ')
            return result
        with patch.object(p, 'recipe_input', side_effect=mutate):
            with self.assertRaisesRegex(ValueError, 'validated input changed before copy'):
                self.run_fake()
        self.assertEqual(self.calls, [])

    def test_recipe_uses_clean_cargo_home_and_pinned_official_base(self):
        self.arguments(); self.run_fake()
        for args in self.calls:
            if args[:2] == ['buildx', 'build']:
                self.assertIn('DEBIAN_BASE=' + self.resolve_args.debian_base, args)
                self.assertFalse(any('CARGO_CACHE_ID' in item for item in args))
        dockerfile = (p.RECIPES / 'Dockerfile').read_text()
        fetch = (p.RECIPES / 'fetch.sh').read_text()
        self.assertNotIn('--mount', dockerfile)
        self.assertNotIn('/warm-cargo', fetch)
        self.assertIn('CARGO_HOME=/opt/cargo', fetch)

    def test_scope_does_not_claim_native_build_or_readiness(self):
        self.arguments()
        def mutate(args,result):
            if args[0]=='start':
                value=json.loads(result);value['nativeBuildVerified']=True;return json.dumps(value).encode()
            return result
        self.mutation=mutate
        with self.assertRaisesRegex(ValueError,'smoke contract'):self.run_fake()


class ManifestNormalizationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location('preparation_install_rust_tests', p.RECIPES/'install_rust.py')
        cls.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.module)

    def values(self):
        public = {'manifest-version':'2', 'date':'2026-10-23', 'profiles':{'minimal':['rustc']},
            'pkg':{'rustc':{'version':'synthetic-version', 'git_commit_hash':'a'*40,
                'target':{'host':{'available':True,'url':'https://static.rust-lang.org/dist/rustc.tar.gz',
                    'hash':'1'*64,'xz_url':'https://static.rust-lang.org/dist/rustc.tar.xz','xz_hash':'2'*64,
                    'components':[{'pkg':'rust-std','target':'host'}],
                    'extensions':[{'pkg':'rust-src','target':'*'}]}, 'unavailable':{'available':False}}}}}
        installed = copy.deepcopy(public)
        del installed['pkg']['rustc']['git_commit_hash']
        host = installed['pkg']['rustc']['target']['host']
        host['components'][0]['is_extension'] = False
        host['extensions'][0]['is_extension'] = True
        installed['pkg']['rustc']['target']['unavailable'].update(components=[], extensions=[])
        return public, installed

    def test_known_rustup_normalization_and_exact_manifest(self):
        public, installed = self.values(); before = copy.deepcopy(public)
        self.module.verify_manifest(public, public)
        self.module.verify_manifest(public, installed)
        self.assertEqual(public, before)

    def test_normalization_keeps_archive_identity_targets_profiles_and_order(self):
        for field, changed in [('url','https://example.invalid/changed'), ('hash','3'*64),
                               ('xz_url','https://example.invalid/changed'), ('xz_hash','4'*64),
                               ('available',False), ('available',1)]:
            public, installed = self.values()
            installed['pkg']['rustc']['target']['host'][field] = changed
            with self.subTest(field=field, changed=changed), self.assertRaises(ValueError):
                self.module.verify_manifest(public, installed)
        for change in ['version','profile','date','target','component','extension','unknown']:
            public, installed = self.values()
            if change=='version': installed['pkg']['rustc']['version']='other'
            if change=='profile': installed['profiles']['minimal'].append('cargo')
            if change=='date': installed['date']='2026-10-24'
            if change=='target': installed['pkg']['rustc']['target']['other']={}
            if change=='component': installed['pkg']['rustc']['target']['host']['components'][0]['pkg']='cargo'
            if change=='extension': installed['pkg']['rustc']['target']['host']['extensions'][0]['is_extension']=False
            if change=='unknown': installed['unreviewed-field']='cannot silently ignore'
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.module.verify_manifest(public, installed)

    def test_retained_commit_or_explicit_extension_flags_cannot_change(self):
        public, installed = self.values()
        installed['pkg']['rustc']['git_commit_hash']='b'*40
        with self.assertRaises(ValueError): self.module.verify_manifest(public, installed)
        public, installed = self.values()
        public['pkg']['rustc']['target']['host']['components'][0]['is_extension']=True
        with self.assertRaises(ValueError): self.module.verify_manifest(public, installed)

    def test_missing_nonempty_components_and_unrecognized_dropped_fields_fail(self):
        public, installed = self.values()
        installed['pkg']['rustc']['target']['host']['components']=[]
        with self.assertRaises(ValueError): self.module.verify_manifest(public, installed)
        public, installed = self.values(); public['pkg']['rustc']['unreviewed']='must retain'
        with self.assertRaises(ValueError): self.module.verify_manifest(public, installed)


if __name__ == '__main__':unittest.main()
