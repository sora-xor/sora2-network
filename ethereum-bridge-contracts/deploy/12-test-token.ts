require("dotenv").config();

import { hashMessage } from "ethers/lib/utils";
import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
  ethers
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();
  let migrationApp = await deployments.get("MigrationApp");

  await deployments.deploy("DAI", {
    contract: "TestToken",
    from: deployer,
    args: [
      "DAI", "DAI"
    ],
    log: true,
    autoMine: true,
  });

  await deployments.execute(
    "DAI",
    {
      from: deployer,
      autoMine: true,
      log: true,
    },
    "mint",
    migrationApp.address,
    "1000000000000000000"
  );

  await deployments.deploy("USDT", {
    contract: "TestToken",
    from: deployer,
    args: [
      "USDT", "USDT"
    ],
    log: true,
    autoMine: true,
  });

  await deployments.execute(
    "USDT",
    {
      from: deployer,
      autoMine: true,
      log: true,
    },
    "mint",
    migrationApp.address,
    "1000000000000000000"
  );

  await deployments.execute(
    "MigrationApp",
    {
      from: deployer,
      autoMine: true,
      log: true,
      value: "10000000000000000000"
    },
    "receivePayment",
  );

  console.log("Eth balance: ", await ethers.provider.getBalance(migrationApp.address));
  console.log("DAI balance: ", await deployments.read("DAI", {}, "balanceOf", migrationApp.address));
  console.log("USDT balance: ", await deployments.read("USDT", {}, "balanceOf", migrationApp.address));
};

module.exports.tags = ["TestTokens"]