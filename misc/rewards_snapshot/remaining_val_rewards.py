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

from decimal import Decimal


def parse_line(line: str):
    if not 'hex!' in line:
        return None
    parts = line.split(',')
    address = parts[0].lstrip().split('"')[1]
    try:
        vested = Decimal(parts[1].split('!(')[1].split(')')[0])
        total = Decimal(parts[2].lstrip().split('!(')[1].split(')')[0])
    except IndexError as e:
        total = Decimal(0)
    except ValueError as e:
        return None

    return {'address': address, 'vested': vested, 'total': total}


def load_data(path: str):
    with open(path) as f:
        lines = f.readlines()
    data = {}
    for line in lines[1:]:
        res = parse_line(line)
        if res is None:
            continue
        try:
            addr = res['address']
            vested = res['vested']
            total = res['total']
            data[addr] = (vested, total)
        except KeyError as e:
            continue
    return data


def create_diffs(old_data, new_data):
    res = {}
    for k, (v, t) in new_data.items():
        try:
            res[k] = t - max(v, old_data[k][0])
        except KeyError as e:
            # Key is not present in old_data
            res[k] = t - v
    return res


def write_to_file(output_dir, filename, data, to_json = True, chunk_size = None):
    def write_to_file_inner(path, data, use_abs_value = True):
        with open(path, 'w') as f:
            print('vec_push![', file=f)
            for k, v in data:
                print('    (hex!("{}").into(), balance!({:.18f})),'.format(k, v), file=f)
            print(']', file=f)

    def write_to_json(path, data, use_abs_value = True):
        with open(path, 'w') as f:
            print('[', file=f)
            for i in range(len(data)):
                k, v = data[i]
                print('  {', file=f)
                print('    "address": "{}",'.format(k), file=f)
                print('    "amount": "{}"'.format(int(v * Decimal(10**18))), file=f)
                if i == len(data) - 1:
                    print('  }', file=f)
                else:
                    print('  },', file=f)
            print(']', file=f)

    if to_json:
        writer = write_to_json
        ext = 'json'
    else:
        writer = write_to_file_inner
        ext = 'in'

    if chunk_size is not None:
        num_chunks = len(data) // chunk_size
        if num_chunks * chunk_size < len(data):
            num_chunks += 1
        if num_chunks > 1:
            data_items = list(data.items())
            for i in range(num_chunks - 1):
                path = f'{output_dir}/{filename}.{i}.{ext}'
                writer(path, data_items[i * chunk_size:(i + 1) * chunk_size])
            writer(
                f'{output_dir}/{filename}.{num_chunks - 1}.{ext}',
                data_items[(num_chunks - 1) * chunk_size:]
            )
        else:
            writer(f'{output_dir}/{filename}.{ext}', list(data.items()))
    else:
        writer(f'{output_dir}/{filename}.{ext}', list(data.items()))


if __name__ == '__main__':
    old_data = load_data('../../node/src/chain_spec/bytes/rewards_val_owners_old.in')
    new_data = load_data('../../node/src/chain_spec/bytes/rewards_val_owners.in')
    diff = create_diffs(old_data, new_data)

    write_to_file(
        '../../pallets/rewards/src/bytes', 'remaining_val_rewards', diff,
        True, None
    )
