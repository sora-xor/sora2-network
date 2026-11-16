use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "sora-nexus-hashi-local-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn reference(name: &str, raw: &[u8]) -> Reference {
    Reference {
        file: name.into(),
        bytes: raw.len(),
        sha256: digest(raw),
    }
}

#[test]
fn reads_only_exact_bounded_relative_artifacts() {
    let root = Directory::new();
    write_new(&root.0.join("public.scale"), &[1, 2, 3]).unwrap();
    let valid = reference("public.scale", &[1, 2, 3]);
    assert_eq!(referenced(&root.0, &valid, 3).unwrap(), [1, 2, 3]);
    assert!(referenced(&root.0, &valid, 2).is_err());
    for name in [
        "../public.scale",
        "/public.scale",
        "./public.scale",
        "public.scale/../public.scale",
        "public\\scale",
    ] {
        assert!(referenced(
            &root.0,
            &Reference {
                file: name.into(),
                ..valid.clone()
            },
            3
        )
        .is_err());
    }
    let changed = Reference {
        sha256: "00".repeat(32),
        ..valid
    };
    assert!(referenced(&root.0, &changed, 3).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_and_hardlink_input_aliases() {
    let root = Directory::new();
    write_new(&root.0.join("public.scale"), &[1]).unwrap();
    std::os::unix::fs::symlink("public.scale", root.0.join("alias.scale")).unwrap();
    assert!(referenced(&root.0, &reference("alias.scale", &[1]), 10).is_err());
    fs::hard_link(root.0.join("public.scale"), root.0.join("hard.scale")).unwrap();
    assert!(referenced(&root.0, &reference("hard.scale", &[1]), 10).is_err());
}

#[test]
fn output_is_exclusive_and_interrupted_attempts_are_not_retried() {
    let root = Directory::new();
    let requests = root.0.join("requests");
    let results = root.0.join("results");
    fs::create_dir(&requests).unwrap();
    fs::create_dir(&results).unwrap();
    let id = digest(b"public request");
    let request = requests.join(format!("{id}.json"));
    write_new(&request, b"public request").unwrap();
    assert!(write_new(&request, b"replacement").is_err());
    assert_eq!(next_request(&requests, &results).unwrap().unwrap().0, id);
    fs::create_dir(results.join(&id)).unwrap(); // A crash after exclusive claim.
    assert!(next_request(&requests, &results).unwrap().is_none());
    assert_eq!(fs::read(request).unwrap(), b"public request");
}

#[test]
fn malformed_or_unbounded_request_queues_fail_closed() {
    let root = Directory::new();
    let requests = root.0.join("requests");
    let results = root.0.join("results");
    fs::create_dir(&requests).unwrap();
    fs::create_dir(&results).unwrap();
    write_new(&requests.join("arbitrary.json"), b"{}").unwrap();
    assert!(next_request(&requests, &results).is_err());
    for invalid in ["", &"a".repeat(63), &"A".repeat(64), &"g".repeat(64)] {
        assert!(!valid_id(invalid));
    }
}

#[test]
fn startup_manifest_pin_and_later_artifact_drift_are_checked() {
    let root = Directory::new();
    let manifest = b"public manifest";
    let bootstrap = b"public code";
    let metadata = b"public metadata";
    for (name, raw) in [
        ("release.json", manifest.as_slice()),
        ("bootstrap.wasm", bootstrap.as_slice()),
        ("metadata.scale", metadata.as_slice()),
    ] {
        write_new(&root.0.join(name), raw).unwrap();
    }
    let row = |name: &str, raw: &[u8]| json!({"file":name,"bytes":raw.len(),"sha256":digest(raw)});
    json_new(&root.0.join("release-input.json"),&json!({"schema":"sora-nexus-hashi-local-release-v1","release":row("release.json",manifest),"bootstrap":row("bootstrap.wasm",bootstrap),"metadata":row("metadata.scale",metadata),"metadataRole":"bootstrapMetadata"})).unwrap();
    let pin = H256::from(sp_io::hashing::sha2_256(manifest));
    assert!(PublicRelease::load(&root.0, H256::zero()).is_err());
    let release = PublicRelease::load(&root.0, pin).unwrap();
    release.guard(&root.0).unwrap();
    fs::write(root.0.join("metadata.scale"), b"changed metadata").unwrap();
    assert!(release.guard(&root.0).is_err());
}
