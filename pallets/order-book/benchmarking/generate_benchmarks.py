#!/usr/bin/python3
import re
from pathlib import Path

"""
Generates multiple variants of benchmarks. Intended to allow comparison of runs between different presets.

The Substrate's benchmarking framework is not suitable for this because instead of switching presets it alters each
parameter independently freezing others at max value in range.
Rust macros are not usable because the benchmarks are already within a macro; they are processed outside-in, thus
making it impossible to accomplish this without modifying Substrate's benchmarking macro.

Usage is the following: edit/add a template, run the script, paste output into `./mod.rs` in benchmarking section.
"""


def generate_fs(range_: range, template: str):
    codes = ""
    for i in range_:
        codes += template.format(i, i)
    return codes


code_template_place = """
        #[extra]
        place_limit_order_without_cross_spread_{} {{
            use periphery::place_limit_order::{{init, Context}};
            let Context {{ caller, order_book_id, price, amount, side, lifespan, .. }} =
                init::<T>(preset_{}());
        }}: {{
            OrderBookPallet::<T>::place_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }}
"""

code_template_cancel_first = """
        #[extra]
        cancel_limit_order_first_{} {{
            use periphery::cancel_limit_order::{{init, Context}};
            let Context {{ caller, order_book_id, order_id, .. }} =
                init::<T>(preset_{}::<T>(), true);
        }}: {{
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }}
"""

code_template_cancel_last = """
        #[extra]
        cancel_limit_order_last_{} {{
            use periphery::cancel_limit_order::{{init, Context}};
            let Context {{ caller, order_book_id, order_id, .. }} =
                init::<T>(preset_{}::<T>(), false);
        }}: {{
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }}
"""

code_template_execute = """
        #[extra]
        execute_market_order_{} {{
            use periphery::execute_market_order::{{init, Context}};
            let Context {{ caller, order_book_id: id, amount, side, .. }} =
                init::<T>(preset_{}::<T>());
        }}: {{
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }}
"""

code_template_quote = """
        #[extra]
        quote_{} {{
            use periphery::quote::{{init, Context}};
            let Context {{ dex_id, input_asset_id, output_asset_id, amount, deduce_fee }} =
                init::<T>(preset_{}::<T>());
        }}: {{
            OrderBookPallet::<T>::quote(&dex_id, &input_asset_id, &output_asset_id, amount, deduce_fee)
                .unwrap();
        }}
"""

code_template_exchange = """
        #[extra]
        exchange_{} {{
            let e in 1u32 .. <T as order_book_imported::Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
            use periphery::exchange_scattered::{{init, Context}};
            let mut settings = preset_1::<T>();
            settings.executed_orders_limit = e;
            let Context {{ caller, order_book_id: id, expected_in, expected_out, .. }} = init(settings.clone());
        }} : {{
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_output(expected_out, expected_in + balance!(1.5)),
            ).unwrap();
        }}
"""

launch_script_template = """#!/bin/bash

if which gawk > /dev/null 2>&1; then
    awk="gawk"
else
    awk="awk"
fi

max_preset={max_preset_default}
repeat=5

# MacOS default getopt doesn't support long args,
# so installing gnu version should make it work.
#
# brew install gnu-getopt
getopt_code=$($awk -f ./misc/getopt.awk <<EOF
Usage: sh ./benchmark_attributes.sh [-p MAX_PRESETS -r REPEATS] args...
Run multiple variants of attribute benchmarks (order-book) storing the results in corresponding files.
    -h, --help                  Show usage message
usage
exit 0
    -r, --repeat [number]       Select how many repetitions of this benchmark should run from within the wasm. (default: $repeat)
    -p, --max-preset [number]   Maximum number of preset to run to avoid running too long. (default: $max_preset)
EOF
)
eval "$getopt_code"

mkdir benches
bench_names=( {bench_names} )
for i in $(seq 1 $max_preset)
do
    # add index to the benchmark name
    # and make comma-separated list for passing into the command
    extrinsics=$(printf ",%s$i" "${{bench_names[@]}}")
    extrinsics=${{extrinsics:1}}
    command="./target/release/framenode benchmark pallet --chain=local  --execution=wasm --wasm-execution=compiled \
--pallet order-book --extra --extrinsic \\"$extrinsics\\" --repeat $repeat --output ./benches/preset_${{i}}.rs \
--json-file ./benches/preset_${{i}}_raw.json $*"
    echo "$command"
    eval "$command"
done
"""

templates = [
    code_template_place,
    code_template_cancel_first,
    code_template_cancel_last,
    code_template_execute,
    code_template_quote,
    code_template_exchange
]

max_preset = 2

for t in templates:
    print(generate_fs(range(1, max_preset+1), t))


def extract_name(template: str) -> str:
    regex = r"^\s+#\[extra\]\s+([\w]+){}"
    name = re.search(regex, template).group(1)
    return name


benchmark_names = " ".join([extract_name(t) for t in templates])
script_path = Path(__file__).parent.resolve() / 'benchmark_attributes.sh'
with script_path.open('w') as file:
    file.write(launch_script_template.format(bench_names=benchmark_names, max_preset_default=max_preset))
