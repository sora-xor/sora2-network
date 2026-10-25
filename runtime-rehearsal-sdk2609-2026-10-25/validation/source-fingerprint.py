#!/usr/bin/env python3
"""Read-only source inventory for build/lint lineage; never updates the Git index."""
import argparse
import datetime
import fnmatch
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

SCHEMA = 1
# Entire roots are inventoried, including ignored source and binary fixtures. This
# is not a list of individual changed files or a list of allowed source suffixes.
SOURCE_ROOTS = ('.cargo', '.github', '.hooks', 'runtime', 'common', 'node', 'pallets',
                'vendor', 'utils', 'scripts', 'housekeeping', 'integration-tests',
                'bridge-scripts', 'bridge-docker', 'misc/runtime_upgrade',
                'misc/dependency_security', 'misc/farming_snapshot/convert',
                'misc/rewards_snapshot/get_lp_tokens')
ROOT_INPUTS = ('Cargo.toml', 'Cargo.lock', 'rust-toolchain', 'rust-toolchain.toml',
               '.gitignore', '.gitattributes', '.gitmodules', '.dockerignore',
               '.nvmrc', 'rustfmt.toml', '.rustfmt.toml', 'clippy.toml', '.clippy.toml',
               'deny.toml', 'package.json', 'package-lock.json', 'Makefile', 'Dockerfile',
               'Jenkinsfile', 'Jenkinsfile-env', 'pallet_list.txt')
CACHE_DIRS = {'.git', 'target', 'node_modules', '__pycache__', '.pytest_cache',
              '.mypy_cache', '.ruff_cache', '.tox', '.venv', 'venv', 'cache',
              'artifacts', '.workdir'}
DOC_SUFFIXES = {'.md', '.rst', '.adoc'}
PROVENANCE_NAMES = {'build-provenance.json', 'UPSTREAM.json', 'native-dependency-review.json',
                    'compatibility-evidence.json', 'SORA-PATCH', 'LICENSE', 'LICENSE.txt',
                    'COPYING', 'NOTICE'}
REPORT_PARENTS = {'misc/dependency_security', 'misc/runtime_upgrade'}
CODE_SUFFIXES = {'.py', '.sh', '.cjs', '.mjs', '.js', '.ts', '.rs'}
ROOT_SUFFIXES = CODE_SUFFIXES | DOC_SUFFIXES | {'.toml', '.lock', '.yaml', '.yml', '.txt'}
TOOLING_NAMES = {'package.json', 'package-lock.json', 'migration-proof-fixture.json',
                 'pre-batch-candidate-metadata.json', 'previous-candidate-metadata.json'}
# Exclude credentials by pathname before lstat/open/readlink, including ignored
# files. Do not inspect their contents to decide whether they are credentials.
SECRET_NAMES = {'credentials', 'credentials.json', 'credentials.toml', 'id_rsa',
                'id_ed25519', 'id_ecdsa', 'authorized_keys', '.netrc', '.npmrc'}
INCLUDE = re.compile(r'\binclude(?:_bytes|_str)?!\s*\(\s*(?:r(?P<hash>\#*)"(?P<raw>.*?)"(?P=hash)|"(?P<plain>[^"\n]+)")', re.S)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode()


def sha(value):
    return hashlib.sha256(value).hexdigest()


def run_git(repo, *args, allow_fail=False):
    env = {**os.environ, 'GIT_OPTIONAL_LOCKS': '0'}
    result = subprocess.run(['git', '-C', str(repo), *args], env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, check=False)
    if result.returncode and not allow_fail:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    return result.stdout if result.returncode == 0 else b''


def denied(path):
    for part in Path(path).parts:
        name = part.casefold()
        if 'sydneycreds' in name or name in SECRET_NAMES or name.endswith(('.pem', '.key', '.p12', '.pfx')):
            return 'credential/private-key pathname'
        if name == '.env' or (name.startswith('.env.') and not name.endswith(('.example', '.sample', '.template'))):
            return 'environment credential pathname'
    return None


def cache_path(path):
    for part in Path(path).parts:
        if part in CACHE_DIRS or part.startswith(('target-', '.venv', '.tmp-docker-')):
            return 'generated build/cache directory'
    if path.endswith(('.pyc', '.pyo', '.rs.bk')) or Path(path).name == '.DS_Store':
        return 'generated cache/backup file'
    return None


def git_inventory(repo):
    head = run_git(repo, 'rev-parse', 'HEAD', allow_fail=True).decode().strip() or None
    object_format = run_git(repo, 'rev-parse', '--show-object-format').decode().strip()
    tree = {}
    if head:
        for row in run_git(repo, 'ls-tree', '-rz', '--full-tree', head).split(b'\0'):
            if row:
                header, path = row.split(b'\t', 1)
                mode, kind, oid = header.decode().split()
                tree[os.fsdecode(path)] = {'mode': mode, 'oid': oid, 'kind': kind}
    index = {}
    for row in run_git(repo, 'ls-files', '--stage', '-z').split(b'\0'):
        if row:
            header, path = row.split(b'\t', 1)
            mode, oid, stage = header.decode().split()
            index.setdefault(os.fsdecode(path), []).append({'mode': mode, 'oid': oid, 'stage': int(stage)})
    index_path = Path(os.fsdecode(run_git(repo, 'rev-parse', '--git-path', 'index').strip()))
    if not index_path.is_absolute():
        index_path = repo / index_path
    return {'head': head, 'objectFormat': object_format, 'headFiles': tree, 'indexFiles': index,
            'indexSha256': sha(index_path.read_bytes()) if index_path.exists() else None}


def input_roots(repo):
    roots = set(SOURCE_ROOTS)
    # Automatically retain newly added workspace crates/path dependencies even if
    # their top-level directory is new. A path outside the checkout is unbound.
    import tomllib
    manifest = repo / 'Cargo.toml'
    if manifest.exists():
        data = tomllib.loads(manifest.read_text())
        for member in data.get('workspace', {}).get('members', []):
            for match in repo.glob(member):
                if match.is_dir(): roots.add(match.relative_to(repo).as_posix())
        def paths(value):
            if isinstance(value, dict):
                if isinstance(value.get('path'), str):
                    candidate = (repo / value['path']).resolve()
                    if not candidate.is_relative_to(repo):
                        raise RuntimeError(f'Cargo path dependency outside source inventory: {value["path"]}')
                    roots.add(candidate.relative_to(repo).as_posix())
                for item in value.values(): paths(item)
            elif isinstance(value, list):
                for item in value: paths(item)
        for key in ('dependencies', 'dev-dependencies', 'build-dependencies', 'patch', 'replace'):
            paths(data.get(key, {}))
        paths(data.get('workspace', {}).get('dependencies', {}))
    return sorted(roots)


def inventories(repo, evidence, git, extra_inputs):
    roots = input_roots(repo)
    names = set(ROOT_INPUTS)
    excluded = {}
    def include_path(relative):
        reason = denied(relative) or cache_path(relative)
        if reason:
            excluded[relative] = reason
            return False
        return True
    for path in repo.iterdir():
        if include_path(path.name) and (path.is_file() or path.is_symlink()) and path.suffix in ROOT_SUFFIXES:
            names.add(path.name)
    for root_name in roots:
        if not include_path(root_name): continue
        start = repo / root_name
        if start.is_symlink():
            names.add(root_name)
            continue
        if not start.exists(): continue
        for base, directories, files in os.walk(start, followlinks=False):
            for name in list(directories):
                full = Path(base) / name
                relative = full.relative_to(repo).as_posix()
                if not include_path(relative): directories.remove(name)
                elif full.is_symlink():
                    names.add(relative)
                    directories.remove(name)
            for name in files:
                relative = (Path(base) / name).relative_to(repo).as_posix()
                if include_path(relative): names.add(relative)
    def in_roots(name):
        root_input = '/' not in name and (name in ROOT_INPUTS or name in PROVENANCE_NAMES or Path(name).suffix in ROOT_SUFFIXES)
        return root_input or any(name == root or name.startswith(root + '/') for root in roots)
    # Union with HEAD and index retains staged and unstaged deletions/renames.
    for name in set(git['headFiles']) | set(git['indexFiles']):
        if in_roots(name) and include_path(name): names.add(name)
    for name in extra_inputs:
        full = (repo / name).resolve()
        if not full.is_relative_to(repo): raise RuntimeError('Explicit inputs must remain inside the repository')
        name = full.relative_to(repo).as_posix()
        if not include_path(name): raise RuntimeError(f'Explicit input is excluded: {name}')
        names.add(name)
    # Nonexistent optional root configuration is not a deletion unless it was
    # versioned. Missing explicit inputs always fail.
    names = {name for name in names if (repo / name).exists() or (repo / name).is_symlink()
             or name in git['headFiles'] or name in git['indexFiles']}
    for name in extra_inputs:
        if not (repo / name).exists(): raise RuntimeError(f'Missing explicit input: {name}')
    tooling = set()
    if evidence.exists():
        for base, directories, files in os.walk(evidence, followlinks=False):
            directories[:] = [name for name in directories if not denied(name) and not cache_path(name)
                              and not (Path(base) / name).is_symlink()]
            for name in files:
                full = Path(base) / name
                relative = full.relative_to(repo).as_posix()
                if include_path(relative) and (full.suffix in CODE_SUFFIXES or name in TOOLING_NAMES):
                    tooling.add(relative)
    # A rehearsal directory must never enter its own source aggregate.
    evidence_prefix = evidence.relative_to(repo).as_posix() + '/'
    names = {name for name in names if not name.startswith(evidence_prefix)}
    return sorted(names), sorted(tooling), roots, excluded


def read_record(repo, relative, object_format):
    path = repo / relative
    try: before = path.lstat()
    except FileNotFoundError: return {'path': relative, 'kind': 'deleted'}
    if stat.S_ISLNK(before.st_mode):
        data = os.fsencode(os.readlink(path))
        target = path.resolve()
        if not target.is_relative_to(repo):
            raise RuntimeError(f'External symlink cannot be bound without reading outside the source inventory: {relative}')
        if denied(target.relative_to(repo).as_posix()):
            raise RuntimeError(f'Symlink points to excluded credentials: {relative}')
        record = {'path': relative, 'kind': 'symlink', 'mode': '120000', 'bytes': len(data),
                  'sha256': sha(data), 'target': os.fsdecode(data),
                  'gitBlobOid': hashlib.new(object_format, b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()}
    elif stat.S_ISREG(before.st_mode):
        digest = hashlib.sha256()
        blob = hashlib.new(object_format, b'blob ' + str(before.st_size).encode() + b'\0')
        with path.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                digest.update(chunk); blob.update(chunk)
        record = {'path': relative, 'kind': 'file', 'mode': '100755' if before.st_mode & 0o111 else '100644',
                  'bytes': before.st_size, 'sha256': digest.hexdigest(), 'gitBlobOid': blob.hexdigest()}
    else:
        raise RuntimeError(f'Unsupported source input (directory, submodule, device or pipe): {relative}')
    after = path.lstat()
    if (before.st_ino, before.st_size, before.st_mtime_ns, before.st_mode) != (after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode):
        raise RuntimeError(f'Source changed while hashing: {relative}')
    record['_captureStamp'] = (after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode)
    return record


def collect(repo, evidence, extra_inputs):
    git = git_inventory(repo)
    names, tooling, roots, excluded = inventories(repo, evidence, git, extra_inputs)
    records = {name: read_record(repo, name, git['objectFormat']) for name in sorted(set(names + tooling))}
    # Documentation embedded by literal include!/include_str!/include_bytes! is
    # a compiler input, even when ordinary documentation is bound separately.
    embedded = set(extra_inputs)
    for name in names:
        if name.endswith('.rs') and records[name]['kind'] == 'file':
            for match in INCLUDE.finditer((repo / name).read_text(errors='replace')):
                value = match.group('raw') if match.group('raw') is not None else match.group('plain')
                target = (repo / name).parent / value
                target = target.resolve()
                if not target.is_relative_to(repo):
                    raise RuntimeError(f'Literal Rust include outside the repository: {name}: {value}')
                if denied(target.relative_to(repo).as_posix()):
                    raise RuntimeError(f'Literal Rust include points to excluded credentials: {name}: {value}')
                embedded.add(target.relative_to(repo).as_posix())
    for name in embedded:
        if name not in records:
            if cache_path(name): continue  # generated OUT_DIR inputs are covered by their generators
            if not (repo / name).exists(): continue  # cfg-disabled include paths may not exist
            records[name] = read_record(repo, name, git['objectFormat'])
        if name not in names:
            names.append(name)
    source, supporting = [], []
    for name in sorted(set(names)):
        path = Path(name)
        report = path.parent.as_posix() in REPORT_PARENTS and path.suffix == '.json' and any(word in path.name for word in ('audit', 'evidence', 'review'))
        category = supporting if name not in embedded and (path.suffix in DOC_SUFFIXES or path.name in PROVENANCE_NAMES or report) else source
        category.append(records[name])
    # All symlink targets must also be present, so a link cannot hide mutable
    # content under an ignored/generated directory.
    for record in source + supporting + [records[name] for name in tooling]:
        if record['kind'] == 'symlink':
            target = (repo / record['path']).resolve().relative_to(repo).as_posix()
            if target not in records:
                raise RuntimeError(f'Symlink target is outside the recorded file inventory: {record["path"]} -> {target}')
    final_git = git_inventory(repo)
    if (git['head'], git['indexSha256']) != (final_git['head'], final_git['indexSha256']):
        raise RuntimeError('Git HEAD/index changed during capture; retry without concurrent Git mutations')
    final_names, final_tooling, final_roots, _ = inventories(repo, evidence, git, extra_inputs)
    expected_names = set(final_names) | {name for name in embedded if name in records}
    if expected_names != set(names) or final_tooling != tooling or final_roots != roots:
        raise RuntimeError('Source inventory changed during capture')
    for name, record in records.items():
        path = repo / name
        if record['kind'] == 'deleted':
            if path.exists() or path.is_symlink():
                raise RuntimeError(f'Deleted source reappeared during capture: {name}')
        else:
            final = path.lstat()
            if record.pop('_captureStamp') != (final.st_ino, final.st_size, final.st_mtime_ns, final.st_mode):
                raise RuntimeError(f'Source changed during capture: {name}')
    policy = {'sourceRoots': roots, 'rootInputs': list(ROOT_INPUTS), 'explicitInputs': sorted(extra_inputs),
              'rootInventory': 'Named build configuration plus recognized root code/configuration/documentation from filesystem and HEAD/index; historical release archives excluded unless explicitly included or referenced by a literal Rust include',
              'rootSuffixes': sorted(ROOT_SUFFIXES),
              'cacheDirectories': sorted(CACHE_DIRS), 'credentialExclusions': sorted(SECRET_NAMES),
              'credentialRules': ['sydneycreds substring (case insensitive)', '*.pem/*.key/*.p12/*.pfx', '.env except example/sample/template'],
              'evidenceDirectory': evidence.relative_to(repo).as_posix(),
              'supportingOnly': sorted(DOC_SUFFIXES | PROVENANCE_NAMES),
              'supportingReports': {'parents': sorted(REPORT_PARENTS), 'suffix': '.json', 'nameContains': ['audit', 'evidence', 'review']},
              'inventory': 'physical files including ignored inputs, union Git HEAD/index paths; no extension filter within source roots'}
    overlay = []
    for record in source + supporting:
        baseline = git['headFiles'].get(record['path'])
        if record['kind'] == 'deleted': state = 'deleted'
        elif baseline is None: state = 'added'
        elif (baseline['oid'], baseline['mode']) != (record['gitBlobOid'], record['mode']): state = 'modified'
        else: continue
        overlay.append({'path': record['path'], 'state': state, 'head': baseline, 'current': record})
    groups = {}
    for label, rows in (('source', source), ('supporting', supporting), ('tooling', [records[name] for name in tooling])):
        rows.sort(key=lambda row: row['path'])
        groups[label] = {'files': rows, 'fileCount': len(rows), 'bytes': sum(row.get('bytes', 0) for row in rows),
                         'sha256': sha(canonical(rows))}
    return {'schemaVersion': SCHEMA, 'createdAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'recorderSha256': sha(Path(__file__).read_bytes()),
            'repositoryRoot': str(repo), 'policy': policy, 'policySha256': sha(canonical(policy)),
            'git': {'head': git['head'], 'objectFormat': git['objectFormat'], 'indexSha256': git['indexSha256'],
                    'indexEntriesSha256': sha(canonical(git['indexFiles'])), 'overlay': sorted(overlay, key=lambda row: row['path']),
                    'overlaySha256': sha(canonical(sorted(overlay, key=lambda row: row['path'])))},
            **groups, 'excludedPaths': [{'path': name, 'reason': reason} for name, reason in sorted(excluded.items())]}


def compare(expected, actual, source_only):
    changes = []
    for key, left, right in [('schemaVersion', expected['schemaVersion'], actual['schemaVersion']),
                             ('recorderSha256', expected['recorderSha256'], actual['recorderSha256']),
                             ('policySha256', expected['policySha256'], actual['policySha256']),
                             ('git.head', expected['git']['head'], actual['git']['head'])]:
        if left != right: changes.append({'field': key, 'expected': left, 'actual': right})
    for group in (['source'] if source_only else ['source', 'supporting', 'tooling']):
        old = {row['path']: row for row in expected[group]['files']}
        new = {row['path']: row for row in actual[group]['files']}
        for name in sorted(old.keys() | new.keys()):
            if old.get(name) != new.get(name):
                changes.append({'group': group, 'path': name, 'expected': old.get(name), 'actual': new.get(name)})
    return changes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--evidence-dir', type=Path, help='Repository-relative generated rehearsal directory')
    parser.add_argument('--include', action='append', default=[], help='Extra repository-relative build input outside automatically discovered roots')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', type=Path, help='Compare current inventory to a recorded JSON fingerprint')
    parser.add_argument('--source-only', action='store_true', help='Compare source/policy/HEAD only; report supporting/tooling hashes separately')
    parser.add_argument('--final', action='store_true', help='Label an explicitly frozen final-source capture; otherwise provisional')
    parser.add_argument('--label', default='provisional')
    args = parser.parse_args()
    repo = args.repo.resolve()
    expected = json.loads(args.check.read_text()) if args.check else None
    evidence_name = args.evidence_dir or (expected['policy']['evidenceDirectory'] if expected else 'runtime-rehearsal-sdk2609-2026-10-25')
    evidence = (repo / evidence_name).resolve()
    if not evidence.is_relative_to(repo): raise RuntimeError('Evidence directory must be inside the repository')
    if args.output and args.output.resolve().is_relative_to(repo) and not args.output.resolve().is_relative_to(evidence):
        raise RuntimeError('Output must be inside generated evidence or outside the repository')
    extras = args.include or (expected['policy']['explicitInputs'] if expected else [])
    current = collect(repo, evidence, extras)
    current['status'] = 'final' if args.final else 'provisional'
    current['label'] = args.label if args.label != 'provisional' or not args.final else 'final'
    result = {'status': current['status'], 'sourceSha256': current['source']['sha256'],
              'supportingSha256': current['supporting']['sha256'], 'toolingSha256': current['tooling']['sha256'],
              'sourceFiles': current['source']['fileCount'], 'sourceBytes': current['source']['bytes'],
              'gitHead': current['git']['head'], 'overlaySha256': current['git']['overlaySha256']}
    if expected:
        changes = compare(expected, current, args.source_only)
        result.update(status='passed' if not changes else 'changed', compared='source' if args.source_only else 'all', changes=changes)
        if args.output: args.output.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))
        return 1 if changes else 0
    output = args.output or evidence / 'validation' / ('source-fingerprint.final.json' if args.final else 'source-fingerprint.provisional.json')
    if output.resolve().is_relative_to(repo) and not output.resolve().is_relative_to(evidence):
        raise RuntimeError('Fingerprint output must be outside the source inventory (inside generated evidence or outside the repository)')
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(current, indent=2) + '\n')
    result['output'] = str(output)
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (AssertionError, OSError, RuntimeError, ValueError) as error:
        print(f'source-fingerprint: {error}', file=sys.stderr)
        sys.exit(2)
