#!/usr/bin/env python3
"""
Encode SCCP mint-proof JSON into Solana verifier Borsh bytes.

Input is expected to come from in-repo SCCP proof tooling, or any equivalent
generator that emits the same proof JSON fields:

It uses the fields:
  - mmr_proof.leaf_index
  - mmr_proof.leaf_count
  - mmr_proof.items (array of 0x-prefixed bytes32)
  - mmr_leaf.*
  - digest_scale (0x-prefixed bytes)

Output:
  - raw bytes file (optional)
  - hex/base64 on stdout
"""

import argparse
import base64
import json
import struct
import sys
from pathlib import Path


def parse_int(v):
    if isinstance(v, int):
        return v
    if isinstance(v, str):
        vv = v.strip()
        if vv.startswith(("0x", "0X")):
            return int(vv, 16)
        return int(vv)
    raise ValueError(f"cannot parse int from {v!r}")


def hex_to_bytes(v, expected_len=None):
    if not isinstance(v, str):
        raise ValueError(f"expected hex string, got {type(v)}")
    vv = v[2:] if v.startswith(("0x", "0X")) else v
    b = bytes.fromhex(vv)
    if expected_len is not None and len(b) != expected_len:
        raise ValueError(f"expected {expected_len} bytes, got {len(b)} for {v}")
    return b


def encode_u8(v):
    return struct.pack("<B", v)


def encode_u32(v):
    return struct.pack("<I", v)


def encode_u64(v):
    return struct.pack("<Q", v)


def encode_vec_bytes(v: bytes):
    return encode_u32(len(v)) + v


def encode_vec_fixed_32(items):
    out = bytearray()
    out += encode_u32(len(items))
    for it in items:
        if len(it) != 32:
            raise ValueError(f"proof item must be 32 bytes, got {len(it)}")
        out += it
    return bytes(out)


def extract_fields(data):
    mmr_proof = data.get("mmr_proof") or data.get("proof")
    if mmr_proof is None:
        raise ValueError("missing mmr_proof/proof")

    mmr_leaf = data.get("mmr_leaf") or data.get("latest_mmr_leaf") or data.get("leaf")
    if mmr_leaf is None:
        raise ValueError("missing mmr_leaf/latest_mmr_leaf/leaf")

    digest_scale_hex = data.get("digest_scale")
    if digest_scale_hex is None:
        raise ValueError("missing digest_scale")

    return mmr_proof, mmr_leaf, digest_scale_hex


def encode_borsh_sora_burn_proof(data):
    mmr_proof, leaf, digest_scale_hex = extract_fields(data)

    leaf_index = parse_int(mmr_proof["leaf_index"])
    leaf_count = parse_int(mmr_proof["leaf_count"])
    items = [hex_to_bytes(x, 32) for x in mmr_proof["items"]]

    version = parse_int(leaf["version"])
    parent_number = parse_int(leaf["parent_number"])
    parent_hash = hex_to_bytes(leaf["parent_hash"], 32)
    next_set_id = parse_int(leaf["next_authority_set_id"])
    next_set_len = parse_int(leaf["next_authority_set_len"])
    next_set_root = hex_to_bytes(leaf["next_authority_set_root"], 32)
    random_seed = hex_to_bytes(leaf["random_seed"], 32)
    digest_hash = hex_to_bytes(leaf["digest_hash"], 32)
    digest_scale = hex_to_bytes(digest_scale_hex)

    out = bytearray()

    # MmrProof
    out += encode_u64(leaf_index)
    out += encode_u64(leaf_count)
    out += encode_vec_fixed_32(items)

    # MmrLeaf
    out += encode_u8(version)
    out += encode_u32(parent_number)
    out += parent_hash
    out += encode_u64(next_set_id)
    out += encode_u32(next_set_len)
    out += next_set_root
    out += random_seed
    out += digest_hash

    # digest_scale: Vec<u8>
    out += encode_vec_bytes(digest_scale)

    return bytes(out)


def main():
    parser = argparse.ArgumentParser(description="Encode SCCP Solana burn proof (Borsh) from SCCP proof JSON")
    parser.add_argument("--input", required=True, help="Path to SCCP mint-proof JSON")
    parser.add_argument("--output", help="Optional output path for raw Borsh bytes")
    parser.add_argument(
        "--format",
        choices=["hex", "base64", "both"],
        default="both",
        help="Stdout format",
    )
    args = parser.parse_args()

    data = json.loads(Path(args.input).read_text())
    encoded = encode_borsh_sora_burn_proof(data)

    if args.output:
        Path(args.output).write_bytes(encoded)

    if args.format in ("hex", "both"):
        print("hex=0x" + encoded.hex())
    if args.format in ("base64", "both"):
        print("base64=" + base64.b64encode(encoded).decode("ascii"))


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
