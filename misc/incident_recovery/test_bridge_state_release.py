"""Synthetic host schema negatives only, never native execution evidence."""
import copy
import unittest
import bridge_state_release as bridge
from test_maintenance_observer_release import fixture

def example_bridge(result, context):
    value = {'schema':bridge.SCHEMA,'policyCommitment':context['policyCommitment'],
        'finalStateRoot':result['blocks'][-1]['stateRoot'],'finalBlockHash':result['blocks'][-1]['hash'],
        'pendingIncomingAudits':0,'observedBlocks':len(result['blocks']),
        'authenticatedIncomingOperations':sum(row.get('action') == 6 for row in context['scenarios']['activation']['steps']),
        'hashiOperations':0,'deferredAcknowledgements':0}
    value.update({key:True for key in bridge.TRUE})
    value.update({key:False for key in bridge.FALSE})
    value.update({key:'aa'*32 for key in bridge.HASHES})
    return value

def data():
    result,context=fixture();context['policyCommitment']='ab'*32
    context['scenarios']['activation']['steps'][0]['action']=6
    result['postStateSemantics']['bridgeStateEquations']=example_bridge(result,context)
    return result,context

class BridgeStateTests(unittest.TestCase):
    def test_narrow_equations_never_promote_worker_or_counterpart_gate(self):
        result,context=data();value=bridge.validate(result,context,'activation')
        self.assertEqual(bridge.supporting_gate(value)['status'],'unverified')
        self.assertFalse(value['workerSigningSuppressionExecuted'])
        self.assertFalse(value['externalFinalityVerified'])
    def test_full_scope_schema_and_native_checks_are_required(self):
        result,context=data()
        for key in bridge.TRUE+bridge.FALSE:
            with self.subTest(key=key):
                bad=copy.deepcopy(result);v=bad['postStateSemantics']['bridgeStateEquations'];v[key]=not v[key]
                self.assertRaises(ValueError,bridge.validate,bad,context,'activation')
        for key in ('schema','pendingIncomingAudits','deferredAckRowsSha256'):
            bad=copy.deepcopy(result);del bad['postStateSemantics']['bridgeStateEquations'][key]
            self.assertRaises(ValueError,bridge.validate,bad,context,'activation')
    def test_native_counts_must_join_bound_work_and_exact_final_identity(self):
        result,context=data()
        for key,badvalue in [('authenticatedIncomingOperations',0),('hashiOperations',2),('observedBlocks',1),
            ('pendingIncomingAudits',1),('pendingIncomingAudits',False),('deferredAcknowledgements',True),
            ('finalStateRoot','0x'+'ee'*32),('policyCommitment','ee'*32),('genericNonceRowsSha256','x')]:
            with self.subTest(key=key):
                bad=copy.deepcopy(result);bad['postStateSemantics']['bridgeStateEquations'][key]=badvalue
                self.assertRaises(ValueError,bridge.validate,bad,context,'activation')
    def test_public_receipts_and_ack_without_incoming_fail(self):
        result,context=data()
        for key,value in [('publicTestCouncilFixture',True),('publicTestHashiFixture',True),('publicSignedTransactions',{})]:
            bad=copy.deepcopy(result);bad[key]=value
            self.assertRaises(ValueError,bridge.validate,bad,context,'activation')
        context['scenarios']['activation']['steps'][0]['action']=5
        value=example_bridge(result,context);value['deferredAcknowledgements']=1
        result['postStateSemantics']['bridgeStateEquations']=value
        self.assertRaises(ValueError,bridge.validate,result,context,'activation')

if __name__=='__main__':unittest.main()
