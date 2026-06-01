# Etherscan Token Setup Packet: XOR

## Target

- New XOR ERC-20: `0x1D03EbBd6bE00B805702dDe42313dcF44aC1a6Eb`
- Current Etherscan page: https://etherscan.io/token/0x1d03ebbd6be00b805702dde42313dcf44ac1a6eb
- Old / legacy XOR ERC-20: `0x40FD72257597aA14C7231A7B1aaa29Fce868F677`
- Current bridge contract: `0x313416870A4da6F12505a550B67bB73c8E21D5d3`
- Bridge add-asset Ethereum tx: `0xe6f241e0ef2355c5c6906e54d6e71f6e0215bbb0d6115d8b223f71ad94ebe2fd`

## Recommended Etherscan Request

Use `Token/Contract Migration` if Etherscan allows a single request to link the old XOR page to the new XOR contract.
Otherwise submit `New/First Time Token Update` for the new contract first, then submit a separate migration/deprecation request for the old contract.

Reason:

- The old XOR contract already has SORA metadata on Etherscan.
- The new XOR contract is the active SORA bridge representation on Ethereum.
- The old contract should be treated as legacy/deprecated to avoid user confusion.

## Token Fields

- Token / project: `SORA`
- On-chain token name: `XOR`
- Symbol: `XOR`
- Decimals: `18`
- Token type: `ERC-20`
- Contract address: `0x1D03EbBd6bE00B805702dDe42313dcF44aC1a6Eb`
- Display name requested on Etherscan: `SORA (XOR)` or `XOR (XOR)` if Etherscan requires the on-chain name.
- Sector/category: `Blockchain Infrastructure`, `DeFi`, or `Cross-chain Bridge`
- Website: https://sora.org/
- Whitepaper: https://sora.org/sora_nexus_whitepaper.pdf
- Docs/wiki: https://wiki.sora.org/
- X/Twitter: https://x.com/sora_xor
- Telegram: https://t.me/sora_xor
- GitHub: https://github.com/sora-xor
- Discord: https://discord.gg/4TXRN6Y4gb
- YouTube: https://www.youtube.com/sora_xor
- Reddit: https://www.reddit.com/r/SORA/
- CoinMarketCap: https://coinmarketcap.com/currencies/sora/
- CoinGecko: https://www.coingecko.com/en/coins/sora/
- Official email: `sora@soramitsu.co.jp` if SORA ops confirms this mailbox is still monitored. The legacy Etherscan listing uses this address.

## Neutral Description

XOR is the native asset of the SORA network. This ERC-20 contract represents XOR bridged from SORA to Ethereum through the SORA HASHI bridge. SORA provides ledger infrastructure for markets, payments, governance, and cross-chain asset movement.

## Copy/Paste Form Values

- Request type: `New/First Time Token Update`, unless `Token/Contract Migration` is available for linking the legacy contract.
- Token contract address: `0x1D03EbBd6bE00B805702dDe42313dcF44aC1a6Eb`
- Token name: `XOR`
- Token symbol: `XOR`
- Token decimals: `18`
- Project name: `SORA`
- Project website: `https://sora.org/`
- Project description: `XOR is the native asset of the SORA network. This ERC-20 contract represents XOR bridged from SORA to Ethereum through the SORA HASHI bridge.`
- Official project email: `sora@soramitsu.co.jp`, after mailbox access is confirmed.
- Logo file upload: `runtime-upgrade-4.8.7/etherscan-xor-logo-32.svg`
- Logo fallback file upload: `runtime-upgrade-4.8.7/etherscan-xor-logo-64.png`
- Additional notes: `This is the active bridged Ethereum representation of SORA XOR. The previous legacy XOR ERC-20 contract is 0x40FD72257597aA14C7231A7B1aaa29Fce868F677. Please link/migrate the legacy listing or mark the legacy contract as deprecated to avoid user confusion.`

## Logo

Etherscan's current guideline asks for SVG 32x32 or PNG 64x64. Prefer the SVG because it is vector and will render more cleanly at different display sizes.

- Preferred upload: `runtime-upgrade-4.8.7/etherscan-xor-logo-32.svg`
- Preferred file type: `image/svg+xml`
- Preferred dimensions: `32 x 32`
- Preferred SHA-256: `5443f7027a1230dd4b53452cdc4bebd20e36656ab8f9208b4f45dfc1f6680a58`
- Fallback upload: `runtime-upgrade-4.8.7/etherscan-xor-logo-64.png`
- Fallback file type: `image/png`
- Fallback dimensions: `64 x 64`
- Transparent background: yes
- Fallback SHA-256: `1d37b30db018daf26b043d009b22be1e1b585ec1c83b9f68ead29dcd4ecc1ffe`
- Source PNG: `runtime-upgrade-4.8.7/sora-xor-source.png`
- Source dimensions: `200 x 200`
- Source PNG SHA-256: `e422c4cb37bc05b5aa1da687926557b0bed65ac95be2bdbad36f1c462ab52e56`
- Public SVG source: https://raw.githubusercontent.com/sora-xor/sora-branding/master/tokens/SORA/svg/XOR.svg
- Public PNG source: https://raw.githubusercontent.com/sora-xor/sora-branding/master/tokens/cmc/sora-xor.png

If Etherscan requires a public logo URL instead of a file upload, upload `etherscan-xor-logo-32.svg` to an official public SORA/SORAMITSU location first and use that URL. Use `etherscan-xor-logo-64.png` only if Etherscan rejects the SVG or asks for a raster asset.

## Ownership Verification Notes

The new token was created by the bridge contract, so normal token-address ownership verification may fail because a contract cannot sign an Etherscan ownership message.

Use Etherscan's bridged-token or contract-created-by-contract ownership flow:

- Etherscan account must be logged in.
- The token source must be accepted as verified. The current page shows `Source Code Verified Similar Match` for `MasterToken`, compiler `v0.8.17`, optimization enabled with 200 runs, license `Apache-2.0`.
- If Etherscan blocks the token update because similar-match is not enough, exact-verify the token contract first.
- If normal ownership verification fails, open an Etherscan `General Inquiry` ticket and include signed-message details.

Likely signing address for the contract-created flow:

- HASHI bridge deployer: `0xbdC3AB9165cf959dA2E8bF4aC177dbb71a938156`

Ticket message draft:

```text
Please verify ownership for the bridged SORA XOR token contract and enable token information update / migration.

New XOR ERC-20:
0x1D03EbBd6bE00B805702dDe42313dcF44aC1a6Eb

Legacy XOR ERC-20:
0x40FD72257597aA14C7231A7B1aaa29Fce868F677

SORA HASHI bridge contract:
0x313416870A4da6F12505a550B67bB73c8E21D5d3

Ethereum bridge add-asset transaction:
0xe6f241e0ef2355c5c6906e54d6e71f6e0215bbb0d6115d8b223f71ad94ebe2fd

The new XOR token contract was created by the SORA HASHI bridge as the active Ethereum representation of SORA XOR.
Please link/migrate the legacy XOR listing to the new contract or mark the legacy contract as deprecated, and allow the new contract page to use the official SORA metadata.

Signed message details:
Address: <signing address>
Message: <Etherscan-provided or agreed message>
Signature: <signature hash>
Version: EIP-191 / personal_sign
```

Suggested signed-message wording if Etherscan does not generate one:

```text
[Etherscan.io 01/06/2026 HH:mm:ss] I, <Etherscan username>, hereby verify that I am authorized by SORA to update the token information for the bridged XOR token contract address 0x1D03EbBd6bE00B805702dDe42313dcF44aC1a6Eb and to identify it as the active Ethereum representation of SORA XOR. The legacy XOR token contract is 0x40FD72257597aA14C7231A7B1aaa29Fce868F677.
```

Use the exact message Etherscan requests if they provide one.

## Submission Checklist

1. Login to Etherscan.
2. Check whether the new token contract's similar-match verification is accepted for token updates.
3. Verify/claim ownership. Use the bridged-token or contract-created-by-contract route if normal verification fails.
4. Submit one request only; Etherscan warns not to submit duplicates.
5. Use the token fields and description above.
6. Upload `etherscan-xor-logo-32.svg` or provide an official public URL for the same file. Keep `etherscan-xor-logo-64.png` as the fallback upload.
7. Request migration/deprecation wording for the old XOR contract.
