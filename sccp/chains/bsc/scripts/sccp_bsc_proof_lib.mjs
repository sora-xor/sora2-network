import { Buffer } from "node:buffer";
import { resolve } from "node:path";
import { loadEthers } from "./load_ethers.mjs";

const {
  Interface,
  concat,
  encodeRlp,
  getAddress,
  getBytes,
  hexlify,
  keccak256,
  toUtf8Bytes,
} = await loadEthers(resolve(import.meta.dirname, ".."));

export const SCCP_DOMAIN_SORA = 0;
export const SCCP_DOMAIN_BSC = 2;
export const SCCP_EVM_BURNS_MAPPING_SLOT = 4n;
export const BURN_EVENT_SIGNATURE = "SccpBurned(bytes32,bytes32,address,uint128,uint32,bytes32,uint64,bytes)";
export const BURN_EVENT_ABI =
  "event SccpBurned(bytes32 indexed messageId, bytes32 indexed soraAssetId, address indexed sender, uint128 amount, uint32 destDomain, bytes32 recipient, uint64 nonce, bytes payload)";
export const BURN_EVENT_TOPIC0 = keccak256(toUtf8Bytes(BURN_EVENT_SIGNATURE));
export const BURN_PREFIX = toUtf8Bytes("sccp:burn:v1");
export const BURN_PAYLOAD_V1_LEN = 97;
export const BURN_EVENT_IFACE = new Interface([BURN_EVENT_ABI]);

export function parseArgs(argv, spec) {
  const out = {};
  for (const [key, rule] of Object.entries(spec)) {
    out[key] = rule.multiple ? [] : null;
  }

  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      throw new Error(`unknown argument '${arg}'`);
    }
    const key = arg.slice(2);
    const rule = spec[key];
    if (!rule) {
      throw new Error(`unknown argument '${arg}'`);
    }
    const value = argv[i + 1];
    if (value === undefined || value.startsWith("--")) {
      throw new Error(`missing value for --${key}`);
    }
    if (rule.multiple) {
      out[key].push(value);
    } else {
      out[key] = value;
    }
    i += 1;
  }

  for (const [key, rule] of Object.entries(spec)) {
    const value = out[key];
    if (!rule.required) {
      continue;
    }
    if ((rule.multiple && value.length === 0) || (!rule.multiple && (value === null || value === ""))) {
      throw new Error(`missing required --${key}`);
    }
  }

  return out;
}

export function usageAndExit(lines, code) {
  // eslint-disable-next-line no-console
  console.error(lines.join("\n"));
  process.exit(code);
}

export function normalizeHex(value, label) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be a string`);
  }
  return hexlify(getBytes(value)).toLowerCase();
}

export function normalizeAddress(value, label) {
  if (typeof value !== "string") {
    throw new Error(`${label} must be a string`);
  }
  return getAddress(value);
}

export function parseFixedHex(value, expectedLen, label) {
  const bytes = getBytes(value);
  if (bytes.length !== expectedLen) {
    throw new Error(`${label} must be ${expectedLen} bytes, got ${bytes.length}`);
  }
  return bytes;
}

export function parseIndexValue(value, label) {
  if (typeof value === "number") {
    if (!Number.isInteger(value) || value < 0) {
      throw new Error(`${label} must be a non-negative integer`);
    }
    return BigInt(value);
  }
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new Error(`${label} must be a non-negative integer`);
    }
    return value;
  }
  if (typeof value !== "string") {
    throw new Error(`${label} must be a string or integer`);
  }
  if (/^0x[0-9a-fA-F]+$/.test(value)) {
    return BigInt(value);
  }
  if (/^[0-9]+$/.test(value)) {
    return BigInt(value);
  }
  throw new Error(`${label} must be hex or decimal`);
}

export function parseMaybeIndexValue(value, fallback, label) {
  if (value === undefined || value === null) {
    return fallback;
  }
  return parseIndexValue(value, label);
}

export function readLE(bytes, offset, width) {
  let value = 0n;
  for (let i = 0; i < width; i += 1) {
    value |= BigInt(bytes[offset + i]) << BigInt(8 * i);
  }
  return value;
}

export function bytes32Slice(bytes, offset) {
  return hexlify(bytes.slice(offset, offset + 32)).toLowerCase();
}

export function decodeBurnPayloadV1(payloadHex) {
  const bytes = getBytes(payloadHex);
  if (bytes.length !== BURN_PAYLOAD_V1_LEN) {
    throw new Error(`burn payload must be ${BURN_PAYLOAD_V1_LEN} bytes, got ${bytes.length}`);
  }

  return {
    version: Number(bytes[0]),
    source_domain: Number(readLE(bytes, 1, 4)),
    dest_domain: Number(readLE(bytes, 5, 4)),
    nonce: readLE(bytes, 9, 8).toString(),
    sora_asset_id: bytes32Slice(bytes, 17),
    amount: readLE(bytes, 49, 16).toString(),
    recipient: bytes32Slice(bytes, 65),
  };
}

export function ensureBscBurnPayload(decodedPayload, label = "decoded burn payload") {
  if (decodedPayload.version !== 1) {
    throw new Error(`${label} version must be 1`);
  }
  if (decodedPayload.source_domain !== SCCP_DOMAIN_BSC) {
    throw new Error(`${label} source_domain must be ${SCCP_DOMAIN_BSC}`);
  }
}

export function computeBurnMessageId(payloadHex) {
  return keccak256(concat([BURN_PREFIX, getBytes(payloadHex)])).toLowerCase();
}

export function bigIntToHexQuantity(value) {
  if (value < 0n) {
    throw new Error("value must be non-negative");
  }
  return `0x${value.toString(16)}`;
}

export function normalizeBlockSelector(value) {
  if (value === null || value === undefined) {
    return "finalized";
  }
  if (["latest", "pending", "earliest", "safe", "finalized"].includes(value)) {
    return value;
  }
  if (/^0x[0-9a-fA-F]+$/.test(value)) {
    return bigIntToHexQuantity(BigInt(value));
  }
  if (/^[0-9]+$/.test(value)) {
    return bigIntToHexQuantity(BigInt(value));
  }
  throw new Error(
    `invalid EVM block selector '${value}'; expected tag, decimal block number, or 0x-prefixed block number`,
  );
}

export function quantityHexToBytes(value, label) {
  if (typeof value !== "string" || !/^0x[0-9a-fA-F]*$/.test(value)) {
    throw new Error(`${label} must be a 0x-prefixed hex quantity`);
  }
  const digits = value.slice(2);
  if (digits.length === 0) {
    throw new Error(`${label} must not be empty`);
  }
  const stripped = digits.replace(/^0+/, "");
  if (stripped.length === 0) {
    return "0x";
  }
  return `0x${stripped.length % 2 === 0 ? stripped : `0${stripped}`}`;
}

export function zeroPaddedU256(value) {
  if (value < 0n) {
    throw new Error("value must be non-negative");
  }
  return getBytes(`0x${value.toString(16).padStart(64, "0")}`);
}

export function computeBurnsSlotBase(messageIdBytes) {
  return getBytes(keccak256(concat([messageIdBytes, zeroPaddedU256(SCCP_EVM_BURNS_MAPPING_SLOT)])));
}

export function computeStorageTrieKey(burnsSlotBaseBytes) {
  return getBytes(keccak256(burnsSlotBaseBytes));
}

export function scaleCompactU32(value) {
  if (!Number.isInteger(value) || value < 0) {
    throw new Error("length must be a non-negative integer");
  }
  if (value < 1 << 6) {
    return Uint8Array.from([(value << 2) | 0]);
  }
  if (value < 1 << 14) {
    const encoded = (value << 2) | 1;
    return Uint8Array.from([encoded & 0xff, (encoded >> 8) & 0xff]);
  }
  if (value < 1 << 30) {
    const encoded = (value << 2) | 2;
    return Uint8Array.from([
      encoded & 0xff,
      (encoded >> 8) & 0xff,
      (encoded >> 16) & 0xff,
      (encoded >> 24) & 0xff,
    ]);
  }
  throw new Error("length too large for compact u32 encoding");
}

export function scaleEncodeBytes(bytes) {
  return getBytes(concat([scaleCompactU32(bytes.length), bytes]));
}

export function scaleEncodeByteVecs(nodes) {
  return getBytes(concat([scaleCompactU32(nodes.length), ...nodes.map((node) => scaleEncodeBytes(node))]));
}

export function scaleEncodeEvmBurnProof(anchorBlockHashBytes, accountProofNodes, storageProofNodes) {
  return getBytes(
    concat([
      anchorBlockHashBytes,
      scaleEncodeByteVecs(accountProofNodes),
      scaleEncodeByteVecs(storageProofNodes),
    ]),
  );
}

export async function rpcCall(rpcUrl, method, params) {
  const response = await fetch(rpcUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method,
      params,
    }),
  });

  if (!response.ok) {
    throw new Error(`${method} failed with HTTP ${response.status}`);
  }

  const payload = await response.json();
  if (payload.error) {
    const message = payload.error.message ?? JSON.stringify(payload.error);
    throw new Error(`${method} RPC error: ${message}`);
  }
  return payload.result;
}

export function parseRpcHexQuantityU256(value, label) {
  if (typeof value !== "string" || !/^0x[0-9a-fA-F]+$/.test(value)) {
    throw new Error(`${label} must be a 0x-prefixed hex quantity`);
  }
  return BigInt(value);
}

export function parseBlockHeaderSummary(block) {
  if (!block || typeof block !== "object") {
    throw new Error("EVM block RPC returned non-object response");
  }
  const hash = normalizeHex(block.hash, "block.hash");
  const stateRoot = normalizeHex(block.stateRoot, "block.stateRoot");
  const number = parseIndexValue(block.number, "block.number");
  return {
    hash,
    hashBytes: getBytes(hash),
    state_root: stateRoot,
    stateRootBytes: getBytes(stateRoot),
    number,
  };
}

export function buildHeaderRlp(block) {
  const items = [
    normalizeHex(block.parentHash, "block.parentHash"),
    normalizeHex(block.sha3Uncles, "block.sha3Uncles"),
    normalizeHex(block.miner, "block.miner"),
    normalizeHex(block.stateRoot, "block.stateRoot"),
    normalizeHex(block.transactionsRoot, "block.transactionsRoot"),
    normalizeHex(block.receiptsRoot, "block.receiptsRoot"),
    normalizeHex(block.logsBloom, "block.logsBloom"),
    quantityHexToBytes(block.difficulty, "block.difficulty"),
    quantityHexToBytes(block.number, "block.number"),
    quantityHexToBytes(block.gasLimit, "block.gasLimit"),
    quantityHexToBytes(block.gasUsed, "block.gasUsed"),
    quantityHexToBytes(block.timestamp, "block.timestamp"),
    normalizeHex(block.extraData, "block.extraData"),
    normalizeHex(block.mixHash ?? block.prevRandao, block.mixHash ? "block.mixHash" : "block.prevRandao"),
    normalizeHex(block.nonce, "block.nonce"),
  ];

  if (block.baseFeePerGas !== undefined && block.baseFeePerGas !== null) {
    items.push(quantityHexToBytes(block.baseFeePerGas, "block.baseFeePerGas"));
  }
  if (block.withdrawalsRoot !== undefined && block.withdrawalsRoot !== null) {
    items.push(normalizeHex(block.withdrawalsRoot, "block.withdrawalsRoot"));
  }
  if (block.blobGasUsed !== undefined || block.excessBlobGas !== undefined) {
    if (block.blobGasUsed === undefined || block.excessBlobGas === undefined) {
      throw new Error(
        `EIP-4844 header fields mismatch: blobGasUsed present=${block.blobGasUsed !== undefined}, excessBlobGas present=${block.excessBlobGas !== undefined}`,
      );
    }
    items.push(quantityHexToBytes(block.blobGasUsed, "block.blobGasUsed"));
    items.push(quantityHexToBytes(block.excessBlobGas, "block.excessBlobGas"));
  }
  if (block.parentBeaconBlockRoot !== undefined && block.parentBeaconBlockRoot !== null) {
    items.push(normalizeHex(block.parentBeaconBlockRoot, "block.parentBeaconBlockRoot"));
  }
  if (block.requestsHash !== undefined && block.requestsHash !== null) {
    items.push(normalizeHex(block.requestsHash, "block.requestsHash"));
  }

  return encodeRlp(items);
}

export function parseBscEpochExtraData(extraDataHex, blockNumber, epochLengthValue) {
  if (epochLengthValue === null || epochLengthValue === undefined) {
    return {
      bsc_epoch_validators: null,
      bsc_epoch_turn_length: null,
    };
  }

  const epochLength = parseIndexValue(epochLengthValue, "bsc-epoch-length");
  if (epochLength === 0n) {
    throw new Error("bsc-epoch-length must be > 0");
  }
  if ((blockNumber % epochLength) !== 0n) {
    return {
      bsc_epoch_validators: null,
      bsc_epoch_turn_length: null,
    };
  }

  const extraData = getBytes(extraDataHex);
  if (extraData.length < 32 + 65) {
    throw new Error("extraData too short for BSC epoch parsing");
  }
  const extraNoSig = extraData.slice(0, extraData.length - 65);
  if (extraNoSig.length < 33) {
    throw new Error("extraData too short");
  }

  const validatorCount = extraNoSig[32];
  const start = 33;
  const validatorBytesLen = 20 + 48;
  const endLuban = start + validatorCount * validatorBytesLen;
  const endPreLuban = start + validatorCount * 20;

  const validators = [];
  let turnLength = null;

  if (validatorCount > 0 && endLuban <= extraNoSig.length) {
    for (let i = 0; i < validatorCount; i += 1) {
      const offset = start + i * validatorBytesLen;
      validators.push(`0x${Buffer.from(extraNoSig.slice(offset, offset + 20)).toString("hex")}`);
    }
    const maybeTurnLength = extraNoSig[endLuban];
    if (maybeTurnLength > 0 && maybeTurnLength <= 64) {
      turnLength = maybeTurnLength;
    }
  } else if (validatorCount > 0 && endPreLuban <= extraNoSig.length) {
    for (let offset = start; offset < endPreLuban; offset += 20) {
      validators.push(`0x${Buffer.from(extraNoSig.slice(offset, offset + 20)).toString("hex")}`);
    }
  } else if (validatorCount !== 0) {
    throw new Error("could not parse BSC epoch validator list from extraData");
  }

  return {
    bsc_epoch_validators: [...new Set(validators)].sort(),
    bsc_epoch_turn_length: turnLength,
  };
}
