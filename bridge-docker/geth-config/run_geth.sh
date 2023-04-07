#!/bin/sh

if [ ! -d /data/geth ]; then
  mkdir /data
  geth --datadir /data init /configs/soranet.json
  echo Network initialized
fi

geth --networkid 4224 \
     --vmdebug \
     --mine --miner.threads 1 \
     --datadir /data \
     --nodiscover --http --http.port "8545" --http.vhosts "*" --http.addr 0.0.0.0 \
     --ws --ws.port "8545" --ws.api "eth,web3,personal,net,debug,txpool" --ws.addr 0.0.0.0 --ws.origins "*" \
     --port "30303" --http.corsdomain "*" \
     --nat "any" --http.api eth,web3,personal,net,debug,txpool \
     --miner.etherbase "0x0000000000000000000000000000000000000001" \
     --allow-insecure-unlock \
     --verbosity 5
