"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
function _export(target, all) {
    for(var name in all)Object.defineProperty(target, name, {
        enumerable: true,
        get: Object.getOwnPropertyDescriptor(all, name).get
    });
}
_export(exports, {
    get system_accountNextIndex () {
        return system_accountNextIndex;
    },
    get system_chain () {
        return system_chain;
    },
    get system_chainType () {
        return system_chainType;
    },
    get system_dryRun () {
        return system_dryRun;
    },
    get system_health () {
        return system_health;
    },
    get system_localListenAddresses () {
        return system_localListenAddresses;
    },
    get system_localPeerId () {
        return system_localPeerId;
    },
    get system_name () {
        return system_name;
    },
    get system_nodeRoles () {
        return system_nodeRoles;
    },
    get system_properties () {
        return system_properties;
    },
    get system_version () {
        return system_version;
    }
});
const _util = require("@polkadot/util");
const system_localPeerId = async ()=>'5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';
const system_nodeRoles = async ()=>[
        'Full'
    ];
const system_localListenAddresses = async ()=>[];
const system_chain = async (context)=>{
    return context.chain.api.getSystemChain();
};
const system_properties = async (context)=>{
    return context.chain.api.getSystemProperties();
};
const system_name = async (context)=>{
    return context.chain.api.getSystemName();
};
const system_version = async (_context)=>{
    return 'chopsticks-v1';
};
const system_chainType = async (_context)=>{
    return 'Development';
};
const system_health = async ()=>{
    return {
        peers: 0,
        isSyncing: false,
        shouldHavePeers: false
    };
};
const system_dryRun = async (context, [extrinsic, at])=>{
    const { outcome } = await context.chain.dryRunExtrinsic(extrinsic, at);
    return outcome.toHex();
};
const system_accountNextIndex = async (context, [address])=>{
    const head = context.chain.head;
    const registry = await head.registry;
    const account = registry.createType('AccountId', address);
    const result = await head.call('AccountNonceApi_account_nonce', [
        account.toHex()
    ]);
    const nonce = registry.createType('Index', (0, _util.hexToU8a)(result.result)).toNumber();
    return nonce + context.chain.txPool.pendingExtrinsicsBy(address).length;
};
