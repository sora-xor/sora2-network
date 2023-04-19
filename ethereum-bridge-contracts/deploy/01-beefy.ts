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

  let mmr = await deployments.deploy("SimplifiedMMRVerification", {
    from: deployer,
    log: true,
    autoMine: true,
  });

  await deployments.deploy("BeefyLightClient", {
    contract: isTest ? "TestBeefyLightClient" : null,
    from: deployer,
    args: [mmr.address],
    libraries: {
      Bitfield: bitFieldLibrary.address,
      ScaleCodec: scaleCodecLibrary.address,
      MerkleProof: merkleProofLibrary.address
    },
    log: true,
    autoMine: true,
  });
};
