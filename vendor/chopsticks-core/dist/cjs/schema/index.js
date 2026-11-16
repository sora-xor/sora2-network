"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "genesisSchema", {
    enumerable: true,
    get: function() {
        return genesisSchema;
    }
});
const _zod = require("zod");
const genesisSchema = _zod.z.object({
    id: _zod.z.string(),
    name: _zod.z.string(),
    properties: _zod.z.object({
        ss58Format: _zod.z.number().optional(),
        tokenDecimals: _zod.z.union([
            _zod.z.number(),
            _zod.z.array(_zod.z.number())
        ]).optional(),
        tokenSymbol: _zod.z.union([
            _zod.z.string(),
            _zod.z.array(_zod.z.string())
        ]).optional()
    }),
    genesis: _zod.z.object({
        raw: _zod.z.object({
            top: _zod.z.record(_zod.z.string(), _zod.z.string())
        })
    })
});
