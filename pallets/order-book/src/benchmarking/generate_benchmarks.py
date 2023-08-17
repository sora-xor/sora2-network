#!/usr/bin/python3

def generate_fs(range_: range, template: str):
    codes = ""
    for i in range_:
        n = 2 ** i
        codes += template.format(
            name_suffix="2_" + str(i),
            max_side_price_count=n,
            max_limit_orders_for_price=n,
            max_opened_limit_orders_per_user=n,
            max_expiring_orders_per_block=n * 8
        )
    return codes


code_template = """
    #[extra]
    delete_orderbook_{name_suffix} {{
        let order_book_id = prepare_delete_orderbook_benchmark::<T>({max_side_price_count}, \
{max_limit_orders_for_price}, {max_opened_limit_orders_per_user}, {max_expiring_orders_per_block});
    }}: {{ OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }}
"""

print(generate_fs(range(4, 11), code_template))
