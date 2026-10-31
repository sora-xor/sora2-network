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

//! This module provides a [`TypeRegistrySet`], which is constructed from a selection of
//! [`crate::TypeRegistry`]'s, and will resolve types by looking through each one in a known order
//! until a match is found. This allows us to compose different sets of types into a single thing
//! which itself also implements [`scale_type_resolver::TypeResolver`].

use crate::type_registry::{
    RuntimeApi, RuntimeApiName, TypeRegistryResolveError, TypeRegistryResolveWithParentError,
};
use crate::{LookupName, TypeRegistry};
use alloc::borrow::Cow;
use alloc::collections::VecDeque;
use hashbrown::{HashMap, HashSet};
use scale_type_resolver::TypeResolver;

/// This can be constructed from an iterator of [`crate::TypeRegistry`]s. When resolving types
/// via [`scale_type_resolver::TypeResolver::resolve_type()`], it will treat the provided
/// registries as a stack, looking in the last provided registry first and then working back
/// through them until it can find the type, failing with an error if none of the registries
/// contain an answer.
#[derive(Debug)]
pub struct TypeRegistrySet<'a> {
    registries: VecDeque<Cow<'a, TypeRegistry>>,
}

impl<'a> TypeRegistrySet<'a> {
    /// Take ownership of this [`TypeRegistrySet`]. If the underlying type registries are
    /// borrowed, then they are cloned in order to take ownership of them.
    pub fn to_owned(self) -> TypeRegistrySet<'static> {
        let registries = self
            .registries
            .into_iter()
            .map(|r| Cow::Owned(r.into_owned()))
            .collect();
        TypeRegistrySet { registries }
    }

    /// Add some types to the beginning of the set of registries. These types will be
    /// checked after all of the others.
    pub fn prepend(&mut self, types: impl Into<Cow<'a, TypeRegistry>>) {
        self.registries.push_front(types.into());
    }

    /// Add some types to the end of the set of registries. These types will be
    /// checked before all of the others.
    pub fn append(&mut self, types: impl Into<Cow<'a, TypeRegistry>>) {
        self.registries.push_back(types.into());
    }

    /// Return a [`LookupName`] corresponding to one concrete type for every entry in the registry.
    /// Any generic parameters are substituted for `()` in order that we can return valid concrete types.
    pub fn keys(&self) -> impl Iterator<Item = LookupName> + use<'_> {
        use alloc::rc::Rc;
        use core::cell::RefCell;
        let seen = Rc::new(RefCell::new(HashSet::<LookupName>::new()));
        self.registries.iter().rev().flat_map(move |registry| {
            let seen = seen.clone();
            registry
                .keys()
                .filter(move |key| seen.borrow_mut().insert(key.clone()))
        })
    }

    /// Resolve some type information by providing a [`LookupName`], which is the concrete name of
    /// a type we want to know how to decode values for, and a `visitor` which will be called in
    /// order to describe how to decode it.
    ///
    /// This will work through the inner type registries from latest to earliest until it finds a
    /// matching type.
    pub fn resolve_type<
        'this,
        V: scale_type_resolver::ResolvedTypeVisitor<'this, TypeId = LookupName>,
    >(
        &'this self,
        mut type_id: LookupName,
        mut visitor: V,
    ) -> Result<V::Value, TypeRegistryResolveError> {
        macro_rules! resolve_type {
            () => {
                for registry in self.registries.iter().rev() {
                    match registry.resolve_type_with_parent(self, type_id, visitor) {
                        // Found a value! return it.
                        Ok(val) => return Ok(val),
                        // Hit some other error; return it.
                        Err(TypeRegistryResolveWithParentError::Other(e)) => return Err(e),
                        // The type wasn't found; we'll continue to the next registry in the vec.
                        Err(TypeRegistryResolveWithParentError::TypeNotFound {
                            type_name: tn,
                            visitor: v,
                        }) => {
                            type_id = tn;
                            visitor = v;
                        }
                    }
                }
            };
        }

        resolve_type!();

        // If the lookup was pallet scoped and we didn't find anything, then now we can remove
        // the pallet scope and try to lookup the type again in the global scope.
        if type_id.take_pallet().is_some() {
            resolve_type!();
        }

        // We couldn't find the type, so call the "not found" method on the visitor.
        Ok(visitor.visit_not_found())
    }

    /// Resolve some type information by providing the string name of the type,
    /// and a `visitor` which will be called in order to describe how to decode it.
    /// This just creates a [`LookupName`] under the hood and uses that to resolve the
    /// type.
    pub fn resolve_type_str<
        'this,
        V: scale_type_resolver::ResolvedTypeVisitor<'this, TypeId = LookupName>,
    >(
        &'this self,
        type_name_str: &str,
        visitor: V,
    ) -> Result<V::Value, TypeRegistryResolveError> {
        let type_id = LookupName::parse(type_name_str)
            .map_err(|e| TypeRegistryResolveError::LookupNameInvalid(type_name_str.into(), e))?;
        self.resolve_type(type_id, visitor)
    }

    /// Return a matching runtime API definition if one exists.
    ///
    /// This will work through the inner type registries from latest to earliest until it finds a
    /// matching API.
    pub fn runtime_api(&self, trait_name: &str, method_name: &str) -> Option<&RuntimeApi> {
        for registry in self.registries.iter().rev() {
            if let Some(api) = registry.runtime_api(trait_name, method_name) {
                return Some(api);
            }
        }
        None
    }

    /// Return an iterator over each of the Runtime APIs that has been defined. First,
    /// [`RuntimeApiName::Trait`] is returned to indicate the trait being iterated over next,
    /// and then [`RuntimeApiName::Method`] is returned for each of the method names in that
    /// trait. A trait will only be iterated over once.
    pub fn runtime_apis(&self) -> impl Iterator<Item = RuntimeApiName<'_>> {
        let it = self
            .registries
            .iter()
            .rev()
            .flat_map(|registry| registry.runtime_apis());

        // We need to aggregate across our registries so that we return each
        // trait name / entry exactly once.
        let mut all_apis = HashMap::<&str, HashSet<&str>>::new();
        let mut current_trait = "";
        for api in it {
            match api {
                RuntimeApiName::Trait(name) => current_trait = name,
                RuntimeApiName::Method(name) => {
                    all_apis.entry(current_trait).or_default().insert(name);
                }
            }
        }

        // Now we can iterate over this aggregation to match the format of `TypeRegistry::runtime_apis`.
        all_apis.into_iter().flat_map(|(trait_name, method_names)| {
            core::iter::once(RuntimeApiName::Trait(trait_name))
                .chain(method_names.into_iter().map(RuntimeApiName::Method))
        })
    }

    /// Return an iterator over each of the Runtime API method names in a given trait.
    pub fn runtime_apis_in_trait<'this, 'tn>(
        &'this self,
        trait_name: &'tn str,
    ) -> impl Iterator<Item = &'this str> + use<'this, 'tn> {
        let mut seen = HashSet::<&str>::new();

        self.registries
            .iter()
            .rev()
            .flat_map(|registry| registry.runtime_apis_in_trait(trait_name))
            .filter(move |method_name| !seen.insert(method_name))
    }
}

impl<'a, R: Into<Cow<'a, TypeRegistry>>> core::iter::FromIterator<R> for TypeRegistrySet<'a> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        TypeRegistrySet {
            registries: iter.into_iter().map(Into::into).collect(),
        }
    }
}

impl<'a> TypeResolver for TypeRegistrySet<'a> {
    type TypeId = LookupName;
    type Error = TypeRegistryResolveError;

    fn resolve_type<
        'this,
        V: scale_type_resolver::ResolvedTypeVisitor<'this, TypeId = Self::TypeId>,
    >(
        &'this self,
        type_id: Self::TypeId,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.resolve_type(type_id, visitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::{to_resolved_info, ResolvedTypeInfo};
    use crate::type_shape::Primitive;
    use crate::{InsertName, TypeShape};
    use alloc::vec;
    use scale_type_resolver::{BitsOrderFormat, BitsStoreFormat};

    fn ln(name: &str) -> LookupName {
        LookupName::parse(name).unwrap()
    }
    fn n(name: &str) -> InsertName {
        InsertName::parse(name).unwrap()
    }

    #[test]
    fn picks_pallet_scope_before_global() {
        let mut a = TypeRegistry::empty();
        a.insert_str("Val", TypeShape::Primitive(Primitive::I8))
            .unwrap();
        a.insert(n("Val").in_pallet("p"), TypeShape::Primitive(Primitive::U8));

        let mut b = TypeRegistry::empty();
        b.insert_str("Val", TypeShape::Primitive(Primitive::I16))
            .unwrap();

        let types = TypeRegistrySet::from_iter([a, b]);

        // Globally, we find the most recent val first.
        assert_eq!(
            to_resolved_info("Val", &types),
            ResolvedTypeInfo::Primitive(Primitive::I16)
        );
        // In pallet, we find the first in-pallet val anywhere before looking at any global ones.
        assert_eq!(
            to_resolved_info(ln("Val").in_pallet("p"), &types),
            ResolvedTypeInfo::Primitive(Primitive::U8)
        );
    }

    #[test]
    fn picks_last_registry_first() {
        let mut a = TypeRegistry::empty();
        a.insert_str("u8", TypeShape::Primitive(Primitive::U8))
            .unwrap();
        a.insert_str("Val", TypeShape::Primitive(Primitive::I8))
            .unwrap();

        let mut b = TypeRegistry::empty();
        b.insert_str("Val", TypeShape::Primitive(Primitive::I16))
            .unwrap();

        let mut c = TypeRegistry::empty();
        c.insert(
            n("Val").in_pallet("balances"),
            TypeShape::Primitive(Primitive::I32),
        );

        // Resolving will look in c, then b, then a.
        let types = TypeRegistrySet::from_iter([a, b, c]);

        // Sanity check; find val decalred inthe "bottom" registry only.
        assert_eq!(
            to_resolved_info("u8", &types),
            ResolvedTypeInfo::Primitive(Primitive::U8)
        );
        // Ignores the pallet val since we aren't in said pallet.
        assert_eq!(
            to_resolved_info("Val", &types),
            ResolvedTypeInfo::Primitive(Primitive::I16)
        );
        // Use the pallet specific val if the ID we pass is in that pallet.
        assert_eq!(
            to_resolved_info(ln("Val").in_pallet("balances"), &types),
            ResolvedTypeInfo::Primitive(Primitive::I32)
        );
    }

    #[test]
    fn resolve_bitvec_backwards_across_registries() {
        let mut a = TypeRegistry::empty();
        a.insert_str(
            "BitVec",
            TypeShape::BitSequence {
                order: ln("bitvec::order::Lsb0"),
                store: ln("Store"),
            },
        )
        .unwrap();

        let mut b = TypeRegistry::empty();
        b.insert_str("bitvec::order::Lsb0", TypeShape::NamedStructOf(vec![]))
            .unwrap();

        let mut c = TypeRegistry::empty();
        c.insert_str("Store", TypeShape::Primitive(Primitive::U8))
            .unwrap();

        // Resolving will look in c, then b, then a.
        let types = TypeRegistrySet::from_iter([a, b, c]);

        // Order and store types are both in different registries. Not only different,
        // but ones we've already looked in before we get to the BitVec.
        assert_eq!(
            to_resolved_info("BitVec", &types),
            ResolvedTypeInfo::BitSequence(BitsStoreFormat::U8, BitsOrderFormat::Lsb0)
        );
    }

    #[test]
    fn resolve_alias_backwards_across_registries() {
        let mut a = TypeRegistry::empty();
        a.insert_str("A", TypeShape::AliasOf(ln("B"))).unwrap();

        let mut b = TypeRegistry::empty();
        b.insert_str("B", TypeShape::AliasOf(ln("C"))).unwrap();

        let mut c = TypeRegistry::empty();
        c.insert_str("C", TypeShape::Primitive(Primitive::Bool))
            .unwrap();

        // Resolving will look in c, then b, then a.
        let types = TypeRegistrySet::from_iter([a, b, c]);

        // We try to resolve the alias A, which is in the last registry to be queried.
        // This will need to backtrack to previous registries in the list to resolve.
        assert_eq!(
            to_resolved_info("A", &types),
            ResolvedTypeInfo::Primitive(Primitive::Bool)
        );
    }

    #[test]
    fn runtime_apis_works_avoiding_dupes() {
        let mut a = TypeRegistry::empty();
        a.try_insert_runtime_api_without_inputs("A", "a1", "bool")
            .unwrap();
        a.try_insert_runtime_api_without_inputs("A", "a2", "u32")
            .unwrap();

        let mut b = TypeRegistry::empty();
        b.try_insert_runtime_api_without_inputs("A", "a2", "bool")
            .unwrap();

        let mut c = TypeRegistry::empty();
        c.try_insert_runtime_api_without_inputs("B", "b1", "bool")
            .unwrap();

        // Resolving will look in c, then b, then a.
        let types = TypeRegistrySet::from_iter([a, b, c]);

        // Return all APIs and aggregate them to check equality and ensure no dupes are returned:
        let mut all_apis = HashMap::<&str, HashSet<&str>>::new();
        let mut current_trait = "";
        for api in types.runtime_apis() {
            match api {
                RuntimeApiName::Trait(name) => {
                    // The trait name should not have been given before.
                    assert!(!all_apis.contains_key(name));

                    current_trait = name;
                }
                RuntimeApiName::Method(name) => {
                    // A trait name should have been given already.
                    assert!(!current_trait.is_empty());
                    // The method name should not have been given before.
                    assert!(!all_apis.entry(current_trait).or_default().contains(name));

                    all_apis.entry(current_trait).or_default().insert(name);
                }
            }
        }

        // This is the set of APIs we should see when iterating them all:
        assert_eq!(
            all_apis,
            HashMap::from_iter([
                ("A", HashSet::from_iter(["a1", "a2"])),
                ("B", HashSet::from_iter(["b1"]))
            ])
        );

        // They should all exist when queried as a sanity check:
        assert!(types.runtime_api("A", "a1").is_some());
        assert!(types.runtime_api("A", "a2").is_some());
        assert!(types.runtime_api("B", "b1").is_some());

        // Because of resolve order, we should see the "bool" version and not the "u32" version.
        assert_eq!(types.runtime_api("A", "a2").unwrap().output, ln("bool"));
    }

    #[test]
    fn keys_returned_without_dupes() {
        let mut a = TypeRegistry::empty();
        a.insert(
            InsertName::parse("u32").unwrap(),
            TypeShape::Primitive(Primitive::U32),
        );
        a.insert(
            InsertName::parse("u64").unwrap(),
            TypeShape::Primitive(Primitive::U64),
        );

        let mut b = TypeRegistry::empty();
        b.insert(
            InsertName::parse("u32").unwrap(),
            TypeShape::Primitive(Primitive::U32),
        );
        b.insert(
            InsertName::parse("u64").unwrap(),
            TypeShape::Primitive(Primitive::U64),
        );
        b.insert(
            InsertName::parse("Foo").unwrap(),
            TypeShape::NamedStructOf(vec![]),
        );
        b.insert(
            InsertName::parse("Bar").unwrap(),
            TypeShape::NamedStructOf(vec![]),
        );

        let mut c = TypeRegistry::empty();
        c.insert(
            InsertName::parse("Foo").unwrap(),
            TypeShape::Primitive(Primitive::U32),
        );

        // Resolving will look in c, then b, then a.
        let types = TypeRegistrySet::from_iter([a, b, c]);

        // No dupes back:
        let keys: HashSet<LookupName> = types.keys().collect();
        assert_eq!(
            keys,
            HashSet::from_iter([ln("Foo"), ln("u32"), ln("u64"), ln("Bar"),])
        );
    }
}
