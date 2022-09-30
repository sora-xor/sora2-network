#!/bin/bash -v

rm -rf .cache artifacts .deployments
npx hardhat deploy --network geth
npx hardhat run --network geth scripts/configure-beefy.ts
