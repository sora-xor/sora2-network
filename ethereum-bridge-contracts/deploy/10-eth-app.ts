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

  let scaleCodecLibrary = await deployments.get("ScaleCodec")

  await deployments.deploy("ETHApp", {
    from: deployer,
    args: [
      channels.inbound.address,
      channels.inbound.address,
      channels.outbound.address,
    ],
    libraries: {
      ScaleCodec: scaleCodecLibrary.address
    },
    log: true,
    autoMine: true,
  });

};
