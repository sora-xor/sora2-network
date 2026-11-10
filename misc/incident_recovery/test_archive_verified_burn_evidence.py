"""Fresh derived graph transport tests; synthetic files are not proof evidence."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import archive_verified_burn_evidence as archive
import hashi_codec as scale


class VerifiedBurnArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        page = gzip.compress(json.dumps({"schema": "sora-nexus-verified-burn-page-v1",
                                         "records": [{"synthetic": True}]}).encode(), mtime=0)
        metadata = b"synthetic identical metadata from distinct executing code"
        (self.root / "page.json.gz").write_bytes(page)
        (self.root / "metadata.scale").write_bytes(metadata)
        reference = {"file": "metadata.scale", "sha256": hashlib.sha256(metadata).hexdigest()}
        self.manifest = {
            "schema": "sora-nexus-verified-burn-stream-v1", "first": scale.CHECKPOINT,
            "last": scale.CHECKPOINT, "checkpointHash": scale.CHECKPOINT_HASH,
            "pages": [{"file": "page.json.gz", "sha256": hashlib.sha256(page).hexdigest()}],
            "metadata": [{"codeHash": "0x" + f"{index:02x}" * 32, "specVersion": 134,
                          "metadata": copy.deepcopy(reference)} for index in (1, 2)],
        }

    def create_archive(self):
        manifest = self.root / archive.MANIFEST_NAME
        manifest.write_text(json.dumps(self.manifest))
        return archive.archive(manifest, self.root / "archive")

    def test_distinct_executing_code_can_share_exact_metadata_file(self):
        report = self.create_archive()
        self.assertEqual(report["files"], 3)
        self.assertFalse(report["productionReady"])
        destination = self.root / "restored"
        archive.restore(self.root / "archive" / archive.INDEX_NAME, destination)
        self.assertEqual((destination / "metadata.scale").read_bytes(), (self.root / "metadata.scale").read_bytes())
        self.assertEqual(json.loads((destination / archive.MANIFEST_NAME).read_bytes()), self.manifest)

    def test_shared_metadata_reference_cannot_disagree_on_content_hash(self):
        self.manifest["metadata"][1]["metadata"]["sha256"] = "f" * 64
        with self.assertRaisesRegex(ValueError, "Contradictory evidence reference"):
            self.create_archive()

    def test_executing_code_witnesses_remain_unique(self):
        self.manifest["metadata"][1]["codeHash"] = self.manifest["metadata"][0]["codeHash"]
        with self.assertRaisesRegex(ValueError, "Duplicate executing code witness"):
            self.create_archive()

    def test_pages_remain_unique(self):
        self.manifest["pages"].append(copy.deepcopy(self.manifest["pages"][0]))
        with self.assertRaisesRegex(ValueError, "Duplicate/malformed verified page"):
            self.create_archive()


if __name__ == "__main__":
    unittest.main()
