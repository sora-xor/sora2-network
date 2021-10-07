import { log } from "console";

export async function main() {
    const hh = require("hardhat");
    log("Run contracts");
    const dl = await hh.deployments.all();
    for (let key in dl) {
        log(key);
        log(dl[key].address);
    }
}