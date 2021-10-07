

import { deployments, ethers } from "hardhat";
import * as hre from "hardhat";
import { log } from "console";

async function makeTransaction() {
    const accounts = await hre.getUnnamedAccounts();
    const account = accounts[0];
    const ethApp = await deployments.get("ETHApp");
    const ethContract = await ethers.getContractAt('ETHApp', ethApp.address);
    const res = await ethContract.lock("0x1212121212121212121212121212121212121212121212121212121212121212", 0, { value: 12 });
    console.log(res);
}

makeTransaction()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
