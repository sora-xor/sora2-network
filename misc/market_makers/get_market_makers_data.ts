// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

import { ApiPromise } from '@polkadot/api';
import { WsProvider } from '@polkadot/rpc-provider';
import { encodeAddress } from '@polkadot/util-crypto'
import { options } from '@sora-substrate/api';
import fs from 'fs'
import BigNumber from 'bignumber.js'

// RUN VIA:
// yarn install
// ts-node get_market_makers_data.ts

// const ENDPOINT = 'wss://mof3.sora.org';
// const FILE_PREFIX = '';

const ENDPOINT = 'wss://ws.stage.sora2.soramitsu.co.jp/';
const FILE_PREFIX = 'stage_'

const MARKET_MAKERS_DISTRIBUTION_PERIOD = 432000;

async function getMarketMakerRecords(): Promise<void> {
    const provider = new WsProvider(ENDPOINT);
    const api = new ApiPromise(options({ provider }));
    await api.isReady;

    const blockHash1 = await api.rpc.chain.getBlockHash(MARKET_MAKERS_DISTRIBUTION_PERIOD);
    let queryResult1 = await api.query.vestedRewards.marketMakersRegistry.entriesAt(blockHash1);

    let count_receiving_rewards_may = 0;
    let records_may: Map<string, {count: BigNumber, volume: BigNumber}> = new Map();

    for (var i = 0; i < queryResult1.length; i++) {
        let account_id = queryResult1[i][0].slice(-32);
        let ss58 = encodeAddress(account_id, 69);
        let count = new BigNumber((queryResult1[i][1] as any).count.toString());
        let volume = new BigNumber((queryResult1[i][1] as any).volume.toString());
        if (!count.isZero()) {
            let entry = {
                count,
                volume
            };
            records_may.set(ss58.toString(), entry);
            if (entry.count.toNumber() >= 500) {
                count_receiving_rewards_may++;
            }
        }
    }

    const blockHash2 = await api.rpc.chain.getBlockHash(MARKET_MAKERS_DISTRIBUTION_PERIOD * 2);
    let queryResult2 = await api.query.vestedRewards.marketMakersRegistry.entriesAt(blockHash2);

    let count_receiving_rewards_june = 0;
    let records_june: Map<string, {count: BigNumber, volume: BigNumber}> = new Map();

    for (var i = 0; i < queryResult2.length; i++) {
        let account_id = queryResult2[i][0].slice(-32);
        let ss58 = encodeAddress(account_id, 69);
        let count = new BigNumber((queryResult2[i][1] as any).count.toString());
        let volume = new BigNumber((queryResult2[i][1] as any).volume.toString());
        if (!count.isZero()) {
            if (records_may.has(ss58.toString())) {
                const may_entry = records_may.get(ss58.toString());
                const june_count = count.minus(may_entry.count);
                const june_volume = volume.minus(may_entry.volume);
                let entry = {
                    count: june_count,
                    volume: june_volume,
                };
                if (!entry.count.isZero()) {
                    records_june.set(ss58.toString(), entry);
                    if (entry.count.toNumber() >= 500) {
                        count_receiving_rewards_june++;
                    }
                }
            } else {
                let entry = {
                    count: count,
                    volume: volume
                };
                records_june.set(ss58.toString(), entry);
                if (entry.count.toNumber() >= 500) {
                    count_receiving_rewards_june++;
                }
            }
        }
    }

    function toArray(map: Map<string, {count: BigNumber, volume: BigNumber}>) {
        return Array.from(map, ([key, value]) => ({ address: key, count: value.count.toString(), volume: value.volume.toFormat(0, null, {decimalSeparator: ''}) }));
    }

    console.log(`ACCOUNTS WITH REWARDS IN MAY ${count_receiving_rewards_may}`);
    console.log(`ACCOUNTS WITH REWARDS IN JUNE ${count_receiving_rewards_june}`);
    const array_records_may = toArray(records_may);
    const array_records_june = toArray(records_june);
    fs.writeFileSync(FILE_PREFIX + "market_makers_may_snapshot.json", JSON.stringify(array_records_may, null, 1));
    fs.writeFileSync(FILE_PREFIX + "market_makers_june_snapshot.json", JSON.stringify(array_records_june, null, 1));
}

getMarketMakerRecords().catch(console.error).finally(() => process.exit());