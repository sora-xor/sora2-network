#!/usr/bin/env python3
"""Negative and positive raw request capture lineage checks."""
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("bridge_capture", HERE / "verify-bridge-capture.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.prefix = "0x" + "ab" * 44
        self.keys = [self.prefix + f"{index:064x}" for index in range(1, 4)]
        self.pages = [{"startKey": None, "rows": [[self.keys[0], "0x0101"], [self.keys[1], "0x0202"]]},
                      {"startKey": self.keys[1], "rows": [[self.keys[2], "0x0303"]]}]
        self.source = [{"key": self.keys[0], "hash": "0x" + self.keys[0][-64:], "rawValue": "0x0101"},
                       {"key": self.keys[2], "hash": "0x" + self.keys[2][-64:], "rawValue": "0x0303"}]
        self.capture = {"status": "passed", "requestScanComplete": True, "pageSize": 2,
                        "requestsCap": 10, "requestCount": 3, "requestCursor": self.keys[1]}

    def verify(self, capture_change=None):
        raw = b"".join((json.dumps(page) + "\n").encode() for page in self.pages)
        capture = {**self.capture, "requestPagesBytes": len(raw), "requestPagesSha256": hashlib.sha256(raw).hexdigest()}
        if capture_change:
            capture_change(capture)
        (self.root / "capture.json").write_text(json.dumps(capture))
        (self.root / "source.json").write_text(json.dumps(self.source))
        with gzip.open(self.root / "pages.gz", "wb") as stream:
            stream.write(raw)
        return MODULE.verify_capture(self.root / "capture.json", self.root / "source.json", self.root / "pages.gz")

    def test_complete_capture_matches_all_source_rows(self):
        result = self.verify()
        self.assertEqual(result["capturedRequests"], 3)
        self.assertEqual(result["extractedRowsMatched"], 2)
        self.assertEqual(result["pagesVerified"], 2)
        self.assertTrue(result["terminalPageVerified"])
        self.assertEqual(result["gzipSha256"], MODULE.digest_file(self.root / "pages.gz"))
        self.assertEqual(result["helperSha256"], MODULE.digest_file(HERE / "verify-bridge-capture.py"))

    def test_wrong_subset_value(self):
        self.source[0]["rawValue"] = "0xff"
        with self.assertRaisesRegex(ValueError, "raw value differs"):
            self.verify()

    def test_missing_subset_key(self):
        self.source[0].update(key=self.prefix + f"{4:064x}", hash="0x" + f"{4:064x}")
        with self.assertRaisesRegex(ValueError, "key missing"):
            self.verify()

    def test_duplicate_source_key(self):
        self.source.append(dict(self.source[0]))
        with self.assertRaisesRegex(ValueError, "Duplicate extracted"):
            self.verify()

    def test_duplicate_raw_key(self):
        self.pages[1]["rows"][0][0] = self.keys[1]
        with self.assertRaisesRegex(ValueError, "Duplicate or unordered"):
            self.verify()

    def test_cursor_mismatch(self):
        self.pages[1]["startKey"] = self.keys[0]
        with self.assertRaisesRegex(ValueError, "page cursor mismatch"):
            self.verify()

    def test_truncated_full_last_page(self):
        self.pages.pop()
        with self.assertRaisesRegex(ValueError, "no short terminal"):
            self.verify()

    def test_hash_mismatch(self):
        with self.assertRaisesRegex(ValueError, "capture hash differs"):
            self.verify(lambda capture: capture.update(requestPagesSha256="00" * 32))

    def test_byte_length_mismatch(self):
        with self.assertRaisesRegex(ValueError, "byte length differs"):
            self.verify(lambda capture: capture.update(requestPagesBytes=0))

    def test_count_mismatch(self):
        with self.assertRaisesRegex(ValueError, "request count differs"):
            self.verify(lambda capture: capture.update(requestCount=4))

    def test_final_cursor_mismatch(self):
        with self.assertRaisesRegex(ValueError, "final cursor differs"):
            self.verify(lambda capture: capture.update(requestCursor=self.keys[0]))

    def test_wrong_source_hash(self):
        self.source[0]["hash"] = "0x" + "00" * 32
        with self.assertRaisesRegex(ValueError, "key/hash mismatch"):
            self.verify()

    def test_raw_prefix_mismatch(self):
        self.pages[0]["rows"][0][0] = "0xcd" + self.keys[0][4:]
        with self.assertRaisesRegex(ValueError, "outside expected"):
            self.verify()

    def test_page_after_short_terminal(self):
        self.pages.append({"startKey": self.keys[2], "rows": []})
        with self.assertRaisesRegex(ValueError, "continues after terminal"):
            self.verify()

    def test_empty_terminal_page_after_full_page(self):
        self.pages = [self.pages[0], {"startKey": self.keys[1], "rows": []}]
        self.source = self.source[:1]
        self.capture["requestCount"] = 2
        self.assertEqual(self.verify()["capturedRequests"], 2)

    def test_exact_cap_without_terminal_is_not_complete(self):
        self.pages = self.pages[:1]
        self.source = self.source[:1]
        self.capture.update(requestCount=2, requestsCap=2)
        with self.assertRaisesRegex(ValueError, "no short terminal"):
            self.verify()

    def test_missing_completion_marker(self):
        with self.assertRaisesRegex(ValueError, "completed request scan"):
            self.verify(lambda capture: capture.update(requestScanComplete=False))


if __name__ == "__main__":
    unittest.main()
