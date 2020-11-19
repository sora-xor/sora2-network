#!/bin/bash
set -ex

api=/usr/local/bin/polkadot-js-api
parachain=/usr/local/bin/parachain-collator

$parachain export-genesis-wasm > /opt/soraneo/genesis.wasm
$parachain export-genesis-state > /opt/soraneo/genesis.state

api_command_template="--ws $RELAYCHAIN_API_ENDPOINT --sudo --seed"
api_runtime_template="{\"scheduling\":\"Always\"} @/opt/soraneo/genesis.wasm `cat /opt/soraneo/genesis.state`"

function api_query() {
    $api \
    $api_command_template "$MNEMO_PHRASE" \
    $1 \
    $PARACHAIN_ID \
    $2
}

if [ "$API_FUNC" == "update" ]; then
    api_query "tx.registrar.deregisterPara"
    api_query "tx.registrar.registerPara" "$api_runtime_template"
elif [ "$API_FUNC" == "init" ]; then
    api_query "tx.registrar.registerPara" "$api_runtime_template"
else
    echo "Wrong API_FUNC env var!"
    exit 1
fi