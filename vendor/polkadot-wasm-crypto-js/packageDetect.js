import { detectPackage } from '@polkadot/util';
import { packageInfo as wasmInfo } from '@polkadot/wasm-crypto-wasm/packageInfo';
import { packageInfo } from './packageInfo.js';
detectPackage(packageInfo, null, [wasmInfo]);
