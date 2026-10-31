import type { HexString } from '@polkadot/util/types';
import type { ChainProperties } from '../../index.js';
import { type Handler } from '../shared.js';
export declare const chainSpec_v1_chainName: Handler<[], string>;
export declare const chainSpec_v1_genesisHash: Handler<[], HexString>;
export declare const chainSpec_v1_properties: Handler<[], ChainProperties>;
