/* Optional archived-reference probe, never linked into the maintained package.
 * Compile against the verified secp256k1-sys 0.4.2 source outside this repo.
 * Input: public synthetic secret 1 followed by the raw digest equal to n.
 */
#include <stdio.h>
#define SECP256K1_INLINE inline
#include "../include/secp256k1.h"
#include "hash_impl.h"

int main(void) {
    unsigned char input[64] = {0};
    const unsigned char digest[32] = {
        0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
        0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xfe,
        0xba,0xae,0xdc,0xe6,0xaf,0x48,0xa0,0x3b,
        0xbf,0xd2,0x5e,0x8c,0xd0,0x36,0x41,0x41
    };
    input[31] = 1;
    memcpy(input + 32, digest, 32);
    rustsecp256k1_v0_4_1_rfc6979_hmac_sha256 state;
    rustsecp256k1_v0_4_1_rfc6979_hmac_sha256_initialize(&state, input, 64);
    for (int i = 0; i < 3; ++i) {
        unsigned char output[32];
        rustsecp256k1_v0_4_1_rfc6979_hmac_sha256_generate(&state, output, 32);
        for (int j = 0; j < 32; ++j) printf("%02x", output[j]);
        puts("");
    }
    rustsecp256k1_v0_4_1_rfc6979_hmac_sha256_finalize(&state);
    return 0;
}
