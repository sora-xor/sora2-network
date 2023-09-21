import { config as dotenv } from "dotenv";
import { resolve } from "path";
import "solidity-coverage"
import * as contracts from "./tasks/contracts";

dotenv({ path: resolve(__dirname, ".env") });

import "hardhat-deploy";
import "hardhat-deploy-ethers";
import "@nomicfoundation/hardhat-toolbox";
import "hardhat-gas-reporter";
import { HardhatUserConfig, task } from "hardhat/config";

const getenv = (name: string) => {
  if (name in process.env) {
    return process.env[name]
  } else {
    throw new Error(`Please set your ${name} in a .env file`);
  }
}

const ropstenPrivateKey = getenv("ROPSTEN_PRIVATE_KEY");
const gethPrivateKey = getenv("GETH_PRIVATE_KEY");
const infuraKey = getenv("INFURA_PROJECT_ID");
const etherscanKey = getenv("ETHERSCAN_API_KEY");

const config: HardhatUserConfig = {
  networks: {
    hardhat: {
      throwOnTransactionFailures: true,
      mining: {
        auto: true,
        interval: 1000,
      },
    },
    docker: {
      url: "http://bridge-geth:8545",
      chainId: 4224,
      accounts: ["21754896455c7e745e7f14d4f7782bbdf7769a0539b2fe8682fa0a2e13f37075"],
      verify: {
        etherscan: {
          apiUrl: "http://bridge-blockscout:4000",
          apiKey: "a"
        }
      }
    },
    ganache: {
      url: "http://127.0.0.1:8545",
      chainId: 1337,
      accounts: {
        mnemonic: "myth like bonus scare over problem client lizard pioneer submit female collect"
      },
    },
    geth: {
      url: "http://127.0.0.1:8545",
      chainId: 4224,
      accounts: [gethPrivateKey],
    },
    ropsten: {
      chainId: 3,
      url: `https://ropsten.infura.io/v3/${infuraKey}`,
      accounts: [ropstenPrivateKey],
    },
  },
  solidity: {
    version: "0.8.15",
    settings: {
      optimizer: {
        enabled: true,
        runs: 200,
      },
    },
  },
  paths: {
    sources: "contracts",
    deployments: '.deployments',
    tests: "test",
    cache: ".cache",
    artifacts: "artifacts"
  },
  mocha: {
    timeout: 60000
  },
  etherscan: {
    apiKey: { "mainnet": etherscanKey },
  },
  typechain: {
    outDir: './typechain',
    target: 'ethers-v6',
    dontOverrideCompile: false,
  },
  gasReporter: {
    enabled: true,
    currency: 'USD',
  },
};

task("contracts", "List of contracts").setAction(contracts.main);
task("contract-address", "Print contract address").addParam("name").setAction(contracts.printAddress);

export default config;
