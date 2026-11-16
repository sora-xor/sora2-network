#!/usr/bin/env python3
"""Export HASHI scan progress from the localhost RPC of a trusted, stopped-worker database snapshot."""

import argparse
import hashlib
import http.client
import ipaddress
import json
from pathlib import Path
from urllib.parse import urlsplit


def export(rpc, networks):
    def storage(key):
        return rpc("offchain_localStorageGet", ["PERSISTENT", "0x" + key.encode().hex()])

    def progress():
        if storage("eth-bridge-ocw::peer-marker") in (None, "0x"):
            raise ValueError("source database lacks the bridge peer marker")
        for key in ["pending-transactions", "failed-pending-transactions"]:
            if storage("eth-bridge-ocw::" + key) not in (None, "0x00"):
                raise ValueError("source has outstanding bridge retries; restore the full database or replay from genesis")
        cursors = {}
        for network in networks:
            if not 0 <= network <= 0xFFFFFFFF:
                raise ValueError("invalid network ID")
            value = storage(f"eth-bridge-ocw::eth-to-handle-from-height-{network}")
            if not isinstance(value, str) or not value.startswith("0x") or len(value) != 18:
                raise ValueError(f"missing or malformed cursor for network {network}")
            cursors[str(network)] = int.from_bytes(bytes.fromhex(value[2:]), "little")
        return cursors

    best = rpc("chain_getBlockHash", [])
    header = rpc("chain_getHeader", [best])
    genesis = rpc("chain_getBlockHash", [0])
    heights = progress()
    if progress() != heights or rpc("chain_getBlockHash", []) != best:
        raise ValueError("source changed during export; use a stable pre-outage database snapshot")
    return {
        "format": "hashi-scan-cursors-v1",
        "genesisHash": genesis,
        "sourceNumber": int(header["number"], 16),
        "sourceHash": best,
        "nextHeights": heights,
    }


def local_rpc(url):
    address = urlsplit(url)
    if address.scheme != "http" or not ipaddress.ip_address(address.hostname).is_loopback:
        raise ValueError("use the HTTP loopback address of the snapshot node")
    if address.username or address.password or address.query or address.fragment:
        raise ValueError("use the snapshot node's local RPC without URL credentials or query parameters")

    def request(method, params):
        conn = http.client.HTTPConnection(address.hostname, address.port or 80, timeout=20)
        try:
            body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
            conn.request("POST", address.path or "/", body, {"Content-Type": "application/json"})
            response = conn.getresponse()
            if response.status != 200:
                raise ValueError(f"source RPC returned HTTP {response.status}")
            result = json.loads(response.read())
            if "error" in result:
                raise ValueError(f"source RPC rejected {method}: {result['error']}")
            return result["result"]
        finally:
            conn.close()

    return request


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rpc", default="http://127.0.0.1:9944")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("networks", nargs="+", type=int)
    args = parser.parse_args()
    try:
        data = (json.dumps(export(local_rpc(args.rpc), args.networks), indent=2) + "\n").encode()
        with args.output.open("xb") as out:
            out.write(data)
        print(f"{hashlib.sha256(data).hexdigest()}  {args.output}")
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"cursor export failed: {error}\n")
