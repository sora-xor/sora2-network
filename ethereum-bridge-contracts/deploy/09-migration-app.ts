require("dotenv").config();

import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  let channels = {
    basic: {
      inbound: await deployments.get("BasicInboundChannel"),
      outbound: await deployments.get("BasicOutboundChannel")
    },
    incentivized: {
      inbound: await deployments.get("IncentivizedInboundChannel"),
      outbound: await deployments.get("IncentivizedOutboundChannel")
    }
  }

  let scaleCodecLibrary = await deployments.get("ScaleCodec")

  await deployments.deploy("MigrationApp", {
    from: deployer,
    args: [
      channels.basic.inbound.address,
      channels.basic.outbound.address,
      channels.incentivized.inbound.address,
      channels.incentivized.outbound.address,
    ],
    libraries: {
      ScaleCodec: scaleCodecLibrary.address
    },
    log: true,
    autoMine: true,
  });

};
