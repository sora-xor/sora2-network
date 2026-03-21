#!/usr/bin/env node

import { Buffer } from "node:buffer";
import { resolve } from "node:path";
import {
  buildHeaderRlp,
  normalizeBlockSelector,
  parseArgs,
  parseBscEpochExtraData,
  parseBlockHeaderSummary,
  parseIndexValue,
  rpcCall,
  usageAndExit,
} from "./sccp_bsc_proof_lib.mjs";
import { loadEthers } from "./load_ethers.mjs";

const { keccak256 } = await loadEthers(resolve(import.meta.dirname, ".."));

async function main() {
  const args = parseArgs(process.argv, {
    "rpc-url": { required: true },
    "block-number": { required: true },
    "bsc-epoch-length": {},
  });

  const blockSelector = normalizeBlockSelector(args["block-number"]);
  if (["latest", "pending", "earliest", "safe", "finalized"].includes(blockSelector)) {
    throw new Error("--block-number must be a decimal or 0x-prefixed block number");
  }

  const [chainIdRaw, block] = await Promise.all([
    rpcCall(args["rpc-url"], "eth_chainId", []),
    rpcCall(args["rpc-url"], "eth_getBlockByNumber", [blockSelector, false]),
  ]);

  const blockSummary = parseBlockHeaderSummary(block);
  const headerRlp = buildHeaderRlp(block);
  const computedHash = keccak256(headerRlp).toLowerCase();
  if (computedHash !== blockSummary.hash) {
    throw new Error(`header RLP hash mismatch: computed=${computedHash} expected=${blockSummary.hash}`);
  }

  const epochData = parseBscEpochExtraData(
    block.extraData,
    blockSummary.number,
    args["bsc-epoch-length"] === null ? null : parseIndexValue(args["bsc-epoch-length"], "bsc-epoch-length"),
  );

  const output = {
    schema: "sccp-bsc-bsc-header-rlp/v1",
    chain_id: Number(parseIndexValue(chainIdRaw, "eth_chainId")),
    block_number: blockSummary.number.toString(),
    block_hash: blockSummary.hash,
    header_rlp_len: Buffer.from(headerRlp.slice(2), "hex").length,
    header_rlp_hex: headerRlp.toLowerCase(),
    header_rlp_base64: Buffer.from(headerRlp.slice(2), "hex").toString("base64"),
    bsc_epoch_validators: epochData.bsc_epoch_validators,
    bsc_epoch_turn_length: epochData.bsc_epoch_turn_length,
  };

  process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
}

try {
  await main();
} catch (error) {
  usageAndExit(
    [
      "Usage:",
      "  node scripts/build_bsc_header_rlp.mjs --rpc-url <url> --block-number <number> [--bsc-epoch-length <number>]",
      "",
      "Builds canonical BSC header RLP bytes for SORA init/submit_bsc_header flows.",
      "",
      `${error instanceof Error ? error.message : String(error)}`,
    ],
    1,
  );
}
