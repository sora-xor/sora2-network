import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
  ethers
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  const isTest = network.name !== "mainnet";

  let scaleCodecLibrary = await deployments.get("ScaleCodec")
  let merkleProofLibrary = await deployments.get("MerkleProof")
  let beefy = await deployments.get("BeefyLightClient")

  await deployments.deploy("InboundChannel", {
    from: deployer,
    contract: isTest ? "TestInboundChannel" : null,
    args: [beefy.address],
    libraries: {
      MerkleProof: merkleProofLibrary.address,
      ScaleCodec: scaleCodecLibrary.address,
    },
    log: true,
    autoMine: true,
  });

  await deployments.deploy("OutboundChannel", {
    from: deployer,
    contract: isTest ? "TestOutboundChannel" : null,
    log: true,
    autoMine: true,
  });
};
