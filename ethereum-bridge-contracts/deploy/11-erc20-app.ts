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

  let migrationApp = await deployments.get("MigrationApp")

  let scaleCodecLibrary = await deployments.get("ScaleCodec")

  await deployments.deploy("ERC20App", {
    from: deployer,
    args: [
      {
        inbound: channels.basic.inbound.address,
        outbound: channels.basic.outbound.address,
      },
      {
        inbound: channels.incentivized.inbound.address,
        outbound: channels.incentivized.outbound.address,
      },
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
      {
        inbound: channels.basic.inbound.address,
        outbound: channels.basic.outbound.address,
      },
      {
        inbound: channels.incentivized.inbound.address,
        outbound: channels.incentivized.outbound.address,
      },
      migrationApp.address
    ],
    libraries: {
      ScaleCodec: scaleCodecLibrary.address
    },
    log: true,
    autoMine: true,
  });

};
