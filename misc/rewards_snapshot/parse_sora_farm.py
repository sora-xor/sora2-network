with open('sora-farm-DB.csv') as f:
    lines = f.readlines()
    lines = lines[1:]
    with open('rewards_pswap_farm_owners.in', 'w') as f:
        print('vec_push![', file=f)
        for line in lines:
            parts = line.split(',')
            balance = parts[0]
            if balance == '0':
                continue
            addr = parts[1].rstrip().replace('000000000000000000000000', '').replace('0x', '')
            balance = 'balance!(%.18f)' % float(balance)
            print('    (hex!("{}").into(), {}),'.format(addr, balance), file=f)
        print(']', file=f)