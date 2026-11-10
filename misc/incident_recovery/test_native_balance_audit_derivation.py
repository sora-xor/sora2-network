"""Transport guards only; synthetic bytes never establish runtime execution."""
import copy
import unittest
import production_build as build
import hashi_codec as scale


class NativeBalanceDerivationTests(unittest.TestCase):
    def setUp(self):
        self.baseline = b'snxrec01\x01\x02' + scale.u32(1) + b'original-witnesses'
        self.incoming = '0x' + (b'snxrec01\x01\x09' + scale.vec(b'audit-source')).hex()
        self.derived = '0x' + (b'snxrec01\x01\x0a' + scale.vec(self.baseline[10:] + b'\x04native-source')).hex()
        self.mixed = {'deriveNativeBalanceAudits': True,
            'auditEnvelopes': [self.incoming, '0x' + self.baseline.hex()]}
        self.report = {'nativeBalanceAuditsDerived': True,
            'baselineAuditEnvelopes': list(self.mixed['auditEnvelopes']),
            'auditEnvelopes': [self.incoming, self.derived], 'nativeBalanceAuditIndices': [1]}

    def validate(self, report=None, mixed=None):
        build.validate_native_balance_derivation(mixed or self.mixed, report or self.report)

    def test_exact_baseline_is_embedded_without_rewriting_any_witness(self):
        self.validate()
        report = copy.deepcopy(self.report)
        report['auditEnvelopes'] = list(self.mixed['auditEnvelopes'])
        report['nativeBalanceAuditIndices'] = []
        self.validate(report)

    def test_replacement_value_or_original_baseline_rebinding_is_rejected(self):
        for index in (10, len(self.baseline) - 1):
            report = copy.deepcopy(self.report)
            changed = bytearray(self.baseline[10:]); changed[index - 10] ^= 1
            report['auditEnvelopes'][1] = '0x' + (b'snxrec01\x01\x0a' + scale.vec(bytes(changed) + b'\x04native-source')).hex()
            with self.assertRaises(ValueError): self.validate(report)
        report = copy.deepcopy(self.report); report['baselineAuditEnvelopes'][1] = '0x00'
        with self.assertRaises(ValueError): self.validate(report)

    def test_derivation_flags_cannot_be_missing_or_integer(self):
        for flag in (None, False, 1, 'true'):
            report = copy.deepcopy(self.report); report['nativeBalanceAuditsDerived'] = flag
            with self.assertRaises(ValueError): self.validate(report)
            mixed = copy.deepcopy(self.mixed); mixed['deriveNativeBalanceAudits'] = flag
            with self.assertRaises(ValueError): self.validate(mixed=mixed)

    def test_index_inventory_is_exact_integer_unique_ordered_and_in_range(self):
        for indices in ([True], [-1], [2], [1, 1], [1, 0], None, ['1']):
            report = copy.deepcopy(self.report); report['nativeBalanceAuditIndices'] = indices
            with self.assertRaises(ValueError): self.validate(report)

    def test_unlisted_change_reordering_or_audit_count_change_is_rejected(self):
        for rows in ([self.derived, self.incoming], [self.incoming], [self.incoming, self.derived, self.incoming]):
            report = copy.deepcopy(self.report); report['auditEnvelopes'] = rows
            with self.assertRaises(ValueError): self.validate(report)
        report = copy.deepcopy(self.report); report['auditEnvelopes'][0] = '0x00'
        with self.assertRaises(ValueError): self.validate(report)

    def test_old_audit_other_action_or_missing_native_source_is_rejected(self):
        for raw in (self.baseline, b'snxrec01\x01\x09' + scale.vec(self.baseline[10:] + b'source'),
                    b'snxrec01\x01\x0a' + scale.vec(self.baseline[10:]),
                    bytes.fromhex(self.derived[2:]) + b'\x00'):
            report = copy.deepcopy(self.report); report['auditEnvelopes'][1] = '0x' + raw.hex()
            with self.assertRaises(ValueError): self.validate(report)


if __name__ == '__main__':
    unittest.main()
