import { log } from "console";
import { HardhatRuntimeEnvironment } from 'hardhat/types';

export async function main() {
    const hh: HardhatRuntimeEnvironment = require("hardhat");
    log("Run contracts");
    const dl = await hh.deployments.all();
    for (let key in dl) {
        log(key);
        log(dl[key].address);
    }
}

export async function printAddress(name: string) {
    const hh: HardhatRuntimeEnvironment = require("hardhat");
    const dl = await hh.deployments.all();
    log(dl[name].address);
}