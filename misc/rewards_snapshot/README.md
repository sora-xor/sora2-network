# XOR owners (reward: VAL)

The snapshot of XOR owners is taken at 12186814 block.

[export-tokenholders-for-contract-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(3).csv] contains balances of accounts.

[export-token-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(1).csv] contains transactions that happened after the snapshot.

[report_full](./report_full) contains addresses that participated in liquidity pools on Uniswap and Mooniswap.

[get_lp_tokens](./get_lp_tokens) is a project that determines how many XOR were put into liquidity pools.

[get_lp_tokens/output] contains the collected data.

[parse_xor_owners.py](./parse_xor_owners.py) is a script that parses [export-tokenholders-for-contract-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(3).csv], [export-token-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(1).csv] and [get_lp_tokens/output] to collect VAL owners.

[rewards_val_owners.in](./rewards_val_owners.in) contains the collected data as Rust code that is included into the genesis block.

# PSWAP farming (reward: PSWAP)

[sora-farm-DB.csv] contains the snapshot of PSWAP farming rewards.

[parse_sora_farm.py](./parse_sora_farm.py) is a script that parses [sora-farm-DB.csv].

[rewards_pswap_farm_owners.in](./rewards_pswap_farm_owners.in) contains the collected data as Rust code that is included into the genesis block.

# NFT waifus (reward: PSWAP)

[parse_eth_blocks.py](./parse_eth_blocks.py) is a script that parsed all blocks since the contract creation until the last one.

[exclude_creators.py](./exclude_creators.py) is a script that excluded accounts who created NFTs.

[nft_owners.py](./nft_owners.py) contains the collected owners.

[write_nft_owners.py] is a script that contains the owners of NFT.

[rewards_pswap_waifu_owners.in](./rewards_pswap_waifu_owners.in) contains the collected data as Rust code that is included into the genesis block.


[export-tokenholders-for-contract-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(3).csv]: ./export-tokenholders-for-contract-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(3).csv
[export-token-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(1).csv]: ./export-token-0x40FD72257597aA14C7231A7B1aaa29Fce868F677(1).csv
[get_lp_tokens/output]: ./output
[sora-farm-DB.csv]: ./sora-farm-DB.csv
[write_nft_owners.py]: ./write_nft_owners.py