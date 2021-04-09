def parse_token_holders():
    with open('export-tokenholders-for-contract-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(3).csv') as f:
        lines = f.readlines()
    data = {}
    for line in lines[1:]:
        parts = line.split(',')
        addr = parts[0].strip('"').replace('000000000000000000000000', '')
        balance = float(parts[1].strip('"'))
        data[addr] = balance
    return data


def exclude_transfers_since_snapshot(data):
    with open('export-token-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(1).csv') as f:
        lines = f.readlines()
    for line in lines[::-1]:
        parts = line.split(',')
        block = int(parts[1].strip('"'))
        if block <= 12186814:
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
    with open('rewards_val_owners.in', 'w') as f:
        print('vec_push![', file=f)
        for addr, balance in data.items():
            addr = addr.replace('0x', '')
            if balance > 0.0037:  # Given the price of VAL being 2.7$, 1 cent is 0.0037 of VAL. All values below that are discarded
                print('    (hex!("{}").into(), balance!({:.18f})),'.format(addr, balance), file=f)
        print(']', file=f)

data = parse_token_holders()
exclude_transfers_since_snapshot(data)
include_lp(data)
remove_zeros(data)
write_to_file(data)
