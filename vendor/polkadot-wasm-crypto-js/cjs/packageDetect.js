"use strict";
const { detectPackage } = require('@polkadot/util');
const { packageInfo: wasmInfo } = require('@polkadot/wasm-crypto-wasm/packageInfo');
const { packageInfo } = require('./packageInfo.js');
detectPackage(packageInfo, null, [wasmInfo]);

