# This file is part of the SORA network and Polkaswap app.

# Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
# SPDX-License-Identifier: BSD-4-Clause

# Redistribution and use in source and binary forms, with or without modification, 
# are permitted provided that the following conditions are met:

# Redistributions of source code must retain the above copyright notice, this list 
# of conditions and the following disclaimer.
# Redistributions in binary form must reproduce the above copyright notice, this 
# list of conditions and the following disclaimer in the documentation and/or other 
# materials provided with the distribution.
# 
# All advertising materials mentioning features or use of this software must display 
# the following acknowledgement: This product includes software developed by Polka Biome
# Ltd., SORA, and Polkaswap.
# 
# Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used 
# to endorse or promote products derived from this software without specific prior written permission.

# THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES, 
# INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR 
# A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY 
# DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, 
# BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; 
# OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, 
# STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE 
# USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

def parse_token_holders():
    with open('xor_tokenholders.csv') as f:
        lines = f.readlines()
    data = {}
    for line in lines[1:]:
        parts = line.split(',')
        addr = parts[0].strip('"').replace('000000000000000000000000', '')
        balance = float(parts[1].strip('"'))
        data[addr] = balance
    return data


def exclude_transfers_since_snapshot(data):
    with open('xor_transfers.csv') as f:
        lines = f.readlines()
    for line in lines[::-1]:
        if 'Txhash' in line:
            continue
        parts = line.split(',')
        block = int(parts[1].strip('"'))
        if block <= 12225000:
            continue
        source = parts[4].strip('"').replace('000000000000000000000000', '')
        target = parts[5].strip('"').replace('000000000000000000000000', '')
        qty = float(parts[6].strip('"\n'))
        if source not in data:
            data[source] = 0
        data[source] += qty
        if target not in data:
            data[target] = 0
        data[target] -= qty


def include_lp(data):
    with open('get_lp_tokens/output') as f:
        for line in f:
            if not line.startswith('address'):
                break
            parts = line.split(',')
            addr = parts[0].split(' ')[1]
            balance = float(parts[1].split(' ')[2])
            if addr not in data:
                data[addr] = 0
            data[addr] += balance


def remove_zeros(data):
    keys = list(data.keys())
    for key in keys:
        if data[key] == 0:
            data.pop(key)


def write_to_file(data):
    pools_addr = [
        '0x01962144d41415cca072900fe87bbe2992a99f10',
        '0x4fd3f9811224bf5a87bbaf002a345560c2d98d76',
        '0xb90d8c0c2ace705fad8ad7e447dcf3e858c20448',
        '0x215470102a05b02a3a2898f317b5382f380afc0e'
    ]
    with open('../../node/src/chain_spec/bytes/rewards_val_owners.in', 'w') as f:
        print('vec_push![', file=f)
        for addr, balance in data.items():
            if addr in pools_addr:
                continue
            addr = addr.replace('0x', '')
            # Assuming the price of XOR being $600, 1 cent is worth ~0.000017 XOR. All values below that are discarded
            if balance > 0.000017:
                balance *= 10.0 # we give 10 VAL per XOR
                print('    (hex!("{}").into(), balance!({:.18f})),'.format(addr, balance), file=f)
        print(']', file=f)


data = parse_token_holders()
exclude_transfers_since_snapshot(data)
include_lp(data)
remove_zeros(data)
write_to_file(data)
