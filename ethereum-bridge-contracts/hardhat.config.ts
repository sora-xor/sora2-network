import { config as dotenv } from "dotenv";
import { resolve } from "path";
import "solidity-coverage"
import * as contracts from "./tasks/contracts";

dotenv({ path: resolve(__dirname, ".env") });

import "@nomiclabs/hardhat-truffle5";
import "@nomiclabs/hardhat-ethers";
import "@nomiclabs/hardhat-web3";
import "@nomiclabs/hardhat-etherscan";
import "hardhat-deploy";
import { HardhatUserConfig, task } from "hardhat/config";

const getenv = (name: string) => {
  if (name in process.env) {
    return process.env[name]
  } else {
    throw new Error(`Please set your ${name} in a .env file`);
  }
}

const ropstenPrivateKey = getenv("ROPSTEN_PRIVATE_KEY");
const infuraKey = getenv("INFURA_PROJECT_ID");
const etherscanKey = getenv("ETHERSCAN_API_KEY");

const config: HardhatUserConfig = {
  networks: {
    hardhat: {
      throwOnTransactionFailures: true,
    },
    ganache: {
      url: "http://127.0.0.1:8545",
      chainId: 1337,
      accounts: {
        mnemonic: "myth like bonus scare over problem client lizard pioneer submit female collect"
      },
      gas: 6000000,
      gasPrice: 5000000000,
    },
    geth: {
      url: "http://127.0.0.1:8545",
      chainId: 4224,
      accounts: ["a78a2acb5b21d4489bff3f7d113ce826c5a2e2ce27740b2ce62e9a923ac6e910"],
    },
    geth2: {
      url: "http://127.0.0.1:8555",
      chainId: 4225,
      accounts: ["a78a2acb5b21d4489bff3f7d113ce826c5a2e2ce27740b2ce62e9a923ac6e910"],
    },
    oe: {
      url: "http://127.0.0.1:8545",
      chainId: 17,
      accounts: ["4d5db4107d237df6a3d58ee5f70ae63d73d7658d4026f2eefd2f204c81682cb7"],
    },
    ropsten: {
      chainId: 3,
      url: `https://ropsten.infura.io/v3/${infuraKey}`,
      accounts: [ropstenPrivateKey],
    }
  },
  solidity: {
    version: "0.8.13"
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
    apiKey: etherscanKey
  }
};

task("contracts", "List of contracts").setAction(contracts.main);
task("contract-address", "Print contract address").addParam("name").setAction(contracts.printAddress);

export default config;
