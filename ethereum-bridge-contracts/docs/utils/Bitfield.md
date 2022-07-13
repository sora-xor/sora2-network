## `Bitfield`






### `randomNBitsWithPriorCheck(uint256 seed, uint256[] prior, uint256 n, uint256 length) → uint256[] bitfield` (public)

Draws a random number, derives an index in the bitfield, and sets the bit if it is in the `prior` and not
yet set. Repeats that `n` times.



### `createBitfield(uint256[] bitsToSet, uint256 length) → uint256[] bitfield` (public)





### `countSetBits(uint256[] self) → uint256` (public)

Calculates the number of set bits by using the hamming weight of the bitfield.
The alogrithm below is implemented after https://en.wikipedia.org/wiki/Hamming_weight#Efficient_implementation.
Further improvements are possible, see the article above.



### `isSet(uint256[] self, uint256 index) → bool` (internal)





### `set(uint256[] self, uint256 index)` (internal)





### `clear(uint256[] self, uint256 index)` (internal)






