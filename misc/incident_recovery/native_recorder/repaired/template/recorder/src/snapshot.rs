//! Immutable base plus authenticated trie deltas for offline maintenance restarts.

use crate::{hex_bytes, local_path, replay, Result};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{storage::StateVersion, H256};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

type RawNode = (Vec<u8>, (Vec<u8>, i32));
type Snapshot = (Vec<RawNode>, H256);
type NodeIndex = HashMap<Vec<u8>, ([u8; 32], i32)>;
const MAX_SNAPSHOT: usize = 4 * 1024 * 1024 * 1024;
const STREAM_BUFFER: usize = 64 * 1024;

/// Preserve the byte-for-byte streaming digest when the digest crate does not
/// implement `io::Write` itself. SCALE's `Output` blanket implementation uses
/// this same sink without allocating the encoded inventory.
struct DigestWriter(Sha256);

impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Hash exactly the compressed bytes used by the decoder from a single open
/// file. A separate checksum pass followed by reopening does redundant I/O and
/// can authenticate a different inode if the path changes between the opens.
struct CheckedReader<R> {
    reader: R,
    digest: Sha256,
    bytes: u64,
}

impl<R: Read> Read for CheckedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(buffer)?;
        self.bytes = self.bytes.checked_add(count as u64).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "snapshot byte count overflow",
            )
        })?;
        if self.bytes > MAX_SNAPSHOT as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "oversized compressed snapshot object",
            ));
        }
        self.digest.update(&buffer[..count]);
        Ok(count)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    file: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    index: u32,
    parent: Option<Object>,
    base: Object,
    delta: Option<Object>,
    root: String,
    raw_snapshot_sha256: String,
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
struct Delta {
    parent_root: H256,
    root: H256,
    removed: Vec<Vec<u8>>,
    updated: Vec<RawNode>,
}

fn digest_encoded(value: &impl Encode) -> String {
    // An actual 1.64-million-node checkpoint benchmark found an extra buffering
    // layer slower after sha2 was optimized. Hash SCALE directly; the complete
    // inventory remains covered without a second serialized allocation.
    let mut digest = DigestWriter(Sha256::new());
    value.encode_to(&mut digest);
    hex::encode(digest.0.finalize())
}

fn file_digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut digest = DigestWriter(Sha256::new());
    std::io::copy(&mut file, &mut digest).map_err(|error| error.to_string())?;
    Ok(hex::encode(digest.0.finalize()))
}

fn write_object(directory: &Path, value: &impl Encode, compression: Compression) -> Result<Object> {
    let temporary = directory.join("snapshot.next.scale.gz");
    let file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let mut encoder = BufWriter::with_capacity(
        STREAM_BUFFER,
        GzEncoder::new(BufWriter::with_capacity(STREAM_BUFFER, file), compression),
    );
    value.encode_to(&mut encoder);
    encoder
        .into_inner()
        .map_err(|error| error.to_string())?
        .finish()
        .map_err(|error| error.to_string())?
        .into_inner()
        .map_err(|error| error.to_string())?
        .sync_all()
        .map_err(|error| error.to_string())?;
    let sha256 = file_digest(&temporary)?;
    let file = format!("snapshot-objects/{sha256}.scale.gz");
    let target = directory.join(&file);
    if target.exists() {
        if file_digest(&target)? != sha256 {
            return Err("snapshot object collision".into());
        }
        fs::remove_file(temporary).map_err(|error| error.to_string())?;
    } else {
        fs::rename(temporary, target).map_err(|error| error.to_string())?;
    }
    Ok(Object { file, sha256 })
}

fn read_object<T: DecodeWithMemTracking>(directory: &Path, object: &Object) -> Result<T> {
    let path = local_path(directory, &object.file)?;
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_SNAPSHOT as u64 {
        return Err("snapshot object size mismatch".into());
    }
    read_checked_object(file, &object.sha256)
}

fn read_checked_object<T: DecodeWithMemTracking>(
    reader: impl Read,
    expected_sha256: &str,
) -> Result<T> {
    let checked = CheckedReader {
        reader,
        digest: Sha256::new(),
        bytes: 0,
    };
    let mut decoder = GzDecoder::new(BufReader::with_capacity(STREAM_BUFFER, checked));
    let mut bytes = Vec::new();
    decoder
        .by_ref()
        .take(MAX_SNAPSHOT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_SNAPSHOT {
        return Err("oversized decoded snapshot object".into());
    }
    // Reaching EOF checks the gzip trailer/CRC. Reject another gzip member or
    // trailing garbage rather than accepting only the first decoded member.
    let mut compressed = decoder.into_inner();
    if compressed
        .read(&mut [0u8; 1])
        .map_err(|error| error.to_string())?
        != 0
    {
        return Err("trailing compressed snapshot bytes".into());
    }
    if hex::encode(compressed.into_inner().digest.finalize()) != expected_sha256 {
        return Err("snapshot object checksum mismatch".into());
    }
    let mut input = bytes.as_slice();
    let value =
        T::decode_with_mem_limit(&mut input, MAX_SNAPSHOT).map_err(|error| error.to_string())?;
    if !input.is_empty() {
        return Err("trailing snapshot object bytes".into());
    }
    Ok(value)
}

fn read_manifest(directory: &Path, object: &Object) -> Result<Manifest> {
    let bytes = replay::artifact(
        directory,
        &replay::Artifact {
            file: object.file.clone(),
            sha256: object.sha256.clone(),
        },
        8192,
    )?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if manifest.schema != "sora-nexus-maintenance-snapshot-v1" || manifest.index > 100_000 {
        return Err("unknown snapshot manifest schema/index".into());
    }
    Ok(manifest)
}

fn pinned_snapshot_path(directory: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("noncanonical pinned snapshot path".into());
    }
    let mut path = directory.to_path_buf();
    for part in relative.components() {
        path.push(part.as_os_str());
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("symlink in pinned snapshot closure".into());
        }
    }
    if !fs::metadata(&path).map_err(|error| error.to_string())?.is_file() {
        return Err("pinned snapshot reference is not a file".into());
    }
    Ok(path)
}

fn pinned_snapshot_digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("noncanonical pinned snapshot digest".into());
    }
    Ok(())
}

/// Reopen a completed journal without creating or changing any journal files.
/// The caller supplies the independently authenticated terminal manifest hash
/// and state root. Validate canonical, symlink-free references over the whole
/// manifest closure, then reuse the original object, trie and inventory checks.
pub(crate) fn restore_pinned(
    directory: &Path,
    latest_file: &str,
    latest_sha256: &str,
    expected_root: H256,
) -> Result<sp_io::TestExternalities> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("pinned snapshot directory is not a real directory".into());
    }
    let directory = directory.canonicalize().map_err(|error| error.to_string())?;
    let latest = Object { file: latest_file.into(), sha256: latest_sha256.into() };
    let mut reference = latest.clone();
    let mut expected_index = None;
    let mut immutable_base = None;
    loop {
        pinned_snapshot_digest(&reference.sha256)?;
        pinned_snapshot_path(&directory, &reference.file)?;
        let manifest = read_manifest(&directory, &reference)?;
        if reference.file != format!("snapshots/{:06}.json", manifest.index)
            || expected_index.is_some_and(|index| index != manifest.index)
        {
            return Err("noncanonical pinned snapshot manifest lineage".into());
        }
        if expected_index.is_none()
            && hex_bytes(&manifest.root)?.as_slice() != expected_root.as_bytes()
        {
            return Err("pinned terminal snapshot root mismatch".into());
        }
        if immutable_base.as_ref().is_some_and(|base| base != &manifest.base) {
            return Err("snapshot changed immutable base".into());
        }
        immutable_base = Some(manifest.base.clone());
        for object in [&manifest.base].into_iter().chain(manifest.delta.as_ref()) {
            pinned_snapshot_digest(&object.sha256)?;
            if object.file != format!("snapshot-objects/{}.scale.gz", object.sha256) {
                return Err("noncanonical pinned snapshot object".into());
            }
            pinned_snapshot_path(&directory, &object.file)?;
        }
        if (manifest.index == 0) != manifest.parent.is_none()
            || (manifest.index == 0) != manifest.delta.is_none()
        {
            return Err("snapshot base/delta lineage mismatch".into());
        }
        match manifest.parent {
            Some(parent) => {
                let index = manifest.index.checked_sub(1).ok_or("snapshot parent underflow")?;
                if parent.file != format!("snapshots/{index:06}.json") {
                    return Err("noncanonical pinned snapshot parent".into());
                }
                expected_index = Some(index);
                reference = parent;
            }
            None => break,
        }
    }
    // restore() reads persisted state afresh and does not consult writer state.
    let journal = Journal {
        directory,
        index: HashMap::new(),
        latest: Some(latest),
        base: None,
        root: expected_root,
        next: 0,
    };
    let mut restored = journal.restore()?;
    if replay::root(&mut restored) != expected_root {
        return Err("pinned restored snapshot root mismatch".into());
    }
    Ok(restored)
}

/// An index of node identities is retained only for choosing the next delta.
/// Restore always reads the persisted base and manifest/delta chain afresh.
pub(crate) struct Journal {
    directory: PathBuf,
    index: NodeIndex,
    latest: Option<Object>,
    base: Option<Object>,
    root: H256,
    next: u32,
}

impl Journal {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        fs::create_dir(directory.join("snapshot-objects")).map_err(|error| error.to_string())?;
        fs::create_dir(directory.join("snapshots")).map_err(|error| error.to_string())?;
        Ok(Self {
            directory: directory.to_path_buf(),
            index: HashMap::new(),
            latest: None,
            base: None,
            root: H256::zero(),
            next: 0,
        })
    }

    pub(crate) fn persist(
        &mut self,
        mut ext: sp_io::TestExternalities,
    ) -> Result<(sp_io::TestExternalities, String)> {
        ext.commit_all()
            .map_err(|error| format!("open maintenance transaction: {error}"))?;
        let expected = replay::root(&mut ext);
        let mut snapshot = ext.into_raw_snapshot();
        snapshot.0.sort_by(|left, right| left.0.cmp(&right.0));
        if snapshot.1 != expected {
            return Err("snapshot backend root differs".into());
        }
        let complete_digest = digest_encoded(&snapshot);
        let mut next_index = HashMap::with_capacity(snapshot.0.len());
        let mut updated = Vec::new();
        for (key, (value, count)) in &snapshot.0 {
            if key.len() < 32 || *count <= 0 {
                return Err("invalid raw trie node".into());
            }
            let stamp = (<[u8; 32]>::from(Sha256::digest(value)), *count);
            if let Some(prior) = self.index.get(key) {
                if prior.0 != stamp.0 {
                    return Err("hash-addressed trie node changed contents".into());
                }
                if prior.1 != stamp.1 {
                    updated.push((key.clone(), (value.clone(), *count)));
                }
            } else if self.latest.is_some() {
                updated.push((key.clone(), (value.clone(), *count)));
            }
            if next_index.insert(key.clone(), stamp).is_some() {
                return Err("duplicate snapshot node".into());
            }
        }
        let mut removed: Vec<_> = self
            .index
            .keys()
            .filter(|key| !next_index.contains_key(*key))
            .cloned()
            .collect();
        removed.sort();
        let delta = if self.latest.is_some() {
            Some(write_object(
                &self.directory,
                &Delta {
                    parent_root: self.root,
                    root: expected,
                    removed,
                    updated,
                },
                Compression::fast(),
            )?)
        } else {
            // Every crash/restart reopens and authenticates this whole base.
            // Stored gzip blocks trade extra disk space for cheaper repeated
            // reads. Keep delta compression and every integrity check intact.
            self.base = Some(write_object(
                &self.directory,
                &snapshot,
                Compression::none(),
            )?);
            None
        };
        drop(snapshot);
        let manifest = Manifest {
            schema: "sora-nexus-maintenance-snapshot-v1".into(),
            index: self.next,
            parent: self.latest.clone(),
            base: self.base.clone().ok_or("missing snapshot base")?,
            delta,
            root: format!("{expected:#x}"),
            raw_snapshot_sha256: complete_digest,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let file = format!("snapshots/{:06}.json", self.next);
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(self.directory.join(&file))
            .map_err(|error| error.to_string())?;
        output
            .write_all(&bytes)
            .map_err(|error| error.to_string())?;
        output.sync_all().map_err(|error| error.to_string())?;
        self.latest = Some(Object {
            file,
            sha256: sha256.clone(),
        });
        self.index = next_index;
        self.root = expected;
        self.next = self.next.checked_add(1).ok_or("snapshot index overflow")?;
        let mut restored = self.restore()?;
        if replay::root(&mut restored) != expected {
            return Err("disk restart changed state root".into());
        }
        Ok((restored, sha256))
    }

    pub(crate) fn restore(&self) -> Result<sp_io::TestExternalities> {
        let mut reference = self.latest.clone().ok_or("no persisted snapshot")?;
        let mut chain = Vec::new();
        let mut expected_index = None;
        loop {
            let manifest = read_manifest(&self.directory, &reference)?;
            if expected_index.is_some_and(|index| index != manifest.index) {
                return Err("snapshot manifest chain index gap".into());
            }
            let parent = manifest.parent.clone();
            expected_index = manifest.index.checked_sub(1);
            if (manifest.index == 0) != parent.is_none()
                || (manifest.index == 0) != manifest.delta.is_none()
            {
                return Err("snapshot base/delta lineage mismatch".into());
            }
            chain.push(manifest);
            if let Some(parent) = parent {
                reference = parent;
            } else {
                break;
            }
        }
        chain.reverse();
        let base = &chain[0].base;
        let (raw, mut root): Snapshot = read_object(&self.directory, base)?;
        let raw_count = raw.len();
        let mut nodes: BTreeMap<_, _> = raw.into_iter().collect();
        if nodes.len() != raw_count {
            return Err("duplicate base snapshot nodes".into());
        }
        for manifest in &chain {
            if &manifest.base != base {
                return Err("snapshot changed immutable base".into());
            }
            if let Some(reference) = &manifest.delta {
                let delta: Delta = read_object(&self.directory, reference)?;
                if delta.parent_root != root {
                    return Err("snapshot delta parent root mismatch".into());
                }
                for key in delta.removed {
                    if nodes.remove(&key).is_none() {
                        return Err("snapshot removal missing from parent".into());
                    }
                }
                for (key, value) in delta.updated {
                    if key.len() < 32
                        || value.1 <= 0
                        || nodes.get(&key).is_some_and(|old| old.0 != value.0)
                    {
                        return Err("invalid snapshot trie node update".into());
                    }
                    nodes.insert(key, value);
                }
                root = delta.root;
            }
            if hex_bytes(&manifest.root)?.as_slice() != root.as_bytes() {
                return Err("snapshot manifest root mismatch".into());
            }
        }
        let snapshot: Snapshot = (nodes.into_iter().collect(), root);
        if digest_encoded(&snapshot)
            != chain
                .last()
                .ok_or("empty snapshot chain")?
                .raw_snapshot_sha256
        {
            return Err(
                "reconstructed snapshot differs from exact persisted trie inventory".into(),
            );
        }
        Ok(sp_io::TestExternalities::from_raw_snapshot(
            snapshot.0,
            snapshot.1,
            StateVersion::V0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinned_test_journal() -> (PathBuf, Object, H256) {
        // Concurrent tests can observe the same wall-clock tick.
        static NEXT_JOURNAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = NEXT_JOURNAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir()
            .join(format!("soranx-pinned-{}-{unique}-{sequence}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let mut journal = Journal::new(&directory).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b"remove", &[1]);
            sp_io::default_child_storage::set(b"child", b"key", &[2]);
        });
        let (mut ext, _) = journal.persist(ext).unwrap();
        ext.execute_with(|| {
            sp_io::storage::clear(b"remove");
            sp_io::storage::set(b"keep", &[3]);
            sp_io::default_child_storage::set(b"child", b"key", &[4]);
        });
        let (mut ext, _) = journal.persist(ext).unwrap();
        let root = replay::root(&mut ext);
        let latest = journal.latest.clone().unwrap();
        // No writer or resident externalities is returned to the reopening test.
        (directory, latest, root)
    }

    #[test]
    fn pinned_reopen_uses_complete_persisted_journal_and_rejects_wrong_pins() {
        let (directory, latest, root) = pinned_test_journal();
        let before = file_digest(&directory.join(&latest.file)).unwrap();
        let mut restored = restore_pinned(&directory, &latest.file, &latest.sha256, root).unwrap();
        assert_eq!(replay::root(&mut restored), root);
        restored.execute_with(|| {
            assert!(sp_io::storage::get(b"remove").is_none());
            assert_eq!(sp_io::storage::get(b"keep").unwrap().as_ref(), &[3]);
            assert_eq!(sp_io::default_child_storage::get(b"child", b"key"), Some(vec![4]));
        });
        assert_eq!(file_digest(&directory.join(&latest.file)).unwrap(), before);
        assert!(restore_pinned(&directory, &latest.file, &"00".repeat(32), root).is_err());
        assert!(restore_pinned(&directory, &latest.file, &latest.sha256, H256::repeat_byte(99))
            .err().unwrap().contains("terminal snapshot root mismatch"));
        for path in [
            format!("../{}", latest.file),
            directory.join(&latest.file).to_string_lossy().into_owned(),
            latest.file.replace("snapshots/", "snapshots//"),
        ] {
            assert!(restore_pinned(&directory, &path, &latest.sha256, root).is_err());
        }
        assert!(restore_pinned(&directory, &latest.file, &"AB".repeat(32), root).is_err());
        let manifest_path = directory.join(&latest.file);
        let original = fs::read(&manifest_path).unwrap();
        let mut manifest: Manifest = serde_json::from_slice(&original).unwrap();
        let parent = manifest.parent.as_mut().unwrap();
        fs::copy(directory.join(&parent.file), directory.join("snapshots/alias.json")).unwrap();
        parent.file = "snapshots/alias.json".into();
        let changed = serde_json::to_vec(&manifest).unwrap();
        fs::write(&manifest_path, &changed).unwrap();
        assert!(restore_pinned(&directory, &latest.file, &hex::encode(Sha256::digest(changed)), root)
            .err().unwrap().contains("noncanonical pinned snapshot parent"));
        fs::write(manifest_path, original).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pinned_reopen_rejects_internal_symlinks_in_every_snapshot_reference_role() {
        let (directory, latest, root) = pinned_test_journal();
        let manifest = read_manifest(&directory, &latest).unwrap();
        let paths = [
            latest.file.clone(),
            manifest.parent.unwrap().file,
            manifest.base.file,
            manifest.delta.unwrap().file,
        ];
        for relative in paths {
            let path = directory.join(&relative);
            let original = path.with_extension("original");
            fs::rename(&path, &original).unwrap();
            std::os::unix::fs::symlink(original.file_name().unwrap(), &path).unwrap();
            assert!(restore_pinned(&directory, &latest.file, &latest.sha256, root)
                .err().unwrap().contains("symlink"), "{relative}");
            fs::remove_file(&path).unwrap();
            fs::rename(original, path).unwrap();
        }
        let snapshots = directory.join("snapshots");
        let original = directory.join("original-snapshots");
        fs::rename(&snapshots, &original).unwrap();
        std::os::unix::fs::symlink("original-snapshots", &snapshots).unwrap();
        assert!(restore_pinned(&directory, &latest.file, &latest.sha256, root)
            .err().unwrap().contains("symlink"));
        fs::remove_file(&snapshots).unwrap();
        fs::rename(original, snapshots).unwrap();
        let alias = directory.with_extension("alias");
        std::os::unix::fs::symlink(&directory, &alias).unwrap();
        assert!(restore_pinned(&alias, &latest.file, &latest.sha256, root).is_err());
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    fn compressed(bytes: &[u8]) -> Vec<u8> {
        let mut writer = GzEncoder::new(Vec::new(), Compression::fast());
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap()
    }

    #[test]
    fn inventory_hash_is_exact_scale_across_common_buffer_boundaries() {
        let value: Vec<_> = [
            0,
            1,
            63,
            64,
            65,
            4097,
            STREAM_BUFFER - 1,
            STREAM_BUFFER,
            STREAM_BUFFER + 1,
        ]
        .into_iter()
        .map(|length| vec![42u8; length])
        .collect();
        assert_eq!(
            digest_encoded(&value),
            hex::encode(Sha256::digest(value.encode()))
        );
    }

    #[test]
    fn checked_decoder_hashes_the_same_bytes_with_short_underlying_reads() {
        struct ShortReader<'a> {
            bytes: &'a [u8],
            consumed: &'a mut usize,
        }
        impl Read for ShortReader<'_> {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                let length = output.len().min(13).min(self.bytes.len());
                output[..length].copy_from_slice(&self.bytes[..length]);
                self.bytes = &self.bytes[length..];
                *self.consumed += length;
                Ok(length)
            }
        }
        let value: Vec<u8> = (0u32..100_000)
            .map(|i| i.wrapping_mul(123_457) as u8)
            .collect();
        let object = compressed(&value.encode());
        let mut consumed = 0;
        let actual: Vec<u8> = read_checked_object(
            ShortReader {
                bytes: &object,
                consumed: &mut consumed,
            },
            &hex::encode(Sha256::digest(&object)),
        )
        .unwrap();
        assert_eq!(actual, value);
        assert_eq!(consumed, object.len());
        assert!(read_checked_object::<Vec<u8>>(object.as_slice(), &"00".repeat(32)).is_err());
    }

    #[test]
    fn valid_object_hash_does_not_waive_gzip_or_scale_completeness() {
        let value = vec![1u8, 2, 3].encode();
        let good = compressed(&value);
        let mut trailing = good.clone();
        trailing.push(0);
        let mut concatenated = good.clone();
        concatenated.extend(&good);
        let mut encoded_trailing = value;
        encoded_trailing.push(0);
        let malformed = [
            good[..good.len() - 1].to_vec(),
            trailing,
            concatenated,
            compressed(&encoded_trailing),
        ];
        for bytes in malformed {
            assert!(read_checked_object::<Vec<u8>>(
                bytes.as_slice(),
                &hex::encode(Sha256::digest(&bytes))
            )
            .is_err());
        }
    }

    /// Optional, read-only measurement on a real persisted base. Set the path to
    /// its content-addressed .scale.gz file; this never rewrites live evidence.
    #[test]
    #[ignore = "requires SORA_RECOVERY_SNAPSHOT_BENCH pointing at a real snapshot object"]
    fn benchmark_real_snapshot_streaming_and_inventory_hash() {
        let path =
            PathBuf::from(std::env::var_os("SORA_RECOVERY_SNAPSHOT_BENCH").expect("snapshot path"));
        let expected = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".scale.gz")
            .expect("content addressed snapshot filename");
        let baseline_start = std::time::Instant::now();
        assert_eq!(file_digest(&path).unwrap(), expected);
        let mut encoded = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(&path).unwrap())
            .take(MAX_SNAPSHOT as u64 + 1)
            .read_to_end(&mut encoded)
            .unwrap();
        assert!(encoded.len() <= MAX_SNAPSHOT);
        let mut input = encoded.as_slice();
        let baseline = Snapshot::decode_with_mem_limit(&mut input, MAX_SNAPSHOT).unwrap();
        assert!(input.is_empty());
        let baseline_read_ms = baseline_start.elapsed().as_millis();
        let unbuffered_start = std::time::Instant::now();
        let mut digest = DigestWriter(Sha256::new());
        baseline.encode_to(&mut digest);
        let expected_inventory = hex::encode(digest.0.finalize());
        let unbuffered_hash_ms = unbuffered_start.elapsed().as_millis();
        let count = baseline.0.len();
        let root = baseline.1;
        drop(baseline);
        drop(encoded);
        let streaming_start = std::time::Instant::now();
        let restored: Snapshot =
            read_checked_object(fs::File::open(&path).unwrap(), expected).unwrap();
        let streaming_read_ms = streaming_start.elapsed().as_millis();
        assert_eq!(restored.0.len(), count);
        assert_eq!(restored.1, root);
        let restored_hash_start = std::time::Instant::now();
        assert_eq!(digest_encoded(&restored), expected_inventory);
        let restored_hash_ms = restored_hash_start.elapsed().as_millis();
        eprintln!(
            "{}",
            serde_json::json!({
                "nodeCount": count, "compressedBytes": fs::metadata(&path).unwrap().len(),
                "baselineReadMs": baseline_read_ms, "streamingReadMs": streaming_read_ms,
                "baselineInventoryHashMs": unbuffered_hash_ms, "restoredInventoryHashMs": restored_hash_ms,
                "inventorySha256": expected_inventory,
                "sameCompleteInventory": true, "freshDiskReadsForBoth": true,
            })
        );
    }

    #[test]
    fn stored_and_legacy_compressed_bases_restore_the_same_complete_inventory() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("soranx-codec-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let mut journal = Journal::new(&directory).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b"preserved", &[42; 4096]);
            sp_io::default_child_storage::set(b"child", b"key", &[7; 4096]);
        });
        let (mut restored, _) = journal.persist(ext).unwrap();
        let expected_root = replay::root(&mut restored);
        let reference = journal.latest.clone().unwrap();
        let mut manifest = read_manifest(&directory, &reference).unwrap();
        let raw: Snapshot = read_object(&directory, &manifest.base).unwrap();
        let stored_object = manifest.base.clone();
        // Older verifier artifacts use compressed gzip bases. Their complete
        // trie inventory must remain readable without a migration or waiver.
        manifest.base = write_object(&directory, &raw, Compression::fast()).unwrap();
        assert_ne!(manifest.base.sha256, stored_object.sha256);
        let bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(directory.join(&reference.file), &bytes).unwrap();
        journal.latest.as_mut().unwrap().sha256 = hex::encode(Sha256::digest(bytes));
        let mut legacy = journal.restore().unwrap();
        assert_eq!(replay::root(&mut legacy), expected_root);
        let mut legacy_snapshot = legacy.into_raw_snapshot();
        legacy_snapshot
            .0
            .sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(legacy_snapshot, raw);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn root_and_valid_object_hash_do_not_waive_complete_raw_node_inventory() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("soranx-inventory-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let mut journal = Journal::new(&directory).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| sp_io::storage::set(b"preserved", &[42]));
        journal.persist(ext).unwrap();
        let reference = journal.latest.as_ref().unwrap();
        let mut manifest = read_manifest(&directory, reference).unwrap();
        let mut raw: Snapshot = read_object(&directory, &manifest.base).unwrap();
        // Raw trie refcounts are not committed by the trie root. A fully
        // checksummed object with the same root must still fail the persisted
        // complete-inventory commitment when its bookkeeping is changed.
        raw.0[0].1 .1 += 1;
        manifest.base = write_object(&directory, &raw, Compression::fast()).unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        fs::write(directory.join(&reference.file), &bytes).unwrap();
        journal.latest.as_mut().unwrap().sha256 = hex::encode(Sha256::digest(bytes));
        assert!(journal
            .restore()
            .err()
            .unwrap()
            .contains("exact persisted trie inventory"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn delta_restart_preserves_deletions_child_state_and_detects_disk_tampering() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("soranx-delta-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let mut journal = Journal::new(&directory).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b"remove", &[1]);
            sp_io::default_child_storage::set(b"child", b"key", &[2]);
        });
        let (mut ext, _) = journal.persist(ext).unwrap();
        ext.execute_with(|| {
            sp_io::storage::clear(b"remove");
            sp_io::storage::set(b"new", &[3]);
            sp_io::default_child_storage::set(b"child", b"key", &[4]);
        });
        let (mut ext, _) = journal.persist(ext).unwrap();
        ext.execute_with(|| {
            assert!(sp_io::storage::get(b"remove").is_none());
            assert_eq!(sp_io::storage::get(b"new").unwrap().as_ref(), &[3]);
            assert_eq!(
                sp_io::default_child_storage::get(b"child", b"key"),
                Some(vec![4])
            );
        });
        let manifest = read_manifest(&directory, journal.latest.as_ref().unwrap()).unwrap();
        assert!(manifest.delta.is_some());
        let path = directory.join(manifest.delta.unwrap().file);
        let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(&[0]).unwrap();
        assert!(journal.restore().is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
