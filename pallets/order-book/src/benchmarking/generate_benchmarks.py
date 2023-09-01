#!/usr/bin/python3

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


code_template_delete = """
        #[extra]
        delete_orderbook_{} {{
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_{}());
        }} : {{ OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }}
"""

code_template_place = """
        #[extra]
        place_limit_order_{} {{
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_{}(), alice::<T>());
        }}: {{
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }}
"""

code_template_cancel_first = """
        #[extra]
        cancel_limit_order_first_{} {{
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_{}::<T>(), alice::<T>(), true);
        }}: {{
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }}
"""

code_template_cancel_last = """
        #[extra]
        cancel_limit_order_last_{} {{
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_{}::<T>(), alice::<T>(), false);
        }}: {{
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }}
"""

code_template_execute = """
        #[extra]
        execute_market_order_{} {{
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_{}(), caller.clone(), false);
        }}: {{
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }}
"""

code_template_quote = """
        #[extra]
        quote_{} {{
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_{}());
        }}: {{
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }}
"""

code_template_exchange = """
        #[extra]
        exchange_{} {{
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_{}(), caller.clone(), true);
        }} : {{
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }}
"""

print(generate_fs(range(1, 8), code_template_delete))
print(generate_fs(range(1, 8), code_template_place))
print(generate_fs(range(1, 8), code_template_cancel_first))
print(generate_fs(range(1, 8), code_template_cancel_last))
print(generate_fs(range(1, 8), code_template_execute))
print(generate_fs(range(1, 8), code_template_quote))
print(generate_fs(range(1, 8), code_template_exchange))
