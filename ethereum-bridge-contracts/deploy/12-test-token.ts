require("dotenv").config();

import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  await deployments.deploy("TestToken", {
    from: deployer,
    args: [
      "DAI", "DAI"
    ],
    log: true,
    autoMine: true,
  });

  await deployments.deploy("TestToken", {
    from: deployer,
    args: [
      "USDT", "USDT"
    ],
    log: true,
    autoMine: true,
  });
};
