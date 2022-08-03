require("dotenv").config();

import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  let channels = {
    inbound: await deployments.get("InboundChannel"),
    outbound: await deployments.get("OutboundChannel")
  }

  let migrationApp = await deployments.get("MigrationApp")

  let scaleCodecLibrary = await deployments.get("ScaleCodec")

  await deployments.deploy("ERC20App", {
    from: deployer,
    args: [
      channels.inbound.address,
      channels.outbound.address,
      migrationApp.address
    ],
    libraries: {
      ScaleCodec: scaleCodecLibrary.address
    },
    log: true,
    autoMine: true,
  });

  await deployments.deploy("SidechainApp", {
    from: deployer,
    args: [
      channels.inbound.address,
      channels.outbound.address,
      migrationApp.address
    ],
    libraries: {
      ScaleCodec: scaleCodecLibrary.address
    },
    log: true,
    autoMine: true,
  });

};
