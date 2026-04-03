> cargo test test_vanilla_plonk_batch_kzg_opening --release --features "parallel print-trace" -- --nocapture --ignored

```
running 1 test
Start:   domain_size = 65536,  curve = ark_bls12_381
··Start:   Setup
····Start:   Computing 196606 scalars powers
····End:     Computing 196606 scalars powers .......................................9.130ms
····Start:   196606-scalar mul in G1
····End:     196606-scalar mul in G1 ...............................................1.291s
····Start:   2-scalar mul in G1
····End:     2-scalar mul in G1 ....................................................5.446ms
··End:     Setup ...................................................................1.318s
··Start:   Preprocessing
····Start:   Committing to batch of 8 polynomials
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................415.679ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................390.461ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................417.321ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................447.720ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................440.322ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................423.799ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................503.049ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................439.959ms
····End:     Committing to batch of 8 polynomials ..................................3.481s
··End:     Preprocessing ...........................................................3.481s
··Start:   Proving
····Start:   Committing to batch of 3 polynomials
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................447.844ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................436.756ms
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................419.075ms
····End:     Committing to batch of 3 polynomials ..................................1.304s
····Start:   Committing to degree 65535 polynomials
····End:     Committing to degree 65535 polynomials ................................402.517ms
····Start:   Committing to degree 196604 polynomials
····End:     Committing to degree 196604 polynomials ...............................1.117s
····Start:   Extra: commiting to the linearization polynomial
······Start:   Committing to degree 65535 polynomials
······End:     Committing to degree 65535 polynomials ..............................420.764ms
····End:     Extra: commiting to the linearization polynomial ......................422.113ms
··End:     Proving .................................................................4.155s
··Start:   Verifying
····Start:   Reconstructing the commitment to the linearization polynomial: 7-multiexp
····End:     Reconstructing the commitment to the linearization polynomial: 7-multiexp 1.072ms
····Start:   KZG batch verification
······Start:   aggregate evaluation claims at zeta
······End:     aggregate evaluation claims at zeta .................................465.700µs
······Start:   batched KZG openning
······End:     batched KZG openning ................................................3.311ms
····End:     KZG batch verification ................................................4.219ms
··End:     Verifying ...............................................................5.789ms
End:     domain_size = 65536,  curve = ark_bls12_381 ...............................8.963s
proof size = 624, preprocessed data size = 392
```

> cargo test test_vanilla_plonk_with_fflonk_opening --release --features "parallel print-trace" -- --nocapture --ignored

```
Start:   domain_size = 65536,  curve = ark_bls12_381
··Start:   Setup
····Start:   Computing 786420 scalars powers
····End:     Computing 786420 scalars powers .......................................32.611ms
····Start:   786420-scalar mul in G1
····End:     786420-scalar mul in G1 ...............................................2.971s
····Start:   2-scalar mul in G1
····End:     2-scalar mul in G1 ....................................................3.355ms
··End:     Setup ...................................................................3.035s
··Start:   Preprocessing
····Start:   Committing to combination #0
······Start:   combining 8 polynomials: t = 8, max_degree = 65535
······End:     combining 8 polynomials: t = 8, max_degree = 65535 ..................9.411ms
······Start:   committing to the combined polynomial: degree = 524287
······End:     committing to the combined polynomial: degree = 524287 ..............1.952s
····End:     Committing to combination #0 ..........................................1.962s
··End:     Preprocessing ...........................................................1.963s
··Start:   Proving
····Start:   Committing to 2 proof polynomials
······Start:   Committing to combination #1
········Start:   combining 4 polynomials: t = 4, max_degree = 131069
········End:     combining 4 polynomials: t = 4, max_degree = 131069 ...............5.066ms
········Start:   committing to the combined polynomial: degree = 524279
········End:     committing to the combined polynomial: degree = 524279 ............1.526s
······End:     Committing to combination #1 ........................................1.532s
······Start:   Committing to combination #2
········Start:   combining 4 polynomials: t = 4, max_degree = 196604
········End:     combining 4 polynomials: t = 4, max_degree = 196604 ...............7.040ms
········Start:   committing to the combined polynomial: degree = 786418
········End:     committing to the combined polynomial: degree = 786418 ............1.970s
······End:     Committing to combination #2 ........................................1.978s
····End:     Committing to 2 proof polynomials .....................................3.515s
····Start:   Opening
······Start:   polynomial divisions
······End:     polynomial divisions ................................................454.483ms
······Start:   commitment to a degree-786410 polynomial
······End:     commitment to a degree-786410 polynomial ............................3.255s
······Start:   linear combination of polynomials
······End:     linear combination of polynomials ...................................87.627ms
····End:     Opening ...............................................................8.278s
··End:     Proving .................................................................11.819s
··Start:   Verifying
····Start:   barycentric evaluations
····End:     barycentric evaluations ...............................................93.500µs
····Start:   multiexp
····End:     multiexp ..............................................................545.300µs
··End:     Verifying ...............................................................5.437ms
End:     domain_size = 65536,  curve = ark_bls12_381 ...............................16.826s
proof size = 904, preprocessed data size = 56
```