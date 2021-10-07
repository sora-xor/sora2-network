const Migrations = artifacts.require("Migrations");
const ETTHApp = artifacts.require("ETHApp");


module.exports = function (deployer) {
  deployer.deploy(ETHApp);
};
