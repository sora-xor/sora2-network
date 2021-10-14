#!/bin/bash -v

rm -rf .cache artifacts .deployments
npx hardhat deploy --network ganache
