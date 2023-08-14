#!/bin/bash

for f in ./pallets/order-book/src/benchmarking/env/*; do
  ./misc/run_with_env.sh -f $f echo "ABOBA: \$MAX_EXPIRING_ORDERS_PER_BLOCK"
done