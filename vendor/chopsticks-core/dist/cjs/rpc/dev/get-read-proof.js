"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_getReadProof", {
    enumerable: true,
    get: function() {
        return dev_getReadProof;
    }
});
const _readproof = require("../read-proof.js");
const dev_getReadProof = async (context, [keys, hash])=>(0, _readproof.buildReadProof)(context, keys, hash);
