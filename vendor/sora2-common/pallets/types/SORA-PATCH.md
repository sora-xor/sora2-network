# SORA Nexus macro maintenance migration

Replaced unmaintained `derivative` with maintained `derive-where` 1.7.0
using its safe expansion mode. Explicit generic bounds are retained, with
no changes to serialized fields or runtime algorithms.

See `vendor/MACRO-MAINTENANCE.md` in the repository root for validation.
