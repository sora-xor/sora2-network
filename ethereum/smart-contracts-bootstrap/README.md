# Bootstrap service   

## API documentation
* SwaggerUI - 'http://localhost:8080/swagger-ui.html' In SwaggerUi it is also possible to make test calls through browser
* Json representation - 'http://localhost:8080/v2/api-docs' In Firefox json is more readable, so try to use it.


## Description
Service provides possibilities to automate DevOps activities.

It is usable for test environments and production. The differences are:
1. Different Ethereum network
2. Different Ethereum wallet
3. Different set of peers
4. Different XOR and VAL smart contract address
5. The example of requests may be found in `test/resources/generated-requests.http`


## Ethereum endpoint

### Deploy smart contracts
There is a function for Bridge contract deployment: 'deploy/{project}/bridge'.
The Sequence to call Bridge contract deployment functions is following:
1 - 'create/wallet' - create the Ethereum wallet for deploy
2 - Send ETH for paying commission for deploy to the wallet from step 1
3 - 'deploy/{project}/bridge' - deploy bridge smart contract

Supported project is `sora2`

### Generate smart contracts abi and bindings
Run `buildEthereumContractsBindings` and `buildEthereumContracts` gradle tasks.
On Windows you can use [win bash](https://sourceforge.net/projects/win-bash/)
