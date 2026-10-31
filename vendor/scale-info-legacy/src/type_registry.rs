// Copyright (C) 2024 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the scale-info-legacy crate.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//         http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This module provides a [`TypeRegistry`], which can be used to store and resolve
//! type information for types based on their names.

use crate::insert_name::{self, InsertName};
use crate::lookup_name::{self, LookupName, LookupNameDef};
use crate::type_shape::{self, Primitive, TypeShape, VariantDesc};
use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hash;
use core::iter::ExactSizeIterator;
use core::str::FromStr;
use hashbrown::HashMap;
use scale_type_resolver::{
    BitsOrderFormat, BitsStoreFormat, Field, ResolvedTypeVisitor, TypeResolver, Variant,
};
use smallvec::SmallVec;

/// An error resolving types in the [`TypeRegistry`].
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TypeRegistryResolveError {
    #[error("'{0}' is not a valid type name: {1}")]
    LookupNameInvalid(String, lookup_name::ParseError),
    #[error(
        "Bitvecs must have an order type with the path bitvec::order::Msb0 or bitvec::order::Lsb0"
    )]
    UnexpectedBitOrderType,
    #[error(
        "Bitvecs must have a store type which resolves to a primitive u8, u16, u32 or u64 type."
    )]
    UnexpectedBitStoreType,
}

/// An error when using [`TypeRegistry::resolve_type_with_parent()`]. This returns the visitor if
/// the type wasn't found, allowing us to use it again with a different registry or whatever.
#[allow(missing_docs)]
#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum TypeRegistryResolveWithParentError<Visitor> {
    #[error(
        "Type '{type_name}' not found (either the name, pallet or number of generic params don't match any known types)"
    )]
    TypeNotFound {
        type_name: LookupName,
        visitor: Visitor,
    },
    #[error(transparent)]
    Other(TypeRegistryResolveError),
}

/// The type info stored in the registry against a given named type.
#[derive(Clone, Debug)]
struct TypeInfo {
    // The generic param names that may be used in the type description below.
    params: SmallVec<[String; 4]>,
    // A description of the shape of the type.
    desc: TypeShape,
}

/// Key pointing at named types we've stored.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeKey {
    // Pallet to look for this type in.
    pallet: Option<String>,
    // Name of the type eg BalanceOf.
    name: String,
    // How many generic params are expected? This allows us to
    // have types with same name but different number of params
    // (eg BalanceOf<T> and BalanceOf<T,I>).
    generic_params: u8,
}

impl core::hash::Hash for TypeKey {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        hash_key(
            self.pallet.as_deref(),
            &self.name,
            self.generic_params,
            state,
        );
    }
}

fn hash_key<H: core::hash::Hasher>(
    pallet: Option<&str>,
    name: &str,
    generic_params: u8,
    state: &mut H,
) {
    pallet.hash(state);
    name.hash(state);
    generic_params.hash(state);
}

/// A type registry that stores a mapping from type names to a description of how to SCALE
/// encode/decode that type.
///
/// To insert a new type description, use [`TypeRegistry::insert()`]. To look up the shape
/// of a type, use [`TypeRegistry::resolve_type()`]. [`TypeRegistry`] implements
/// [`scale_type_resolver::TypeResolver`], which means that it can be used in conjunction with
/// the `scale-encode` crate to SCALE encode types, or the `scale-decode` crate to SCALE decode
/// bytes into some type.
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    /// A hash from the name of a type (like `Vec` or `usize`) to a description
    /// of the shape of the type (which may involve generic params or just be
    /// concrete types or aliases to other types).
    types: HashMap<TypeKey, TypeInfo>,
    /// A map of the available runtime APIs and their input/output types.
    /// The first entry is the trait name, the next is the method name.
    runtime_apis: HashMap<String, HashMap<String, RuntimeApi>>,
}

/// This is handed back when we iterate over Runtime APIs via [`TypeRegistry::runtime_apis()`].
/// First, [`RuntimeApiName::Trait`] will be returned to indicate which trait is being iterated
/// over, and then [`RuntimeApiName::Method`] will be returned with the name of each method in the
/// aforementioned trait.
///
/// Each trait will be iterated exactly once.
pub enum RuntimeApiName<'a> {
    /// The name of the trait that the methods given next are defined on.
    Trait(&'a str),
    /// The name of a method in the most recently returned [`RuntimeApiName::Trait`].
    Method(&'a str),
}

/// This represents a single Runtime API.
#[derive(Debug, Clone)]
pub struct RuntimeApi {
    /// The input arguments to the API
    pub inputs: Vec<RuntimeApiInput>,
    /// The output type returned from the API.
    pub output: LookupName,
}

/// A single input argument to a Runtime API call.   
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct RuntimeApiInput {
    /// The name of this runtime API input.
    pub name: String,
    /// The type of this runtime API input.
    pub id: LookupName,
}

impl From<LookupName> for RuntimeApiInput {
    fn from(id: LookupName) -> Self {
        RuntimeApiInput {
            name: String::new(),
            id,
        }
    }
}
impl TryFrom<&str> for RuntimeApiInput {
    type Error = lookup_name::ParseError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl core::str::FromStr for RuntimeApiInput {
    type Err = lookup_name::ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let id = LookupName::parse(value)?;
        Ok(id.into())
    }
}
impl<S: Into<String>, L: TryInto<LookupName>> TryFrom<(S, L)> for RuntimeApiInput {
    type Error = L::Error;
    fn try_from((name, id): (S, L)) -> Result<Self, Self::Error> {
        let id = id.try_into()?;
        let name = name.into();
        Ok(RuntimeApiInput { id, name })
    }
}

impl TypeRegistry {
    /// Create a new and completely empty [`TypeRegistry`]. Most of the time you should
    /// start with [`TypeRegistry::basic()`] to get a registry pre-populated with built in
    /// rust types.
    pub fn empty() -> Self {
        TypeRegistry {
            types: HashMap::new(),
            runtime_apis: HashMap::new(),
        }
    }

    /// Create a new [`TypeRegistry`] that's pre-populated with built-in rust types.
    pub fn basic() -> Self {
        let mut registry = TypeRegistry::empty();

        // Basic types that we expect to work by default.
        let basic_types = [
            // The point of this "Unknown" type is that we can point to it,
            // and it's a valid type that can be converted to other formats for
            // instance (eg V14 type info), but nothing can actually be decoded
            // into it or encoded from it, since it has no variants.
            ("special::Unknown", TypeShape::EnumOf(Vec::new())),
            ("bool", TypeShape::Primitive(Primitive::Bool)),
            ("char", TypeShape::Primitive(Primitive::Char)),
            ("u8", TypeShape::Primitive(Primitive::U8)),
            ("u16", TypeShape::Primitive(Primitive::U16)),
            ("u32", TypeShape::Primitive(Primitive::U32)),
            ("u64", TypeShape::Primitive(Primitive::U64)),
            ("u128", TypeShape::Primitive(Primitive::U128)),
            ("u256", TypeShape::Primitive(Primitive::U256)),
            ("i8", TypeShape::Primitive(Primitive::I8)),
            ("i16", TypeShape::Primitive(Primitive::I16)),
            ("i32", TypeShape::Primitive(Primitive::I32)),
            ("i64", TypeShape::Primitive(Primitive::I64)),
            ("i128", TypeShape::Primitive(Primitive::I128)),
            ("i256", TypeShape::Primitive(Primitive::I256)),
            ("str", TypeShape::Primitive(Primitive::Str)),
            ("String", TypeShape::Primitive(Primitive::Str)),
            (
                "Box<T>",
                TypeShape::AliasOf(LookupName::parse("T").unwrap()),
            ),
            (
                "Arc<T>",
                TypeShape::AliasOf(LookupName::parse("T").unwrap()),
            ),
            ("Rc<T>", TypeShape::AliasOf(LookupName::parse("T").unwrap())),
            (
                "Cell<T>",
                TypeShape::AliasOf(LookupName::parse("T").unwrap()),
            ),
            (
                "RefCell<T>",
                TypeShape::AliasOf(LookupName::parse("T").unwrap()),
            ),
            (
                "Vec<T>",
                TypeShape::SequenceOf(LookupName::parse("T").unwrap()),
            ),
            (
                "LinkedList<T>",
                TypeShape::SequenceOf(LookupName::parse("T").unwrap()),
            ),
            (
                "VecDeque<T>",
                TypeShape::SequenceOf(LookupName::parse("T").unwrap()),
            ),
            (
                "BTreeMap<K,V>",
                TypeShape::SequenceOf(LookupName::parse("(K, V)").unwrap()),
            ),
            (
                "BTreeSet<V>",
                TypeShape::SequenceOf(LookupName::parse("V").unwrap()),
            ),
            (
                "BinaryHeap<V>",
                TypeShape::SequenceOf(LookupName::parse("V").unwrap()),
            ),
            (
                "Cow<T>",
                TypeShape::UnnamedStructOf(vec![LookupName::parse("T").unwrap()]),
            ),
            (
                "Option<T>",
                TypeShape::EnumOf(vec![
                    type_shape::Variant {
                        index: 0,
                        name: "None".to_owned(),
                        fields: type_shape::VariantDesc::UnnamedStructOf(vec![]),
                    },
                    type_shape::Variant {
                        index: 1,
                        name: "Some".to_owned(),
                        fields: type_shape::VariantDesc::UnnamedStructOf(vec![LookupName::parse(
                            "T",
                        )
                        .unwrap()]),
                    },
                ]),
            ),
            (
                "Result<T,E>",
                TypeShape::EnumOf(vec![
                    type_shape::Variant {
                        index: 0,
                        name: "Ok".to_owned(),
                        fields: type_shape::VariantDesc::UnnamedStructOf(vec![LookupName::parse(
                            "T",
                        )
                        .unwrap()]),
                    },
                    type_shape::Variant {
                        index: 1,
                        name: "Err".to_owned(),
                        fields: type_shape::VariantDesc::UnnamedStructOf(vec![LookupName::parse(
                            "E",
                        )
                        .unwrap()]),
                    },
                ]),
            ),
            ("PhantomData", TypeShape::UnnamedStructOf(vec![])),
            // These exist just so that resolving bitvecs using these store types will work.
            ("bitvec::order::Lsb0", TypeShape::NamedStructOf(vec![])),
            ("bitvec::order::Msb0", TypeShape::NamedStructOf(vec![])),
            // So long as the store type is a suitable primitive and the order type one of the above, this will work out.
            (
                "bitvec::vec::BitVec<Store, Order>",
                TypeShape::BitSequence {
                    store: LookupName::parse("Store").unwrap(),
                    order: LookupName::parse("Order").unwrap(),
                },
            ),
            // Types seem to mostly use this alias to BitVec:
            (
                "BitVec<Store, Order>",
                TypeShape::AliasOf(LookupName::parse("bitvec::vec::BitVec<Store, Order>").unwrap()),
            ),
            // Defining this once means that anything wanting compact types can alias to this:
            (
                "codec::Compact<T>",
                TypeShape::Compact(LookupName::parse("T").unwrap()),
            ),
            (
                "Compact<T>",
                TypeShape::AliasOf(LookupName::parse("codec::Compact<T>").unwrap()),
            ),
        ];

        for (name, desc) in basic_types {
            let name = InsertName::from_str(name).unwrap();
            registry.insert(name, desc);
        }

        registry
    }

    /// Return a [`LookupName`] corresponding to one concrete type for every entry in the registry.
    /// Any generic parameters are substituted for `()` in order that we can return valid concrete types.
    pub fn keys(&self) -> impl Iterator<Item = LookupName> + use<'_> {
        self.types.keys().map(|key| {
            let insert_name = InsertName {
                name: key.name.clone(),
                params: (0..key.generic_params).map(|_| "()".to_owned()).collect(),
                pallet: key.pallet.clone(),
            };
            LookupName::from(insert_name)
        })
    }

    /// Insert a new type into the registry.
    ///
    /// # Panics
    ///
    /// Panics if the type being inserted has more than 255 generic parameters.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, TypeShape, InsertName, LookupName };
    ///
    /// let mut registry = TypeRegistry::empty();
    ///
    /// // Add a basic type description to our registry:
    /// let insert_name = InsertName::parse("Foo<T>").unwrap();
    /// registry.insert(insert_name, TypeShape::SequenceOf(LookupName::parse("T").unwrap()));
    ///
    /// // We can add types that are scoped to a specific pallet, too:
    /// let scoped_insert_name = InsertName::parse("Bar<A,B,C>").unwrap().in_pallet("balances");
    /// registry.insert(scoped_insert_name, TypeShape::SequenceOf(LookupName::parse("T").unwrap()));
    /// ```
    pub fn insert(&mut self, name: InsertName, shape: TypeShape) {
        let generic_params = name
            .params
            .len()
            .try_into()
            .expect("Expecting between 0 and 255 generic params");
        self.types.insert(
            TypeKey {
                pallet: name.pallet,
                name: name.name,
                generic_params,
            },
            TypeInfo {
                desc: shape,
                params: name.params,
            },
        );
    }

    /// Insert a new type into the registry.
    ///
    /// Unlike [`TypeRegistry::insert()`], this accepts a string and attempts to parse it into a
    /// valid [`InsertName`] internally, returning any error encountered in doing so. If you need
    /// to scope an insert to a specific pallet, use [`TypeRegistry::insert()`] so that you can
    /// create a scoped [`InsertName`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, TypeShape, InsertName, LookupName };
    ///
    /// let mut registry = TypeRegistry::empty();
    ///
    /// // Add a basic type description to our registry:
    /// registry.insert_str(
    ///     "Foo<T>",
    ///     TypeShape::SequenceOf(LookupName::parse("T").unwrap())
    /// ).unwrap();
    /// ```
    pub fn insert_str(
        &mut self,
        name: &str,
        shape: TypeShape,
    ) -> Result<(), insert_name::ParseError> {
        let name = InsertName::parse(name)?;
        let generic_params = name
            .params
            .len()
            .try_into()
            .expect("Expecting between 0 and 255 generic params");
        self.types.insert(
            TypeKey {
                pallet: name.pallet,
                name: name.name,
                generic_params,
            },
            TypeInfo {
                desc: shape,
                params: name.params,
            },
        );
        Ok(())
    }

    /// Resolve some type information by providing a [`LookupName`], which is the
    /// concrete name of a type we want to know how to decode values for, and a
    /// `visitor` which will be called in order to describe how to decode it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, TypeShape, LookupName };
    /// use scale_type_resolver::visitor;
    ///
    /// // Name a type that you want to know how to encode/decode:
    /// let name = LookupName::parse("Vec<(bool, u32)>").unwrap();
    ///
    /// // Here we provide a dumb visitor (ie set of callbacks) which will return
    /// // true if the type we ask about is a sequence, and false otherwise.
    /// let my_visitor = visitor::new((), |_, _| false)
    ///     .visit_sequence(|_, _, _| true);
    ///
    /// // Query this name in our registry, passing our visitor:
    /// let mut registry = TypeRegistry::basic();
    /// let is_sequence = registry.resolve_type(name, my_visitor).unwrap();
    ///
    /// assert!(is_sequence);
    /// ```
    pub fn resolve_type<'this, V: ResolvedTypeVisitor<'this, TypeId = LookupName>>(
        &'this self,
        type_id: LookupName,
        visitor: V,
    ) -> Result<V::Value, TypeRegistryResolveError> {
        match self.resolve_type_with_parent(self, type_id, visitor) {
            Err(TypeRegistryResolveWithParentError::Other(e)) => Err(e),
            Err(TypeRegistryResolveWithParentError::TypeNotFound {
                visitor,
                mut type_name,
            }) => {
                if type_name.pallet().is_some() {
                    // if the lookup was pallet scoped, try again without any pallet scope
                    // to see if we do any better that way!
                    type_name.take_pallet();
                    self.resolve_type(type_name, visitor)
                } else {
                    // else, follow our rules and call the `visit_not_found` function because
                    // we couldn't find any matches.
                    Ok(visitor.visit_not_found())
                }
            }
            Ok(val) => Ok(val),
        }
    }

    /// Resolve some type information by providing the string name of the type,
    /// and a `visitor` which will be called in order to describe how to decode it.
    /// This just creates a [`LookupName`] under the hood and uses that to resolve the
    /// type.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, TypeShape };
    /// use scale_type_resolver::visitor;
    ///
    /// // Provide a dumb visitor (ie set of callbacks) to tell us about the type that
    /// // we query. Here, all we do is return true if the type is a sequence and
    /// // false otherwise.
    /// let my_visitor = visitor::new((), |_, _| false)
    ///     .visit_sequence(|_, _, _| true);
    ///
    /// // Query the string name in our registry, passing our visitor:
    /// let mut registry = TypeRegistry::basic();
    /// let is_sequence = registry.resolve_type_str("Vec<(bool, u32)>", my_visitor).unwrap();
    ///
    /// assert!(is_sequence);
    /// ```
    pub fn resolve_type_str<'this, V: ResolvedTypeVisitor<'this, TypeId = LookupName>>(
        &'this self,
        type_name_str: &str,
        visitor: V,
    ) -> Result<V::Value, TypeRegistryResolveError> {
        let type_id = LookupName::parse(type_name_str).map_err(|e| {
            TypeRegistryResolveError::LookupNameInvalid(type_name_str.to_owned(), e)
        })?;
        self.resolve_type(type_id, visitor)
    }

    /// Insert a new Runtime API definition into the registry. We use [`LookupName`]
    /// rather than [`InsertName`] here because we are not defining any new types,
    /// simply pointing to types which need adding separately via [`Self::insert()`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, LookupName, RuntimeApiInput };
    ///
    /// let mut registry = TypeRegistry::empty();
    /// registry.insert_runtime_api(
    ///     "Metadata",
    ///     "metadata_versions",
    ///     vec![],
    ///     LookupName::parse("Vec<u32>").unwrap(),
    /// );
    /// registry.insert_runtime_api(
    ///     "Metadata",
    ///     "metadata_at_version",
    ///     vec![RuntimeApiInput { name: "version".into(), id: LookupName::parse("u32").unwrap() }],
    ///     LookupName::parse("Option<Vec<u8>>").unwrap(),
    /// );
    /// ```
    pub fn insert_runtime_api(
        &mut self,
        trait_name: impl Into<String>,
        method_name: impl Into<String>,
        inputs: Vec<RuntimeApiInput>,
        output: LookupName,
    ) {
        self.runtime_apis
            .entry(trait_name.into())
            .or_default()
            .insert(method_name.into(), RuntimeApi { inputs, output });
    }

    /// This method is like [`Self::insert_runtime_api()`], but is more forgiving
    /// in terms of the arguments it accepts, allowing for easier usage at the cost of
    /// potential errors if invalid values are provided.
    ///
    /// If there are no inputs to the Runtime API, [`Self::try_insert_runtime_api_without_inputs`]
    /// can be used instead to avoid type inference issues.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, LookupName };
    ///
    /// let mut registry = TypeRegistry::empty();
    /// registry.try_insert_runtime_api(
    ///     "Metadata",
    ///     "metadata_at_version",
    ///     ["u32"],
    ///     "Option<Vec<u8>>",
    /// );
    /// ```
    pub fn try_insert_runtime_api<I, O>(
        &mut self,
        trait_name: impl Into<String>,
        method_name: impl Into<String>,
        inputs: impl IntoIterator<Item = I>,
        output: O,
    ) -> Result<(), O::Error>
    where
        I: TryInto<RuntimeApiInput, Error = O::Error>,
        O: TryInto<LookupName>,
    {
        let inputs = inputs
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        let output = output.try_into()?;
        self.insert_runtime_api(trait_name, method_name, inputs, output);
        Ok(())
    }

    /// This is like [`Self::try_insert_runtime_api`], but is a shorthand for inserting
    /// APIs which do not have any input arguments. This exists because type inference can
    /// be tricky using [`Self::try_insert_runtime_api`] and providing no inputs.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scale_info_legacy::{ TypeRegistry, LookupName };
    ///
    /// let mut registry = TypeRegistry::empty();
    /// registry.try_insert_runtime_api_without_inputs(
    ///     "Metadata",
    ///     "metadata_versions",
    ///     "Vec<u32>",
    /// );
    /// ```
    pub fn try_insert_runtime_api_without_inputs<O>(
        &mut self,
        trait_name: impl Into<String>,
        method_name: impl Into<String>,
        output: O,
    ) -> Result<(), O::Error>
    where
        O: TryInto<LookupName>,
    {
        let output = output.try_into()?;
        self.insert_runtime_api(trait_name, method_name, vec![], output);
        Ok(())
    }

    /// Return a matching runtime API definition if one exists.
    pub fn runtime_api(&self, trait_name: &str, method_name: &str) -> Option<&RuntimeApi> {
        self.runtime_apis.get(trait_name)?.get(method_name)
    }

    /// Return an iterator over each of the Runtime APIs that has been defined. First,
    /// [`RuntimeApiName::Trait`] is returned to indicate the trait being iterated over next,
    /// and then [`RuntimeApiName::Method`] is returned for each of the method names in that
    /// trait. A trait will only be iterated over once.
    pub fn runtime_apis(&self) -> impl Iterator<Item = RuntimeApiName<'_>> {
        self.runtime_apis.iter().flat_map(|(k, v)| {
            core::iter::once(RuntimeApiName::Trait(k))
                .chain(v.keys().map(|method| RuntimeApiName::Method(method)))
        })
    }

    /// Return an iterator over each of the Runtime API method names in a given trait.
    pub fn runtime_apis_in_trait(&self, trait_name: &str) -> impl Iterator<Item = &'_ str> {
        let Some(apis) = self.runtime_apis.get(trait_name) else {
            return Either::A(core::iter::empty());
        };

        let it = apis.keys().map(|k| &**k);
        Either::B(it)
    }

    /// Extend this type registry with the one provided. In case of any matches, the provided types
    /// and runtime API methods will overwrite the existing ones.
    pub fn extend(&mut self, other: TypeRegistry) {
        self.types.extend(other.types);

        // Merge at the trait level, but overwrite at the method level in case of duplicates:
        for (trait_name, methods) in other.runtime_apis {
            let current_methods = self.runtime_apis.entry(trait_name).or_default();
            current_methods.extend(methods);
        }
    }

    /// Like [`TypeRegistry::resolve_type()`], but:
    ///
    /// - If the type wasn't found, we return an error with the original [`LookupName`] and visitor in,
    ///   allowing them to be reused elsewhere.
    /// - If we need to internally resolve an inner type, for example some alias type name, or bitvec
    ///   store/order types, then we'll call the provided parent resolver to handle this.
    /// - If given a pallet scoped type, this will _only_ look for it in said pallet scope. You should
    ///   remove the pallet and try again if you also want to look up the type in the global scope.
    #[allow(clippy::result_large_err)]
    pub(crate) fn resolve_type_with_parent<'this, Parent, V>(
        &'this self,
        parent: &'this Parent,
        type_id: LookupName,
        visitor: V,
    ) -> Result<V::Value, TypeRegistryResolveWithParentError<V>>
    where
        Parent: TypeResolver<Error = TypeRegistryResolveError, TypeId = LookupName>,
        V: ResolvedTypeVisitor<'this, TypeId = LookupName>,
    {
        // The pallet that our lookups will be performed within:
        let pallet = type_id.pallet();

        match type_id.def() {
            LookupNameDef::Named(ty) => {
                // Types are uniquely hashed by name, pallet and number of generic params.
                let generic_params: u8 = ty
                    .param_defs()
                    .count()
                    .try_into()
                    .expect("Expecting 0-255 generic params");
                let Some((ty_key, type_info)) =
                    lookup(pallet, ty.name(), generic_params, &self.types)
                else {
                    return Err(TypeRegistryResolveWithParentError::TypeNotFound {
                        type_name: type_id,
                        visitor,
                    });
                };

                // Build a mapping from generic ident to the concrete type def we've been given.
                // We use this to update generic type names like Vec<T> into concrete ones that the
                // user can access in the registry, like Vec<u32>,
                let param_mapping: SmallVec<[(&str, LookupNameDef<'_>); 4]> = type_info
                    .params
                    .iter()
                    .map(|ident| ident.as_str())
                    .zip(ty.param_defs())
                    .collect();

                // Depending on the type description, we we call the relevant visitor callback
                // with the relevant details.
                match &type_info.desc {
                    TypeShape::NamedStructOf(fields) => {
                        let path_iter = ty_key.name.split("::");
                        let fields_iter = fields.iter().map(|field| Field {
                            name: Some(&field.name),
                            id: apply_param_mapping(pallet, field.value.clone(), &param_mapping),
                        });
                        Ok(visitor.visit_composite(path_iter, fields_iter))
                    }
                    TypeShape::UnnamedStructOf(tys) => {
                        let path_iter = ty_key.name.split("::");
                        let fields_iter = tys.iter().map(|ty| Field {
                            name: None,
                            id: apply_param_mapping(pallet, ty.clone(), &param_mapping),
                        });
                        Ok(visitor.visit_composite(path_iter, fields_iter))
                    }
                    TypeShape::EnumOf(variants) => {
                        let path_iter = ty_key.name.split("::");
                        let variants_iter = variants.iter().map(|var| Variant {
                            index: var.index,
                            name: &var.name,
                            fields: match &var.fields {
                                VariantDesc::NamedStructOf(fields) => {
                                    let field_iter = fields.iter().map(|field| Field {
                                        name: Some(&field.name),
                                        id: apply_param_mapping(
                                            pallet,
                                            field.value.clone(),
                                            &param_mapping,
                                        ),
                                    });
                                    Either::A(field_iter)
                                }
                                VariantDesc::UnnamedStructOf(fields) => {
                                    let field_iter = fields.iter().map(|ty| Field {
                                        name: None,
                                        id: apply_param_mapping(pallet, ty.clone(), &param_mapping),
                                    });
                                    Either::B(field_iter)
                                }
                            },
                        });
                        Ok(visitor.visit_variant(path_iter, variants_iter))
                    }
                    TypeShape::SequenceOf(seq) => {
                        let path_iter = ty_key.name.split("::");
                        let type_id = apply_param_mapping(pallet, seq.clone(), &param_mapping);
                        Ok(visitor.visit_sequence(path_iter, type_id))
                    }
                    TypeShape::BitSequence { order, store } => {
                        let order = apply_param_mapping(pallet, order.clone(), &param_mapping);
                        let store = apply_param_mapping(pallet, store.clone(), &param_mapping);

                        // This is quite verbose because, in the event of an error, we need to
                        // change the visitor type being handed back while avoiding consuming it
                        // in something like a `.map_err()` closure.
                        macro_rules! resolve_format {
                            ($ty:ident, $visitor:ident, $err_variant:ident) => {{
                                match parent.resolve_type($ty, $visitor) {
                                    // Found a type of the right shape:
                                    Ok(Some(v)) => v,
                                    // Didn't find a type of the right shape:
                                    Ok(None) => {
                                        return Err(TypeRegistryResolveWithParentError::Other(
                                            TypeRegistryResolveError::$err_variant,
                                        ))
                                    }
                                    // Some other error:
                                    Err(e) => {
                                        return Err(TypeRegistryResolveWithParentError::Other(e))
                                    }
                                }
                            }};
                        }

                        let order_format =
                            resolve_format!(order, OrderVisitor, UnexpectedBitOrderType);
                        let store_format =
                            resolve_format!(store, StoreVisitor, UnexpectedBitStoreType);

                        Ok(visitor.visit_bit_sequence(store_format, order_format))
                    }
                    TypeShape::Compact(ty) => {
                        let type_id = apply_param_mapping(pallet, ty.clone(), &param_mapping);
                        Ok(visitor.visit_compact(type_id))
                    }
                    TypeShape::Primitive(p) => Ok(visitor.visit_primitive(*p)),
                    TypeShape::AliasOf(ty) => {
                        let type_id = apply_param_mapping(pallet, ty.clone(), &param_mapping);
                        parent
                            .resolve_type(type_id, visitor)
                            .map_err(TypeRegistryResolveWithParentError::Other)
                    }
                }
            }
            LookupNameDef::Unnamed(ty) => {
                let type_ids = ty.param_defs().map(|def| def.into_type_name());
                Ok(visitor.visit_tuple(type_ids))
            }
            LookupNameDef::Array(ty) => {
                let len = ty.length();
                let type_id = ty.param_def().into_type_name();
                Ok(visitor.visit_array(type_id, len))
            }
        }
    }
}

impl TypeResolver for TypeRegistry {
    type TypeId = LookupName;
    type Error = TypeRegistryResolveError;

    fn resolve_type<'this, V: ResolvedTypeVisitor<'this, TypeId = Self::TypeId>>(
        &'this self,
        type_id: Self::TypeId,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.resolve_type(type_id, visitor)
    }
}

impl<'a> From<TypeRegistry> for alloc::borrow::Cow<'a, TypeRegistry> {
    fn from(value: TypeRegistry) -> Self {
        alloc::borrow::Cow::Owned(value)
    }
}

impl<'a> From<&'a TypeRegistry> for alloc::borrow::Cow<'a, TypeRegistry> {
    fn from(value: &'a TypeRegistry) -> Self {
        alloc::borrow::Cow::Borrowed(value)
    }
}

impl core::iter::FromIterator<(InsertName, TypeShape)> for TypeRegistry {
    fn from_iter<T: IntoIterator<Item = (InsertName, TypeShape)>>(iter: T) -> Self {
        let mut registry = TypeRegistry::empty();
        for (name, shape) in iter.into_iter() {
            registry.insert(name, shape);
        }
        registry
    }
}

// We store complex owned keys in our hashmap, but want to do lookups using borrowed values.
// That's surprisingly difficult to do! This is one approach; manually hash the borrowed values
// and then lookup corresponding entries by hash, manually comparing keys to find the right one.
// We need to ensure that the way we are hashing things here corresponds to hashing of `TypeKey`,
// so we manually impl `Hash` and use the same function in both places.
fn lookup<'map>(
    pallet: Option<&str>,
    name: &str,
    generic_params: u8,
    map: &'map HashMap<TypeKey, TypeInfo>,
) -> Option<(&'map TypeKey, &'map TypeInfo)> {
    // Manually construct the key hash; pallet first and then name and then generic_params due to struct order.
    let hash = {
        use core::hash::{BuildHasher, Hasher};
        let mut state = map.hasher().build_hasher();
        hash_key(pallet, name, generic_params, &mut state);
        state.finish()
    };

    // Look up the entry by hash, and then compare keys we get back to find the matching key.
    map.raw_entry()
        .from_hash(hash, |k| k.name == name && k.pallet.as_deref() == pallet)
}

#[derive(Copy, Clone)]
struct OrderVisitor;
impl<'resolver> scale_type_resolver::ResolvedTypeVisitor<'resolver> for OrderVisitor {
    type TypeId = LookupName;
    type Value = Option<BitsOrderFormat>;

    fn visit_unhandled(self, _kind: scale_type_resolver::UnhandledKind) -> Self::Value {
        None
    }
    fn visit_composite<Path, Fields>(self, mut path: Path, _fields: Fields) -> Self::Value
    where
        Path: scale_type_resolver::PathIter<'resolver>,
        Fields: scale_type_resolver::FieldIter<'resolver, Self::TypeId>,
    {
        // use the path to determine whether this is the Lsb0 or Msb0
        // ordering type we're looking for, returning None if not.
        if path.next()? != "bitvec" {
            return None;
        }
        if path.next()? != "order" {
            return None;
        }

        let ident = path.next()?;
        if ident == "Lsb0" {
            Some(BitsOrderFormat::Lsb0)
        } else if ident == "Msb0" {
            Some(BitsOrderFormat::Msb0)
        } else {
            None
        }
    }
}

#[derive(Copy, Clone)]
struct StoreVisitor;
impl<'resolver> scale_type_resolver::ResolvedTypeVisitor<'resolver> for StoreVisitor {
    type TypeId = LookupName;
    type Value = Option<BitsStoreFormat>;

    fn visit_unhandled(self, _kind: scale_type_resolver::UnhandledKind) -> Self::Value {
        None
    }
    fn visit_primitive(self, p: Primitive) -> Self::Value {
        // use the primitive type to pick the correct bit store format.
        match p {
            Primitive::U8 => Some(BitsStoreFormat::U8),
            Primitive::U16 => Some(BitsStoreFormat::U16),
            Primitive::U32 => Some(BitsStoreFormat::U32),
            Primitive::U64 => Some(BitsStoreFormat::U64),
            _ => None,
        }
    }
}

/// Takes a LookupName and a mapping from generic ident to new defs, and returns a new LookupName where
/// said generic params are replaced with concrete types.
fn apply_param_mapping(
    pallet: Option<&str>,
    ty: LookupName,
    param_mapping: &SmallVec<[(&str, LookupNameDef<'_>); 4]>,
) -> LookupName {
    let new_ty = param_mapping
        .iter()
        .fold(ty, |ty, (ident, def)| ty.with_substitution(ident, *def));

    // Scope lookups of the new type name to whatever pallet the original was defined in
    if let Some(pallet) = pallet {
        new_ty.in_pallet(pallet)
    } else {
        new_ty
    }
}

// A quick enum iterator to be able to handle two branches without boxing, above.
enum Either<A, B> {
    A(A),
    B(B),
}

impl<Item, A: Iterator<Item = Item>, B: Iterator<Item = Item>> Iterator for Either<A, B> {
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Either::A(a) => a.next(),
            Either::B(b) => b.next(),
        }
    }
}
impl<Item, A: ExactSizeIterator<Item = Item>, B: ExactSizeIterator<Item = Item>> ExactSizeIterator
    for Either<A, B>
{
    fn len(&self) -> usize {
        match self {
            Either::A(a) => a.len(),
            Either::B(b) => b.len(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::to_resolved_info;
    use alloc::boxed::Box;

    type ResolvedTypeInfo = crate::test_utils::ResolvedTypeInfo<TypeRegistryResolveError>;

    #[test]
    fn resolve_basic_types() {
        let types = TypeRegistry::basic();

        let cases = [
            ("bool", ResolvedTypeInfo::Primitive(Primitive::Bool)),
            ("char", ResolvedTypeInfo::Primitive(Primitive::Char)),
            ("String", ResolvedTypeInfo::Primitive(Primitive::Str)),
            ("u8", ResolvedTypeInfo::Primitive(Primitive::U8)),
            ("u16", ResolvedTypeInfo::Primitive(Primitive::U16)),
            ("u32", ResolvedTypeInfo::Primitive(Primitive::U32)),
            ("u64", ResolvedTypeInfo::Primitive(Primitive::U64)),
            ("u128", ResolvedTypeInfo::Primitive(Primitive::U128)),
            ("i8", ResolvedTypeInfo::Primitive(Primitive::I8)),
            ("i16", ResolvedTypeInfo::Primitive(Primitive::I16)),
            ("i32", ResolvedTypeInfo::Primitive(Primitive::I32)),
            ("i64", ResolvedTypeInfo::Primitive(Primitive::I64)),
            ("i128", ResolvedTypeInfo::Primitive(Primitive::I128)),
            (
                "Vec<bool>",
                ResolvedTypeInfo::SequenceOf(Box::new(ResolvedTypeInfo::Primitive(
                    Primitive::Bool,
                ))),
            ),
            ("Box<bool>", ResolvedTypeInfo::Primitive(Primitive::Bool)),
            ("Arc<bool>", ResolvedTypeInfo::Primitive(Primitive::Bool)),
            ("Rc<bool>", ResolvedTypeInfo::Primitive(Primitive::Bool)),
            (
                "[String; 32]",
                ResolvedTypeInfo::ArrayOf(
                    Box::new(ResolvedTypeInfo::Primitive(Primitive::Str)),
                    32,
                ),
            ),
            (
                "(bool, u32, Vec<String>)",
                ResolvedTypeInfo::TupleOf(vec![
                    ResolvedTypeInfo::Primitive(Primitive::Bool),
                    ResolvedTypeInfo::Primitive(Primitive::U32),
                    ResolvedTypeInfo::SequenceOf(Box::new(ResolvedTypeInfo::Primitive(
                        Primitive::Str,
                    ))),
                ]),
            ),
            (
                "bitvec::vec::BitVec<u8, bitvec::order::Msb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U8, BitsOrderFormat::Msb0),
            ),
            (
                "bitvec::vec::BitVec<u16, bitvec::order::Msb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U16, BitsOrderFormat::Msb0),
            ),
            (
                "bitvec::vec::BitVec<u32, bitvec::order::Msb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U32, BitsOrderFormat::Msb0),
            ),
            (
                "bitvec::vec::BitVec<u64, bitvec::order::Msb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U64, BitsOrderFormat::Msb0),
            ),
            (
                "bitvec::vec::BitVec<u8, bitvec::order::Lsb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U8, BitsOrderFormat::Lsb0),
            ),
            (
                "bitvec::vec::BitVec<u16, bitvec::order::Lsb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U16, BitsOrderFormat::Lsb0),
            ),
            (
                "bitvec::vec::BitVec<u32, bitvec::order::Lsb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U32, BitsOrderFormat::Lsb0),
            ),
            (
                "bitvec::vec::BitVec<u64, bitvec::order::Lsb0>",
                ResolvedTypeInfo::BitSequence(BitsStoreFormat::U64, BitsOrderFormat::Lsb0),
            ),
            (
                "Option<char>",
                ResolvedTypeInfo::VariantOf(vec![
                    ("None".to_owned(), vec![]),
                    (
                        "Some".to_owned(),
                        vec![(None, ResolvedTypeInfo::Primitive(Primitive::Char))],
                    ),
                ]),
            ),
            (
                "Result<char, String>",
                ResolvedTypeInfo::VariantOf(vec![
                    (
                        "Ok".to_owned(),
                        vec![(None, ResolvedTypeInfo::Primitive(Primitive::Char))],
                    ),
                    (
                        "Err".to_owned(),
                        vec![(None, ResolvedTypeInfo::Primitive(Primitive::Str))],
                    ),
                ]),
            ),
        ];

        for (name, expected) in cases.into_iter() {
            assert_eq!(to_resolved_info(name, &types), expected);
        }
    }

    #[test]
    fn resolve_aliases() {
        let mut types = TypeRegistry::basic();

        // We test a bunch of things here:
        // - Multiple indirection of aliases,
        // - Aliases with generics,
        // - That bitvec resolving works through aliases.
        types
            .insert_str(
                "BitVecLsb0Alias1",
                TypeShape::AliasOf(LookupName::parse("bitvec::order::Lsb0").unwrap()),
            )
            .unwrap();
        types
            .insert_str(
                "BitVecLsb0Alias2",
                TypeShape::AliasOf(LookupName::parse("BitVecLsb0Alias1").unwrap()),
            )
            .unwrap();
        types
            .insert_str(
                "AliasForU16",
                TypeShape::AliasOf(LookupName::parse("u16").unwrap()),
            )
            .unwrap();
        types
            .insert_str(
                "AliasForBitVec<S,O>",
                TypeShape::AliasOf(LookupName::parse("bitvec::vec::BitVec<S,O>").unwrap()),
            )
            .unwrap();

        let resolved = to_resolved_info("AliasForBitVec<AliasForU16, BitVecLsb0Alias2>", &types);

        assert_eq!(
            resolved,
            ResolvedTypeInfo::BitSequence(BitsStoreFormat::U16, BitsOrderFormat::Lsb0)
        );
    }

    #[test]
    fn resolve_pallet_scoped_types() {
        const PALLET: &str = "balances";
        let mut types = TypeRegistry::empty();

        // Global types
        types
            .insert_str("u16", TypeShape::Primitive(Primitive::U16))
            .unwrap();
        types
            .insert_str("Primitive", TypeShape::Primitive(Primitive::U16))
            .unwrap();

        // Type in balances pallet to override it. (u128, not u16 here)
        types.insert(
            InsertName::parse("Primitive").unwrap().in_pallet(PALLET),
            TypeShape::Primitive(Primitive::U128),
        );

        // A couple of composite scoped types:
        types.insert(
            InsertName::parse("Foo<T>").unwrap().in_pallet(PALLET),
            TypeShape::SequenceOf(LookupName::parse("T").unwrap()),
        );
        types.insert(
            InsertName::parse("Bar<T>").unwrap().in_pallet(PALLET),
            TypeShape::SequenceOf(LookupName::parse("T").unwrap()),
        );

        // Now, try resolving some types in the context of the "balances" pallet to check that we use scoped types appropriately.
        let scoped_tests = [
            // Will still find global types if not overridden:
            ("u16", ResolvedTypeInfo::Primitive(Primitive::U16)),
            // Will use overridden pallet scoped types:
            ("Primitive", ResolvedTypeInfo::Primitive(Primitive::U128)),
            // Pallet scoped-ness is preserved through complex type lookups:
            (
                "Foo<Primitive>",
                ResolvedTypeInfo::SequenceOf(Box::new(ResolvedTypeInfo::Primitive(
                    Primitive::U128,
                ))),
            ),
            (
                "Foo<Bar<Primitive>>",
                ResolvedTypeInfo::SequenceOf(Box::new(ResolvedTypeInfo::SequenceOf(Box::new(
                    ResolvedTypeInfo::Primitive(Primitive::U128),
                )))),
            ),
        ];

        for (input, expected) in scoped_tests {
            let scoped_name = LookupName::parse(input).unwrap().in_pallet("balances");
            let actual = to_resolved_info(scoped_name, &types);
            assert_eq!(expected, actual, "error with type name {input}");
        }
    }

    #[test]
    fn test_runtime_apis() {
        let mut types = TypeRegistry::empty();

        types
            .try_insert_runtime_api(
                "Foo",
                "methodname",
                [("version", "u32"), ("id", "AccountId32")],
                "bool",
            )
            .unwrap();

        types
            .try_insert_runtime_api(
                "Bar",
                "other",
                [("arg", LookupName::parse("Wibble").unwrap())],
                LookupName::parse("u64").unwrap(),
            )
            .unwrap();

        let foo = types.runtime_api("Foo", "methodname").unwrap();
        assert_eq!(
            &foo.inputs,
            &[
                RuntimeApiInput {
                    name: "version".into(),
                    id: LookupName::parse("u32").unwrap()
                },
                RuntimeApiInput {
                    name: "id".into(),
                    id: LookupName::parse("AccountId32").unwrap()
                }
            ]
        );
        assert_eq!(&foo.output, &LookupName::parse("bool").unwrap());

        let bar = types.runtime_api("Bar", "other").unwrap();
        assert_eq!(
            &bar.inputs,
            &[RuntimeApiInput {
                name: "arg".into(),
                id: LookupName::parse("Wibble").unwrap()
            },]
        );
        assert_eq!(&bar.output, &LookupName::parse("u64").unwrap());
    }
}
