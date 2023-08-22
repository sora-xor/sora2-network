#!/usr/bin/python3

def generate_fs(range_: range, template: str):
    codes = ""
    for i in range_:
        codes += template.format(i, i)
    return codes


code_template_delete= """
    #[extra]
    delete_orderbook_{} {{
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_{}());
    }} : {{ OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }}
"""

code_template_place= """
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

print(generate_fs(range(1, 8), code_template_delete))
print(generate_fs(range(1, 8), code_template_place))
