This repo aims to build a collection of tools for augmenting polynomial commitment schemes (PCS, from now on). 

## Shplonk
[Shplonk](https://eprint.iacr.org/2020/081.pdf), scheme #2 (aka private aggregation scheme from [Halo Infinite](https://eprint.iacr.org/2020/1536.pdf), section 4) accumulates `k` opening tuples `(Cf, z, v)`, each claiming that `f(z) = v, commit(f) = Cf` for a univariate polynomial `f` and points `z, v`, into a single one `(C', z', v')`, such that a proof for the aggregate claim attests that all other `k` claims are valid. It allows to compile a simple PCS, capable of proving single evaluation of a single polynomial at a time, into a PCS opening multiple polynomials each in different sets of points with a single proof. The overhead limits to producing the valid aggregate claim, that is generating one commitment by the prover, linear combination of `k+2` commitments by the verifier, `O(klog(k))` field operations for both, and one commitment of communication.

Notice that this type of aggregation is different from [aggregation for vector commitments](https://eprint.iacr.org/2020/527.pdf) in that the proof for the aggregated claim is produced from scratch, while in the latter case the aggregate proof is computed from the proofs for the individual claims. At least, it is not implemented.      

## FFlonk

[FFlonk](https://eprint.iacr.org/2021/1167.pdf) from `n` polynomials of degrees not exceeding `d` constructs a polynomial of degree less that `nd`, such that evaluating each of the individual polynomials in the same `k` points is equivalent to evaluating  the combined polynomial in `nk` points. In combination with a PCS enjoying efficient multipoint openings, asymptotically reduces the number of commitments to transfer and improves verifier performance. 

### What is implemented
1. Traits for a minimal PCS (far from being perfect). 
2. Simplest form of KZG implementing these traits.
3. Halo Inifinite private aggregation (aka Schplonk scheme #2) generic over the traits.
4. Fflonk routines: combining polynomials, converting evaluations.
5. Fflonk PCS from [the original paper](https://eprint.iacr.org/2021/1167.pdf): opens Ffflonk-combined polynomials with Shplonk-compiled KZG.
6. [A test](/tests/plonk) comparing opening a very vanilla (not zk, arithmetic gate only, no polynomial splitting) Plonk polynomial assignment using 
   * Batching verification of KZG proofs in different points and linearization, as described in the original [P lonK](plonk)
   * The scheme described in [ff lonK](https://eprint.iacr.org/2021/1167.pdf), Section 7.

#### KZG benchmarks
```
cargo test bench_minimal_kzg --release --features "parallel print-trace" -- --nocapture --ignored
```
outputs timings for generating a setup, committing to a 2^16-degree polynomial, proving and verifying an opening in a single point.