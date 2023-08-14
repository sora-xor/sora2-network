#!/usr/bin/python3
from dataclasses import dataclass
from pathlib import Path
from inspect import cleandoc
import argparse


@dataclass
class BenchmarkConfig:
    max_side_price_count: int
    max_limit_orders_for_price: int
    max_opened_limit_orders_per_user: int
    max_expiring_orders_per_block: int

    def generate_env_contents(self) -> str:
        return cleandoc(
            """
            MAX_SIDE_PRICE_COUNT={}
            MAX_LIMIT_ORDERS_FOR_PRICE={}
            MAX_OPENED_LIMIT_ORDERS_PER_USER={}
            MAX_EXPIRING_ORDERS_PER_BLOCK={}
            """.format(
                self.max_side_price_count,
                self.max_limit_orders_for_price,
                self.max_opened_limit_orders_per_user,
                self.max_expiring_orders_per_block,
            )
        ) + "\n"


parser = argparse.ArgumentParser(
    prog="generate_envs",
    description="Generate environment files for running a benchmark\
        (to be used with `source ...` command, for example)",
)
parser.add_argument(
    "-d", "--directory", default="./pallets/order-book/src/benchmarking/env/"
)
args = parser.parse_args()
destination = Path(args.directory)

# name: config
configs: dict[str, BenchmarkConfig] = {}
for i in range(10, 3, -1):
    n = 2**i
    configs["2_" + str(i) + ".env"] = BenchmarkConfig(n, n, n, n)

for name in configs:
    with open(destination.joinpath(Path(name)), "w+") as f:
        f.writelines(configs[name].generate_env_contents())
