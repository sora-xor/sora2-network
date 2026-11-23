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
    get defaultLogger () {
        return defaultLogger;
    },
    get pinoLogger () {
        return pinoLogger;
    },
    get truncate () {
        return truncate;
    }
});
const _pino = require("pino");
const _env = require("./env.js");
const pinoLogger = (0, _pino.pino)({
    level: _env.environment.LOG_LEVEL,
    transport: {
        target: 'pino-pretty',
        options: {
            ignore: 'pid,hostname',
            hideObject: _env.environment.LOG_COMPACT
        }
    }
});
const defaultLogger = pinoLogger.child({
    app: 'chopsticks'
});
const innerTruncate = (level = 0)=>(val)=>{
        const verboseLog = _env.environment.VERBOSE_LOG;
        const levelLimit = verboseLog ? 10 : 5;
        if (val == null) {
            return val;
        }
        if (level > levelLimit) {
            return '( Too Deep )';
        }
        switch(typeof val){
            case 'string':
                {
                    const maxLength = verboseLog ? 10 * 1024 : 66;
                    if (val.length > maxLength) {
                        return `${val.slice(0, 34)}…${val.slice(-32)}`;
                    }
                    return val;
                }
            case 'object':
                if (Array.isArray(val)) {
                    return val.map(innerTruncate(level + 1));
                }
                return Object.fromEntries(Object.entries(val.toJSON ? val.toJSON() : val).map(([k, v])=>[
                        k,
                        innerTruncate(level + 1)(v)
                    ]));
            default:
                return val;
        }
    };
const truncate = (val)=>innerTruncate(0)(val);
