#!/bin/sh

START_DIR=$(pwd)
for f in ./artifacts/*.wasm
do
    tx=$(junod tx wasm store $f --from belsy --node https://rpc.uni.junonetwork.io:443/  --chain-id uni-5 --fees 200000ujunox --gas auto --gas-adjustment 2 --yes --output json | jq -r '.txhash')
    sleep 10
    code_id=$(junod query tx $tx --node https://rpc.uni.junonetwork.io:443/  --chain-id uni-5 --output json | jq -r '.logs[0].events[1].attributes[0].value')
    echo "$f $code_id"
done
