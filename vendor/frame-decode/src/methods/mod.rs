// Copyright (C) 2022-2025 Parity Technologies (UK) Ltd. (admin@parity.io)
// This file is a part of the frame-decode crate.
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

pub mod constant_decoder;
pub mod constant_type_info;
pub mod custom_value_decoder;
pub mod custom_value_type_info;
pub mod extrinsic_decoder;
pub mod extrinsic_encoder;
pub mod extrinsic_type_info;
pub mod runtime_api_decoder;
pub mod runtime_api_encoder;
pub mod runtime_api_type_info;
pub mod storage_decoder;
pub mod storage_encoder;
pub mod storage_type_info;
pub mod view_function_decoder;
pub mod view_function_encoder;
pub mod view_function_type_info;

/// This represents either an entry name, or the name of the thing that the entry is
/// in (for instance the name of a pallet or of a Runtime API trait).
///
/// Iterators returning this will iterate containers in order, first returning
/// [`Entry::In`] to communicate which container (eg pallet or Runtime API trait) is being
/// iterated over next, and then a [`Entry::Name`]s for the name of each of the entries in
/// the given container.
///
/// A container name will not be handed back more than once.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry<In, Name> {
    /// The name of the thing that the following entries are in/under.
    In(In),
    /// The name of the entry in/under the last given [`Entry::In`].
    Name(Name),
}

impl<T> Entry<T, T> {
    /// Map the values in the entry, assuming they are the same.
    pub fn map<R, F: FnOnce(T) -> R>(self, f: F) -> Entry<R, R> {
        match self {
            Entry::In(t) => Entry::In(f(t)),
            Entry::Name(t) => Entry::Name(f(t)),
        }
    }
}

impl<In, Name> Entry<In, Name> {
    /// Iterate over all of the entries in a specific container (ie all of the entries
    /// which follow a specific [`Entry::In`]).
    pub fn entries_in(
        entries: impl Iterator<Item = Entry<In, Name>>,
        container: impl PartialEq<In>,
    ) -> impl Iterator<Item = Name>
    where
        In: PartialEq,
        Name: PartialEq,
    {
        entries
            .skip_while(move |c| !matches!(c, Entry::In(c) if &container == c))
            .skip(1)
            .take_while(|c| matches!(c, Entry::Name(_)))
            .filter_map(|c| match c {
                Entry::In(_) => None,
                Entry::Name(name) => Some(name),
            })
    }

    /// Iterate over all of the entries, returning tuples of `(entry_in, entry_name)`.
    /// This can be easier to work with in some cases.
    pub fn tuples_of(
        entries: impl Iterator<Item = Entry<In, Name>>,
    ) -> impl Iterator<Item = (In, Name)>
    where
        In: Clone,
    {
        let mut entry_in = None;
        entries.filter_map(move |entry| match entry {
            Entry::In(e_in) => {
                entry_in = Some(e_in);
                None
            }
            Entry::Name(e_name) => {
                let e_in = entry_in.as_ref().unwrap().clone();
                Some((e_in, e_name))
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_entries_in() {
        fn entries() -> impl Iterator<Item = Entry<&'static str, &'static str>> {
            [
                Entry::In("Baz"),
                Entry::In("Foo"),
                Entry::Name("foo_a"),
                Entry::Name("foo_b"),
                Entry::Name("foo_c"),
                Entry::In("Bar"),
                Entry::Name("bar_a"),
                Entry::In("Wibble"),
            ]
            .into_iter()
        }

        assert!(Entry::entries_in(entries(), "Baz").next().is_none());
        assert!(Entry::entries_in(entries(), "Wibble").next().is_none());

        let foos: Vec<String> = Entry::entries_in(entries(), "Foo")
            .map(|s| s.to_owned())
            .collect();
        assert_eq!(
            foos,
            Vec::from_iter(["foo_a".to_owned(), "foo_b".to_owned(), "foo_c".to_owned(),])
        );

        let bars: Vec<String> = Entry::entries_in(entries(), "Bar")
            .map(|s| s.to_owned())
            .collect();
        assert_eq!(bars, Vec::from_iter(["bar_a".to_owned(),]));
    }
}
