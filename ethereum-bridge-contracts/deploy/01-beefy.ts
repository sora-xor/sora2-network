import { HardhatRuntimeEnvironment } from "hardhat/types";

module.exports = async ({
  deployments,
  getUnnamedAccounts,
  network,
  ethers
}: HardhatRuntimeEnvironment) => {
  let [deployer] = await getUnnamedAccounts();

  const isTest = network.name !== "mainnet";
  console.log(`Deploying to ${network.name}, using ${isTest ? "test" : "production"} contracts`);

  let scaleCodecLibrary = await deployments.get("ScaleCodec")
  let bitFieldLibrary = await deployments.get("Bitfield")
  let merkleProofLibrary = await deployments.get("MerkleProof")

  let registry = await deployments.deploy("ValidatorRegistry", {
    from: deployer,
    libraries: {
      MerkleProof: merkleProofLibrary.address
    },
    log: true,
    autoMine: true,
  });

  let mmr = await deployments.deploy("SimplifiedMMRVerification", {
    from: deployer,
    log: true,
    autoMine: true,
  });

  await deployments.deploy("BeefyLightClient", {
    contract: isTest ? "TestBeefyLightClient" : null,
    from: deployer,
    args: [registry.address, mmr.address, 0],
    libraries: {
      Bitfield: bitFieldLibrary.address,
      ScaleCodec: scaleCodecLibrary.address,
    },
    log: true,
    autoMine: true,
  });
};
