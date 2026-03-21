import { defineConfig } from 'hardhat/config';

import hardhatEthers from '@nomicfoundation/hardhat-ethers';
import hardhatMocha from '@nomicfoundation/hardhat-mocha';
import { SCCP_EVM_SOLIDITY } from '../evm/shared/hardhat_solidity_config.mjs';

export default defineConfig({
  plugins: [hardhatEthers, hardhatMocha],
  solidity: SCCP_EVM_SOLIDITY,
});
