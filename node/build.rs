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

use std::{borrow::Cow, env, process::Command};

use substrate_build_script_utils::rerun_if_git_head_changed;

fn git_commit() -> Cow<'static, str> {
    if let Ok(commit) = env::var("GIT_COMMIT") {
        let commit = commit.trim();
        if !commit.is_empty() {
            return Cow::Owned(commit.to_owned());
        }
    }

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            Cow::Owned(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        }
        Ok(output) => {
            println!(
                "cargo:warning=Git command failed with status: {}",
                output.status
            );
            Cow::Borrowed("unknown")
        }
        Err(error) => {
            println!("cargo:warning=Failed to execute git command: {}", error);
            Cow::Borrowed("unknown")
        }
    }
}

fn target_platform() -> String {
    env::var("TARGET").unwrap_or_else(|_| "unknown-target".into())
}

fn impl_version() -> String {
    let package_version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let commit = git_commit();

    if commit.is_empty() {
        format!("{package_version}-{}", target_platform())
    } else {
        format!("{package_version}-{commit}-{}", target_platform())
    }
}

fn main() {
    println!(
        "cargo:rustc-env=SUBSTRATE_CLI_IMPL_VERSION={}",
        impl_version()
    );
    println!("cargo:rerun-if-env-changed=GIT_COMMIT");
    rerun_if_git_head_changed();
}
