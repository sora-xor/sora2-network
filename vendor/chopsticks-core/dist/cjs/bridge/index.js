"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "bridgeLogger", {
    enumerable: true,
    get: function() {
        return bridgeLogger;
    }
});
const _logger = require("../logger.js");
_export_star(require("./encode.js"), exports);
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
const bridgeLogger = _logger.defaultLogger.child({
    name: 'bridge'
});
