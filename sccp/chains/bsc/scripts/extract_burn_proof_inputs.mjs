#!/usr/bin/env node

import { readFileSync } from "node:fs";
import {
  BURN_EVENT_IFACE,
  BURN_EVENT_SIGNATURE,
  BURN_EVENT_TOPIC0,
  computeBurnMessageId,
  decodeBurnPayloadV1,
  ensureBscBurnPayload,
  normalizeAddress,
  normalizeHex,
  parseArgs,
  parseIndexValue,
  parseMaybeIndexValue,
  usageAndExit,
} from "./sccp_bsc_proof_lib.mjs";

function selectBurnLog(receipt, router, requestedLogIndex) {
  if (!receipt || !Array.isArray(receipt.logs)) {
    throw new Error("receipt JSON must contain a logs array");
  }

  const matches = [];
  for (let i = 0; i < receipt.logs.length; i += 1) {
    const log = receipt.logs[i];
    if (!log || !Array.isArray(log.topics) || log.topics.length === 0) {
      continue;
    }
    if (normalizeHex(log.topics[0], "log topic0") !== BURN_EVENT_TOPIC0) {
      continue;
    }

    const logAddress = normalizeAddress(log.address, "log address");
    if (router !== null && logAddress !== router) {
      continue;
    }

    const logIndex = parseMaybeIndexValue(log.logIndex ?? log.index, BigInt(i), "log index");
    if (requestedLogIndex !== null && logIndex !== requestedLogIndex) {
      continue;
    }

    matches.push({ log, logIndex });
  }

  if (matches.length === 0) {
    throw new Error("no matching SccpBurned log found in receipt");
  }
  if (matches.length > 1) {
    throw new Error("multiple matching SccpBurned logs found; pass --log-index or --router");
  }
  return matches[0];
}

function buildOutput(receipt, selected) {
  const parsed = BURN_EVENT_IFACE.parseLog(selected.log);
  if (!parsed) {
    throw new Error("failed to decode SccpBurned log");
  }

  const messageId = normalizeHex(parsed.args.messageId, "messageId");
  const soraAssetId = normalizeHex(parsed.args.soraAssetId, "soraAssetId");
  const sender = normalizeAddress(parsed.args.sender, "sender");
  const amount = parsed.args.amount.toString();
  const destDomain = Number(parsed.args.destDomain);
  const recipient = normalizeHex(parsed.args.recipient, "recipient");
  const nonce = parsed.args.nonce.toString();
  const payloadHex = normalizeHex(parsed.args.payload, "payload");

  const decodedPayload = decodeBurnPayloadV1(payloadHex);
  ensureBscBurnPayload(decodedPayload);
  const recomputedMessageId = computeBurnMessageId(payloadHex);

  if (recomputedMessageId !== messageId) {
    throw new Error(`burn payload messageId mismatch: expected ${messageId}, recomputed ${recomputedMessageId}`);
  }
  if (decodedPayload.sora_asset_id !== soraAssetId) {
    throw new Error("event soraAssetId does not match encoded payload");
  }
  if (decodedPayload.amount !== amount) {
    throw new Error("event amount does not match encoded payload");
  }
  if (decodedPayload.dest_domain !== destDomain) {
    throw new Error("event destDomain does not match encoded payload");
  }
  if (decodedPayload.recipient !== recipient) {
    throw new Error("event recipient does not match encoded payload");
  }
  if (decodedPayload.nonce !== nonce) {
    throw new Error("event nonce does not match encoded payload");
  }

  const router = normalizeAddress(selected.log.address, "router address");
  const blockNumberValue = receipt.blockNumber ?? selected.log.blockNumber;
  if (blockNumberValue === undefined || blockNumberValue === null) {
    throw new Error("receipt blockNumber missing");
  }
  const blockNumber = parseIndexValue(blockNumberValue, "receipt blockNumber");
  const status = receipt.status === undefined || receipt.status === null
    ? undefined
    : parseMaybeIndexValue(receipt.status, 0n, "receipt status").toString();

  return {
    schema: "sccp-bsc-burn-proof-inputs/v1",
    event_name: "SccpBurned",
    event_signature: BURN_EVENT_SIGNATURE,
    event_topic0: BURN_EVENT_TOPIC0,
    router,
    transaction_hash: normalizeHex(
      receipt.transactionHash ?? selected.log.transactionHash,
      "transactionHash",
    ),
    block_hash: normalizeHex(receipt.blockHash ?? selected.log.blockHash, "blockHash"),
    block_number: blockNumber.toString(),
    log_index: selected.logIndex.toString(),
    receipt_status: status,
    message_id: messageId,
    payload_hex: payloadHex,
    indexed_event_fields: {
      message_id: messageId,
      sora_asset_id: soraAssetId,
      sender,
    },
    event_fields: {
      amount,
      dest_domain: destDomain,
      recipient,
      nonce,
    },
    decoded_payload: decodedPayload,
    proof_public_inputs: {
      router,
      event_topic0: BURN_EVENT_TOPIC0,
      message_id: messageId,
      payload_hex: payloadHex,
      source_domain: decodedPayload.source_domain,
      dest_domain: decodedPayload.dest_domain,
    },
  };
}

function main() {
  const args = parseArgs(process.argv, {
    "receipt-file": { required: true },
    router: {},
    "log-index": {},
  });

  const router = args.router ? normalizeAddress(args.router, "router") : null;
  const requestedLogIndex = args["log-index"] ? parseIndexValue(args["log-index"], "log-index") : null;
  const receipt = JSON.parse(readFileSync(args["receipt-file"], "utf8"));
  const selected = selectBurnLog(receipt, router, requestedLogIndex);
  const output = buildOutput(receipt, selected);
  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

try {
  main();
} catch (error) {
  usageAndExit(
    [
      "Usage:",
      "  node scripts/extract_burn_proof_inputs.mjs --receipt-file <path> [--router 0x<address>] [--log-index <u64>]",
      "",
      "Extracts the canonical BSC -> SORA burn-proof public inputs from a transaction receipt JSON.",
      "The receipt JSON must contain a logs array in ethers or JSON-RPC shape.",
      "",
      `${error instanceof Error ? error.message : String(error)}`,
    ],
    1,
  );
}
