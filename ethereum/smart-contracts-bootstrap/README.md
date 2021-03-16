# Bootstrap service   

## API documentation
* SwaggerUI - 'http://localhost:8080/swagger-ui.html' In SwaggerUi it is also possible to make test calls through browser
* Json representation - 'http://localhost:8080/v2/api-docs' In Firefox json is more readable, so try to use it.


## Description
Service provides possibilities to automate DevOps activities.

It is usable for test environments and production. Only difference is that for test environments we 
may generate blockchain credentials using this service but for production owners of private keys 
should provide public keys for us.


## Ethereum endpoint

### Deploy smart contracts
There are function to deploy all smart contracts by one API call: 'eth/deploy/d3/smartContracts' and functions to deploy initial ethereum contracts separately.
Only owner can call contracts later.
The Sequence to call smart contract deployment functions is following:
1 - 'eth/deploy/D3/masterContract' - then take resulting smart contract address as an argument to the next function

### Generate smart contracts abi and bindings
Run `buildEthereumContractsBindings` and `buildEthereumContracts` gradle tasks.
On Windows you can use [win bash](https://sourceforge.net/projects/win-bash/)
