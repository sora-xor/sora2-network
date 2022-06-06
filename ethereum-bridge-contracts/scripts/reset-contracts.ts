import { ApiPromise, WsProvider } from '@polkadot/api';
import * as hre from "hardhat";

const soraEndpoint = process.env.RELAYCHAIN_ENDPOINT;

async function configureBeefy() {
  const beefyDeployment = await hre.deployments.get("BeefyLightClient");
  const beefy = await hre.ethers.getContractAt("TestBeefyLightClient", beefyDeployment.address);

  const basicInboundChannelDeployment = await hre.deployments.get("BasicInboundChannel");
  const basicInboundChannel = await hre.ethers.getContractAt("TestBasicInboundChannel", basicInboundChannelDeployment.address);

  const incentivizedInboundChannelDeployment = await hre.deployments.get("IncentivizedInboundChannel");
  const incentivizedInboundChannel = await hre.ethers.getContractAt("TestIncentivizedInboundChannel", incentivizedInboundChannelDeployment.address);

  const basicOutboundChannelDeployment = await hre.deployments.get("BasicOutboundChannel");
  const basicOutboundChannel = await hre.ethers.getContractAt("TestBasicOutboundChannel", basicOutboundChannelDeployment.address);

  const incentivizedOutboundChannelDeployment = await hre.deployments.get("IncentivizedOutboundChannel");
  const incentivizedOutboundChannel = await hre.ethers.getContractAt("TestIncentivizedOutboundChannel", incentivizedOutboundChannelDeployment.address);

  console.log({
    beefy: beefyDeployment.address,
    incentivizedInbound: incentivizedInboundChannelDeployment.address, basicInbound: basicInboundChannelDeployment.address,
    incentivizedOutbound: incentivizedOutboundChannelDeployment.address, basicOutbound: basicOutboundChannelDeployment.address
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

  console.log("Reset channels")
  result = await basicInboundChannel.reset();
  console.log(result);
  console.log(await result.wait());
  result = await incentivizedInboundChannel.reset();
  console.log(result);
  console.log(await result.wait());
  result = await basicOutboundChannel.reset();
  console.log(result);
  console.log(await result.wait());
  result = await incentivizedOutboundChannel.reset();
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
