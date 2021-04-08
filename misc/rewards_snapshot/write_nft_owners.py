from nft_owners import NFT_OWNERS

with open('rewards_pswap_waifu_owners.in', 'w') as f:
    print('vec_push![', file=f)
    for owner, tokens in NFT_OWNERS.items():
        addr = owner.replace('000000000000000000000000', '').replace('0x', '')
        balance = sum(tokens.values())
        print('    (hex!("{}").into(), balance!({})),'.format(addr, balance), file=f)
    print(']', file=f)

for owner, tokens in NFT_OWNERS.items():
    addr = owner.replace('000000000000000000000000', '')
    balance = sum(tokens.values())
    if balance > 50:
        print(addr, balance * 100, tokens)
