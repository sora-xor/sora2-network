import { ApiPromise, WsProvider } from '@polkadot/api';
import * as hre from "hardhat";
import * as sora from '@sora-substrate/api';

const relaychainEndpoint = process.env.RELAYCHAIN_ENDPOINT;

async function configureBeefy() {
  const beefyDeployment = await hre.deployments.get("BeefyLightClient");

  const validatorRegistryDeployment = await hre.deployments.get("ValidatorRegistry");
  const validatorRegistry = await hre.ethers.getContractAt("ValidatorRegistry", validatorRegistryDeployment.address);
  console.log(`Contract address ${validatorRegistryDeployment.address}`);

  /*
  const relayChainProvider = new WsProvider(relaychainEndpoint);
  const relaychainAPI = await ApiPromise.create(sora.options({
    provider: relayChainProvider,
  }))

  const authorities = await relaychainAPI.query.mmrLeaf.beefyNextAuthorities()
  */
  // const authorities = {
  //   id: 1,
  //   len: 3,
  //   root: '0x42b63941ec636f52303b3c33f53349830d8a466e9456d25d22b28f4bb0ad0365'
  // };
  const authorities = {
    id: 1,
    len: 5,
    root: '0x304803fa5a91d9852caafe04b4b867a4ed27a07a5bee3d1507b4b187a68777a2'
  };
  console.log(authorities);
  const root = authorities['root'].toString();
  const numValidators = authorities['len'].toString();
  const id = authorities['id'].toString();


  console.log("Configuring ValidatorRegistry with updated validators")
  console.log({
    root, numValidators, id
  });

  let result = await validatorRegistry.update(root, numValidators, id)
  console.log(result);
  console.log(await result.wait());

  console.log("Transferring ownership of ValidatorRegistry to BeefyLightClient")
  console.log({
    beefyAddress: beefyDeployment.address,
  });

  result = await validatorRegistry.transferOwnership(beefyDeployment.address)
  console.log(result);
  console.log(await result.wait());

  return;
}

// We recommend this pattern to be able to use async/await everywhere
// and properly handle errors.
configureBeefy()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
