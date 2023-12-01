#!/usr/bin/env bash

# This script can generate different hard deriviated (HD means mnemo + path) keys, an "username" will be used as path:
# Validator ID - use `gen.sh keygen get_id "mnemo phrase" "username"` - It will provide a validator id
# HD sr25519 key - use `gen.sh keygen get_sr_key "mnemo phrase" "username"` - It will provide an HD sr25519 key
# HD sr25519 keys list - use `gen.sh sr_keygen_set "mnemo phrase" 20` - the last argument is number of required keys. It will generate list of HD sr25519 keys.
# 
# All required keys - use `gen.sh get_et "mnemo phrase" "username"` - It will provide a list of validator id, sr25519 key, ed25519 key, sr25519 pub key and ed25519 pub key.
# Get an sr25519 key - use `gen.sh get_mnemo_sr_key "mnemo phrase"` - Need to specify only a mnemonic phrase! It will provide an sr25519 key.
# Generate a random node libp2p key - use `get_p2p_key "path_to priv_key"` - Need to specify only a path where to save the private key! It will print its peer ID.

set -e
set -o pipefail

_DIR="$( cd $( dirname ${BASH_SOURCE[0]} ) > /dev/null 2>&1 && pwd )"
_REGEX="(?<=SS58 Address:\s{6})[a-zA-Z0-9]{47,48}$"
_REGEX_RAW_SEED="(?<=Secret seed:\s{8})[a-zA-Z0-9]{66}$"
_REGEX_PUB="(?<=Public key \(hex\):\s{2}0x)[a-zA-Z0-9]{64,66}$"
_REGEX_ECDSA_PUB="(?<=Public key \(hex\):\s{2}0x)[a-zA-Z0-9]{66}$"
_REGEX_SS58_PUB="(?<=Public key \(SS58\):\s{1})[a-zA-Z0-9]{49,50}$"
_COMMAND="${_DIR}/subkey"
_PD_COMMAND="docker run --rm --entrypoint bash parity/polkadot:v0.9.1 -c "
_SUBKEY_COMMAND="polkadot key inspect"
_INSPECT_KEY="${_PD_COMMAND} ${_SUBKEY_COMMAND}"

function error() {
	echo "SCRIPT ERROR: $1"
	exit 1
}
function regex_error() {
	echo "SCRIPT ERROR: Regex doesn't parse key for the \"$1\" and \"$2\" combination!!"
	exit 1
}
function get_et() {
    if [[ $1 != "" && $2 != "" ]]; then
        _ALL_SHCHEMES=`${_PD_COMMAND} "${_SUBKEY_COMMAND} \"$1//$2\"; ${_SUBKEY_COMMAND} \"$1//$2\" --scheme ed25519; ${_SUBKEY_COMMAND} \"$1//$2\" --scheme ecdsa;"`
        get_id "$1" "$2" || regex_error "$1" "$2"
        echo "$_ALL_SHCHEMES" | grep -Po "${_REGEX}|${_REGEX_PUB}|${_REGEX_SS58_PUB}" || regex_error "$1" "$2"
    else
        error "For \"keygen_list\" option you must specify a mnemo phrase and an username"
    fi
}
function get_id() {
    ${_PD_COMMAND} "${_SUBKEY_COMMAND} \"$1//$2//stash\"" | grep -Po "${_REGEX}"
}
function get_sr_key() {
    ${_PD_COMMAND} "${_SUBKEY_COMMAND} \"$1//$2\"" | grep -Po "${_REGEX}"
}
function get_mnemo_sr_key() {
    if [[ $1 != "" ]]; then
        ${_PD_COMMAND} "${_SUBKEY_COMMAND} \"$1\"" | grep -Po "${_REGEX}" || error "Regex doesn't work for your mnemonic phrase - \"$1\""
    else
        error "For \"get_mnemo_sr_key\" option you must specify a mnemo phrase"
    fi
}
function keygen() {
    if [[ $1 != "" && $2 != "" && $3 != "" ]]; then
        $1 "$2" "$3" || regex_error "$2" "$3"
    else
        error "For \"$1\" option you must specify a mnemo phrase and an username"
    fi
}
function sr_keygen_list() {
    re='^[0-9]*$'
    if [[ "$1" != "" && "$2" =~ $re ]]; then
        for i in $(seq 1 "$2");
            do get_sr_key "$1" $i || regex_error "$1" "$i";
        done
    else
        error "For \"sr_keygen_list\" option you must specify a mnemo phrase and a number of required keys"
    fi
}
function get_p2p_key() {
    if [[ $1 != "" ]]; then
        _P2P_KEYS=`${_PD_COMMAND} "polkadot key generate-node-key --file ~/key; cat ~/key" 2>&1`
        _P2P_PRIV=`echo "$_P2P_KEYS" | head -n 1`
        _P2P_PUB=`echo "$_P2P_KEYS" | tail -n 1`
        echo "    para_peer_id: \"${_P2P_PRIV}\""; ansible-vault encrypt_string --encrypt-vault-id $1 "$_P2P_PUB" --name '    para_peer_key';
    else
        error "For \"get_p2p_key\" tool you must specify a vault id!"
    fi
}
function get_playbook_file() {
    if [[ $1 != "" && $2 != "" ]]; then
        (echo \
        'sora2_nodes:
  accessnode-'$1':
    role: "access"
    type: "parachain"
    ws_max_connections: "10000"
    p2p_port: "30333"
    bind_host: "0.0.0.0"
    healthchecker: true
    monitoring_enable: true
    common_ws_hostname: "ws.sora2.soramitsu.co.jp"
    common_rpc_hostname: "rpc.sora2.soramitsu.co.jp"'
        get_p2p_key $2
        ) > playbooks/sora2/prod/group_vars/tag_Name_sora2_prod_ac$1.yml
    else
        error "For \"get_playbook_file\" tool you must specify a name of node and a vault id!"
    fi
}
function get_set_of_playbooks() {
    if [[ $1 != "" && $2 != "" && $3 != "" ]]; then
        for i in `seq $1 $2`; do get_playbook_file $i $3; done
    else
        error "For \"get_set_of_playbooks\" option you must specify the range of numbers and a vault id!"
    fi
}

test -d $_DIR || error "Can't define the script path"
test `declare -F $1` || error "There are no such option"
$1 "$2" "$3" "$4"