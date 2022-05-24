import { ApiPromise, WsProvider } from '@polkadot/api';
import * as hre from "hardhat";

const soraEndpoint = process.env.RELAYCHAIN_ENDPOINT;

async function configureBeefy() {
  const beefyDeployment = await hre.deployments.get("BeefyLightClient");

  const validatorRegistryDeployment = await hre.deployments.get("ValidatorRegistry");
  const validatorRegistry = await hre.ethers.getContractAt("ValidatorRegistry", validatorRegistryDeployment.address);
  console.log(`Contract address ${validatorRegistryDeployment.address}`);

  const wsProvider = new WsProvider(soraEndpoint);
  const api = await ApiPromise.create({ provider: wsProvider });

  const blockHash = await api.rpc.chain.getBlockHash(1);
  const authorities = await (await api.at(blockHash)).query.mmrLeaf.beefyNextAuthorities();
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
