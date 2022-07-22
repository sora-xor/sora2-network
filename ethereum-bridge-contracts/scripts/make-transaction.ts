

import { deployments, ethers } from "hardhat";
import * as hre from "hardhat";
import { log } from "console";

async function makeTransaction() {
    const accounts = await hre.getUnnamedAccounts();
    const account = accounts[0];
    const ethApp = await deployments.get("ETHApp");
    const ethContract = await ethers.getContractAt('ETHApp', ethApp.address);
    const res = await ethContract.lock("0x1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c", 0, { value: 10000000 });
    console.log(res);
    const wait = await res.wait();
    console.log(wait);
}

makeTransaction()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
