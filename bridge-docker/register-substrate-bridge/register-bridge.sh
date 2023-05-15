#!/bin/sh

# Wait for parachain to start
sleep 30

relayer \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	--parachain-url ws://bridge-parachain-alice:9844 \
	--parachain-key //Alice \
	bridge register sora parachain trusted \
	--peers KWAp8rpaW89FYd23Ge8qV2fZHVJ6zpnXULnTxdZsxqpfbqhTD \
	--peers KW49Z85ywrc4MjxbHQVRyiHifdNBx79M846X9oJrz6hMAgzcX
	# --peers KW4AUp9FxDBRnViUtakrz8BprQQbfaH6pmX4gLMNKWXBBirSn
	# --peers KWB9S4JpCvBfwAMFRkRNqLgtKxvmyCzDSCDf4d9Lhgx8XYfoS

relayer \
	--substrate-url ws://bridge-sora-alice:9944 \
	--substrate-key //Alice \
	--parachain-url ws://bridge-parachain-alice:9844 \
	--parachain-key //Alice \
	bridge register parachain trusted \
	--peers KWAp8rpaW89FYd23Ge8qV2fZHVJ6zpnXULnTxdZsxqpfbqhTD \
	--peers KW49Z85ywrc4MjxbHQVRyiHifdNBx79M846X9oJrz6hMAgzcX
	# --peers KW4AUp9FxDBRnViUtakrz8BprQQbfaH6pmX4gLMNKWXBBirSn
	# --peers KWB9S4JpCvBfwAMFRkRNqLgtKxvmyCzDSCDf4d9Lhgx8XYfoS