#!/bin/sh

if [ ! -f /data/deploy.json ]; then
  echo "No deploy.json found, deploying..."
  npx hardhat deploy --network docker --export /data/deploy.json
  RELAYCHAIN_ENDPOINT=ws://bridge-sora-alice:9944 npx hardhat run --network docker scripts/configure-beefy.ts
  npx hardhat etherscan-verify --network docker --solc-input
fi
