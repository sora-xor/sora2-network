#!/usr/bin/env python3

import argparse
import json
import os
import pathlib
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Callable, Dict, List, Optional, Sequence, Tuple


REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
VALIDATORS = [
    ("alice", "Alice"),
    ("bob", "Bob"),
    ("charlie", "Charlie"),
    ("dave", "Dave"),
    ("eve", "Eve"),
    ("ferdie", "Ferdie"),
]
PEER_ID_RE = re.compile(r"Local node identity is:\s*([A-Za-z0-9]+)")
SUITE_ORDER = [
    "smoke",
    "consensus",
    "bridge",
    "rewards",
    "assets",
    "oracle",
    "iroha-migration",
    "negative-rpc",
    "adversarial-rpc",
    "adversarial-network",
    "negative-launch",
]
ALICE_SS58 = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
BOB_SS58 = "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
ZERO_HASH = "0x" + ("00" * 32)
ETH_ZERO_ADDRESS = "0x" + ("00" * 20)
ETH_SAMPLE_ADDRESS = "0x68339de68c9af6577c54867728dbb2db9d7368bf"
UNKNOWN_CROWDLOAN_TAG = "repair"
VESTED_MOCK_TAG = "mock-vesting"
BALANCE_UNIT = 10**18


def predefined_asset_id(index: int) -> str:
    return "0x" + bytes([2, 0, index, *([0] * 29)]).hex()


XOR_ASSET_ID = predefined_asset_id(0)
DAI_ASSET_ID = predefined_asset_id(6)
VAL_ASSET_ID = predefined_asset_id(4)
PSWAP_ASSET_ID = predefined_asset_id(5)
ETH_ASSET_ID = predefined_asset_id(7)
XST_ASSET_ID = predefined_asset_id(9)
TBCD_ASSET_ID = predefined_asset_id(10)
KUSD_ASSET_ID = predefined_asset_id(12)
KARMA_ASSET_ID = predefined_asset_id(15)
DOT_ASSET_ID = "0x00dc9b4341fde46c9ac80b623d0d43afd9ac205baabdc087cadaa06f92b309c7"
ORACLE_MOCK_SYMBOL_RATES = {
    "USD": 1 * BALANCE_UNIT,
    "XAU": 2345 * BALANCE_UNIT,
}
ORACLE_MOCK_FUTURE_SYMBOL = "FUTURE"
FARMING_REWARD_DOUBLING_ASSETS = {
    PSWAP_ASSET_ID,
    VAL_ASSET_ID,
    DAI_ASSET_ID,
    ETH_ASSET_ID,
    XST_ASSET_ID,
    TBCD_ASSET_ID,
    DOT_ASSET_ID,
}
MARKET_MOCK_DEX_ID = 99
MARKET_MOCK_LOW_PRECISION_ASSET_ID = (
    "0xa100000000000000000000000000000000000000000000000000000000000001"
)
MARKET_MOCK_HIGH_PRECISION_ASSET_ID = (
    "0xa100000000000000000000000000000000000000000000000000000000000002"
)
MARKET_MOCK_NON_MINTABLE_ASSET_ID = (
    "0xa100000000000000000000000000000000000000000000000000000000000003"
)
MARKET_MOCK_ASSET_INFOS = {
    MARKET_MOCK_LOW_PRECISION_ASSET_ID: {
        "symbol": "EDGE1",
        "name": "Low precision integration mock",
        "precision": "1",
        "is_mintable": True,
        "total_supply": 0,
    },
    MARKET_MOCK_HIGH_PRECISION_ASSET_ID: {
        "symbol": "EDGE18",
        "name": "Max precision integration mock",
        "precision": "18",
        "is_mintable": True,
        "total_supply": 123_456_789,
    },
    MARKET_MOCK_NON_MINTABLE_ASSET_ID: {
        "symbol": "LOCKED",
        "name": "Non mintable integration mock",
        "precision": "18",
        "is_mintable": False,
        "total_supply": 42,
    },
}
GENESIS_REWARD_CASES = [
    (
        "genesis val holder",
        "0xd170a274320333243b9f860e8891c6792de1ec19",
        [995 * BALANCE_UNIT, 0, 0],
    ),
    (
        "genesis val and farm holder",
        "0xd67fea281b2c5dc3271509c1b628e0867a9815d7",
        [444 * BALANCE_UNIT, 555 * BALANCE_UNIT, 0],
    ),
    (
        "genesis waifu holder",
        "0x886021f300dc809269cfc758a2364a2baf63af0c",
        [0, 0, 333 * BALANCE_UNIT],
    ),
]
LIVE_MOCK_REWARD_CASES = [
    (
        "live-sampled val-only holder",
        "0x2478332fe393ba40ddc9caf8353a333fa64fdd3f",
        [68_708_909_536_239_484_066_990, 0, 0],
    ),
    (
        "live-sampled val and farm holder",
        "0x890f1815a0935b10126bcfe6dd48ce37ed3064ed",
        [
            10_530_506_339_846_422_493_486,
            123_461_943_358_928_754_925_727_844,
            0,
        ],
    ),
    (
        "live-sampled waifu holder",
        "0x726cdc837384a7deb8bbea64beba2e7b4d7346c0",
        [0, 0, 6_936_000_000_000_000_000_000_000],
    ),
    (
        "live-sampled val and farm smaller holder",
        "0x02dc26bda75321d2eb8ea62c5b9dcd04f6c7b740",
        [1_132_321_182_306_036_013_583, 25_924_337_340_061_185_386_730, 0],
    ),
    (
        "live-sampled val and waifu holder",
        "0x345b47bfa3d61b8826a1fb4ac6f4c18cd15a6079",
        [71_691_285_795_332_743_843, 0, 24_000_000_000_000_000_000_000],
    ),
    (
        "live-sampled farm and waifu holder",
        "0x39979745b166572c25b4c7e4e0939c9298efe79d",
        [0, 590_432_601_791_111_900, 24_000_000_000_000_000_000_000],
    ),
    (
        "live-sampled claimable-greater-than-total holder",
        "0x179456bf16752fe5eb8789148e5c98eb39d87fe5",
        [41_364_451_495_963_165_621_569, 0, 0],
    ),
]
VESTED_MOCK_CLAIMABLE_CASES = [
    ("Alice XOR", ALICE_SS58, XOR_ASSET_ID, 200 * BALANCE_UNIT),
    ("Alice PSWAP", ALICE_SS58, PSWAP_ASSET_ID, 125 * BALANCE_UNIT),
    ("Bob XOR", BOB_SS58, XOR_ASSET_ID, 750 * BALANCE_UNIT),
    ("Bob PSWAP", BOB_SS58, PSWAP_ASSET_ID, 275 * BALANCE_UNIT),
]
IROHA_MOCK_ADDRESSES = [
    "did_sora_mock_balance@sora",
    "did_sora_mock_zero@sora",
    "did_sora_mock_referrer@sora",
    "did_sora_mock_referral@sora",
    "did_sora_mock_multisig@sora",
    "did_sora_mock_duplicate_key@sora",
    "did_sora_mock_unicode_123@sora",
]
IROHA_UNKNOWN_ADDRESS = "did_sora_unknown@sora"
BRIDGE_MOCK_NETWORK_ID = 1
BRIDGE_MOCK_NETWORK = {"evmLegacy": BRIDGE_MOCK_NETWORK_ID}
BRIDGE_MOCK_DUPLICATE_EVM_ADDRESS = "0x1111111111111111111111111111111111111111"
BRIDGE_MOCK_ZERO_EVM_ADDRESS = ETH_ZERO_ADDRESS
BRIDGE_MOCK_ASSET_IDS = {PSWAP_ASSET_ID, KUSD_ASSET_ID, TBCD_ASSET_ID, KARMA_ASSET_ID}
BRIDGE_MOCK_EXPECTED_KINDS = {"Thischain", "Sidechain", "SidechainOwned"}
EVM_LEGACY_NETWORK = {"evmLegacy": 0}
BRIDGE_RPC_METHODS = [
    "ethBridge_getRequests",
    "ethBridge_getApprovedRequests",
    "ethBridge_getApprovals",
    "ethBridge_getAccountRequests",
    "ethBridge_getRegisteredAssets",
    "bridgeProxy_listApps",
    "bridgeProxy_listAssets",
]
REWARD_RPC_METHODS = [
    "rewards_claimables",
    "vestedRewards_crowdloanClaimable",
    "vestedRewards_crowdloanLease",
]
ASSET_MARKET_RPC_METHODS = [
    "assets_freeBalance",
    "assets_totalSupply",
    "assets_listAssetIds",
    "assets_listAssetInfos",
    "assets_getAssetInfo",
    "dexManager_listDEXIds",
    "tradingPair_listEnabledPairs",
    "tradingPair_isPairEnabled",
    "tradingPair_listEnabledSourcesForPair",
    "tradingPair_isSourceEnabledForPair",
]
IROHA_MIGRATION_RPC_METHODS = [
    "irohaMigration_needsMigration",
]
ORACLE_FARMING_RPC_METHODS = [
    "oracleProxy_quote",
    "oracleProxy_listEnabledSymbols",
    "farming_rewardDoublingAssets",
]


@dataclass
class Node:
    index: int
    flag: str
    name: str
    base_path: pathlib.Path
    log_path: pathlib.Path
    p2p_port: int
    rpc_port: int
    prometheus_port: int
    process: Optional[subprocess.Popen] = None
    peer_id: Optional[str] = None

    @property
    def rpc_url(self) -> str:
        return f"http://127.0.0.1:{self.rpc_port}"

    @property
    def ws_url(self) -> str:
        return f"ws://127.0.0.1:{self.rpc_port}"


@dataclass
class IntegrationContext:
    binary: pathlib.Path
    chain_spec: pathlib.Path
    chain_id: str
    workdir: pathlib.Path
    bootnode: str
    nodes: List[Node]
    args: argparse.Namespace
    peer_health: List[Dict[str, object]]
    block_numbers: List[int]
    finalized_numbers: List[int]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Start a local SORA integration network and run smoke/external tests."
    )
    parser.add_argument(
        "--binary",
        default=os.environ.get("SORA_INTEGRATION_BINARY"),
        help="Path to a framenode binary built with the private-net feature.",
    )
    parser.add_argument(
        "--build-binary",
        action="store_true",
        help="Build framenode with private-net before starting the network.",
    )
    parser.add_argument(
        "--release",
        action="store_true",
        help="Use cargo --release when --build-binary is set.",
    )
    parser.add_argument(
        "--chain",
        help=(
            "Built-in chain spec to materialize before starting peers. "
            "Defaults to an integration chain based on --peers and --mock-state."
        ),
    )
    parser.add_argument(
        "--mock-state",
        choices=[
            "default",
            "adversarial-rewards",
            "adversarial-bridge",
            "adversarial-market",
            "adversarial-vesting",
            "adversarial-iroha",
            "adversarial-oracle",
            "adversarial-all",
        ],
        default="default",
        help=(
            "Select a deterministic mocked genesis state profile. "
            "adversarial-rewards seeds extra reward claim edge cases; "
            "adversarial-bridge seeds extra bridge asset edge cases; "
            "adversarial-market seeds extra asset/trading-pair edge cases; "
            "adversarial-vesting seeds vested reward/crowdloan edge cases; "
            "adversarial-iroha seeds Iroha migration edge cases; "
            "adversarial-oracle seeds Band/oracle rate edge cases; "
            "adversarial-all enables all profiles."
        ),
    )
    parser.add_argument(
        "--peers",
        type=int,
        default=4,
        help="Number of local validator peers to start. Minimum and default: 4.",
    )
    parser.add_argument(
        "--workdir",
        type=pathlib.Path,
        help="Directory for chain spec, node databases, and logs. Defaults to a temp dir.",
    )
    parser.add_argument(
        "--keep-workdir",
        action="store_true",
        help="Keep the work directory after a successful run. Failed runs are always kept.",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=180,
        help="Seconds to wait for startup, peer connections, block production, and finality.",
    )
    parser.add_argument(
        "--min-blocks",
        type=int,
        default=3,
        help="Minimum best block number every peer must reach.",
    )
    parser.add_argument(
        "--min-finalized-blocks",
        type=int,
        default=1,
        help="Minimum finalized block number every peer must reach.",
    )
    parser.add_argument(
        "--execution",
        choices=["native", "wasm", "native-else-wasm"],
        default="native",
        help="Runtime execution mode passed to framenode.",
    )
    parser.add_argument(
        "--enable-offchain-workers",
        action="store_true",
        help="Enable offchain workers. They are disabled by default for deterministic smoke tests.",
    )
    parser.add_argument(
        "--prepare-bridge-keys",
        action="store_true",
        help="Insert ethb keys and copy misc/eth.json into each peer base path.",
    )
    parser.add_argument(
        "--test-command",
        action="append",
        default=[],
        help="Shell command to run after the network is healthy. Can be passed multiple times.",
    )
    parser.add_argument(
        "--suite",
        action="append",
        choices=["all", "none", *SUITE_ORDER],
        help=(
            "Built-in test suite to run after the network is healthy. "
            "Defaults to all. Can be passed multiple times."
        ),
    )
    parser.add_argument(
        "--hold",
        action="store_true",
        help="Keep the network running after tests until interrupted.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print node commands before starting them.",
    )
    return parser.parse_args()


def choose_binary(args: argparse.Namespace) -> pathlib.Path:
    if args.binary:
        return pathlib.Path(args.binary).expanduser().resolve()

    for candidate in (
        REPO_ROOT / "target" / "debug" / "framenode",
        REPO_ROOT / "target" / "release" / "framenode",
        REPO_ROOT / "framenode",
    ):
        if candidate.exists():
            return candidate

    return REPO_ROOT / "target" / "debug" / "framenode"


def build_binary(release: bool) -> pathlib.Path:
    cmd = [
        str(REPO_ROOT / "scripts" / "with_llvm_env.sh"),
        "cargo",
        "build",
        "--features",
        "private-net",
    ]
    if release:
        cmd.append("--release")

    print(f"Building framenode with private-net: {' '.join(cmd)}")
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)
    profile = "release" if release else "debug"
    return REPO_ROOT / "target" / profile / "framenode"


def reserve_ports(count: int) -> List[int]:
    sockets = []
    try:
        for _ in range(count):
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.bind(("127.0.0.1", 0))
            sockets.append(sock)
        return [sock.getsockname()[1] for sock in sockets]
    finally:
        for sock in sockets:
            sock.close()


def run_checked(cmd: List[str], *, cwd: pathlib.Path = REPO_ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def materialize_chain_spec(binary: pathlib.Path, chain: str, workdir: pathlib.Path) -> pathlib.Path:
    chain_spec = workdir / "chain-spec.json"
    cmd = [str(binary), "build-spec", "--chain", chain, "--raw"]
    with chain_spec.open("w") as out:
        completed = subprocess.run(
            cmd,
            cwd=REPO_ROOT,
            text=True,
            stdout=out,
            stderr=subprocess.PIPE,
            check=False,
        )

    if completed.returncode != 0:
        chain_spec.unlink(missing_ok=True)
        raise RuntimeError(
            "failed to build the integration chain spec with "
            f"{binary}\n\n"
            f"Command: {' '.join(cmd)}\n"
            f"stderr:\n{completed.stderr}\n"
            "Use --build-binary, or set SORA_INTEGRATION_BINARY to a framenode "
            "binary built with --features private-net."
        )

    return chain_spec


def default_chain_alias(peers: int, mock_state: str) -> str:
    suffix = "" if peers == 4 else f"-{peers}"
    if mock_state == "adversarial-rewards":
        return f"integration-mock-rewards{suffix}"
    if mock_state == "adversarial-bridge":
        return f"integration-mock-bridge{suffix}"
    if mock_state == "adversarial-market":
        return f"integration-mock-market{suffix}"
    if mock_state == "adversarial-vesting":
        return f"integration-mock-vesting{suffix}"
    if mock_state == "adversarial-iroha":
        return f"integration-mock-iroha{suffix}"
    if mock_state == "adversarial-oracle":
        return f"integration-mock-oracle{suffix}"
    if mock_state == "adversarial-all":
        return f"integration-mock-adversarial{suffix}"
    return "integration" if peers == 4 else f"integration-{peers}"


def rewards_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-rewards", "adversarial-all"}


def bridge_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-bridge", "adversarial-all"}


def market_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-market", "adversarial-all"}


def vesting_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-vesting", "adversarial-all"}


def iroha_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-iroha", "adversarial-all"}


def oracle_mock_enabled(mock_state: str) -> bool:
    return mock_state in {"adversarial-oracle", "adversarial-all"}


def read_chain_id(chain_spec: pathlib.Path) -> str:
    with chain_spec.open() as spec_file:
        spec = json.load(spec_file)
    chain_id = spec.get("id")
    if not isinstance(chain_id, str) or not chain_id:
        raise RuntimeError(f"chain spec {chain_spec} does not contain a valid id")
    return chain_id


def make_nodes(workdir: pathlib.Path, peers: int) -> List[Node]:
    if peers < 4:
        raise RuntimeError("--peers must be at least 4")
    if peers > len(VALIDATORS):
        raise RuntimeError(f"--peers cannot exceed {len(VALIDATORS)} with built-in dev keys")

    ports = reserve_ports(peers * 3)
    nodes = []
    for index, (flag, name) in enumerate(VALIDATORS[:peers]):
        nodes.append(
            Node(
                index=index,
                flag=flag,
                name=name,
                base_path=workdir / f"node-{index}-{flag}",
                log_path=workdir / "logs" / f"node-{index}-{flag}.log",
                p2p_port=ports[index * 3],
                rpc_port=ports[index * 3 + 1],
                prometheus_port=ports[index * 3 + 2],
            )
        )
    return nodes


def prepare_bridge_keys(
    binary: pathlib.Path, node: Node, chain_spec: pathlib.Path, chain_id: str
) -> None:
    node.base_path.mkdir(parents=True, exist_ok=True)
    cmd = [
        str(binary),
        "key",
        "insert",
        "--chain",
        str(chain_spec),
        "--suri",
        f"//{node.flag}",
        "--scheme",
        "ecdsa",
        "--key-type",
        "ethb",
        "--base-path",
        str(node.base_path),
    ]
    completed = run_checked(cmd)
    if completed.returncode != 0:
        raise RuntimeError(
            f"failed to insert bridge key for {node.name}\n"
            f"stderr:\n{completed.stderr}"
        )

    eth_config = REPO_ROOT / "misc" / "eth.json"
    if eth_config.exists():
        bridge_dir = node.base_path / "chains" / chain_id / "bridge"
        bridge_dir.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(eth_config, bridge_dir / "eth.json")


def node_command(
    binary: pathlib.Path,
    chain_spec: pathlib.Path,
    node: Node,
    bootnode: Optional[str],
    args: argparse.Namespace,
) -> List[str]:
    cmd = [
        str(binary),
        "--chain",
        str(chain_spec),
        "--base-path",
        str(node.base_path),
        f"--{node.flag}",
        "--port",
        str(node.p2p_port),
        "--rpc-port",
        str(node.rpc_port),
        "--prometheus-port",
        str(node.prometheus_port),
        "--rpc-methods",
        "unsafe",
        "--rpc-cors",
        "all",
        "--no-telemetry",
        "--no-mdns",
        "--allow-private-ip",
        "--unsafe-force-node-key-generation",
        "--state-pruning",
        "archive",
        "--blocks-pruning",
        "archive",
        "--execution",
        args.execution,
        "--wasm-execution",
        "compiled",
        "--disable-log-color",
    ]
    if not args.enable_offchain_workers:
        cmd.extend(["--offchain-worker", "never"])
    else:
        cmd.extend(["--enable-offchain-indexing", "true"])
    if bootnode:
        cmd.extend(["--bootnodes", bootnode])
    return cmd


def start_node(
    binary: pathlib.Path,
    chain_spec: pathlib.Path,
    node: Node,
    bootnode: Optional[str],
    args: argparse.Namespace,
) -> None:
    node.base_path.mkdir(parents=True, exist_ok=True)
    node.log_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = node_command(binary, chain_spec, node, bootnode, args)
    if args.verbose:
        print(f"{node.name}: {' '.join(cmd)}")

    log_file = node.log_path.open("w")
    node.process = subprocess.Popen(
        cmd,
        cwd=REPO_ROOT,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    log_file.close()


def tail(path: pathlib.Path, lines: int = 80) -> str:
    if not path.exists():
        return ""
    content = path.read_text(errors="replace").splitlines()
    return "\n".join(content[-lines:])


def check_processes(nodes: List[Node]) -> None:
    for node in nodes:
        if node.process and node.process.poll() is not None:
            raise RuntimeError(
                f"{node.name} exited with code {node.process.returncode}\n"
                f"log: {node.log_path}\n{tail(node.log_path)}"
            )


def wait_until(
    label: str,
    timeout: int,
    nodes: List[Node],
    predicate: Callable[[], Optional[object]],
) -> object:
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            check_processes(nodes)
            result = predicate()
            if result:
                return result
        except (urllib.error.URLError, TimeoutError, ConnectionError, OSError) as exc:
            last_error = exc
        time.sleep(1)

    detail = f" Last error: {last_error}" if last_error else ""
    raise RuntimeError(f"timed out waiting for {label}.{detail}")


def wait_for_peer_id(node: Node, timeout: int, nodes: List[Node]) -> str:
    def scan_log() -> Optional[str]:
        if not node.log_path.exists():
            return None
        match = PEER_ID_RE.search(node.log_path.read_text(errors="replace"))
        return match.group(1) if match else None

    peer_id = wait_until(f"{node.name} peer id", timeout, nodes, scan_log)
    node.peer_id = str(peer_id)
    return node.peer_id


def rpc_call(url: str, method: str, params: Optional[List[object]] = None) -> object:
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}
    ).encode()
    request = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        body = json.loads(response.read().decode())
    if "error" in body:
        raise RuntimeError(f"{method} failed on {url}: {body['error']}")
    return body["result"]


def http_post_raw(url: str, body: bytes) -> Tuple[int, str]:
    request = urllib.request.Request(
        url,
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            return response.status, response.read().decode(errors="replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode(errors="replace")


def http_get_raw(url: str) -> Tuple[int, str]:
    request = urllib.request.Request(url, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            return response.status, response.read().decode(errors="replace")
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read().decode(errors="replace")


def rpc_post_json(url: str, payload: object) -> Tuple[int, object]:
    status, text = http_post_raw(url, json.dumps(payload).encode())
    try:
        return status, json.loads(text)
    except json.JSONDecodeError as exc:
        raise AssertionError(f"RPC response was not JSON: status={status}, body={text}") from exc


def rpc_response(
    node: Node, method: str, params: Optional[List[object]] = None, request_id: int = 1
) -> Dict[str, object]:
    status, body = rpc_post_json(
        node.rpc_url,
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params or []},
    )
    assert status == 200, f"{method} returned HTTP {status}: {body}"
    assert isinstance(body, dict), f"{method} returned non-object response: {body}"
    return body


def expect_rpc_error(
    node: Node, method: str, params: Optional[List[object]] = None, request_id: int = 1
) -> Dict[str, object]:
    response = rpc_response(node, method, params, request_id)
    assert "error" in response, f"{method} unexpectedly succeeded: {response}"
    assert isinstance(response["error"], dict), f"{method} error is malformed: {response}"
    return response


def assert_non_empty_string(value: object, label: str) -> str:
    assert isinstance(value, str) and value, f"{label} should be a non-empty string: {value}"
    return value


def block_number(node: Node) -> int:
    header = rpc_call(node.rpc_url, "chain_getHeader")
    if not isinstance(header, dict):
        raise RuntimeError(f"unexpected header from {node.name}: {header}")
    return int(str(header["number"]), 16)


def finalized_number(node: Node) -> int:
    block_hash = rpc_call(node.rpc_url, "chain_getFinalizedHead")
    header = rpc_call(node.rpc_url, "chain_getHeader", [block_hash])
    if not isinstance(header, dict):
        raise RuntimeError(f"unexpected finalized header from {node.name}: {header}")
    return int(str(header["number"]), 16)


def wait_for_rpc(nodes: List[Node], timeout: int) -> None:
    for node in nodes:
        wait_until(
            f"{node.name} RPC",
            timeout,
            nodes,
            lambda node=node: rpc_call(node.rpc_url, "system_health"),
        )


def wait_for_peers(nodes: List[Node], timeout: int) -> List[Dict[str, object]]:
    def connected() -> Optional[List[Dict[str, object]]]:
        health = []
        for node in nodes:
            value = rpc_call(node.rpc_url, "system_health")
            if not isinstance(value, dict):
                return None
            health.append(value)

        peer_counts = [int(item.get("peers", 0)) for item in health]
        bootnode_ready = peer_counts[0] >= len(nodes) - 1
        other_nodes_ready = all(count >= 1 for count in peer_counts[1:])
        if bootnode_ready and other_nodes_ready:
            return health
        return None

    return wait_until("peer connections", timeout, nodes, connected)  # type: ignore[return-value]


def wait_for_blocks(nodes: List[Node], min_blocks: int, timeout: int) -> List[int]:
    def produced() -> Optional[List[int]]:
        numbers = [block_number(node) for node in nodes]
        if min(numbers) >= min_blocks and max(numbers) - min(numbers) <= 3:
            return numbers
        return None

    return wait_until("block production", timeout, nodes, produced)  # type: ignore[return-value]


def wait_for_finality(nodes: List[Node], min_finalized: int, timeout: int) -> List[int]:
    def finalized() -> Optional[List[int]]:
        numbers = [finalized_number(node) for node in nodes]
        if min(numbers) >= min_finalized:
            return numbers
        return None

    return wait_until("finality", timeout, nodes, finalized)  # type: ignore[return-value]


def expand_suites(suites: Optional[Sequence[str]]) -> List[str]:
    selected = list(suites or ["all"])
    if "none" in selected:
        return []

    expanded: List[str] = []
    for suite in selected:
        if suite == "all":
            expanded.extend(SUITE_ORDER)
        else:
            expanded.append(suite)

    unique: List[str] = []
    for suite in expanded:
        if suite not in unique:
            unique.append(suite)
    return unique


def run_case(name: str, case: Callable[[], None]) -> None:
    print(f"  {name} ... ", end="", flush=True)
    case()
    print("ok")


def run_cases(suite: str, cases: Sequence[Tuple[str, Callable[[], None]]]) -> None:
    print(f"Running built-in suite: {suite}")
    for name, case in cases:
        run_case(name, case)


def run_builtin_suites(ctx: IntegrationContext, suites: Optional[Sequence[str]]) -> None:
    suite_names = expand_suites(suites)
    if not suite_names:
        return

    runners: Dict[str, Callable[[IntegrationContext], None]] = {
        "smoke": run_smoke_suite,
        "consensus": run_consensus_suite,
        "bridge": run_bridge_suite,
        "rewards": run_rewards_suite,
        "assets": run_assets_suite,
        "oracle": run_oracle_suite,
        "iroha-migration": run_iroha_migration_suite,
        "negative-rpc": run_negative_rpc_suite,
        "adversarial-rpc": run_adversarial_rpc_suite,
        "adversarial-network": run_adversarial_network_suite,
        "negative-launch": run_negative_launch_suite,
    }

    for suite in suite_names:
        runners[suite](ctx)


def run_smoke_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "smoke",
        [
            ("all nodes expose chain identity", lambda: assert_chain_identity(ctx)),
            ("runtime versions are consistent", lambda: assert_runtime_versions(ctx)),
            ("token properties are present", lambda: assert_chain_properties(ctx)),
            ("genesis hash is identical", lambda: assert_genesis_hash(ctx)),
            ("peer health matches topology", lambda: assert_peer_health(ctx)),
            ("validator roles are reported", lambda: assert_validator_roles(ctx)),
            ("common best block hash is visible", lambda: assert_common_best_hash(ctx)),
            ("common finalized block hash is visible", lambda: assert_common_finalized_hash(ctx)),
        ],
    )


def assert_chain_identity(ctx: IntegrationContext) -> None:
    chains = [assert_non_empty_string(rpc_call(node.rpc_url, "system_chain"), "system_chain") for node in ctx.nodes]
    names = [assert_non_empty_string(rpc_call(node.rpc_url, "system_name"), "system_name") for node in ctx.nodes]
    versions = [
        assert_non_empty_string(rpc_call(node.rpc_url, "system_version"), "system_version")
        for node in ctx.nodes
    ]

    assert len(set(chains)) == 1, f"nodes disagree on chain name: {chains}"
    assert len(set(names)) == 1, f"nodes disagree on implementation name: {names}"
    assert len(set(versions)) == 1, f"nodes disagree on implementation version: {versions}"


def assert_runtime_versions(ctx: IntegrationContext) -> None:
    versions = [rpc_call(node.rpc_url, "state_getRuntimeVersion") for node in ctx.nodes]
    assert all(isinstance(version, dict) for version in versions), f"bad runtime versions: {versions}"

    spec_names = [version.get("specName") for version in versions if isinstance(version, dict)]
    spec_versions = [version.get("specVersion") for version in versions if isinstance(version, dict)]
    transaction_versions = [
        version.get("transactionVersion") for version in versions if isinstance(version, dict)
    ]
    assert len(set(spec_names)) == 1, f"nodes disagree on specName: {spec_names}"
    assert len(set(spec_versions)) == 1, f"nodes disagree on specVersion: {spec_versions}"
    assert len(set(transaction_versions)) == 1, (
        f"nodes disagree on transactionVersion: {transaction_versions}"
    )


def assert_chain_properties(ctx: IntegrationContext) -> None:
    properties = [rpc_call(node.rpc_url, "system_properties") for node in ctx.nodes]
    assert all(isinstance(value, dict) for value in properties), f"bad properties: {properties}"

    token_symbols = [value.get("tokenSymbol") for value in properties if isinstance(value, dict)]
    assert all("XOR" in str(symbol) for symbol in token_symbols), (
        f"unexpected token symbols: {token_symbols}"
    )


def assert_genesis_hash(ctx: IntegrationContext) -> None:
    hashes = [rpc_call(node.rpc_url, "chain_getBlockHash", [0]) for node in ctx.nodes]
    assert all(isinstance(value, str) and value.startswith("0x") for value in hashes), (
        f"bad genesis hashes: {hashes}"
    )
    assert len(set(hashes)) == 1, f"nodes disagree on genesis hash: {hashes}"


def assert_peer_health(ctx: IntegrationContext) -> None:
    health = [rpc_call(node.rpc_url, "system_health") for node in ctx.nodes]
    assert all(isinstance(value, dict) for value in health), f"bad health responses: {health}"

    peer_counts = [int(value.get("peers", 0)) for value in health if isinstance(value, dict)]
    assert peer_counts[0] >= len(ctx.nodes) - 1, f"bootnode peer count too low: {peer_counts}"
    assert all(count >= 1 for count in peer_counts[1:]), f"non-bootnode peer count too low: {peer_counts}"
    assert all(value.get("isSyncing") is False for value in health if isinstance(value, dict)), (
        f"nodes should not be syncing after readiness gate: {health}"
    )


def assert_validator_roles(ctx: IntegrationContext) -> None:
    roles = [rpc_call(node.rpc_url, "system_nodeRoles") for node in ctx.nodes]
    assert all("Authority" in str(value) for value in roles), f"nodes are not authorities: {roles}"


def assert_common_best_hash(ctx: IntegrationContext) -> None:
    common_number = min(block_number(node) for node in ctx.nodes)
    hashes = [rpc_call(node.rpc_url, "chain_getBlockHash", [common_number]) for node in ctx.nodes]
    assert len(set(hashes)) == 1, f"nodes disagree on block {common_number}: {hashes}"


def assert_common_finalized_hash(ctx: IntegrationContext) -> None:
    common_number = min(finalized_number(node) for node in ctx.nodes)
    hashes = [rpc_call(node.rpc_url, "chain_getBlockHash", [common_number]) for node in ctx.nodes]
    assert len(set(hashes)) == 1, f"nodes disagree on finalized block {common_number}: {hashes}"


def run_consensus_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "consensus",
        [
            ("best blocks stay within skew budget", lambda: assert_best_block_skew(ctx)),
            ("best block advances after startup", lambda: assert_best_block_advances(ctx)),
            ("finality advances after startup", lambda: assert_finality_advances(ctx)),
            ("finalized hash remains common", lambda: assert_common_finalized_hash(ctx)),
            ("logs contain no crash signatures", lambda: assert_logs_have_no_crash_signatures(ctx)),
        ],
    )


def assert_best_block_skew(ctx: IntegrationContext) -> None:
    numbers = [block_number(node) for node in ctx.nodes]
    assert max(numbers) - min(numbers) <= 3, f"best block skew is too high: {numbers}"


def assert_best_block_advances(ctx: IntegrationContext) -> None:
    before = [block_number(node) for node in ctx.nodes]

    def advanced() -> Optional[List[int]]:
        after = [block_number(node) for node in ctx.nodes]
        if min(after) >= min(before) + 1 and max(after) - min(after) <= 3:
            return after
        return None

    wait_until("best block advancement", min(ctx.args.timeout, 45), ctx.nodes, advanced)


def assert_finality_advances(ctx: IntegrationContext) -> None:
    before = [finalized_number(node) for node in ctx.nodes]

    def advanced() -> Optional[List[int]]:
        after = [finalized_number(node) for node in ctx.nodes]
        if min(after) >= min(before) + 1:
            return after
        return None

    wait_until("finality advancement", min(ctx.args.timeout, 60), ctx.nodes, advanced)


def assert_logs_have_no_crash_signatures(ctx: IntegrationContext) -> None:
    crash_terms = [
        "panicked at",
        "fatal runtime error",
        "stack backtrace:",
        "memory allocation of",
        "segmentation fault",
    ]
    for node in ctx.nodes:
        content = node.log_path.read_text(errors="replace").lower()
        for term in crash_terms:
            assert term not in content, f"{node.name} log contains crash signature {term!r}"


def run_bridge_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "bridge",
        [
            ("bridge rpc methods are exposed", lambda: assert_bridge_rpc_methods_exposed(ctx)),
            ("eth bridge registered assets are consistent", lambda: assert_bridge_eth_assets_consistent(ctx)),
            ("bridge proxy apps and assets are consistent", lambda: assert_bridge_proxy_apps_and_assets(ctx)),
            ("mock bridge state profile matches", lambda: assert_mock_bridge_state_profile(ctx)),
            ("empty bridge queues are deterministic", lambda: assert_bridge_empty_request_reads(ctx)),
            ("historical bridge state reads work", lambda: assert_bridge_historical_reads(ctx)),
            ("local bridge setup files are usable", lambda: assert_bridge_local_setup_files(ctx)),
            ("invalid bridge parameters are rejected", lambda: assert_bridge_invalid_params_rejected(ctx)),
            ("adversarial bridge batch is contained", lambda: assert_bridge_adversarial_batch(ctx)),
            ("large bridge request vectors are contained", lambda: assert_large_bridge_query_contained(ctx)),
            ("network remains healthy after bridge tests", lambda: assert_network_healthy(ctx)),
        ],
    )


def bridge_result_ok(node: Node, method: str, params: Optional[List[object]] = None) -> object:
    response = rpc_response(node, method, params)
    assert "result" in response, f"{method} did not return a result: {response}"
    result = response["result"]
    assert isinstance(result, dict) and "Ok" in result, (
        f"{method} should return a runtime Ok result: {response}"
    )
    return result["Ok"]


def assert_hex_string(value: object, byte_len: int, label: str) -> str:
    assert isinstance(value, str), f"{label} should be a hex string: {value}"
    assert value.startswith("0x"), f"{label} should start with 0x: {value}"
    assert len(value) == 2 + byte_len * 2, (
        f"{label} should be {byte_len} bytes, got {len(value) - 2} hex chars: {value}"
    )
    int(value[2:], 16)
    return value


def bridge_registered_assets(
    node: Node, network_id: Optional[int] = 0, at: Optional[str] = None
) -> List[object]:
    params: List[object] = []
    if network_id is not None or at is not None:
        params.append(network_id)
    if at is not None:
        params.append(at)
    assets = bridge_result_ok(node, "ethBridge_getRegisteredAssets", params)
    assert isinstance(assets, list), f"registered bridge assets should be a list: {assets}"
    return assets


def bridge_proxy_assets(
    node: Node, network_id: Optional[object] = None, at: Optional[str] = None
) -> List[object]:
    params: List[object] = [network_id if network_id is not None else EVM_LEGACY_NETWORK]
    if at is not None:
        params.append(at)
    assets = rpc_call(node.rpc_url, "bridgeProxy_listAssets", params)
    assert isinstance(assets, list), f"bridge proxy assets should be a list: {assets}"
    return assets


def bridge_proxy_apps(node: Node, at: Optional[str] = None) -> List[object]:
    params: List[object] = [at] if at is not None else []
    apps = rpc_call(node.rpc_url, "bridgeProxy_listApps", params)
    assert isinstance(apps, list), f"bridge proxy apps should be a list: {apps}"
    return apps


def assert_bridge_rpc_methods_exposed(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        methods_response = rpc_call(node.rpc_url, "rpc_methods")
        assert isinstance(methods_response, dict), (
            f"{node.name} returned malformed rpc_methods: {methods_response}"
        )
        methods = methods_response.get("methods")
        assert isinstance(methods, list), f"{node.name} returned malformed methods: {methods}"
        missing = [method for method in BRIDGE_RPC_METHODS if method not in methods]
        assert not missing, f"{node.name} is missing bridge RPC methods: {missing}"


def assert_bridge_eth_assets_consistent(ctx: IntegrationContext) -> None:
    assets_by_node = [bridge_registered_assets(node) for node in ctx.nodes]
    first = assets_by_node[0]
    assert len(first) >= 3, f"expected bridge genesis assets, got: {first}"
    for assets in assets_by_node[1:]:
        assert assets == first, f"nodes disagree on registered bridge assets: {assets_by_node}"
    assert_registered_asset_shape(first)

    unknown_assets = bridge_registered_assets(ctx.nodes[0], 9999)
    assert unknown_assets == [], f"unknown eth bridge network should be empty: {unknown_assets}"


def assert_registered_asset_shape(assets: Sequence[object]) -> None:
    expected_kinds = {"Thischain", "Sidechain", "SidechainOwned"}
    for index, entry in enumerate(assets):
        assert isinstance(entry, list) and len(entry) == 3, (
            f"registered asset #{index} has unexpected shape: {entry}"
        )
        kind, asset_tuple, sidechain_tuple = entry
        assert kind in expected_kinds, f"registered asset #{index} has bad kind: {kind}"
        assert isinstance(asset_tuple, list) and len(asset_tuple) == 2, (
            f"registered asset #{index} has bad asset tuple: {asset_tuple}"
        )
        assert_hex_string(asset_tuple[0], 32, f"registered asset #{index} asset id")
        assert isinstance(asset_tuple[1], int) and asset_tuple[1] >= 0, (
            f"registered asset #{index} has bad precision: {asset_tuple}"
        )
        if sidechain_tuple is not None:
            assert isinstance(sidechain_tuple, list) and len(sidechain_tuple) == 2, (
                f"registered asset #{index} has bad sidechain tuple: {sidechain_tuple}"
            )
            assert_hex_string(sidechain_tuple[0], 20, f"registered asset #{index} evm address")
            assert isinstance(sidechain_tuple[1], int) and sidechain_tuple[1] >= 0, (
                f"registered asset #{index} has bad sidechain precision: {sidechain_tuple}"
            )


def assert_bridge_proxy_apps_and_assets(ctx: IntegrationContext) -> None:
    apps_by_node = [bridge_proxy_apps(node) for node in ctx.nodes]
    first_apps = apps_by_node[0]
    for apps in apps_by_node[1:]:
        assert apps == first_apps, f"nodes disagree on bridge proxy apps: {apps_by_node}"
    assert_bridge_app_shape(first_apps)
    app_json = json.dumps(first_apps, sort_keys=True)
    assert "HashiBridge" in app_json, f"HashiBridge app is missing: {first_apps}"
    assert "ValMaster" in app_json, f"ValMaster app is missing: {first_apps}"

    proxy_assets_by_node = [bridge_proxy_assets(node) for node in ctx.nodes]
    first_proxy_assets = proxy_assets_by_node[0]
    for assets in proxy_assets_by_node[1:]:
        assert assets == first_proxy_assets, (
            f"nodes disagree on bridge proxy assets: {proxy_assets_by_node}"
        )
    assert len(first_proxy_assets) >= 3, f"expected bridge proxy genesis assets: {first_proxy_assets}"

    eth_asset_ids = set(extract_registered_asset_ids(bridge_registered_assets(ctx.nodes[0])))
    proxy_asset_ids = set(extract_bridge_proxy_asset_ids(first_proxy_assets))
    assert proxy_asset_ids == eth_asset_ids, (
        f"bridge proxy assets do not match eth bridge assets: "
        f"proxy={proxy_asset_ids}, eth={eth_asset_ids}"
    )

    unknown_assets = bridge_proxy_assets(ctx.nodes[0], {"evmLegacy": 9999})
    assert unknown_assets == [], f"unknown bridge proxy network should be empty: {unknown_assets}"


def assert_mock_bridge_state_profile(ctx: IntegrationContext) -> None:
    if not bridge_mock_enabled(ctx.args.mock_state):
        return

    legacy_assets = bridge_registered_assets(ctx.nodes[0])
    legacy_asset_ids = set(extract_registered_asset_ids(legacy_assets))
    assert BRIDGE_MOCK_ASSET_IDS.issubset(legacy_asset_ids), (
        f"mock bridge assets should be visible on legacy eth bridge network: "
        f"expected subset={BRIDGE_MOCK_ASSET_IDS}, actual={legacy_asset_ids}"
    )
    legacy_sidechain_addresses = extract_registered_sidechain_addresses(legacy_assets)
    assert legacy_sidechain_addresses.count(BRIDGE_MOCK_DUPLICATE_EVM_ADDRESS) == 2, (
        f"legacy mock bridge should expose duplicate sidechain token mapping: "
        f"{legacy_sidechain_addresses}"
    )
    assert BRIDGE_MOCK_ZERO_EVM_ADDRESS in legacy_sidechain_addresses, (
        f"legacy mock bridge should expose zero-address sidechain token: "
        f"{legacy_sidechain_addresses}"
    )

    proxy_assets_by_node = [bridge_proxy_assets(node) for node in ctx.nodes]
    first_proxy_assets = proxy_assets_by_node[0]
    for assets in proxy_assets_by_node[1:]:
        assert assets == first_proxy_assets, (
            f"nodes disagree on mock legacy bridge proxy assets: {proxy_assets_by_node}"
        )
    proxy_asset_ids = set(extract_bridge_proxy_asset_ids(first_proxy_assets))
    assert BRIDGE_MOCK_ASSET_IDS.issubset(proxy_asset_ids), (
        f"mock bridge assets should be visible through bridge proxy: "
        f"expected subset={BRIDGE_MOCK_ASSET_IDS}, actual={proxy_asset_ids}"
    )
    proxy_sidechain_addresses = extract_bridge_proxy_sidechain_addresses(first_proxy_assets)
    assert proxy_sidechain_addresses.count(BRIDGE_MOCK_DUPLICATE_EVM_ADDRESS) == 2, (
        f"mock bridge proxy should expose duplicate token mapping: {proxy_sidechain_addresses}"
    )
    assert BRIDGE_MOCK_ZERO_EVM_ADDRESS in proxy_sidechain_addresses, (
        f"mock bridge proxy should expose zero-address token: {proxy_sidechain_addresses}"
    )

    assets_by_node = [bridge_registered_assets(node, BRIDGE_MOCK_NETWORK_ID) for node in ctx.nodes]
    first_assets = assets_by_node[0]
    for assets in assets_by_node[1:]:
        assert assets == first_assets, f"nodes disagree on mock bridge assets: {assets_by_node}"
    assert_registered_asset_shape(first_assets)

    asset_ids = set(extract_registered_asset_ids(first_assets))
    assert asset_ids == BRIDGE_MOCK_ASSET_IDS, (
        f"mock bridge asset ids mismatch: expected={BRIDGE_MOCK_ASSET_IDS}, actual={asset_ids}"
    )
    asset_kinds = extract_registered_asset_kinds(first_assets)
    assert BRIDGE_MOCK_EXPECTED_KINDS.issubset(asset_kinds), (
        f"mock bridge should cover each asset kind: expected={BRIDGE_MOCK_EXPECTED_KINDS}, "
        f"actual={asset_kinds}"
    )
    sidechain_addresses = extract_registered_sidechain_addresses(first_assets)
    assert sidechain_addresses.count(BRIDGE_MOCK_DUPLICATE_EVM_ADDRESS) == 2, (
        f"mock bridge should expose duplicate sidechain token mapping: {sidechain_addresses}"
    )
    assert BRIDGE_MOCK_ZERO_EVM_ADDRESS in sidechain_addresses, (
        f"mock bridge should expose zero-address sidechain token: {sidechain_addresses}"
    )

    assert bridge_registered_assets(ctx.nodes[0], BRIDGE_MOCK_NETWORK_ID + 1) == []
    assert bridge_proxy_assets(ctx.nodes[0], BRIDGE_MOCK_NETWORK) == []
    assert bridge_result_ok(
        ctx.nodes[0], "ethBridge_getRequests", [[], BRIDGE_MOCK_NETWORK_ID, True]
    ) == []
    assert bridge_result_ok(
        ctx.nodes[0], "ethBridge_getApprovedRequests", [[], BRIDGE_MOCK_NETWORK_ID]
    ) == []
    assert bridge_result_ok(
        ctx.nodes[0], "ethBridge_getApprovals", [[], BRIDGE_MOCK_NETWORK_ID]
    ) == []


def assert_bridge_app_shape(apps: Sequence[object]) -> None:
    assert len(apps) >= 2, f"expected bridge proxy apps: {apps}"
    for index, app in enumerate(apps):
        assert isinstance(app, dict) and len(app) == 1, (
            f"bridge app #{index} has unexpected shape: {app}"
        )
        variant, value = next(iter(app.items()))
        assert variant in {"evm", "sub", "TON", "tON"}, (
            f"bridge app #{index} has bad variant: {app}"
        )
        if variant == "evm":
            assert isinstance(value, list) and len(value) == 2, (
                f"evm bridge app #{index} has bad tuple: {value}"
            )
            assert value[0] == EVM_LEGACY_NETWORK, (
                f"evm bridge app #{index} should be on legacy network 0: {value}"
            )
            assert isinstance(value[1], dict), f"evm bridge app #{index} has bad info: {value}"
            assert_hex_string(
                value[1].get("evm_address") or value[1].get("evmAddress"),
                20,
                f"evm bridge app #{index} address",
            )


def extract_registered_asset_ids(assets: Sequence[object]) -> List[str]:
    asset_ids = []
    for index, entry in enumerate(assets):
        assert isinstance(entry, list) and len(entry) == 3, (
            f"registered asset #{index} has unexpected shape: {entry}"
        )
        asset_tuple = entry[1]
        assert isinstance(asset_tuple, list) and len(asset_tuple) == 2, (
            f"registered asset #{index} has bad asset tuple: {asset_tuple}"
        )
        asset_ids.append(assert_hex_string(asset_tuple[0], 32, f"registered asset #{index} asset id"))
    return asset_ids


def extract_registered_asset_kinds(assets: Sequence[object]) -> set:
    kinds = set()
    for index, entry in enumerate(assets):
        assert isinstance(entry, list) and len(entry) == 3, (
            f"registered asset #{index} has unexpected shape: {entry}"
        )
        kinds.add(entry[0])
    return kinds


def extract_registered_sidechain_addresses(assets: Sequence[object]) -> List[str]:
    addresses = []
    for index, entry in enumerate(assets):
        assert isinstance(entry, list) and len(entry) == 3, (
            f"registered asset #{index} has unexpected shape: {entry}"
        )
        sidechain_tuple = entry[2]
        if sidechain_tuple is None:
            continue
        assert isinstance(sidechain_tuple, list) and len(sidechain_tuple) == 2, (
            f"registered asset #{index} has bad sidechain tuple: {sidechain_tuple}"
        )
        addresses.append(
            assert_hex_string(sidechain_tuple[0], 20, f"registered asset #{index} evm address")
        )
    return addresses


def extract_bridge_proxy_asset_ids(assets: Sequence[object]) -> List[str]:
    asset_ids = []
    for index, asset in enumerate(assets):
        assert isinstance(asset, dict) and len(asset) == 1, (
            f"bridge proxy asset #{index} has unexpected shape: {asset}"
        )
        variant, value = next(iter(asset.items()))
        assert variant == "evmLegacy", f"expected legacy evm bridge asset #{index}: {asset}"
        assert isinstance(value, dict), f"bridge proxy asset #{index} has bad payload: {value}"
        asset_id = value.get("asset_id") or value.get("assetId")
        asset_ids.append(assert_hex_string(asset_id, 32, f"bridge proxy asset #{index} asset id"))
        evm_address = value.get("evm_address") or value.get("evmAddress")
        if evm_address is not None:
            assert_hex_string(evm_address, 20, f"bridge proxy asset #{index} evm address")
    return asset_ids


def extract_bridge_proxy_sidechain_addresses(assets: Sequence[object]) -> List[str]:
    addresses = []
    for index, asset in enumerate(assets):
        assert isinstance(asset, dict) and len(asset) == 1, (
            f"bridge proxy asset #{index} has unexpected shape: {asset}"
        )
        variant, value = next(iter(asset.items()))
        assert variant == "evmLegacy", f"expected legacy evm bridge asset #{index}: {asset}"
        assert isinstance(value, dict), f"bridge proxy asset #{index} has bad payload: {value}"
        evm_address = value.get("evm_address") or value.get("evmAddress")
        if evm_address is not None:
            addresses.append(
                assert_hex_string(evm_address, 20, f"bridge proxy asset #{index} evm address")
            )
    return addresses


def assert_bridge_empty_request_reads(ctx: IntegrationContext) -> None:
    empty_calls: Sequence[Tuple[str, List[object]]] = [
        ("ethBridge_getRequests", [[], None, True]),
        ("ethBridge_getRequests", [[ZERO_HASH], None, False]),
        ("ethBridge_getApprovedRequests", [[], None]),
        ("ethBridge_getApprovedRequests", [[ZERO_HASH], None]),
        ("ethBridge_getApprovals", [[], None]),
        ("ethBridge_getApprovals", [[ZERO_HASH], None]),
        ("ethBridge_getAccountRequests", [ALICE_SS58, None]),
    ]
    for node in ctx.nodes:
        for method, params in empty_calls:
            result = bridge_result_ok(node, method, params)
            assert result == [], f"{node.name} {method} should be empty: {result}"


def assert_bridge_historical_reads(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")

    current_assets = bridge_registered_assets(ctx.nodes[0])
    historical_assets = bridge_registered_assets(ctx.nodes[0], 0, finalized_hash)
    assert historical_assets == current_assets, (
        f"historical eth bridge assets diverged: current={current_assets}, at={historical_assets}"
    )

    current_apps = bridge_proxy_apps(ctx.nodes[0])
    historical_apps = bridge_proxy_apps(ctx.nodes[0], finalized_hash)
    assert historical_apps == current_apps, (
        f"historical bridge apps diverged: current={current_apps}, at={historical_apps}"
    )

    current_proxy_assets = bridge_proxy_assets(ctx.nodes[0])
    historical_proxy_assets = bridge_proxy_assets(ctx.nodes[0], EVM_LEGACY_NETWORK, finalized_hash)
    assert historical_proxy_assets == current_proxy_assets, (
        "historical bridge proxy assets diverged: "
        f"current={current_proxy_assets}, at={historical_proxy_assets}"
    )


def assert_bridge_local_setup_files(ctx: IntegrationContext) -> None:
    if not (ctx.args.prepare_bridge_keys or ctx.args.enable_offchain_workers):
        return

    for node in ctx.nodes:
        config_path = node.base_path / "chains" / ctx.chain_id / "bridge" / "eth.json"
        assert config_path.exists(), f"{node.name} bridge config is missing at {config_path}"
        with config_path.open() as config_file:
            config = json.load(config_file)
        networks = config.get("networks")
        assert isinstance(networks, dict) and networks, (
            f"{node.name} bridge config has no networks: {config}"
        )
        network_zero = networks.get("0")
        assert isinstance(network_zero, dict) and isinstance(network_zero.get("url"), str), (
            f"{node.name} bridge config network 0 has no url: {config}"
        )


def assert_bridge_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls: Sequence[Tuple[str, List[object]]] = [
        ("bridgeProxy_listAssets", []),
        ("bridgeProxy_listAssets", [{"EVMLegacy": 0}]),
        ("bridgeProxy_listAssets", [0]),
        ("bridgeProxy_listAssets", [{"evm": 0}]),
        ("bridgeProxy_listAssets", [{"sub": "Rococo"}]),
        ("ethBridge_getRegisteredAssets", [{"evmLegacy": 0}]),
        ("ethBridge_getRegisteredAssets", ["definitely-not-a-network"]),
        ("ethBridge_getRegisteredAssets", [-1]),
        ("ethBridge_getRequests", [["0x00"], None, True]),
        ("ethBridge_getRequests", [[123], None, True]),
        ("ethBridge_getRequests", [[ZERO_HASH], {"evmLegacy": 0}, True]),
        ("ethBridge_getApprovedRequests", [["not-hex"], None]),
        ("ethBridge_getApprovals", [["0x00"], None]),
        ("ethBridge_getAccountRequests", ["not-an-account", None]),
        ("ethBridge_getAccountRequests", ["0x" + ("00" * 32), None]),
        ("ethBridge_getAccountRequests", [ALICE_SS58, "DefinitelyBadStatus"]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def assert_bridge_adversarial_batch(ctx: IntegrationContext) -> None:
    valid_ids = set()
    payload = []
    for index in range(36):
        if index % 6 == 0:
            method, params = "bridgeProxy_listApps", []
            valid_ids.add(index)
        elif index % 6 == 1:
            method, params = "bridgeProxy_listAssets", [EVM_LEGACY_NETWORK]
            valid_ids.add(index)
        elif index % 6 == 2:
            method, params = "ethBridge_getRegisteredAssets", [0]
            valid_ids.add(index)
        elif index % 6 == 3:
            method, params = "bridgeProxy_listAssets", [{"EVMLegacy": index}]
        elif index % 6 == 4:
            method, params = "ethBridge_getRequests", [["0x00"], None, True]
        else:
            method, params = "ethBridge_getAccountRequests", ["not-an-account", None]
        payload.append({"jsonrpc": "2.0", "id": index, "method": method, "params": params})

    status, body = rpc_post_json(ctx.nodes[0].rpc_url, payload)
    assert status == 200, f"bridge adversarial batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == len(payload), (
        f"bridge adversarial batch returned malformed body: {body}"
    )

    successes = [item for item in body if isinstance(item, dict) and "result" in item]
    errors = [item for item in body if isinstance(item, dict) and "error" in item]
    assert len(successes) == len(valid_ids), (
        f"bridge batch success count mismatch: successes={successes}, body={body}"
    )
    assert len(errors) == len(payload) - len(valid_ids), (
        f"bridge batch error count mismatch: errors={errors}, body={body}"
    )
    for item in successes:
        assert item.get("id") in valid_ids, f"unexpected successful bridge batch item: {item}"


def assert_large_bridge_query_contained(ctx: IntegrationContext) -> None:
    repeated_hashes = [ZERO_HASH for _ in range(64)]
    assert bridge_result_ok(
        ctx.nodes[0], "ethBridge_getRequests", [repeated_hashes, None, True]
    ) == []
    assert bridge_result_ok(
        ctx.nodes[0], "ethBridge_getApprovedRequests", [repeated_hashes, None]
    ) == []
    assert bridge_result_ok(ctx.nodes[0], "ethBridge_getApprovals", [repeated_hashes, None]) == []


def run_rewards_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "rewards",
        [
            ("reward rpc methods are exposed", lambda: assert_reward_rpc_methods_exposed(ctx)),
            ("legacy reward claimables are deterministic", lambda: assert_legacy_reward_claimables(ctx)),
            ("mock reward state profile matches", lambda: assert_mock_reward_state_profile(ctx)),
            ("vested reward claim reads are deterministic", lambda: assert_vested_reward_claim_reads(ctx)),
            ("mock vested reward state profile matches", lambda: assert_mock_vested_reward_state_profile(ctx)),
            ("historical reward claim reads work", lambda: assert_reward_historical_reads(ctx)),
            ("invalid reward claim parameters are rejected", lambda: assert_reward_invalid_params_rejected(ctx)),
            ("unsigned reward claim extrinsics are rejected", lambda: assert_reward_claim_extrinsics_rejected(ctx)),
            ("claim-shaped fee queries decode", lambda: assert_reward_claim_fee_queries_decode(ctx)),
            ("adversarial reward claim batch is contained", lambda: assert_reward_adversarial_batch(ctx)),
            ("large reward claim inputs are contained", lambda: assert_large_reward_inputs_contained(ctx)),
            ("network remains healthy after reward tests", lambda: assert_network_healthy(ctx)),
        ],
    )


def scale_compact_u32(value: int) -> bytes:
    if value < 0:
        raise ValueError(f"compact encoding only supports non-negative values: {value}")
    if value < 1 << 6:
        return bytes([value << 2])
    if value < 1 << 14:
        return ((value << 2) | 1).to_bytes(2, "little")
    if value < 1 << 30:
        return ((value << 2) | 2).to_bytes(4, "little")
    raw = value.to_bytes((value.bit_length() + 7) // 8, "little")
    return bytes([((len(raw) - 4) << 2) | 3]) + raw


def unsigned_extrinsic(call: bytes) -> str:
    payload = b"\x04" + call
    return "0x" + (scale_compact_u32(len(payload)) + payload).hex()


def reward_claim_extrinsics() -> Dict[str, str]:
    legacy_short_signature = bytes([8, 0]) + scale_compact_u32(64) + bytes(64)
    legacy_bad_signature = bytes([8, 0]) + scale_compact_u32(65) + bytes(64) + b"\x1b"
    vested_claim_rewards = bytes([40, 0])
    vested_claim_crowdloan = (
        bytes([40, 1])
        + scale_compact_u32(len(UNKNOWN_CROWDLOAN_TAG))
        + UNKNOWN_CROWDLOAN_TAG.encode()
    )
    return {
        "legacy short signature claim": unsigned_extrinsic(legacy_short_signature),
        "legacy bad signature claim": unsigned_extrinsic(legacy_bad_signature),
        "vested claim rewards": unsigned_extrinsic(vested_claim_rewards),
        "vested crowdloan claim": unsigned_extrinsic(vested_claim_crowdloan),
    }


def assert_reward_rpc_methods_exposed(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        methods_response = rpc_call(node.rpc_url, "rpc_methods")
        assert isinstance(methods_response, dict), (
            f"{node.name} returned malformed rpc_methods: {methods_response}"
        )
        methods = methods_response.get("methods")
        assert isinstance(methods, list), f"{node.name} returned malformed methods: {methods}"
        missing = [method for method in REWARD_RPC_METHODS if method not in methods]
        assert not missing, f"{node.name} is missing reward RPC methods: {missing}"


def reward_claimables(node: Node, eth_address: str, at: Optional[str] = None) -> List[object]:
    params: List[object] = [eth_address]
    if at is not None:
        params.append(at)
    claimables = rpc_call(node.rpc_url, "rewards_claimables", params)
    assert isinstance(claimables, list), f"reward claimables should be a list: {claimables}"
    return claimables


def assert_legacy_reward_claimables(ctx: IntegrationContext) -> None:
    for address in [ETH_ZERO_ADDRESS, ETH_SAMPLE_ADDRESS]:
        claimables_by_node = [reward_claimables(node, address) for node in ctx.nodes]
        first = claimables_by_node[0]
        for claimables in claimables_by_node[1:]:
            assert claimables == first, (
                f"nodes disagree on legacy rewards for {address}: {claimables_by_node}"
            )
        assert_legacy_reward_shape(first, address)

    for label, address, expected in GENESIS_REWARD_CASES:
        assert_reward_case(ctx, label, address, expected)


def assert_legacy_reward_shape(claimables: Sequence[object], address: str) -> None:
    assert len(claimables) == 3, (
        f"legacy rewards for {address} should expose VAL/farm/waifu balances: {claimables}"
    )
    for index, item in enumerate(claimables):
        assert isinstance(item, dict), f"legacy reward #{index} has bad shape: {item}"
        balance = item.get("balance")
        assert isinstance(balance, str) and balance.isdigit(), (
            f"legacy reward #{index} has non-numeric balance: {item}"
        )


def reward_balances(claimables: Sequence[object]) -> List[int]:
    balances = []
    for index, item in enumerate(claimables):
        assert isinstance(item, dict), f"reward #{index} has bad shape: {item}"
        balance = item.get("balance")
        assert isinstance(balance, str) and balance.isdigit(), (
            f"reward #{index} has non-numeric balance: {item}"
        )
        balances.append(int(balance))
    return balances


def assert_reward_case(
    ctx: IntegrationContext, label: str, address: str, expected: Sequence[int]
) -> None:
    claimables_by_node = [reward_claimables(node, address) for node in ctx.nodes]
    first = claimables_by_node[0]
    for claimables in claimables_by_node[1:]:
        assert claimables == first, (
            f"nodes disagree on {label} reward state for {address}: {claimables_by_node}"
        )
    actual = reward_balances(first[: len(expected)])
    assert actual == list(expected), (
        f"{label} reward state mismatch for {address}: expected={list(expected)}, actual={actual}"
    )


def assert_mock_reward_state_profile(ctx: IntegrationContext) -> None:
    if not rewards_mock_enabled(ctx.args.mock_state):
        return
    for label, address, expected in LIVE_MOCK_REWARD_CASES:
        assert_reward_case(ctx, label, address, expected)


def vested_crowdloan_claimable(
    node: Node,
    tag: str = UNKNOWN_CROWDLOAN_TAG,
    account: str = ALICE_SS58,
    asset_id: str = PSWAP_ASSET_ID,
    at: Optional[str] = None,
) -> object:
    params: List[object] = [tag, account, asset_id]
    if at is not None:
        params.append(at)
    return rpc_call(node.rpc_url, "vestedRewards_crowdloanClaimable", params)


def vested_crowdloan_lease(
    node: Node, tag: str = VESTED_MOCK_TAG, at: Optional[str] = None
) -> Dict[str, object]:
    params: List[object] = [tag]
    if at is not None:
        params.append(at)
    lease = rpc_call(node.rpc_url, "vestedRewards_crowdloanLease", params)
    assert isinstance(lease, dict), f"crowdloan lease should be an object: {lease}"
    return lease


def assert_vested_reward_claim_reads(ctx: IntegrationContext) -> None:
    claimable_calls = [
        (UNKNOWN_CROWDLOAN_TAG, ALICE_SS58, XOR_ASSET_ID),
        (UNKNOWN_CROWDLOAN_TAG, ALICE_SS58, PSWAP_ASSET_ID),
        ("", ALICE_SS58, PSWAP_ASSET_ID),
    ]
    for node in ctx.nodes:
        for tag, account, asset_id in claimable_calls:
            result = vested_crowdloan_claimable(node, tag, account, asset_id)
            assert result is None, (
                f"{node.name} nonexistent crowdloan claimable should be null: {result}"
            )
        error = expect_rpc_error(node, "vestedRewards_crowdloanLease", [UNKNOWN_CROWDLOAN_TAG])
        assert "Crowdloan not found" in str(error["error"]), (
            f"{node.name} unexpected missing crowdloan lease error: {error}"
        )


def assert_mock_vested_reward_state_profile(ctx: IntegrationContext) -> None:
    if not vesting_mock_enabled(ctx.args.mock_state):
        return

    leases = [vested_crowdloan_lease(node) for node in ctx.nodes]
    first_lease = leases[0]
    for lease in leases[1:]:
        assert lease == first_lease, f"nodes disagree on mock vested lease: {leases}"
    assert str(field(first_lease, "start_block")) == "0", (
        f"mock vested lease should start at block 0: {first_lease}"
    )
    assert str(field(first_lease, "total_days")) == "0", (
        f"mock vested lease length should be shorter than one day: {first_lease}"
    )
    assert int(str(field(first_lease, "blocks_per_day"))) > 0, (
        f"mock vested lease should expose blocks per day: {first_lease}"
    )

    for label, account, asset_id, expected in VESTED_MOCK_CLAIMABLE_CASES:
        claimables = [
            vested_crowdloan_claimable(node, VESTED_MOCK_TAG, account, asset_id)
            for node in ctx.nodes
        ]
        first_claimable = claimables[0]
        for claimable in claimables[1:]:
            assert claimable == first_claimable, (
                f"nodes disagree on mock vested reward {label}: {claimables}"
            )
        assert balance_value(first_claimable, f"mock vested reward {label}") == expected, (
            f"mock vested reward {label} mismatch: "
            f"expected={expected}, actual={first_claimable}"
        )

    assert vested_crowdloan_claimable(
        ctx.nodes[0], VESTED_MOCK_TAG, ALICE_SS58, DAI_ASSET_ID
    ) is None
    assert vested_crowdloan_claimable(
        ctx.nodes[0], VESTED_MOCK_TAG, BOB_SS58, DAI_ASSET_ID
    ) is None

    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")
    current_claimable = vested_crowdloan_claimable(
        ctx.nodes[0], VESTED_MOCK_TAG, ALICE_SS58, XOR_ASSET_ID
    )
    historical_claimable = vested_crowdloan_claimable(
        ctx.nodes[0], VESTED_MOCK_TAG, ALICE_SS58, XOR_ASSET_ID, finalized_hash
    )
    assert historical_claimable == current_claimable, (
        f"historical mock vested reward diverged: "
        f"current={current_claimable}, at={historical_claimable}"
    )
    assert vested_crowdloan_lease(ctx.nodes[0], VESTED_MOCK_TAG, finalized_hash) == first_lease


def assert_reward_historical_reads(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")

    current_claimables = reward_claimables(ctx.nodes[0], ETH_SAMPLE_ADDRESS)
    historical_claimables = reward_claimables(ctx.nodes[0], ETH_SAMPLE_ADDRESS, finalized_hash)
    assert historical_claimables == current_claimables, (
        f"historical legacy reward claimables diverged: "
        f"current={current_claimables}, at={historical_claimables}"
    )

    current_vested = vested_crowdloan_claimable(ctx.nodes[0])
    historical_vested = vested_crowdloan_claimable(ctx.nodes[0], at=finalized_hash)
    assert historical_vested == current_vested, (
        f"historical vested reward claimable diverged: current={current_vested}, "
        f"at={historical_vested}"
    )


def assert_reward_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls: Sequence[Tuple[str, List[object]]] = [
        ("rewards_claimables", []),
        ("rewards_claimables", ["0x00"]),
        ("rewards_claimables", ["not-hex"]),
        ("rewards_claimables", ["0x" + ("00" * 19)]),
        ("rewards_claimables", ["0x" + ("00" * 21)]),
        ("rewards_claimables", [ETH_ZERO_ADDRESS, "0x00"]),
        ("vestedRewards_crowdloanClaimable", []),
        ("vestedRewards_crowdloanClaimable", [[], ALICE_SS58, PSWAP_ASSET_ID]),
        ("vestedRewards_crowdloanClaimable", ["x" * 129, ALICE_SS58, PSWAP_ASSET_ID]),
        ("vestedRewards_crowdloanClaimable", [UNKNOWN_CROWDLOAN_TAG, "not-an-account", PSWAP_ASSET_ID]),
        ("vestedRewards_crowdloanClaimable", [UNKNOWN_CROWDLOAN_TAG, "0x" + ("00" * 32), PSWAP_ASSET_ID]),
        ("vestedRewards_crowdloanClaimable", [UNKNOWN_CROWDLOAN_TAG, ALICE_SS58, "0x00"]),
        ("vestedRewards_crowdloanClaimable", [UNKNOWN_CROWDLOAN_TAG, ALICE_SS58, "not-an-asset"]),
        ("vestedRewards_crowdloanClaimable", [UNKNOWN_CROWDLOAN_TAG, ALICE_SS58, PSWAP_ASSET_ID, "0x00"]),
        ("vestedRewards_crowdloanLease", []),
        ("vestedRewards_crowdloanLease", [[]]),
        ("vestedRewards_crowdloanLease", ["x" * 129]),
        ("vestedRewards_crowdloanLease", [UNKNOWN_CROWDLOAN_TAG, "0x00"]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def expect_rpc_error_containing(
    node: Node, method: str, params: Optional[List[object]], text: str
) -> Dict[str, object]:
    response = expect_rpc_error(node, method, params)
    assert text in str(response["error"]), (
        f"{method} error should contain {text!r}: {response}"
    )
    return response


def assert_reward_claim_extrinsics_rejected(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        for label, extrinsic in reward_claim_extrinsics().items():
            expect_rpc_error_containing(
                node,
                "author_submitExtrinsic",
                [extrinsic],
                "NoUnsignedValidator",
            )


def assert_reward_claim_fee_queries_decode(ctx: IntegrationContext) -> None:
    for label, extrinsic in reward_claim_extrinsics().items():
        result = rpc_call(ctx.nodes[0].rpc_url, "payment_queryInfo", [extrinsic])
        assert isinstance(result, dict), f"{label} fee query returned malformed result: {result}"
        weight = result.get("weight")
        assert isinstance(weight, dict), f"{label} fee query returned malformed weight: {result}"
        assert result.get("class") == "normal", f"{label} fee query has bad class: {result}"
        assert isinstance(result.get("partialFee"), str), (
            f"{label} fee query has bad partialFee: {result}"
        )


def assert_reward_adversarial_batch(ctx: IntegrationContext) -> None:
    extrinsics = list(reward_claim_extrinsics().values())
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    payload = []
    valid_ids = set()
    for index in range(42):
        if index % 7 == 0:
            method, params = "rewards_claimables", [ETH_ZERO_ADDRESS]
            valid_ids.add(index)
        elif index % 7 == 1:
            method, params = "rewards_claimables", [ETH_SAMPLE_ADDRESS, finalized_hash]
            valid_ids.add(index)
        elif index % 7 == 2:
            method, params = "vestedRewards_crowdloanClaimable", [
                UNKNOWN_CROWDLOAN_TAG,
                ALICE_SS58,
                PSWAP_ASSET_ID,
            ]
            valid_ids.add(index)
        elif index % 7 == 3:
            method, params = "payment_queryInfo", [extrinsics[index % len(extrinsics)]]
            valid_ids.add(index)
        elif index % 7 == 4:
            method, params = "rewards_claimables", ["0x00"]
        elif index % 7 == 5:
            method, params = "vestedRewards_crowdloanClaimable", [
                UNKNOWN_CROWDLOAN_TAG,
                "not-an-account",
                PSWAP_ASSET_ID,
            ]
        else:
            method, params = "author_submitExtrinsic", [extrinsics[index % len(extrinsics)]]
        payload.append({"jsonrpc": "2.0", "id": index, "method": method, "params": params})

    status, body = rpc_post_json(ctx.nodes[0].rpc_url, payload)
    assert status == 200, f"reward adversarial batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == len(payload), (
        f"reward adversarial batch returned malformed body: {body}"
    )
    successes = [item for item in body if isinstance(item, dict) and "result" in item]
    errors = [item for item in body if isinstance(item, dict) and "error" in item]
    assert len(successes) == len(valid_ids), (
        f"reward batch success count mismatch: successes={successes}, body={body}"
    )
    assert len(errors) == len(payload) - len(valid_ids), (
        f"reward batch error count mismatch: errors={errors}, body={body}"
    )
    for item in successes:
        assert item.get("id") in valid_ids, f"unexpected successful reward batch item: {item}"


def assert_large_reward_inputs_contained(ctx: IntegrationContext) -> None:
    oversized_signature_call = bytes([8, 0]) + scale_compact_u32(4096) + bytes(4096)
    oversized_signature_extrinsic = unsigned_extrinsic(oversized_signature_call)
    expect_rpc_error_containing(
        ctx.nodes[0],
        "author_submitExtrinsic",
        [oversized_signature_extrinsic],
        "NoUnsignedValidator",
    )

    invalid_address_batch = [
        {
            "jsonrpc": "2.0",
            "id": index,
            "method": "rewards_claimables",
            "params": ["0x" + f"{index:02x}" * 19],
        }
        for index in range(32)
    ]
    status, body = rpc_post_json(ctx.nodes[0].rpc_url, invalid_address_batch)
    assert status == 200, f"large invalid reward batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == len(invalid_address_batch), (
        f"large invalid reward batch returned malformed body: {body}"
    )
    assert all(isinstance(item, dict) and "error" in item for item in body), (
        f"large invalid reward batch should only contain errors: {body}"
    )


def run_assets_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "assets",
        [
            ("asset and market rpc methods are exposed", lambda: assert_asset_market_rpc_methods_exposed(ctx)),
            ("asset ids and infos are deterministic", lambda: assert_asset_reads_deterministic(ctx)),
            ("dex and trading-pair reads are deterministic", lambda: assert_market_reads_deterministic(ctx)),
            ("mock market state profile matches", lambda: assert_mock_market_state_profile(ctx)),
            ("historical asset and market reads work", lambda: assert_asset_market_historical_reads(ctx)),
            ("invalid asset and market parameters are rejected", lambda: assert_asset_market_invalid_params_rejected(ctx)),
            ("network remains healthy after asset tests", lambda: assert_network_healthy(ctx)),
        ],
    )


def assert_asset_market_rpc_methods_exposed(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        methods_response = rpc_call(node.rpc_url, "rpc_methods")
        assert isinstance(methods_response, dict), (
            f"{node.name} returned malformed rpc_methods: {methods_response}"
        )
        methods = methods_response.get("methods")
        assert isinstance(methods, list), f"{node.name} returned malformed methods: {methods}"
        missing = [method for method in ASSET_MARKET_RPC_METHODS if method not in methods]
        assert not missing, f"{node.name} is missing asset/market RPC methods: {missing}"


def assert_asset_reads_deterministic(ctx: IntegrationContext) -> None:
    ids_by_node = [asset_ids(node) for node in ctx.nodes]
    first_ids = ids_by_node[0]
    assert XOR_ASSET_ID in first_ids, f"XOR asset is missing from asset ids: {first_ids}"
    assert PSWAP_ASSET_ID in first_ids, f"PSWAP asset is missing from asset ids: {first_ids}"
    for ids in ids_by_node[1:]:
        assert ids == first_ids, f"nodes disagree on asset ids: {ids_by_node}"

    infos_by_node = [asset_infos(node) for node in ctx.nodes]
    first_infos = infos_by_node[0]
    assert len(first_infos) >= len(first_ids), f"asset infos are unexpectedly short: {first_infos}"
    for infos in infos_by_node[1:]:
        assert infos == first_infos, "nodes disagree on asset infos"

    xor_info = asset_info(ctx.nodes[0], XOR_ASSET_ID)
    assert_asset_info_shape(xor_info, XOR_ASSET_ID)
    assert string_field(xor_info, "symbol") == "XOR", f"unexpected XOR symbol: {xor_info}"
    assert_balance_info(asset_total_supply(ctx.nodes[0], XOR_ASSET_ID), "XOR total supply")


def assert_market_reads_deterministic(ctx: IntegrationContext) -> None:
    ids_by_node = [dex_ids(node) for node in ctx.nodes]
    first_ids = ids_by_node[0]
    assert 0 in first_ids, f"Polkaswap DEX id is missing: {first_ids}"
    for ids in ids_by_node[1:]:
        assert ids == first_ids, f"nodes disagree on DEX ids: {ids_by_node}"

    pairs_by_node = [trading_pairs(node, 0) for node in ctx.nodes]
    first_pairs = pairs_by_node[0]
    assert first_pairs, "default DEX should expose genesis trading pairs"
    for pairs in pairs_by_node[1:]:
        assert pairs == first_pairs, f"nodes disagree on default trading pairs: {pairs_by_node}"

    assert trading_pair_enabled(ctx.nodes[0], 0, XOR_ASSET_ID, DAI_ASSET_ID), (
        "XOR/DAI should be enabled on the default DEX"
    )
    assert not trading_pair_enabled(ctx.nodes[0], 9999, XOR_ASSET_ID, DAI_ASSET_ID), (
        "unknown DEX should not report an enabled pair"
    )


def assert_mock_market_state_profile(ctx: IntegrationContext) -> None:
    if not market_mock_enabled(ctx.args.mock_state):
        return

    ids = set(asset_ids(ctx.nodes[0]))
    expected_ids = set(MARKET_MOCK_ASSET_INFOS)
    assert expected_ids.issubset(ids), (
        f"mock market assets missing from asset id list: expected={expected_ids}, actual={ids}"
    )

    for asset_id, expected in MARKET_MOCK_ASSET_INFOS.items():
        info = asset_info(ctx.nodes[0], asset_id)
        assert_asset_info_shape(info, asset_id)
        assert string_field(info, "symbol") == expected["symbol"], (
            f"mock asset symbol mismatch for {asset_id}: {info}"
        )
        assert string_field(info, "name") == expected["name"], (
            f"mock asset name mismatch for {asset_id}: {info}"
        )
        assert str(field(info, "precision")) == expected["precision"], (
            f"mock asset precision mismatch for {asset_id}: {info}"
        )
        assert bool_field(info, "is_mintable") == expected["is_mintable"], (
            f"mock asset mintability mismatch for {asset_id}: {info}"
        )
        expected_supply = int(expected["total_supply"])
        assert (
            balance_value(asset_total_supply(ctx.nodes[0], asset_id), f"{asset_id} total supply")
            == expected_supply
        )
        assert (
            balance_value(
                asset_free_balance(ctx.nodes[0], ALICE_SS58, asset_id),
                f"{asset_id} Alice balance",
            )
            == expected_supply
        )

    dex_ids_by_node = [dex_ids(node) for node in ctx.nodes]
    for ids_for_node in dex_ids_by_node:
        assert MARKET_MOCK_DEX_ID in ids_for_node, (
            f"mock DEX id missing from DEX list: {dex_ids_by_node}"
        )

    expected_pairs = {
        (MARKET_MOCK_LOW_PRECISION_ASSET_ID, MARKET_MOCK_HIGH_PRECISION_ASSET_ID),
        (MARKET_MOCK_HIGH_PRECISION_ASSET_ID, MARKET_MOCK_LOW_PRECISION_ASSET_ID),
        (MARKET_MOCK_LOW_PRECISION_ASSET_ID, MARKET_MOCK_LOW_PRECISION_ASSET_ID),
    }
    pairs_by_node = [trading_pairs(node, MARKET_MOCK_DEX_ID) for node in ctx.nodes]
    first_pairs = pairs_by_node[0]
    for pairs in pairs_by_node[1:]:
        assert pairs == first_pairs, f"nodes disagree on mock DEX trading pairs: {pairs_by_node}"
    actual_pairs = {trading_pair_tuple(pair) for pair in first_pairs}
    assert expected_pairs == actual_pairs, (
        f"mock DEX trading pairs mismatch: expected={expected_pairs}, actual={actual_pairs}"
    )

    assert trading_pair_enabled(
        ctx.nodes[0],
        MARKET_MOCK_DEX_ID,
        MARKET_MOCK_LOW_PRECISION_ASSET_ID,
        MARKET_MOCK_HIGH_PRECISION_ASSET_ID,
    )
    assert trading_pair_enabled(
        ctx.nodes[0],
        MARKET_MOCK_DEX_ID,
        MARKET_MOCK_LOW_PRECISION_ASSET_ID,
        MARKET_MOCK_LOW_PRECISION_ASSET_ID,
    ), "identical-asset pair should expose the adversarial seeded state"
    assert not trading_pair_source_enabled(
        ctx.nodes[0],
        MARKET_MOCK_DEX_ID,
        MARKET_MOCK_LOW_PRECISION_ASSET_ID,
        MARKET_MOCK_HIGH_PRECISION_ASSET_ID,
        "XYKPool",
    )
    assert trading_pair_sources(
        ctx.nodes[0],
        MARKET_MOCK_DEX_ID,
        MARKET_MOCK_LOW_PRECISION_ASSET_ID,
        MARKET_MOCK_HIGH_PRECISION_ASSET_ID,
    ) == []


def assert_asset_market_historical_reads(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")
    assert asset_ids(ctx.nodes[0], finalized_hash) == asset_ids(ctx.nodes[0])
    assert dex_ids(ctx.nodes[0], finalized_hash) == dex_ids(ctx.nodes[0])
    assert trading_pairs(ctx.nodes[0], 0, finalized_hash) == trading_pairs(ctx.nodes[0], 0)
    if market_mock_enabled(ctx.args.mock_state):
        assert asset_info(ctx.nodes[0], MARKET_MOCK_HIGH_PRECISION_ASSET_ID, finalized_hash) == asset_info(
            ctx.nodes[0], MARKET_MOCK_HIGH_PRECISION_ASSET_ID
        )
        assert trading_pairs(ctx.nodes[0], MARKET_MOCK_DEX_ID, finalized_hash) == trading_pairs(
            ctx.nodes[0], MARKET_MOCK_DEX_ID
        )


def assert_asset_market_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls: Sequence[Tuple[str, List[object]]] = [
        ("assets_getAssetInfo", []),
        ("assets_getAssetInfo", ["not-an-asset"]),
        ("assets_totalSupply", ["0x00"]),
        ("assets_freeBalance", ["not-an-account", XOR_ASSET_ID]),
        ("assets_freeBalance", [ALICE_SS58, "not-an-asset"]),
        ("dexManager_listDEXIds", [123]),
        ("tradingPair_listEnabledPairs", []),
        ("tradingPair_listEnabledPairs", ["not-a-dex"]),
        ("tradingPair_isPairEnabled", [0, "0x00", XOR_ASSET_ID]),
        ("tradingPair_isSourceEnabledForPair", [0, XOR_ASSET_ID, DAI_ASSET_ID, "NoSuchSource"]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def asset_ids(node: Node, at: Optional[str] = None) -> List[str]:
    params = [at] if at is not None else []
    result = rpc_call(node.rpc_url, "assets_listAssetIds", params)
    assert isinstance(result, list), f"asset ids should be a list: {result}"
    return [assert_hex_string(asset_id, 32, "asset id") for asset_id in result]


def asset_infos(node: Node, at: Optional[str] = None) -> List[object]:
    params = [at] if at is not None else []
    result = rpc_call(node.rpc_url, "assets_listAssetInfos", params)
    assert isinstance(result, list), f"asset infos should be a list: {result}"
    for item in result:
        assert isinstance(item, dict), f"asset info has bad shape: {item}"
    return result


def asset_info(node: Node, asset_id: str, at: Optional[str] = None) -> Dict[str, object]:
    params: List[object] = [asset_id]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "assets_getAssetInfo", params)
    assert isinstance(result, dict), f"asset info should be an object: {result}"
    return result


def asset_total_supply(node: Node, asset_id: str, at: Optional[str] = None) -> object:
    params: List[object] = [asset_id]
    if at is not None:
        params.append(at)
    return rpc_call(node.rpc_url, "assets_totalSupply", params)


def asset_free_balance(node: Node, account_id: str, asset_id: str, at: Optional[str] = None) -> object:
    params: List[object] = [account_id, asset_id]
    if at is not None:
        params.append(at)
    return rpc_call(node.rpc_url, "assets_freeBalance", params)


def dex_ids(node: Node, at: Optional[str] = None) -> List[int]:
    params = [at] if at is not None else []
    result = rpc_call(node.rpc_url, "dexManager_listDEXIds", params)
    assert isinstance(result, list), f"DEX ids should be a list: {result}"
    assert all(isinstance(dex_id, int) for dex_id in result), f"bad DEX ids: {result}"
    return result


def trading_pairs(node: Node, dex_id: int, at: Optional[str] = None) -> List[object]:
    params: List[object] = [dex_id]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "tradingPair_listEnabledPairs", params)
    assert isinstance(result, list), f"trading pairs should be a list: {result}"
    for pair in result:
        trading_pair_tuple(pair)
    return result


def trading_pair_enabled(
    node: Node, dex_id: int, base_asset_id: str, target_asset_id: str, at: Optional[str] = None
) -> bool:
    params: List[object] = [dex_id, base_asset_id, target_asset_id]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "tradingPair_isPairEnabled", params)
    assert isinstance(result, bool), f"isPairEnabled should return bool: {result}"
    return result


def trading_pair_sources(
    node: Node, dex_id: int, base_asset_id: str, target_asset_id: str, at: Optional[str] = None
) -> List[object]:
    params: List[object] = [dex_id, base_asset_id, target_asset_id]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "tradingPair_listEnabledSourcesForPair", params)
    assert isinstance(result, list), f"enabled sources should be a list: {result}"
    return result


def trading_pair_source_enabled(
    node: Node,
    dex_id: int,
    base_asset_id: str,
    target_asset_id: str,
    source_type: str,
    at: Optional[str] = None,
) -> bool:
    params: List[object] = [dex_id, base_asset_id, target_asset_id, source_type]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "tradingPair_isSourceEnabledForPair", params)
    assert isinstance(result, bool), f"isSourceEnabledForPair should return bool: {result}"
    return result


def assert_asset_info_shape(info: Dict[str, object], asset_id: str) -> None:
    assert field(info, "asset_id") == asset_id or field(info, "assetId") == asset_id
    assert_hex_string(str(field(info, "asset_id")), 32, "asset info asset id")
    assert isinstance(string_field(info, "symbol"), str)
    assert isinstance(string_field(info, "name"), str)
    int(str(field(info, "precision")))
    bool_field(info, "is_mintable")


def field(value: Dict[str, object], name: str) -> object:
    if name in value:
        return value[name]
    parts = name.split("_")
    camel = parts[0] + "".join(part.title() for part in parts[1:])
    if camel in value:
        return value[camel]
    raise AssertionError(f"missing field {name}/{camel}: {value}")


def string_field(value: Dict[str, object], name: str) -> str:
    item = field(value, name)
    assert isinstance(item, str), f"{name} should be string: {value}"
    return item


def bool_field(value: Dict[str, object], name: str) -> bool:
    item = field(value, name)
    if isinstance(item, bool):
        return item
    if isinstance(item, str) and item in {"true", "false"}:
        return item == "true"
    raise AssertionError(f"{name} should be bool-like: {value}")


def balance_value(value: object, label: str) -> int:
    info = assert_balance_info(value, label)
    balance = info["balance"]
    assert isinstance(balance, str) and balance.isdigit(), f"{label} balance is bad: {value}"
    return int(balance)


def assert_balance_info(value: object, label: str) -> Dict[str, object]:
    assert isinstance(value, dict), f"{label} should be an object: {value}"
    balance = value.get("balance")
    assert isinstance(balance, str) and balance.isdigit(), f"{label} has bad balance: {value}"
    return value


def trading_pair_tuple(pair: object) -> Tuple[str, str]:
    assert isinstance(pair, dict), f"trading pair should be an object: {pair}"
    base = field(pair, "base_asset_id")
    target = field(pair, "target_asset_id")
    return (
        assert_hex_string(base, 32, "trading pair base asset id"),
        assert_hex_string(target, 32, "trading pair target asset id"),
    )


def run_oracle_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "oracle",
        [
            ("oracle and farming rpc methods are exposed", lambda: assert_oracle_rpc_methods_exposed(ctx)),
            ("farming reward doubling assets are deterministic", lambda: assert_farming_reward_doubling_assets(ctx)),
            ("oracle reads are deterministic", lambda: assert_oracle_reads_deterministic(ctx)),
            ("mock oracle state profile matches", lambda: assert_mock_oracle_state_profile(ctx)),
            ("historical oracle and farming reads work", lambda: assert_oracle_historical_reads(ctx)),
            ("invalid oracle and farming parameters are rejected", lambda: assert_oracle_invalid_params_rejected(ctx)),
            ("adversarial oracle batch is contained", lambda: assert_oracle_adversarial_batch(ctx)),
            ("network remains healthy after oracle tests", lambda: assert_network_healthy(ctx)),
        ],
    )


def assert_oracle_rpc_methods_exposed(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        methods_response = rpc_call(node.rpc_url, "rpc_methods")
        assert isinstance(methods_response, dict), (
            f"{node.name} returned malformed rpc_methods: {methods_response}"
        )
        methods = methods_response.get("methods")
        assert isinstance(methods, list), f"{node.name} returned malformed methods: {methods}"
        missing = [method for method in ORACLE_FARMING_RPC_METHODS if method not in methods]
        assert not missing, f"{node.name} is missing oracle/farming RPC methods: {missing}"


def symbol_param(symbol: str) -> str:
    return symbol


def symbol_string(value: object) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, list) and all(isinstance(item, int) for item in value):
        return bytes(value).decode("ascii")
    raise AssertionError(f"unexpected oracle symbol encoding: {value}")


def unwrap_runtime_result(value: object) -> object:
    if isinstance(value, dict):
        if "Ok" in value:
            return value["Ok"]
        if "ok" in value:
            return value["ok"]
        if "Err" in value or "err" in value:
            raise AssertionError(f"runtime result was an error: {value}")
    return value


def runtime_result_is_err(value: object) -> bool:
    return isinstance(value, dict) and ("Err" in value or "err" in value)


def oracle_quote_raw(node: Node, symbol: str, at: Optional[str] = None) -> object:
    params: List[object] = [symbol_param(symbol)]
    if at is not None:
        params.append(at)
    return rpc_call(node.rpc_url, "oracleProxy_quote", params)


def oracle_quote(node: Node, symbol: str, at: Optional[str] = None) -> object:
    return unwrap_runtime_result(oracle_quote_raw(node, symbol, at))


def oracle_enabled_symbols(node: Node, at: Optional[str] = None) -> List[Tuple[str, int]]:
    params = [at] if at is not None else []
    result = unwrap_runtime_result(rpc_call(node.rpc_url, "oracleProxy_listEnabledSymbols", params))
    assert isinstance(result, list), f"enabled oracle symbols should be a list: {result}"
    symbols = []
    for item in result:
        assert isinstance(item, list) and len(item) == 2, f"bad oracle symbol entry: {item}"
        symbols.append((symbol_string(item[0]), int(item[1])))
    return symbols


def farming_reward_doubling_assets(node: Node, at: Optional[str] = None) -> List[str]:
    params = [at] if at is not None else []
    result = rpc_call(node.rpc_url, "farming_rewardDoublingAssets", params)
    assert isinstance(result, list), f"reward doubling assets should be a list: {result}"
    return [assert_hex_string(asset_id, 32, "reward doubling asset id") for asset_id in result]


def rate_value(rate: object, label: str) -> int:
    assert isinstance(rate, dict), f"{label} rate should be an object: {rate}"
    value = field(rate, "value")
    if isinstance(value, int):
        return value
    assert isinstance(value, str) and value.isdigit(), f"{label} rate value is bad: {rate}"
    return int(value)


def assert_farming_reward_doubling_assets(ctx: IntegrationContext) -> None:
    assets_by_node = [farming_reward_doubling_assets(node) for node in ctx.nodes]
    first = assets_by_node[0]
    for assets in assets_by_node[1:]:
        assert assets == first, f"nodes disagree on reward doubling assets: {assets_by_node}"
    assert set(first) == FARMING_REWARD_DOUBLING_ASSETS, (
        f"unexpected farming reward doubling assets: expected={FARMING_REWARD_DOUBLING_ASSETS}, "
        f"actual={set(first)}"
    )


def assert_oracle_reads_deterministic(ctx: IntegrationContext) -> None:
    symbols_by_node = [oracle_enabled_symbols(node) for node in ctx.nodes]
    first_symbols = symbols_by_node[0]
    for symbols in symbols_by_node[1:]:
        assert symbols == first_symbols, f"nodes disagree on enabled oracle symbols: {symbols_by_node}"

    missing_quote_by_node = [oracle_quote(node, "MISSING") for node in ctx.nodes]
    assert all(result is None for result in missing_quote_by_node), (
        f"missing oracle symbol should have no rate: {missing_quote_by_node}"
    )


def assert_mock_oracle_state_profile(ctx: IntegrationContext) -> None:
    if not oracle_mock_enabled(ctx.args.mock_state):
        return

    symbols_by_node = [oracle_enabled_symbols(node) for node in ctx.nodes]
    first_symbols = symbols_by_node[0]
    for symbols in symbols_by_node[1:]:
        assert symbols == first_symbols, f"nodes disagree on mock oracle symbols: {symbols_by_node}"
    symbol_names = {symbol for symbol, _ in first_symbols}
    assert set(ORACLE_MOCK_SYMBOL_RATES).issubset(symbol_names), (
        f"mock oracle rates are missing from enabled symbols: {first_symbols}"
    )
    assert ORACLE_MOCK_FUTURE_SYMBOL in symbol_names, (
        f"future-dated mock oracle symbol is missing: {first_symbols}"
    )

    genesis_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getBlockHash", [0])
    assert_hex_string(genesis_hash, 32, "genesis hash")
    for symbol, expected in ORACLE_MOCK_SYMBOL_RATES.items():
        current_quotes = [oracle_quote_raw(node, symbol) for node in ctx.nodes]
        assert all(runtime_result_is_err(quote) for quote in current_quotes), (
            f"current mock oracle {symbol} should be stale: {current_quotes}"
        )
        quotes = [oracle_quote(node, symbol, genesis_hash) for node in ctx.nodes]
        first_quote = quotes[0]
        for quote in quotes[1:]:
            assert quote == first_quote, (
                f"nodes disagree on genesis mock oracle quote {symbol}: {quotes}"
            )
        assert rate_value(first_quote, f"mock oracle {symbol}") == expected, (
            f"mock oracle {symbol} rate mismatch: expected={expected}, actual={first_quote}"
        )

    future_result = oracle_quote_raw(ctx.nodes[0], ORACLE_MOCK_FUTURE_SYMBOL)
    assert runtime_result_is_err(future_result), (
        f"future-dated oracle rate should return a runtime error: {future_result}"
    )


def assert_oracle_historical_reads(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")
    assert farming_reward_doubling_assets(ctx.nodes[0], finalized_hash) == farming_reward_doubling_assets(
        ctx.nodes[0]
    )
    assert oracle_enabled_symbols(ctx.nodes[0], finalized_hash) == oracle_enabled_symbols(ctx.nodes[0])
    if oracle_mock_enabled(ctx.args.mock_state):
        genesis_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getBlockHash", [0])
        assert oracle_quote(ctx.nodes[0], "USD", genesis_hash) == oracle_quote(
            ctx.nodes[0], "USD", genesis_hash
        )


def assert_oracle_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls: Sequence[Tuple[str, List[object]]] = [
        ("oracleProxy_quote", []),
        ("oracleProxy_quote", [123]),
        ("oracleProxy_quote", [None]),
        ("oracleProxy_quote", [{"symbol": "USD"}]),
        ("oracleProxy_quote", [symbol_param("USD"), "0x00"]),
        ("oracleProxy_listEnabledSymbols", [123]),
        ("oracleProxy_listEnabledSymbols", ["0x00"]),
        ("farming_rewardDoublingAssets", [123]),
        ("farming_rewardDoublingAssets", ["0x00"]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def assert_oracle_adversarial_batch(ctx: IntegrationContext) -> None:
    payload = []
    valid_ids = set()
    for index in range(60):
        request_id = index + 1
        if index % 6 == 0:
            method, params = "farming_rewardDoublingAssets", []
            valid_ids.add(request_id)
        elif index % 6 == 1:
            method, params = "oracleProxy_listEnabledSymbols", []
            valid_ids.add(request_id)
        elif index % 6 == 2:
            method, params = "oracleProxy_quote", [symbol_param("USD")]
            valid_ids.add(request_id)
        elif index % 6 == 3 and oracle_mock_enabled(ctx.args.mock_state):
            method, params = "oracleProxy_quote", [symbol_param(ORACLE_MOCK_FUTURE_SYMBOL)]
            valid_ids.add(request_id)
        elif index % 6 == 4:
            method, params = "oracleProxy_quote", [123]
        else:
            method, params = "oracleProxy_listEnabledSymbols", [123]
        payload.append({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})

    status, body = rpc_post_json(ctx.nodes[0].rpc_url, payload)
    assert status == 200, f"oracle adversarial batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == len(payload), (
        f"oracle adversarial batch returned malformed body: {body}"
    )
    successes = [item for item in body if isinstance(item, dict) and "result" in item]
    errors = [item for item in body if isinstance(item, dict) and "error" in item]
    assert len(successes) == len(valid_ids), (
        f"oracle batch success count mismatch: successes={successes}, body={body}"
    )
    assert len(errors) == len(payload) - len(valid_ids), (
        f"oracle batch error count mismatch: errors={errors}, body={body}"
    )
    for item in successes:
        assert item.get("id") in valid_ids, f"unexpected successful oracle batch item: {item}"


def run_iroha_migration_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "iroha-migration",
        [
            ("iroha migration rpc methods are exposed", lambda: assert_iroha_rpc_methods_exposed(ctx)),
            ("iroha migration reads are deterministic", lambda: assert_iroha_reads_deterministic(ctx)),
            ("mock iroha migration state profile matches", lambda: assert_mock_iroha_state_profile(ctx)),
            ("historical iroha migration reads work", lambda: assert_iroha_historical_reads(ctx)),
            ("invalid iroha migration parameters are rejected", lambda: assert_iroha_invalid_params_rejected(ctx)),
            ("unsigned iroha migration extrinsics are rejected", lambda: assert_iroha_extrinsics_rejected(ctx)),
            ("iroha migration fee queries decode", lambda: assert_iroha_fee_queries_decode(ctx)),
            ("adversarial iroha migration batch is contained", lambda: assert_iroha_adversarial_batch(ctx)),
            ("network remains healthy after iroha migration tests", lambda: assert_network_healthy(ctx)),
        ],
    )


def assert_iroha_rpc_methods_exposed(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        methods_response = rpc_call(node.rpc_url, "rpc_methods")
        assert isinstance(methods_response, dict), (
            f"{node.name} returned malformed rpc_methods: {methods_response}"
        )
        methods = methods_response.get("methods")
        assert isinstance(methods, list), f"{node.name} returned malformed methods: {methods}"
        missing = [method for method in IROHA_MIGRATION_RPC_METHODS if method not in methods]
        assert not missing, f"{node.name} is missing iroha migration RPC methods: {missing}"


def iroha_needs_migration(node: Node, address: str, at: Optional[str] = None) -> bool:
    params: List[object] = [address]
    if at is not None:
        params.append(at)
    result = rpc_call(node.rpc_url, "irohaMigration_needsMigration", params)
    assert isinstance(result, bool), f"needsMigration should return bool: {result}"
    return result


def assert_iroha_reads_deterministic(ctx: IntegrationContext) -> None:
    addresses = [IROHA_UNKNOWN_ADDRESS, "", "did_sora_mock_balance@sora"]
    for address in addresses:
        results = [iroha_needs_migration(node, address) for node in ctx.nodes]
        first = results[0]
        for result in results[1:]:
            assert result == first, f"nodes disagree on iroha migration for {address}: {results}"
    assert not iroha_needs_migration(ctx.nodes[0], IROHA_UNKNOWN_ADDRESS)
    assert not iroha_needs_migration(ctx.nodes[0], "")


def assert_mock_iroha_state_profile(ctx: IntegrationContext) -> None:
    if not iroha_mock_enabled(ctx.args.mock_state):
        return

    for address in IROHA_MOCK_ADDRESSES:
        results = [iroha_needs_migration(node, address) for node in ctx.nodes]
        assert all(results), f"mock iroha address should need migration: {address}, {results}"

    assert not iroha_needs_migration(ctx.nodes[0], IROHA_UNKNOWN_ADDRESS)
    assert not iroha_needs_migration(ctx.nodes[0], IROHA_MOCK_ADDRESSES[0].upper())


def assert_iroha_historical_reads(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    assert_hex_string(finalized_hash, 32, "finalized hash")
    addresses = [IROHA_UNKNOWN_ADDRESS]
    if iroha_mock_enabled(ctx.args.mock_state):
        addresses.extend(IROHA_MOCK_ADDRESSES)

    for address in addresses:
        current = iroha_needs_migration(ctx.nodes[0], address)
        historical = iroha_needs_migration(ctx.nodes[0], address, finalized_hash)
        assert historical == current, (
            f"historical iroha migration read diverged for {address}: "
            f"current={current}, at={historical}"
        )


def assert_iroha_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls: Sequence[Tuple[str, List[object]]] = [
        ("irohaMigration_needsMigration", []),
        ("irohaMigration_needsMigration", [123]),
        ("irohaMigration_needsMigration", [None]),
        ("irohaMigration_needsMigration", [{"address": IROHA_UNKNOWN_ADDRESS}]),
        ("irohaMigration_needsMigration", [["not", "a", "string"]]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def iroha_migration_extrinsics() -> Dict[str, str]:
    valid_shape = iroha_migration_call(
        "did_sora_mock_balance@sora",
        "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9",
        "00" * 64,
    )
    long_shape = iroha_migration_call(
        "did_sora_" + ("x" * 512) + "@sora",
        "dd54e9efb95531154316cf3e28e2232abab349296dde94353febc9ebbb3ff283",
        "11" * 64,
    )
    return {
        "iroha migration valid-shaped call": unsigned_extrinsic(valid_shape),
        "iroha migration long-shaped call": unsigned_extrinsic(long_shape),
    }


def iroha_migration_call(address: str, public_key: str, signature: str) -> bytes:
    encoded_address = address.encode()
    encoded_public_key = public_key.encode()
    encoded_signature = signature.encode()
    return (
        bytes([35, 0])
        + scale_compact_u32(len(encoded_address))
        + encoded_address
        + scale_compact_u32(len(encoded_public_key))
        + encoded_public_key
        + scale_compact_u32(len(encoded_signature))
        + encoded_signature
    )


def assert_iroha_extrinsics_rejected(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        for label, extrinsic in iroha_migration_extrinsics().items():
            expect_rpc_error_containing(
                node,
                "author_submitExtrinsic",
                [extrinsic],
                "NoUnsignedValidator",
            )


def assert_iroha_fee_queries_decode(ctx: IntegrationContext) -> None:
    for label, extrinsic in iroha_migration_extrinsics().items():
        result = rpc_call(ctx.nodes[0].rpc_url, "payment_queryInfo", [extrinsic])
        assert isinstance(result, dict), f"{label} fee query returned malformed result: {result}"
        assert isinstance(result.get("weight"), dict), f"{label} fee query has bad weight: {result}"
        assert result.get("class") == "normal", f"{label} fee query has bad class: {result}"
        assert isinstance(result.get("partialFee"), str), (
            f"{label} fee query has bad partialFee: {result}"
        )


def assert_iroha_adversarial_batch(ctx: IntegrationContext) -> None:
    finalized_hash = rpc_call(ctx.nodes[0].rpc_url, "chain_getFinalizedHead")
    payload = []
    valid_ids = set()
    addresses = [IROHA_UNKNOWN_ADDRESS, "", "did_sora_" + ("x" * 256) + "@sora"]
    if iroha_mock_enabled(ctx.args.mock_state):
        addresses.extend(IROHA_MOCK_ADDRESSES)

    for index in range(36):
        if index % 6 == 0:
            method, params = "irohaMigration_needsMigration", [addresses[index % len(addresses)]]
            valid_ids.add(index)
        elif index % 6 == 1:
            method, params = "irohaMigration_needsMigration", [
                addresses[index % len(addresses)],
                finalized_hash,
            ]
            valid_ids.add(index)
        elif index % 6 == 2:
            method, params = "payment_queryInfo", [
                list(iroha_migration_extrinsics().values())[index % 2]
            ]
            valid_ids.add(index)
        elif index % 6 == 3:
            method, params = "irohaMigration_needsMigration", [123]
        elif index % 6 == 4:
            method, params = "irohaMigration_needsMigration", [IROHA_UNKNOWN_ADDRESS, "0x00"]
        else:
            method, params = "author_submitExtrinsic", [
                list(iroha_migration_extrinsics().values())[index % 2]
            ]
        payload.append({"jsonrpc": "2.0", "id": index, "method": method, "params": params})

    status, body = rpc_post_json(ctx.nodes[0].rpc_url, payload)
    assert status == 200, f"iroha migration adversarial batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == len(payload), (
        f"iroha migration adversarial batch returned malformed body: {body}"
    )
    successes = [item for item in body if isinstance(item, dict) and "result" in item]
    errors = [item for item in body if isinstance(item, dict) and "error" in item]
    assert len(successes) == len(valid_ids), (
        f"iroha migration batch success count mismatch: successes={successes}, body={body}"
    )
    assert len(errors) == len(payload) - len(valid_ids), (
        f"iroha migration batch error count mismatch: errors={errors}, body={body}"
    )
    for item in successes:
        assert item.get("id") in valid_ids, f"unexpected successful iroha batch item: {item}"


def run_negative_rpc_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "negative-rpc",
        [
            ("unknown methods are rejected", lambda: assert_unknown_methods_rejected(ctx)),
            ("invalid parameter shapes are rejected", lambda: assert_invalid_params_rejected(ctx)),
            ("malformed extrinsics are rejected", lambda: assert_bad_extrinsics_rejected(ctx)),
            ("malformed json body is rejected", lambda: assert_malformed_json_rejected(ctx)),
            ("mixed json-rpc batch preserves errors", lambda: assert_mixed_batch_errors(ctx)),
            ("http get does not execute rpc", lambda: assert_http_get_rejected(ctx)),
        ],
    )


def assert_unknown_methods_rejected(ctx: IntegrationContext) -> None:
    methods = [
        "sora_noSuchMethod",
        "system_health\x00",
        "chain_getHeader_but_wrong",
    ]
    for node in ctx.nodes:
        for index, method in enumerate(methods):
            expect_rpc_error(node, method, request_id=index + 1)


def assert_invalid_params_rejected(ctx: IntegrationContext) -> None:
    invalid_calls = [
        ("chain_getBlockHash", ["not-a-number"]),
        ("chain_getHeader", [123, 456]),
        ("state_getStorage", ["0xzz"]),
        ("state_getKeysPaged", ["0x", "not-a-number", "0x00"]),
        ("state_call", ["NoSuchRuntimeApi_call", "0x"]),
    ]
    for node in ctx.nodes:
        for method, params in invalid_calls:
            expect_rpc_error(node, method, params)


def assert_bad_extrinsics_rejected(ctx: IntegrationContext) -> None:
    bad_extrinsics = ["", "0x", "0x00", "0xdeadbeef", "not-hex"]
    for node in ctx.nodes:
        for extrinsic in bad_extrinsics:
            expect_rpc_error(node, "author_submitExtrinsic", [extrinsic])


def assert_malformed_json_rejected(ctx: IntegrationContext) -> None:
    status, text = http_post_raw(ctx.nodes[0].rpc_url, b'{"jsonrpc":"2.0","method":')
    assert status >= 400 or "error" in text.lower(), (
        f"malformed JSON should fail, got status={status}, body={text}"
    )


def assert_mixed_batch_errors(ctx: IntegrationContext) -> None:
    status, body = rpc_post_json(
        ctx.nodes[0].rpc_url,
        [
            {"jsonrpc": "2.0", "id": 1, "method": "system_health", "params": []},
            {"jsonrpc": "2.0", "id": 2, "method": "sora_noSuchMethod", "params": []},
            {"jsonrpc": "2.0", "id": 3, "method": "author_submitExtrinsic", "params": ["not-hex"]},
        ],
    )
    assert status == 200, f"batch returned HTTP {status}: {body}"
    assert isinstance(body, list) and len(body) == 3, f"unexpected batch response: {body}"
    assert any(isinstance(item, dict) and "result" in item for item in body), (
        f"batch should contain a successful response: {body}"
    )
    assert sum(1 for item in body if isinstance(item, dict) and "error" in item) == 2, (
        f"batch should contain two errors: {body}"
    )


def assert_http_get_rejected(ctx: IntegrationContext) -> None:
    status, text = http_get_raw(ctx.nodes[0].rpc_url)
    assert status >= 400, f"GET should not execute JSON-RPC: status={status}, body={text}"


def run_adversarial_rpc_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "adversarial-rpc",
        [
            ("invalid batch barrage is contained", lambda: assert_invalid_batch_barrage(ctx)),
            ("bad extrinsic barrage is contained", lambda: assert_bad_extrinsic_barrage(ctx)),
            ("large invalid extrinsic is contained", lambda: assert_large_bad_extrinsic(ctx)),
            ("invalid reserved peer mutations are rejected", lambda: assert_reserved_peer_errors(ctx)),
            ("network remains healthy after adversarial rpc", lambda: assert_network_healthy(ctx)),
        ],
    )


def assert_invalid_batch_barrage(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        payload = [
            {
                "jsonrpc": "2.0",
                "id": index,
                "method": f"sora_invalid_{index}",
                "params": [{"unexpected": ["shape", index]}],
            }
            for index in range(25)
        ]
        status, body = rpc_post_json(node.rpc_url, payload)
        assert status == 200, f"{node.name} invalid batch returned HTTP {status}: {body}"
        assert isinstance(body, list) and len(body) == len(payload), (
            f"{node.name} invalid batch response length mismatch: {body}"
        )
        assert all(isinstance(item, dict) and "error" in item for item in body), (
            f"{node.name} invalid batch should only contain errors: {body}"
        )


def assert_bad_extrinsic_barrage(ctx: IntegrationContext) -> None:
    for node in ctx.nodes:
        for index in range(10):
            expect_rpc_error(node, "author_submitExtrinsic", [f"0x{index:02x}deadbeef"])


def assert_large_bad_extrinsic(ctx: IntegrationContext) -> None:
    large_invalid_extrinsic = "0x" + ("00" * 65_536)
    expect_rpc_error(ctx.nodes[0], "author_submitExtrinsic", [large_invalid_extrinsic])


def assert_reserved_peer_errors(ctx: IntegrationContext) -> None:
    invalid_peer_addresses = [
        "not-a-multiaddr",
        "/ip4/127.0.0.1/tcp/not-a-port",
        "/ip4/127.0.0.1/tcp/1/p2p/not-a-peer-id",
    ]
    for address in invalid_peer_addresses:
        expect_rpc_error(ctx.nodes[0], "system_addReservedPeer", [address])
        expect_rpc_error(ctx.nodes[0], "system_removeReservedPeer", [address])


def assert_network_healthy(ctx: IntegrationContext) -> None:
    check_processes(ctx.nodes)
    numbers = [block_number(node) for node in ctx.nodes]
    assert min(numbers) >= min(ctx.block_numbers), (
        f"best block regressed after suite checks: before={ctx.block_numbers}, after={numbers}"
    )
    assert_peer_health(ctx)


def run_adversarial_network_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "adversarial-network",
        [
            ("non-authority validator joins without stalling", lambda: assert_non_authority_join(ctx)),
            ("wrong-chain peer remains isolated", lambda: assert_wrong_chain_peer_isolated(ctx)),
        ],
    )


def extra_node(ctx: IntegrationContext, flag: str, name: str, stem: str) -> Node:
    p2p_port, rpc_port, prometheus_port = reserve_ports(3)
    return Node(
        index=len(ctx.nodes),
        flag=flag,
        name=name,
        base_path=ctx.workdir / "adversarial" / stem,
        log_path=ctx.workdir / "logs" / f"{stem}.log",
        p2p_port=p2p_port,
        rpc_port=rpc_port,
        prometheus_port=prometheus_port,
    )


def assert_non_authority_join(ctx: IntegrationContext) -> None:
    if len(ctx.nodes) >= len(VALIDATORS):
        return

    flag, name = VALIDATORS[len(ctx.nodes)]
    node = extra_node(ctx, flag, f"{name}-non-authority", f"non-authority-{flag}")
    all_nodes = [*ctx.nodes, node]
    start_node(ctx.binary, ctx.chain_spec, node, ctx.bootnode, ctx.args)
    try:
        wait_for_peer_id(node, min(ctx.args.timeout, 45), all_nodes)
        wait_for_rpc([node], min(ctx.args.timeout, 45))

        def synced() -> Optional[int]:
            health = rpc_call(node.rpc_url, "system_health")
            if isinstance(health, dict) and int(health.get("peers", 0)) >= 1:
                number = block_number(node)
                if number >= min(block_number(peer) for peer in ctx.nodes):
                    return number
            return None

        wait_until("non-authority peer sync", min(ctx.args.timeout, 60), all_nodes, synced)
        assert_best_block_advances(ctx)
    finally:
        stop_nodes([node])


def assert_wrong_chain_peer_isolated(ctx: IntegrationContext) -> None:
    alien_dir = ctx.workdir / "adversarial" / "wrong-chain"
    alien_dir.mkdir(parents=True, exist_ok=True)
    alien_chain_spec = materialize_chain_spec(ctx.binary, "dev", alien_dir)
    node = extra_node(ctx, "eve", "Eve-wrong-chain", "wrong-chain-eve")
    all_nodes = [*ctx.nodes, node]
    start_node(ctx.binary, alien_chain_spec, node, ctx.bootnode, ctx.args)
    try:
        wait_for_peer_id(node, min(ctx.args.timeout, 45), all_nodes)
        wait_for_rpc([node], min(ctx.args.timeout, 45))
        time.sleep(8)
        health = rpc_call(node.rpc_url, "system_health")
        assert isinstance(health, dict), f"wrong-chain node returned bad health: {health}"
        assert int(health.get("peers", 0)) == 0, (
            f"wrong-chain node should not stay connected to integration peers: {health}"
        )
    finally:
        stop_nodes([node])


def run_negative_launch_suite(ctx: IntegrationContext) -> None:
    run_cases(
        "negative-launch",
        [
            ("missing chain spec is rejected", lambda: assert_missing_chain_spec_rejected(ctx)),
            ("unknown chain alias is rejected", lambda: assert_unknown_chain_alias_rejected(ctx)),
            ("malformed bootnode address is rejected", lambda: assert_bad_bootnode_rejected(ctx)),
            ("duplicate base path is rejected", lambda: assert_duplicate_base_path_rejected(ctx)),
        ],
    )


def run_expected_failure(cmd: List[str], label: str, timeout: int = 20) -> str:
    completed = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )
    combined = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode != 0, f"{label} unexpectedly succeeded:\n{combined}"
    return combined


def assert_missing_chain_spec_rejected(ctx: IntegrationContext) -> None:
    p2p_port, rpc_port, prometheus_port = reserve_ports(3)
    missing = ctx.workdir / "negative" / "missing-chain-spec.json"
    base_path = ctx.workdir / "negative" / "missing-chain-spec-node"
    cmd = [
        str(ctx.binary),
        "--chain",
        str(missing),
        "--base-path",
        str(base_path),
        "--alice",
        "--port",
        str(p2p_port),
        "--rpc-port",
        str(rpc_port),
        "--prometheus-port",
        str(prometheus_port),
        "--no-mdns",
        "--unsafe-force-node-key-generation",
    ]
    output = run_expected_failure(cmd, "missing chain spec")
    assert "Error opening spec file" in output or "No such file" in output, output


def assert_unknown_chain_alias_rejected(ctx: IntegrationContext) -> None:
    output = run_expected_failure(
        [str(ctx.binary), "build-spec", "--chain", "definitely-not-a-sora-chain", "--raw"],
        "unknown chain alias",
    )
    assert "Error opening spec file" in output or "No such file" in output, output


def assert_bad_bootnode_rejected(ctx: IntegrationContext) -> None:
    node = extra_node(ctx, "eve", "Eve-bad-bootnode", "bad-bootnode-eve")
    cmd = node_command(
        ctx.binary,
        ctx.chain_spec,
        node,
        "/ip4/127.0.0.1/tcp/1/p2p/not-a-peer-id",
        ctx.args,
    )
    output = run_expected_failure(cmd, "bad bootnode")
    assert "bootnodes" in output.lower() or "multiaddr" in output.lower() or "peer" in output.lower(), output


def assert_duplicate_base_path_rejected(ctx: IntegrationContext) -> None:
    p2p_port, rpc_port, prometheus_port = reserve_ports(3)
    node = Node(
        index=len(ctx.nodes),
        flag="eve",
        name="Eve-duplicate-base-path",
        base_path=ctx.nodes[0].base_path,
        log_path=ctx.workdir / "logs" / "duplicate-base-path-eve.log",
        p2p_port=p2p_port,
        rpc_port=rpc_port,
        prometheus_port=prometheus_port,
    )
    start_node(ctx.binary, ctx.chain_spec, node, ctx.bootnode, ctx.args)
    try:
        deadline = time.monotonic() + 20
        while node.process and node.process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.5)
        assert node.process is not None, "duplicate base-path node was not started"
        assert node.process.poll() is not None, (
            f"duplicate base-path node did not exit; log:\n{tail(node.log_path)}"
        )
        assert node.process.returncode != 0, (
            f"duplicate base-path node exited successfully; log:\n{tail(node.log_path)}"
        )
    finally:
        stop_nodes([node])


def external_env(
    workdir: pathlib.Path, chain_spec: pathlib.Path, nodes: List[Node]
) -> Dict[str, str]:
    env = os.environ.copy()
    rpc_urls = [node.rpc_url for node in nodes]
    ws_urls = [node.ws_url for node in nodes]
    env["SORA_INTEGRATION_WORKDIR"] = str(workdir)
    env["SORA_INTEGRATION_CHAIN_SPEC"] = str(chain_spec)
    env["SORA_INTEGRATION_RPC_URLS"] = ",".join(rpc_urls)
    env["SORA_INTEGRATION_WS_URLS"] = ",".join(ws_urls)
    env["SORA_INTEGRATION_BOOTNODE_RPC_URL"] = rpc_urls[0]
    env["SORA_INTEGRATION_BOOTNODE_WS_URL"] = ws_urls[0]
    for node in nodes:
        prefix = f"SORA_INTEGRATION_NODE{node.index}"
        env[f"{prefix}_NAME"] = node.name
        env[f"{prefix}_RPC_URL"] = node.rpc_url
        env[f"{prefix}_WS_URL"] = node.ws_url
        env[f"{prefix}_P2P_PORT"] = str(node.p2p_port)
        env[f"{prefix}_LOG"] = str(node.log_path)
    return env


def run_external_tests(
    commands: List[str], workdir: pathlib.Path, chain_spec: pathlib.Path, nodes: List[Node]
) -> None:
    if not commands:
        return

    env = external_env(workdir, chain_spec, nodes)
    for command in commands:
        print(f"Running external integration command: {command}")
        subprocess.run(command, cwd=REPO_ROOT, shell=True, env=env, check=True)


def stop_nodes(nodes: List[Node]) -> None:
    for node in nodes:
        process = node.process
        if not process or process.poll() is not None:
            continue
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            continue

    deadline = time.monotonic() + 15
    for node in nodes:
        process = node.process
        if not process:
            continue
        while process.poll() is None and time.monotonic() < deadline:
            time.sleep(0.2)
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass


def print_summary(
    nodes: List[Node],
    peer_health: List[Dict[str, object]],
    block_numbers: List[int],
    finalized_numbers: List[int],
) -> None:
    print("Integration network is healthy.")
    for node, health, best, finalized in zip(
        nodes, peer_health, block_numbers, finalized_numbers
    ):
        print(
            f"  {node.name}: rpc={node.rpc_url} p2p={node.p2p_port} "
            f"peers={health.get('peers')} best={best} finalized={finalized} "
            f"log={node.log_path}"
        )


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(line_buffering=True)

    args = parse_args()
    if args.peers < 4:
        print("error: --peers must be at least 4", file=sys.stderr)
        return 2
    if args.peers > len(VALIDATORS):
        print(
            f"error: --peers cannot exceed {len(VALIDATORS)} with built-in dev keys",
            file=sys.stderr,
        )
        return 2

    chain = args.chain or default_chain_alias(args.peers, args.mock_state)

    binary = build_binary(args.release) if args.build_binary else choose_binary(args)
    if not binary.exists():
        print(
            f"error: framenode binary not found at {binary}. "
            "Run with --build-binary or set SORA_INTEGRATION_BINARY.",
            file=sys.stderr,
        )
        return 2

    created_tempdir = args.workdir is None
    workdir = (
        pathlib.Path(tempfile.mkdtemp(prefix="sora-integration-"))
        if created_tempdir
        else args.workdir.expanduser().resolve()
    )
    workdir.mkdir(parents=True, exist_ok=True)

    nodes: List[Node] = []
    success = False
    try:
        chain_spec = materialize_chain_spec(binary, chain, workdir)
        chain_id = read_chain_id(chain_spec)
        nodes = make_nodes(workdir, args.peers)

        if args.prepare_bridge_keys or args.enable_offchain_workers:
            for node in nodes:
                prepare_bridge_keys(binary, node, chain_spec, chain_id)

        print(f"Workdir: {workdir}")
        print(f"Chain spec: {chain_spec}")

        start_node(binary, chain_spec, nodes[0], None, args)
        peer_id = wait_for_peer_id(nodes[0], args.timeout, nodes)
        bootnode = f"/ip4/127.0.0.1/tcp/{nodes[0].p2p_port}/p2p/{peer_id}"
        print(f"Bootnode: {bootnode}")

        for node in nodes[1:]:
            start_node(binary, chain_spec, node, bootnode, args)

        for node in nodes[1:]:
            wait_for_peer_id(node, args.timeout, nodes)

        wait_for_rpc(nodes, args.timeout)
        peer_health = wait_for_peers(nodes, args.timeout)
        block_numbers = wait_for_blocks(nodes, args.min_blocks, args.timeout)
        finalized_numbers = wait_for_finality(
            nodes, args.min_finalized_blocks, args.timeout
        )
        print_summary(nodes, peer_health, block_numbers, finalized_numbers)

        context = IntegrationContext(
            binary=binary,
            chain_spec=chain_spec,
            chain_id=chain_id,
            workdir=workdir,
            bootnode=bootnode,
            nodes=nodes,
            args=args,
            peer_health=peer_health,
            block_numbers=block_numbers,
            finalized_numbers=finalized_numbers,
        )
        run_builtin_suites(context, args.suite)
        run_external_tests(args.test_command, workdir, chain_spec, nodes)

        if args.hold:
            print("Holding network. Press Ctrl-C to stop it.")
            while True:
                check_processes(nodes)
                time.sleep(2)

        success = True
        return 0
    except KeyboardInterrupt:
        print("Interrupted; stopping nodes.", file=sys.stderr)
        return 130
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    finally:
        stop_nodes(nodes)
        if created_tempdir and success and not args.keep_workdir:
            shutil.rmtree(workdir, ignore_errors=True)
        else:
            print(f"Integration workdir retained at {workdir}", file=sys.stderr)


if __name__ == "__main__":
    sys.exit(main())
