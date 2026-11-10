# SORA Nexus maintenance migration

Original consumer: `indicatif 0.17.11`, crates.io archive SHA-256 `183b3088984b400f4cfac3620d5e076c84da5364016b4f49473de574b2586235`. The package version identifies the original source; dependencies are actually replaced rather than relabeled or exempted from audit.

Replace number_prefix with maintained unit-prefix 0.5.2, following upstream indicatif 0.18.6. The adapter changes the actual imported dependency; all public formatting/progress APIs remain compatible with the SDK consumer. Upstream reference archive SHA-256: 9433806cd6b4ec1aba79c021c7e4c58fb4c3b9977c085062e611ac929998fb0c. Reference: https://crates.io/crates/indicatif/0.18.6 .

Validation: all-feature compilation and all 47 upstream library tests pass, preserving displayed byte/unit formatting.
