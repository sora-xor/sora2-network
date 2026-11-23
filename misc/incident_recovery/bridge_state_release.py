"""Validate same-invocation bridge equations after owned exact replay admission."""
import artifact_identity_release as identity

GATE = 'bridgeHaltNonceAndAcknowledgements'
SCHEMA = 'sora-nexus-real-bridge-state-equations-v1'
TRUE = ('exactInboundNonceSequenceVerified', 'outboundAndReportedNoncesUnchanged',
    'noNewOutboundCommitments', 'haltRetainedAtEveryObservedBlock', 'transientContextsAbsent',
    'opaqueAcceptedAuditFinalStateVerified', 'deferredAcknowledgementsExactlyMatchAuthenticatedMessages',
    'readOnlyStateRootMatches')
FALSE = ('fixtureOnly', 'outboundQueueDispositionVerified', 'workerSigningSuppressionExecuted',
    'counterpartReconciliationVerified', 'externalFinalityVerified',
    'bridgeHaltNonceAndAcknowledgementsVerified', 'productionReady')
COUNTS = ('observedBlocks', 'authenticatedIncomingOperations', 'hashiOperations', 'deferredAcknowledgements')
HASHES = ('substrateNonceRowsSha256', 'genericNonceRowsSha256', 'deferredAckRowsSha256')

def require(ok, message):
    identity.require(ok, 'bridge state equations: ' + message)

def validate(result, context, scenario):
    require(scenario == 'activation' and result.get('publicTestCouncilFixture') is False
        and result.get('publicTestHashiFixture') is False and result.get('publicSignedTransactions') is None,
        'real ordinary activation required')
    value = result['postStateSemantics'].get('bridgeStateEquations')
    require(isinstance(value, dict) and set(value) == {'schema', 'policyCommitment', 'finalStateRoot', 'finalBlockHash',
        'pendingIncomingAudits', *TRUE, *FALSE, *COUNTS, *HASHES} and value['schema'] == SCHEMA, 'fresh native schema differs')
    last = result['blocks'][-1]
    require(value['policyCommitment'] == context['policyCommitment'] and value['finalStateRoot'] == last['stateRoot']
        and value['finalBlockHash'] == last['hash'], 'native state/policy identity differs')
    for key in TRUE:
        require(value[key] is True, 'missing native equation: ' + key)
    for key in FALSE:
        require(value[key] is False, 'unsupported broad scope: ' + key)
    for key in COUNTS:
        require(type(value[key]) is int and 0 <= value[key] <= 100_000, 'bounded native count: ' + key)
    for key in HASHES:
        identity.digest_hex(value[key])
    require(type(value['pendingIncomingAudits']) is int and value['pendingIncomingAudits'] == 0, 'pending incoming audit')
    require(value['observedBlocks'] == len(result['blocks']), 'missing block observation')
    incoming = [row for row in context['scenarios'][scenario]['steps'] if row.get('action') == 6]
    # The native collector also requires a one-to-one committed incoming/audit set.
    # Context admission exposes action tags; no supplied result count is trusted.
    require(value['authenticatedIncomingOperations'] == len(incoming), 'incoming operation count differs from bound work')
    require(value['hashiOperations'] <= len(incoming), 'HASHI count exceeds authenticated operations')
    require(len(incoming) > 0 or value['deferredAcknowledgements'] == 0, 'ACK without authenticated incoming')
    return value

def supporting_gate(value):
    return {'status': 'unverified', 'freshSupportingEvidenceVerified': True,
        'coverageScope': 'sameProcessHaltNonceAndDeferredAckEquations', 'evidence': value,
        'reason': 'Exact real replay bridge state equations passed; outbound queue disposition, worker signing suppression and counterpart reconciliation remain incomplete',
        'remaining': ['Exercise outbound worker/signing suppression and cancelled-queue/status-report behavior against exact artifacts.',
            'Join complete counterpart nonces and all redeemable authorizations to external finalized reconciliation evidence.'],
        'productionReady': False}
