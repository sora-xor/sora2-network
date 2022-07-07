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

  await deployments.deploy("BasicInboundChannel", {
    from: deployer,
    contract: isTest ? "TestBasicInboundChannel" : null,
    args: [beefy.address],
    libraries: {
      MerkleProof: merkleProofLibrary.address,
      ScaleCodec: scaleCodecLibrary.address,
    },
    log: true,
    autoMine: true,
  });

  await deployments.deploy("IncentivizedInboundChannel", {
    from: deployer,
    contract: isTest ? "TestIncentivizedInboundChannel" : null,
    args: [beefy.address],
    libraries: {
      MerkleProof: merkleProofLibrary.address,
      ScaleCodec: scaleCodecLibrary.address,
    },
    log: true,
    autoMine: true,
  });

  await deployments.deploy("BasicOutboundChannel", {
    contract: isTest ? "TestBasicOutboundChannel" : null,
    from: deployer,
    log: true,
    autoMine: true,
  });

  await deployments.deploy("IncentivizedOutboundChannel", {
    from: deployer,
    contract: isTest ? "TestIncentivizedOutboundChannel" : null,
    log: true,
    autoMine: true,
  });
};
