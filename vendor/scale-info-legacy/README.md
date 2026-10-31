# scale-info-legacy

This crate provides a set of types which build on each other. The ultimate goal is to be able to define
the necessary type information needed to describe how historic Substrate types are SCALE encoded.
The main types exposed here are as follows:

- `TypeRegistry`: the lowest level type which one can populate with type information (via `TypeRegistry::insert()`)
  and then query to resolve some type name to the relevant information (via `TypeRegistry::resolve_type()`).
- `TypeRegistrySet`: a set of the above, which will resolve types (via `TypeRegistrySet::resolve_type()`) by working
  through the inner type registries until it finds the relevant information (or doesn't find anything). This allows us
  to combine type registries in different ways to alter how we resolve things.
- `ChainTypeRegistry`: this type is constructed by deserializing some JSON (or similar) which describes all of the
  types that we need to know about on a given chain. The main function here is `ChainTypeRegistry::for_spec_version()`,
  which returns a `TypeRegistrySet` for resolving types for the given spec version.

We also expose an `InsertName`, which is built by parsing type names like `Vec<T>` from strings, and used in
`TypeRegistry::insert()` to insert types, and then `LookupName`, which is built by parsing type names like
`Vec<T>`, `u8; 32` and `(bool, u32)` and is used to lookup the corresponding type information via `TypeRegistry::resolve_type()`
and similar. Finally, `TypeShape` is an enum used to describe the shape of the type inserted via `TypeRegistry::insert()`.
This crate, like `scale-info`, can be used in conjunction with crates like:

- [`scale-decode`](https://github.com/paritytech/scale-decode) to decode SCALE encoded bytes
  into custom types based on this type information.
- [`scale-encode`](https://github.com/paritytech/scale-encode) to SCALE encode custom types
  into bytes based on this type information.
- [`scale-value`](https://github.com/paritytech/scale-value) to SCALE encode or decode from a
  `Value` type (a bit like `serde_json`'s `Value` type).