#!/usr/bin/env python3
"""Offline SORA Nexus HASHI template encoder; never reads keys or contacts a service.

Native signatures remain witnesses. Rust admission performs cryptographic signature,
peer-membership, nonce, request and application checks. Header-chain validation here
checks ancestry/lineage, not GRANDPA finality; the supplied finalized head must come
from independently approved finality evidence.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

CHECKPOINT = 28_191_817
CHECKPOINT_HASH = "0x7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0"
GENESIS_HASH = "0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5"
ATTACKER_SORA = "0x64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d"
ATTACKER_EVM = "0xaf6fd7a11087bfbaddc5c4e1ab14ab0c592af63f"
MAX_BYTES = 512 * 1024
DOMAIN = b"sora-nexus/hashi-incoming-operation/v1"
SOURCE_DOMAIN = b"sora-nexus/hashi-authenticated-source-operation/v1"
SOURCE_FORMAT = "sora-nexus-hashi-authenticated-source-template-v1"
MAX_SIGNED_TRANSACTION_BYTES = 128 * 1024
MAGIC = b"soranx-recov-v1\0"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def raw(value, length=None, nonzero=False):
    require(isinstance(value, str) and re.fullmatch(r"0x(?:[0-9a-f]{2})*", value), "Expected canonical lowercase hex")
    result = bytes.fromhex(value[2:])
    require(len(result) <= MAX_BYTES, "Hex input exceeds size bound")
    require(length is None or len(result) == length, "Wrong fixed-byte length")
    require(not nonzero or any(result), "Zero evidence/identity hash")
    return result


def integer(value, maximum=2**32 - 1):
    require(type(value) is int and 0 <= value <= maximum, "Invalid unsigned integer")
    return value


def u32(value):
    return integer(value).to_bytes(4, "little")


def compact(value):
    integer(value, 2**128 - 1)
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, "little")
    if value < 1 << 30:
        return ((value << 2) | 2).to_bytes(4, "little")
    size = (value.bit_length() + 7) // 8
    return bytes([((size - 4) << 2) | 3]) + value.to_bytes(size, "little")


def vec(value):
    return compact(len(value)) + value


class Reader:
    def __init__(self, data):
        self.data, self.offset = data, 0

    def take(self, length):
        require(0 <= length <= len(self.data) - self.offset, "Truncated SCALE")
        result = self.data[self.offset:self.offset + length]
        self.offset += length
        return result

    def compact(self):
        start = self.offset
        first = self.take(1)[0]
        mode = first & 3
        if mode == 0:
            result = first >> 2
        elif mode in (1, 2):
            result = int.from_bytes(bytes([first]) + self.take((1 << mode) - 1), "little") >> 2
        else:
            result = int.from_bytes(self.take((first >> 2) + 4), "little")
        require(self.data[start:self.offset] == compact(result), "Noncanonical SCALE compact")
        return result

    def vector(self, maximum=MAX_BYTES):
        length = self.compact()
        require(length <= maximum, "SCALE vector exceeds bound")
        return self.take(length)

    def end(self):
        require(self.offset == len(self.data), "Trailing SCALE bytes")


def decode_extra(encoded):
    """The pinned native tuple encodes exactly Era, Compact<u32>, Compact<u128>."""
    reader = Reader(encoded)
    first = reader.take(1)[0]
    require(first != 0, "Immortal approvals are forbidden")
    era = first + (reader.take(1)[0] << 8)
    period = 2 << (era & 15)
    quantize = max(period >> 12, 1)
    phase = (era >> 4) * quantize
    require(period >= 4 and phase < period, "Invalid native mortal era")
    nonce, tip = reader.compact(), reader.compact()
    integer(nonce)
    require(nonce < 2**32 - 1 and tip == 0, "Nonce exhausted or nonzero fee tip")
    reader.end()
    return period, phase, nonce


def network(value):
    if isinstance(value, dict) and set(value) == {"evmLegacy"}:
        return b"\x02" + u32(value["evmLegacy"])
    require(value == {"sub": "Mainnet"}, "Only HASHI source and SORA Nexus receiver registries are supported")
    return b"\x01\x00"


def registry_bytes(registries, source_network):
    require(isinstance(registries, list) and len(registries) == 2, "Exactly two reviewed registries are required")
    result, seen = compact(2), set()
    for registry in registries:
        require(set(registry) == {"network", "accounts", "noKnownAttackers", "attestation"}, "Unexpected registry fields")
        identifier = network(registry["network"])
        require(identifier not in seen, "Duplicate registry network")
        seen.add(identifier)
        source = registry["network"] == {"evmLegacy": source_network}
        receiver = registry["network"] == {"sub": "Mainnet"}
        require(source or receiver, "Registry belongs to another network")
        accounts = registry["accounts"]
        require(isinstance(accounts, list) and len(accounts) <= 128, "Registry account bound")
        require(type(registry["noKnownAttackers"]) is bool and registry["noKnownAttackers"] == (len(accounts) == 0), "Missing explicit absence attestation")
        encoded_accounts = []
        for account in accounts:
            require(isinstance(account, dict) and set(account) == ({"evm"} if source else {"sora"}), "Wrong network account encoding")
            encoded_accounts.append((b"\x00" + raw(account["evm"], 20)) if source else (b"\x01" + raw(account["sora"], 32)))
        require(len(set(encoded_accounts)) == len(encoded_accounts), "Duplicate attacker account")
        if receiver:
            require({"sora": ATTACKER_SORA} in accounts, "Missing published SORA Nexus attacker")
        if source and source_network == 0:
            require({"evm": ATTACKER_EVM} in accounts, "Missing published Ethereum attacker")
        result += identifier + compact(len(accounts)) + b"".join(encoded_accounts)
        result += bytes([int(registry["noKnownAttackers"])]) + raw(registry["attestation"], 32, True)
    return result


def template_parts(template):
    require(isinstance(template, dict), "Expected template object")
    source_proof = template.get("format") == SOURCE_FORMAT
    fields = {"format", "index", "network", "request", "approvals", "registries", "finalizedSourceEvidence"}
    if source_proof:
        fields |= {"rawTransaction", "expectedChainId"}
    require(set(template) == fields, "Unexpected template fields")
    require(source_proof or template["format"] == "sora-nexus-hashi-template-v1", "Unknown template format")
    index, source = u32(template["index"]), u32(template["network"])
    request = raw(template["request"])
    require(request and request[0] == 0, "Expected original native IncomingRequest::Transfer")
    approvals = template["approvals"]
    require(isinstance(approvals, list) and 1 <= len(approvals) <= 100, "Approval count bound")
    encoded, signers = [], set()
    for approval in approvals:
        require(set(approval) == {"signer", "call", "extra"}, "Signatures belong in the witness file, not the template")
        signer = raw(approval["signer"], 32, True)
        require(signer not in signers and approval["signer"] != ATTACKER_SORA, "Duplicate or attacker signer")
        signers.add(signer)
        call, extra = raw(approval["call"]), raw(approval["extra"])
        require(call, "Empty native call")
        decode_extra(extra)
        encoded.append(signer + vec(call) + vec(extra))
    registries = registry_bytes(template["registries"], template["network"])
    evidence = raw(template["finalizedSourceEvidence"], 32, True)
    source_fields = b""
    if source_proof:
        # Native IncomingRequest::Transfer starts with variant 0 and fixed H160.
        # This encoder does not authenticate the raw transaction or native call.
        require(len(request) >= 21 and request[1:21] == bytes(20), "Authenticated-source requests must preserve native from=zero")
        transaction = raw(template["rawTransaction"])
        require(0 < len(transaction) <= MAX_SIGNED_TRANSACTION_BYTES, "Signed transaction bound")
        chain = template["expectedChainId"]
        # Hex is exactly 32-byte BIG ENDIAN for review; SCALE U256 is little endian.
        domain = b"\x00" if chain is None else b"\x01" + raw(chain, 32)[::-1]
        source_fields = vec(transaction) + domain
    require(len(index + source + vec(request) + source_fields + b"".join(encoded) + registries + evidence) <= MAX_BYTES, "Template exceeds operation bound")
    return index, source, request, source_fields, encoded, registries, evidence


def commitment(template):
    index, source, request, source_fields, approvals, registries, evidence = template_parts(template)
    preimage = vec(SOURCE_DOMAIN if source_fields else DOMAIN) + index + source + vec(request) + source_fields + compact(len(approvals)) + b"".join(approvals) + registries + evidence
    return "0x" + hashlib.blake2b(preimage, digest_size=32).hexdigest()


def encode_operation(template, signatures):
    index, source, request, source_fields, approvals, registries, evidence = template_parts(template)
    require(isinstance(signatures, list) and len(signatures) == len(approvals), "One native signature witness per approval is required")
    witnesses = []
    for signature in signatures:
        value = raw(signature)
        require(value and ((value[0] in (0, 1) and len(value) == 65) or (value[0] == 2 and len(value) == 66)), "Invalid native MultiSignature encoding")
        witnesses.append(value)
    encoded = index + (b"\x03" if source_fields else b"\x02") + source + vec(request) + source_fields + compact(len(approvals))
    encoded += b"".join(template + witness for template, witness in zip(approvals, witnesses)) + registries + evidence
    require(len(encoded) <= MAX_BYTES, "Signed operation exceeds bound")
    return encoded


def native_signing_payload(call, extra, spec_version, transaction_version, genesis_hash, birth_hash):
    decode_extra(extra)
    value = call + extra + u32(spec_version) + u32(transaction_version) + raw(genesis_hash, 32, True) + raw(birth_hash, 32, True)
    return hashlib.blake2b(value, digest_size=32).digest() if len(value) > 256 else value


def decode_header(encoded, policy_hash):
    reader = Reader(encoded)
    parent = reader.take(32)
    number = reader.compact()
    integer(number)
    reader.take(64)  # state root, extrinsics root
    count = reader.compact()
    require(count <= 64, "Excessive header digest count")
    lineage = 0
    expected = MAGIC + raw(CHECKPOINT_HASH, 32) + raw(policy_hash, 32, True)
    for _ in range(count):
        kind = reader.take(1)[0]
        if kind == 0:
            item = reader.vector()
            if item.startswith(MAGIC):
                require(item == expected, "Conflicting recovery lineage")
                lineage += 1
        elif kind in (4, 5, 6):
            reader.take(4)
            reader.vector()
        else:
            require(kind == 8, "Unsupported native digest variant")
    reader.end()
    require(lineage == 1, "Missing or duplicate recovery lineage digest")
    return number, parent, hashlib.blake2b(encoded, digest_size=32).digest()


def signing_payloads(template, context):
    template_parts(template)
    require(set(context) == {"specVersion", "transactionVersion", "genesisHash", "executionCodeHash", "metadataHash", "executionBlock", "lineage"}, "Unexpected signing context")
    require(context["genesisHash"] == GENESIS_HASH, "Wrong SORA Nexus genesis")
    raw(context["executionCodeHash"], 32, True)
    raw(context["metadataHash"], 32, True)
    current = integer(context["executionBlock"])
    require(current > CHECKPOINT + 1, "A post-checkpoint birth header must already exist")
    lineage = context["lineage"]
    require(set(lineage) == {"policyHash", "headers", "expectedFinalizedHeadHash", "finalityEvidenceSha256"}, "Unexpected lineage evidence")
    raw(lineage["finalityEvidenceSha256"], 32, True)
    headers = lineage["headers"]
    require(isinstance(headers, list) and 1 <= len(headers) <= 100_000, "Supply bounded contiguous recovery headers")
    previous, hashes = raw(CHECKPOINT_HASH, 32), {}
    for offset, header in enumerate(headers):
        number, parent, value = decode_header(raw(header), lineage["policyHash"])
        require(number == CHECKPOINT + offset + 1 and parent == previous, "Broken checkpoint ancestry")
        previous, hashes[number] = value, value
    require(previous == raw(lineage["expectedFinalizedHeadHash"], 32, True), "Finalized head does not match supplied ancestry")
    require(max(hashes) < current, "Signing context includes unexecuted future headers")
    result = []
    for approval in template["approvals"]:
        extra = raw(approval["extra"])
        period, phase, nonce = decode_extra(extra)
        birth = (max(current, phase) - phase) // period * period + phase
        require(CHECKPOINT < birth < current and birth in hashes, "Missing post-fork mortal birth header")
        payload = native_signing_payload(raw(approval["call"]), extra, context["specVersion"], context["transactionVersion"], context["genesisHash"], "0x" + hashes[birth].hex())
        result.append({"signer": approval["signer"], "nonce": nonce, "birthBlock": birth, "birthHash": "0x" + hashes[birth].hex(), "payload": "0x" + payload.hex()})
    return {"templateCommitment": commitment(template), "payloads": result,
            "lineageVerification": "contiguous checkpoint ancestry and policy digests checked; GRANDPA finality requires the independently approved evidence",
            "executionCodeHash": context["executionCodeHash"], "metadataHash": context["metadataHash"]}


def read_json(path):
    data = Path(path).read_bytes()
    require(len(data) <= 64 * 1024 * 1024, "JSON input exceeds bound")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "Duplicate JSON key")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "signing-payloads", "assemble"))
    parser.add_argument("--template", required=True)
    parser.add_argument("--context")
    parser.add_argument("--signatures")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    template = read_json(args.template)
    result = {"format": "sora-nexus-hashi-prepared-v1", "templateCommitment": commitment(template), "template": template}
    if args.mode == "signing-payloads":
        require(args.context is not None and args.signatures is None, "Supply only the public signing context")
        result = signing_payloads(template, read_json(args.context))
    elif args.mode == "assemble":
        require(args.signatures is not None and args.context is None, "Supply only public signature witnesses")
        operation = encode_operation(template, read_json(args.signatures))
        result = {"format": "sora-nexus-hashi-witness-v1", "templateCommitment": commitment(template),
                  "operationScale": "0x" + operation.hex(), "operationSha256": hashlib.sha256(operation).hexdigest(),
                  "signatureVerification": "required by native runtime admission; assembly does not authenticate signatures"}
    else:
        require(args.context is None and args.signatures is None, "Preparation accepts no signature or signing context")
    with Path(args.output).open("x", encoding="utf8") as output:
        json.dump(result, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    main()
