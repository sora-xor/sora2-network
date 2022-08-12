import { ApiPromise, WsProvider } from '@polkadot/api';
import * as hre from "hardhat";

const soraEndpoint = process.env.RELAYCHAIN_ENDPOINT;

async function configureBeefy() {
  const beefyDeployment = await hre.deployments.get("BeefyLightClient");
  const beefy = await hre.ethers.getContractAt("TestBeefyLightClient", beefyDeployment.address);

  const inboundChannelDeployment = await hre.deployments.get("InboundChannel");
  const inboundChannel = await hre.ethers.getContractAt("TestInboundChannel", inboundChannelDeployment.address);

  const outboundChannelDeployment = await hre.deployments.get("OutboundChannel");
  const outboundChannel = await hre.ethers.getContractAt("TestOutboundChannel", outboundChannelDeployment.address);

  console.log({
    beefy: beefyDeployment.address,
    inbound: inboundChannelDeployment.address,
    outbound: outboundChannelDeployment.address,
  });

  const wsProvider = new WsProvider(soraEndpoint);
  const api = await ApiPromise.create({ provider: wsProvider });

  const blockHash = await api.rpc.chain.getBlockHash(1);
  const authorities = await (await api.at(blockHash)).query.mmrLeaf.beefyNextAuthorities();

  const root = authorities['root'].toString();
  const numValidators = authorities['len'].toString();
  const id = authorities['id'].toString();

  console.log("Reset BeefyLightClient")
  console.log({
    root, numValidators, id
  });

  let result = await beefy.reset(0, root, numValidators, id);
  console.log(result);
  console.log(await result.wait());

  result = await inboundChannel.reset();
  console.log(result);
  console.log(await result.wait());
  result = await outboundChannel.reset();
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
