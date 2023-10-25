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

use sp_core::{Bytes, OpaqueMetadata};
use std::path::PathBuf;
use std::str::FromStr;

fn main() {
    let metadata: Bytes = OpaqueMetadata::new(framenode_runtime::Runtime::metadata().into()).into();
    let out_dir = PathBuf::from_str(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .unwrap()
        .join("bytes");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("metadata.scale"), metadata.0).unwrap();
    let workspace_root = PathBuf::from_str(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    println!("cargo:rerun-if-changed={workspace_root}/runtime");
    println!("cargo:rerun-if-changed={workspace_root}/pallets");
    println!("cargo:rerun-if-changed={workspace_root}/common");
}
