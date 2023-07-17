require("dotenv").config();

import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  if (!("CHANNEL_FEE" in process.env)) {
    throw "Missing CHANNEL_FEE in environment config"
  }
  const fee = process.env.CHANNEL_FEE

  let channels = {
    inbound: await deployments.get("InboundChannel"),
    outbound: await deployments.get("OutboundChannel")
  };

  let ethApp = await deployments.get("ETHApp");

  console.log("Configuring OutboundChannel")
  await deployments.execute(
    "OutboundChannel",
    {
      from: deployer,
      autoMine: true,
    },
    "initialize",
    [channels.inbound.address],
    [ethApp.address],
    fee
  );

  console.log("Configuring InboundChannel")
  await deployments.execute(
    "InboundChannel",
    {
      from: deployer,
      autoMine: true,
    },
    "initialize"
  );

  // Mark deployment to run only once
  return true;
};

module.exports.id = "configure-channels"
