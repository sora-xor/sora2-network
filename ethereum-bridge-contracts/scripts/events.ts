import { deployments, ethers } from "hardhat";
import * as hre from "hardhat";
import { log } from "console";

async function watchEvents() {
    let contracts = await deployments.all();
    let addressNameMap = {}
    for (const key in contracts) {
        addressNameMap[contracts[key].address] = key;
    }
    ethers.provider.addListener({}, async (arg) => {
        if (arg.address in addressNameMap) {
            log(`Event from ${addressNameMap[arg.address]}`);
        }
        log(arg);
    });
    while (true) {
        setTimeout(() => { }, 1000);
        await ethers.provider.poll();
    }
}

watchEvents()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });
