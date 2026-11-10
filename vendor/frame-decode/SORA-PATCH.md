# SORA Nexus legacy YAML parser maintenance migration

Based on the checksum-verified crates.io `frame-decode` 0.18.1 source archive.
Replaces `serde_yaml` / unmaintained `unsafe-libyaml` (RUSTSEC-2024-0449) with
`serde-saphyr` 2.0.0, including development examples. All three built-in chain
registries and upstream validation tests remain. The public parser
continues to return a boxed error and `ChainTypeRegistry`; its callers do not
expose a concrete YAML parser type. No on-chain execution or encoding changes.
This library is validated against the root workspace lockfile.

The strict parser exposed duplicate type definitions in the upstream YAML.
The three fixtures now omit 47 shadowed definitions and retain their final
definitions verbatim. Of those removed entries, 32 equal their final definition
and 15 differ from it. The prior `DeserializableChainTypes.types` deserialized
into a `HashMap<InsertName, DeserializableShape>`, where the final insertion wins;
the cleanup preserves that behavior without accepting duplicate input globally.

`legacy-yaml-deduplication.json` records every removed definition, its final
replacement, original line numbers, input/output hashes, and a complete registry
comparison. The comparison ignores only the order of the global type map and
reduces its repeated keys using the prior last-insert rule. Every other mapping,
sequence, struct field, enum variant, and spec-version override retains its
order and content. All three complete registry representations compare equal.

The deliberately retained conflicting definitions are `ValidatorIndex = u16`
in all three registries; `OpaqueNetworkState`, `BitVec`, and both generic
`VestingSchedule` forms in Kusama relay; and lookup source, `RefCount`,
`AccountInfo`, `T::ClassId`, `T::InstanceId`, `Outcome`, and `XcmError` in Kusama
Asset Hub. Regression tests cover these winners, the field/variant order of the
changed Asset Hub shapes, and rejection of both identical and conflicting new
duplicates. The parser's duplicate-key policy stays strict.

Eight reserved scalar keys are explicitly quoted: five `Null` enum names and
three `Off` enum names. They were String keys in the previous typed
deserializer, so their spelling, indices and payloads remain unchanged. A
separate regression checks the complete `MultiLocation` variant order and
the empty `Null` variants. The `JunctionV1.OnlyChild: Null` value remains an
unquoted unit payload, and null spec-version bounds remain optional bounds.
The complete-registry comparison models these typed distinctions instead of
coercing String keys through a generic YAML value representation.
