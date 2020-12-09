# Java wrapper for ethereum smart contracts

## Solidity compiler
You have to have solidity compiler of required version and setup the path variable for it to use mentioned commands. 
The same is for web3j command line tools
1. compile contract with ABI:
   > D:\ethTools\solidity_compiler\0.7.4\solc-windows.exe Bridge.sol --bin --abi -o ../java-wrapper/src/main/resources/contracts 
   
2. create java wrappers for contract:
   > web3j solidity generate -b Bridge.bin  -a Bridge.abi -o D:\Soramitsu\github\SoraNeo-substrate\ethereum\java-wrapper\src\main\java\net\sora\substrate -p net.sora
## Tutorial

Some beginners tutorial: https://www.youtube.com/watch?v=fzUGvU2dXxU&list=PL16WqdAj66SCOdL6XIFbke-XQg2GW_Avg&index=31
