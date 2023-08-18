#!/usr/bin/python3

def generate_fs(range_: range, template: str):
    codes = ""
    for i in range_:
        codes += template.format(i, i)
    return codes


code_template = """
    #[extra]
    delete_orderbook_{} {{
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_{}());
    }} : {{ OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }}
"""

print(generate_fs(range(1, 8), code_template))
