// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Item<Value: PartialEq> {
    Original(Value),
    Updated(Value),
    Removed,
}

impl<Value: PartialEq> Item<Value> {
    pub fn value(&self) -> Option<&Value> {
        match self {
            Item::Original(value) => Some(value),
            Item::Updated(value) => Some(value),
            Item::Removed => None,
        }
    }

    fn mark_as_updated(&mut self) {
        let value = core::mem::replace(self, Item::Removed);
        *self = match value {
            Item::Original(v) => Item::Updated(v),
            Item::Updated(v) => Item::Updated(v),
            Item::Removed => Item::Removed,
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut Value> {
        self.mark_as_updated();
        match self {
            Item::Original(value) => Some(value),
            Item::Updated(value) => Some(value),
            Item::Removed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Item;

    #[test]
    fn enum_variant_switches() {
        let mut item: Item<i32> = Item::Original(123);
        item.mark_as_updated();
        assert_eq!(item, Item::Updated(123));
        // should be idempotent
        item.mark_as_updated();
        assert_eq!(item, Item::Updated(123));

        let mut item: Item<i32> = Item::Removed;
        item.mark_as_updated();
        assert_eq!(item, Item::Removed);
    }
}
