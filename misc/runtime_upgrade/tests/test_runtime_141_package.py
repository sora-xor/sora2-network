"""Check that the council ZIP ships exactly the generation reviewed for spec 141."""

import hashlib
import json
from pathlib import Path
import unittest
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[3]
PACKAGE = ROOT / "runtime-upgrade-4.8.14"
ZIP = ROOT / "council-runtime-4.8.14.zip"
MAINNET_GENESIS = "0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5"
SEEDED_GUARDIANS = [
    "cnSECvLF9MwFojPXk94PVKS4N34WsgZDPNDYMjxcbJWWTPWkY",
    "cnU8UgdM4fs9iiCxkRDW8vp2bx4e68LbePkcuQPv1LERKH5oH",
]


def blake2(data):
    return "0x" + hashlib.blake2b(data, digest_size=32).hexdigest()


def read_hex(name):
    return bytes.fromhex((PACKAGE / name).read_text().strip().removeprefix("0x"))


class Runtime141PackageTests(unittest.TestCase):
    def test_zip_and_manifest_identify_one_complete_generation(self):
        lines = (PACKAGE / "SHA256SUMS").read_text().splitlines()
        digests = {}
        for line in lines:
            checksum, name = line.split("  ", 1)
            self.assertNotIn(name, digests)
            self.assertEqual(hashlib.sha256((PACKAGE / name).read_bytes()).hexdigest(), checksum, name)
            digests[name] = checksum

        digest = hashlib.sha256(ZIP.read_bytes()).hexdigest()
        self.assertEqual(ZIP.with_suffix(".zip.sha256").read_text().strip(), f"{digest}  {ZIP.name}")
        with ZipFile(ZIP) as archive:
            names = [f"{PACKAGE.name}/{name}" for name in [*digests, "SHA256SUMS"]]
            self.assertCountEqual(archive.namelist(), names)
            for name in names:
                self.assertEqual(archive.read(name), (ROOT / name).read_bytes(), name)

    def test_all_governance_calls_belong_to_the_rehearsed_candidate(self):
        report = json.loads((PACKAGE / "governance-calls.json").read_text())
        rehearsal = json.loads((PACKAGE / "live-fork-rehearsal.json").read_text())
        self.assertTrue(rehearsal["ok"])
        self.assertEqual(report["genesisHash"], MAINNET_GENESIS)
        self.assertEqual(rehearsal["genesisHash"], MAINNET_GENESIS)
        self.assertNotIn("intermediate", rehearsal)
        wasm = (PACKAGE / report["wasm"]["file"]).read_bytes()
        wasm_digest = hashlib.sha256(wasm).hexdigest()
        self.assertEqual(rehearsal["candidate"]["sha256"], wasm_digest)
        self.assertEqual(report["wasm"]["sha256"], wasm_digest)
        self.assertEqual(report["rehearsal"]["candidateSha256"], wasm_digest)
        self.assertEqual(report["rehearsal"]["base"], rehearsal["base"])

        calls = report["calls"]
        names = {call["file"] for call in calls.values()} | {"set-code-call.hex"}
        self.assertEqual({path.name for path in PACKAGE.glob("*.hex")}, names)
        council = report["council"]["threshold"]
        committee = report["technicalCommittee"]["threshold"]
        self.assertEqual(set(calls), {
            "preimage-note-call",
            "utility-guarded-external-majority-call",
            f"council-propose-guarded-external-majority-threshold-{council}-call",
            "democracy-fast-track-call",
            f"technical-committee-fast-track-threshold-{committee}-call",
        })
        for call in calls.values():
            data = read_hex(call["file"])
            self.assertEqual(len(data), call["bytes"])
            self.assertEqual(blake2(data), call["hash"])

        set_code = (PACKAGE / report["setCode"]["file"]).read_bytes()
        self.assertEqual(read_hex("set-code-call.hex"), set_code)
        self.assertEqual(blake2(set_code), report["setCode"]["proposalHash"])
        # Both calls wrap a SCALE Vec<u8>. These package-sized vectors use four-byte compact lengths.
        for call, payload in [(set_code, wasm), (read_hex(calls["preimage-note-call"]["file"]), set_code)]:
            self.assertEqual(call[2:6], ((len(payload) << 2) | 2).to_bytes(4, "little"))
            self.assertEqual(call[6:], payload)

    def test_direct_rehearsal_seeded_both_guardians_and_each_halted(self):
        direct = json.loads((PACKAGE / "live-fork-rehearsal.json").read_text())
        self.assertTrue(all(check["ok"] for check in direct["checks"]))
        transition = direct["guardianTransition"]
        self.assertEqual(transition["priorSpec"], 138)
        self.assertCountEqual(transition["guardians"], SEEDED_GUARDIANS)
        names = {check["name"] for check in direct["checks"]}
        self.assertIn("guardian halt advances the previous number", names)
        self.assertIn(f"seeded guardian {SEEDED_GUARDIANS[1]} halts after the cooldown", names)
        self.assertIn(f"seeded guardian {SEEDED_GUARDIANS[1]} paid no fee", names)

    def test_governance_path_was_rehearsed_on_the_packaged_wasm(self):
        report = json.loads((PACKAGE / "governance-path-rehearsal.json").read_text())
        direct = json.loads((PACKAGE / "live-fork-rehearsal.json").read_text())
        self.assertTrue(report["ok"])
        self.assertTrue(all(check["ok"] for check in report["checks"]))
        self.assertEqual(report["genesisHash"], MAINNET_GENESIS)
        self.assertEqual(report["base"], direct["base"])
        self.assertEqual(report["candidate"]["sha256"], direct["candidate"]["sha256"])
        self.assertEqual(report["candidate"]["specVersion"], 141)
        names = {check["name"] for check in report["checks"]}
        self.assertTrue({
            "the deciding council aye and the close in one batchAll execute the motion",
            "a non-member closes the decided committee motion and the referendum starts",
            "the referendum passes",
            "the scheduler dispatches the Root call with Ok",
            "Root's recovery latch is removed on runtime 141",
        } <= names)

    def test_superseded_140_upgrade_preserves_the_halt_for_the_packaged_wasm(self):
        diagnostic = json.loads((PACKAGE / "via-140-rehearsal.json").read_text())
        direct = json.loads((PACKAGE / "live-fork-rehearsal.json").read_text())
        self.assertTrue(diagnostic["ok"])
        self.assertEqual(diagnostic["genesisHash"], MAINNET_GENESIS)
        self.assertEqual(diagnostic["base"], direct["base"])
        self.assertEqual(diagnostic["candidate"]["sha256"], direct["candidate"]["sha256"])
        previous = ROOT / "runtime-upgrade-4.8.14-spec-140/framenode-runtime-4.8.14.compact.compressed.wasm"
        self.assertEqual(diagnostic["intermediate"]["codeSha256"], hashlib.sha256(previous.read_bytes()).hexdigest())
        transition = diagnostic["guardianTransition"]
        self.assertEqual(transition["priorSpec"], 140)
        self.assertIsNotNone(transition["before"]["lastHalt"])
        self.assertEqual(transition["after"], transition["before"])


if __name__ == "__main__":
    unittest.main()
