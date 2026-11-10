import unittest

from export_hashi_scan_cursors import export


class CursorExportTests(unittest.TestCase):
    def rpc(self, *, pending=None, unstable=None):
        reads = 0

        def request(method, params):
            nonlocal reads
            if method == "chain_getBlockHash":
                return "0x" + ("01" if params else "02") * 32
            if method == "chain_getHeader":
                return {"number": "0x30"}
            self.assertEqual(params[0], "PERSISTENT")
            key = bytes.fromhex(params[1][2:]).decode()
            if key.endswith("peer-marker"):
                return "0x10"
            if key.endswith("pending-transactions"):
                return pending
            self.assertTrue(key.endswith("from-height-0"))
            reads += 1
            return "0x" + (5 if reads == 1 or not unstable else 7).to_bytes(8, "little").hex()
        return request

    def test_exports_first_unscanned_height_and_source_tip(self):
        result = export(self.rpc(), [0])
        self.assertEqual(result["sourceNumber"], 48)
        self.assertEqual(result["nextHeights"], {"0": 5})

    def test_refuses_unsettled_or_advancing_source(self):
        with self.assertRaisesRegex(ValueError, "outstanding"):
            export(self.rpc(pending="0x04"), [0])
        with self.assertRaisesRegex(ValueError, "changed"):
            export(self.rpc(unstable=True), [0])


if __name__ == "__main__":
    unittest.main()
