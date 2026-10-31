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
    get environment () {
        return environment;
    },
    get environmentSchema () {
        return environmentSchema;
    }
});
const _zod = /*#__PURE__*/ _interop_require_wildcard(require("zod"));
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return {
            default: obj
        };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {
        __proto__: null
    };
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for(var key in obj){
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}
const environmentSchema = _zod.object({
    /**
   * Disable auto HRMP on setup. Default is `false`.
   */ DISABLE_AUTO_HRMP: _zod.enum([
        'true',
        'false'
    ]).default('false').transform((v)=>v === 'true'),
    /**
   * Set port for Chopsticks to listen on, default is `8000`.
   */ PORT: _zod.string().optional(),
    /**
   * Disable plugins for faster startup. Default is `false`.
   */ DISABLE_PLUGINS: _zod.enum([
        'true',
        'false'
    ]).default('false').transform((v)=>v === 'true'),
    HTTP_PROXY: _zod.string().optional(),
    http_proxy: _zod.string().optional(),
    HTTPS_PROXY: _zod.string().optional(),
    https_proxy: _zod.string().optional(),
    /**
   * Chopsticks log level, "fatal" | "error" | "warn" | "info" | "debug" | "trace".
   * Default is "info".
   */ LOG_LEVEL: _zod.enum([
        'fatal',
        'error',
        'warn',
        'info',
        'debug',
        'trace'
    ]).default('info'),
    /**
   * Don't truncate log messages, show full log output. Default is `false`.
   */ VERBOSE_LOG: _zod.enum([
        'true',
        'false'
    ]).default('false').transform((v)=>v === 'true'),
    /**
   * Don't log objects. Default is `false`.
   */ LOG_COMPACT: _zod.enum([
        'true',
        'false'
    ]).default('false').transform((v)=>v === 'true')
});
const environment = environmentSchema.parse(typeof process === 'object' ? process.env : {});
