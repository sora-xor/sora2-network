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

// RUN VIA:
// yarn install
// ts-node get_market_makers_data.ts

// archive node, local is preferred if running intensive queries
const ENDPOINT = 'ws://localhost:9944/';
const MARKET_MAKERS_DISTRIBUTION_PERIOD = 432000;

function toHexString(byteArray: any) {
    var s = '';
    byteArray.forEach(function(byte: any) {
        s += ('0' + (byte & 0xFF).toString(16)).slice(-2);
    });
    return s;
}

async function getMarketMakerRecords(): Promise<void> {
    const provider = new WsProvider(ENDPOINT);
    const api = new ApiPromise(options({ provider }));
    await api.isReady;

    const blockHash = await api.rpc.chain.getBlockHash(MARKET_MAKERS_DISTRIBUTION_PERIOD);
    let queryResult = await api.query.vestedRewards.marketMakersRegistry.entriesAt(blockHash);

    let count_receiving_rewards = 0;
    let records: Array<any> = Array();

    for (var i = 0; i < queryResult.length; i++) {
        let account_id = queryResult[i][0].slice(-32);
        let ss58 = encodeAddress(account_id, 69);
        let count = (queryResult[i][1] as any).count.toString();
        let volume = (queryResult[i][1] as any).volume.toString();
        let count_num = parseInt(count, 10);
        
        if (count_num != 0) {
            let entry = {
                address: ss58.toString(),
                count,
                volume,
            };
            records.push(entry);
        }
        if (count_num >= 500) {
            count_receiving_rewards++;
        }
    }

    console.log(`ACCOUNTS WITH REWARDS ${count_receiving_rewards}`);
    fs.writeFileSync("market_makers_may_snapshot.json", JSON.stringify(records, null, 1));
}

getMarketMakerRecords().catch(console.error).finally(() => process.exit());