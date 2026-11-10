"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "dev_timeTravel", {
    enumerable: true,
    get: function() {
        return dev_timeTravel;
    }
});
const _timetravel = require("../../utils/time-travel.js");
const _shared = require("../shared.js");
const dev_timeTravel = async (context, [date])=>{
    const timestamp = typeof date === 'string' ? Date.parse(date) : date;
    if (Number.isNaN(timestamp)) throw new _shared.ResponseError(1, 'Invalid date');
    await (0, _timetravel.timeTravel)(context.chain, timestamp);
    return timestamp;
};
