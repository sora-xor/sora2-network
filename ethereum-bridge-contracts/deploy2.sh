#!/bin/bash -v

#rm -rf .cache artifacts .deployments
npx hardhat deploy --network geth2
npx hardhat run --network geth2 scripts/configure-beefy.ts
