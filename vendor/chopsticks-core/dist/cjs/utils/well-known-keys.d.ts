import type { Registry } from '@polkadot/types-codec/types';
import type { HexString } from '@polkadot/util/types';
export declare const decodeWellKnownKey: (registry: Registry, key: HexString, value?: HexString | null) => {
    name: string;
    key: any[];
    value: import("@polkadot/types-codec/types").AnyJson;
} | undefined;
