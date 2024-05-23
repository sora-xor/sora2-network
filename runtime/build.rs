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

use std::path::PathBuf;
use std::{env, fs};

#[cfg(feature = "build-wasm-binary")]
use substrate_wasm_builder::WasmBuilder;

fn main() {
    // #[cfg(feature = "build-wasm-binary")]
    // WasmBuilder::new()
    //     .with_current_project()
    //     .import_memory()
    //     .export_heap_base()
    //     .build();

    // let root_path = PathBuf::new()
    //     .join(env::var("CARGO_MANIFEST_DIR").unwrap())
    //     .parent()
    //     .unwrap()
    //     .to_owned();
    // let pre_commit_hook_path = root_path.join(".hooks/pre-commit");
    // println!(
    //     "cargo:rerun-if-changed={}",
    //     pre_commit_hook_path.to_string_lossy()
    // );
    // let enabled_hooks_dir = root_path.join(".git/hooks");
    // fs::create_dir_all(&enabled_hooks_dir).expect("Failed to create '.git/hooks' dir");
    // fs::copy(&pre_commit_hook_path, enabled_hooks_dir.join("pre-commit"))
    //     .expect("Failed to copy '.hooks/pre_commit' to '.git/hooks/pre_commit'");
}
