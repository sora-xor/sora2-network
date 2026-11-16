# SORA Nexus compiler maintenance

The 2026-10-25 Clippy cleanup replaces an unused nested Default-bound helper
with a function-pointer type check in the actual generic genesis builder. This
checks the intended `GenesisConfig<T>: Default` contract in std and no_std without
calling Default, constructing another genesis value, or changing storage writes.

Legacy stored-call reads preserve their key and `(Vec<u8>, AccountId, Balance)`
encoding. A five-byte compact-length probe rejects oversized calls and tuple
values before allocating a runtime buffer. The bounded tuple is decoded with
depth and memory limits, including its generic account/balance suffix; malformed
or trailing bytes are rejected. Protocol validation reserves one additional
database read for this probe. The runtime's nested bridge-call filter uses the
same loader. Quorum cancellation remains independent of call decoding, so
oversized or malformed historical values can still be removed.

This limits runtime copying and decoding. The storage host still fetches the
full backend value and its trie proof, so it does not establish a size bound on
historical or orphaned values, nor replace a complete public-state inventory.
