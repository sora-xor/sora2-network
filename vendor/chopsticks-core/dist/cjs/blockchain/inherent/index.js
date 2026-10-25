"use strict";
Object.defineProperty(exports, "__esModule", {
    value: true
});
Object.defineProperty(exports, "inherentProviders", {
    enumerable: true,
    get: function() {
        return inherentProviders;
    }
});
const _datarenewal = require("./data-renewal.js");
const _paraenter = require("./para-enter.js");
const _baberandomness = require("./parachain/babe-randomness.js");
const _nimbusauthorinherent = require("./parachain/nimbus-author-inherent.js");
const _validationdata = require("./parachain/validation-data.js");
const _timestamp = require("./timestamp.js");
const _transactionstorage = require("./transaction-storage.js");
const inherentProviders = [
    new _timestamp.SetTimestamp(),
    new _validationdata.SetValidationData(),
    new _paraenter.ParaInherentEnter(),
    new _nimbusauthorinherent.SetNimbusAuthorInherent(),
    new _baberandomness.SetBabeRandomness(),
    new _transactionstorage.SetTransactionStorageProof(),
    new _datarenewal.ProcessPendingRenewals()
];
