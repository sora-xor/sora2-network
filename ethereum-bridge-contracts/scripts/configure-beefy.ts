import { ApiPromise, WsProvider } from '@polkadot/api';
import * as hre from "hardhat";

const soraEndpoint = process.env.RELAYCHAIN_ENDPOINT;

async function configureBeefy() {
  const beefyDeployment = await hre.deployments.get("BeefyLightClient");
  const beefy = await hre.ethers.getContractAt("TestBeefyLightClient", beefyDeployment.address);
  console.log(`Contract address ${beefyDeployment.address}`);

  const wsProvider = new WsProvider(soraEndpoint);
  const api = await ApiPromise.create({ provider: wsProvider });

  const blockHash = await api.rpc.chain.getBlockHash(1);
  const nextAuthorities = (await (await api.at(blockHash)).query.mmrLeaf.beefyNextAuthorities()).toJSON();
  const authorities = (await (await api.at(blockHash)).query.mmrLeaf.beefyAuthorities()).toJSON();

  console.log("Configuring ValidatorRegistry with updated validators")
  console.log("Current validator set", authorities);
  console.log("Next validator set", nextAuthorities);

  let result = await beefy.reset(1, {
    length: authorities["len"],
    root: authorities["root"],
    id: authorities["id"],
  }, {
    length: nextAuthorities["len"],
    root: nextAuthorities["root"],
    id: nextAuthorities["id"],
  });
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
