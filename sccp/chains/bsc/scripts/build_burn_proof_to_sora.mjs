#!/usr/bin/env node

import { Buffer } from "node:buffer";
import {
  computeBurnMessageId,
  computeBurnsSlotBase,
  computeStorageTrieKey,
  decodeBurnPayloadV1,
  ensureBscBurnPayload,
  normalizeAddress,
  normalizeBlockSelector,
  normalizeHex,
  parseArgs,
  parseBlockHeaderSummary,
  parseFixedHex,
  parseRpcHexQuantityU256,
  rpcCall,
  scaleEncodeEvmBurnProof,
  usageAndExit,
} from "./sccp_bsc_proof_lib.mjs";

function parseProofNodes(nodes, label) {
  if (!Array.isArray(nodes)) {
    throw new Error(`${label} must be an array of hex strings`);
  }
  return nodes.map((node, idx) => parseFixedHex(node, Buffer.from(normalizeHex(node, `${label}[${idx}]`).slice(2), "hex").length, `${label}[${idx}]`));
}

function resolveMessageId(payloadHex, explicitMessageId) {
  if (payloadHex === null && explicitMessageId === null) {
    throw new Error("provide either --payload or --message-id");
  }

  if (payloadHex !== null) {
    const normalizedPayload = normalizeHex(payloadHex, "payload");
    const decodedPayload = decodeBurnPayloadV1(normalizedPayload);
    ensureBscBurnPayload(decodedPayload);
    const payloadMessageId = computeBurnMessageId(normalizedPayload);
    if (explicitMessageId !== null) {
      const normalizedMessageId = normalizeHex(explicitMessageId, "message-id");
      if (normalizedMessageId !== payloadMessageId) {
        throw new Error(
          `payload-derived messageId ${payloadMessageId} does not match explicit messageId ${normalizedMessageId}`,
        );
      }
    }
    return {
      message_id: payloadMessageId,
      payload_hex: normalizedPayload,
      decoded_payload: decodedPayload,
    };
  }

  return {
    message_id: normalizeHex(explicitMessageId, "message-id"),
    payload_hex: null,
    decoded_payload: null,
  };
}

async function resolveBlock(rpcUrl, requestedBlock, requestedBlockHash) {
  if (requestedBlock !== null && requestedBlockHash !== null) {
    throw new Error("provide either --block or --block-hash, not both");
  }

  if (requestedBlockHash !== null) {
    const normalizedBlockHash = normalizeHex(requestedBlockHash, "block-hash");
    const block = await rpcCall(rpcUrl, "eth_getBlockByHash", [normalizedBlockHash, false]);
    const summary = parseBlockHeaderSummary(block);
    if (summary.hash !== normalizedBlockHash) {
      throw new Error(
        `eth_getBlockByHash returned hash ${summary.hash} that does not match requested block-hash ${normalizedBlockHash}`,
      );
    }
    return {
      requested_block: normalizedBlockHash,
      block_selector: `0x${summary.number.toString(16)}`,
      summary,
    };
  }

  const blockSelector = normalizeBlockSelector(requestedBlock);
  const block = await rpcCall(rpcUrl, "eth_getBlockByNumber", [blockSelector, false]);
  return {
    requested_block: blockSelector,
    block_selector: blockSelector,
    summary: parseBlockHeaderSummary(block),
  };
}

function selectStorageEntry(storageProofItems, burnsSlotBaseHex) {
  if (!Array.isArray(storageProofItems)) {
    throw new Error("eth_getProof response missing storageProof array");
  }
  const requestedKey = BigInt(burnsSlotBaseHex);
  const entry = storageProofItems.find((item) => {
    if (!item || typeof item !== "object" || typeof item.key !== "string") {
      return false;
    }
    try {
      return parseRpcHexQuantityU256(item.key, "eth_getProof.storageProof[].key") === requestedKey;
    } catch {
      return false;
    }
  });
  if (!entry) {
    throw new Error(`eth_getProof.storageProof did not include requested storage slot ${burnsSlotBaseHex}`);
  }
  return entry;
}

async function main() {
  const args = parseArgs(process.argv, {
    "rpc-url": { required: true },
    router: { required: true },
    payload: {},
    "message-id": {},
    block: {},
    "block-hash": {},
  });

  const router = normalizeAddress(args.router, "router");
  const messageInfo = resolveMessageId(args.payload, args["message-id"]);
  const messageIdBytes = parseFixedHex(messageInfo.message_id, 32, "message-id");
  const burnsSlotBaseBytes = computeBurnsSlotBase(messageIdBytes);
  const burnsSlotBaseHex = `0x${Buffer.from(burnsSlotBaseBytes).toString("hex")}`;
  const storageTrieKeyHex = `0x${Buffer.from(computeStorageTrieKey(burnsSlotBaseBytes)).toString("hex")}`;

  const resolvedBlock = await resolveBlock(args["rpc-url"], args.block, args["block-hash"]);
  const proofResponse = await rpcCall(args["rpc-url"], "eth_getProof", [router, [burnsSlotBaseHex], resolvedBlock.block_selector]);

  if (!proofResponse || typeof proofResponse !== "object") {
    throw new Error("eth_getProof returned non-object response");
  }
  if (normalizeAddress(proofResponse.address, "eth_getProof.address") !== router) {
    throw new Error(`eth_getProof.address ${proofResponse.address} does not match requested router ${router}`);
  }

  const accountProof = parseProofNodes(proofResponse.accountProof, "eth_getProof.accountProof");
  if (accountProof.length === 0) {
    throw new Error("eth_getProof.accountProof must contain at least one trie node");
  }

  const storageEntry = selectStorageEntry(proofResponse.storageProof, burnsSlotBaseHex);
  const storageValue = parseRpcHexQuantityU256(storageEntry.value, "eth_getProof.storageProof[0].value");
  if (storageValue === 0n) {
    throw new Error(`eth_getProof returned zero storage value for burns[messageId].sender at slot ${burnsSlotBaseHex}`);
  }
  const storageProof = parseProofNodes(storageEntry.proof, "eth_getProof.storageProof[0].proof");
  if (storageProof.length === 0) {
    throw new Error("eth_getProof.storageProof[0].proof must contain at least one trie node");
  }

  const proofBytes = scaleEncodeEvmBurnProof(resolvedBlock.summary.hashBytes, accountProof, storageProof);
  const totalNodeBytes = [...accountProof, ...storageProof].reduce((sum, node) => sum + node.length, 0);
  const proofHex = `0x${Buffer.from(proofBytes).toString("hex")}`;
  const suggestedCallName = messageInfo.decoded_payload
    ? (messageInfo.decoded_payload.dest_domain === 0 ? "mint_from_proof" : "attest_burn")
    : null;

  const output = {
    schema: "sccp-bsc-burn-proof/v1",
    router,
    message_id: messageInfo.message_id,
    payload_hex: messageInfo.payload_hex,
    decoded_payload: messageInfo.decoded_payload,
    burns_mapping_slot: 4,
    burns_slot_base: burnsSlotBaseHex,
    storage_trie_key: storageTrieKeyHex,
    requested_block: resolvedBlock.requested_block,
    block: {
      number: resolvedBlock.summary.number.toString(),
      hash: resolvedBlock.summary.hash,
      state_root: resolvedBlock.summary.state_root,
    },
    storage_value: `0x${storageValue.toString(16)}`,
    proof_scale_hex: proofHex,
    proof_scale_base64: Buffer.from(proofBytes).toString("base64"),
    proof_scale_bytes: proofBytes.length,
    proof_node_counts: {
      account: accountProof.length,
      storage: storageProof.length,
    },
    proof_node_bytes_total: totalNodeBytes,
    account_proof_rlp: proofResponse.accountProof.map((node, idx) => normalizeHex(node, `eth_getProof.accountProof[${idx}]`)),
    storage_proof_rlp: storageEntry.proof.map((node, idx) => normalizeHex(node, `eth_getProof.storageProof[0].proof[${idx}]`)),
    suggested_sora_call: suggestedCallName === null
      ? null
      : {
        call_name: suggestedCallName,
        args: {
          source_domain: 2,
          payload: messageInfo.payload_hex,
          proof: proofHex,
        },
      },
  };

  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

try {
  await main();
} catch (error) {
  usageAndExit(
    [
      "Usage:",
      "  node scripts/build_burn_proof_to_sora.mjs --rpc-url <url> --router 0x<address> (--payload 0x<payload> | --message-id 0x<messageId>) [--block <selector> | --block-hash 0x<blockHash>]",
      "",
      "Builds the canonical trustless BSC -> SORA EvmBurnProofV1 artifact from eth_getProof.",
      "",
      `${error instanceof Error ? error.message : String(error)}`,
    ],
    1,
  );
}
