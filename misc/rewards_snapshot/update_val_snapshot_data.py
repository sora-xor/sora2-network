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


def parse_line(line: str):
    if not 'hex!' in line:
        return None
    parts = line.split(',')
    address = parts[0].lstrip().split('"')[1]
    try:
        balance = float(parts[1].split('(')[1].split(')')[0])
    except ValueError as e:
        return None
    return {'address': address, 'balance': balance}


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
            balance = res['balance']
            data[addr] = balance
        except KeyError as e:
            continue
    return data


def create_diffs(old_data, new_data):
    positive = {}
    negative = {}
    for k, v in new_data.items():
        try:
            diff = v - old_data[k]
            if diff > 0:
                positive[k] = diff
            if diff < 0:
                negative[k] = diff
        except KeyError as e:
            # Key is not present in old_data
            positive[k] = v
    return positive, negative


def write_to_file(output_dir, filename, ext, data, chunk_size = None, use_abs_value = True):
    def write_to_file_inner(path, data, use_abs_value = True):
        with open(path, 'w') as f:
            print('vec_push![', file=f)
            for addr, balance in data:
                if balance < 0 and use_abs_value:
                    balance = -balance
                print('    (hex!("{}").into(), balance!({:.18f})),'.format(addr, balance), file=f)
            print(']', file=f)

    if chunk_size is not None:
        num_chunks = len(data) // chunk_size
        if num_chunks * chunk_size < len(data):
            num_chunks += 1
        if num_chunks > 1:
            data_items = list(data.items())
            for i in range(num_chunks - 1):
                path = f'{output_dir}/{filename}.{i}.{ext}'
                write_to_file_inner(path, data_items[i * chunk_size:(i + 1) * chunk_size])
            write_to_file_inner(
                f'{output_dir}/{filename}.{num_chunks - 1}.{ext}',
                data_items[(num_chunks - 1) * chunk_size:]
            )
        else:
            write_to_file_inner(f'{output_dir}/{filename}.{ext}', list(data.items()))
    else:
        write_to_file_inner(f'{output_dir}/{filename}.{ext}', list(data.items()))


if __name__ == '__main__':
    old_data = load_data('../../node/src/chain_spec/bytes/rewards_val_owners_old.in')
    new_data = load_data('../../node/src/chain_spec/bytes/rewards_val_owners.in')
    positive, negative = create_diffs(old_data, new_data)

    write_to_file(
        '../../pallets/rewards/src/bytes', 'val_rewards_airdrop_adjustment', 'in',
        positive, 512
    )

    # For the meantime writing all amounts paid in excess in a single file
    # Will split in chunks for the next runtime upgrade when strategic vesting is handled
    write_to_file(
        '../../pallets/rewards/src/bytes', 'val_rewards_paid_in_excess', 'in', negative
    )
