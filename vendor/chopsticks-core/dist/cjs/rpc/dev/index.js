"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "default", {
    enumerable: true,
    get: function() {
        return _default;
    }
});
const _getreadproof = _export_star(require("./get-read-proof.js"), exports);
const _newblock = _export_star(require("./new-block.js"), exports);
const _setblockbuildmode = _export_star(require("./set-block-build-mode.js"), exports);
const _sethead = _export_star(require("./set-head.js"), exports);
const _setruntimeloglevel = _export_star(require("./set-runtime-log-level.js"), exports);
const _setstorage = _export_star(require("./set-storage.js"), exports);
const _timetravel = _export_star(require("./time-travel.js"), exports);
function _export_star(from, to) {
    Object.keys(from).forEach(function(k) {
        if (k !== "default" && !Object.prototype.hasOwnProperty.call(to, k)) {
            Object.defineProperty(to, k, {
                enumerable: true,
                get: function() {
                    return from[k];
                }
            });
        }
    });
    return from;
}
const handlers = {
    dev_getReadProof: _getreadproof.dev_getReadProof,
    dev_newBlock: _newblock.dev_newBlock,
    dev_setBlockBuildMode: _setblockbuildmode.dev_setBlockBuildMode,
    dev_setHead: _sethead.dev_setHead,
    dev_setRuntimeLogLevel: _setruntimeloglevel.dev_setRuntimeLogLevel,
    dev_setStorage: _setstorage.dev_setStorage,
    dev_timeTravel: _timetravel.dev_timeTravel
};
const _default = handlers;
