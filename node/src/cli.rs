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

use sc_cli::RunCmd;

#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[clap(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub run: RunCmd,

    /// Disable BEEFY gadget. Enabled by default
    #[clap(long)]
    pub disable_beefy: bool,

    /// Directory of public recovery inherent payloads named by parent hash.
    #[clap(long)]
    pub recovery_inherent_dir: Option<std::path::PathBuf>,

    /// Fresh evidence directory for a passive recovery verification node.
    #[clap(long)]
    pub recovery_evidence_dir: Option<std::path::PathBuf>,

    /// Public local HASHI witness requests/results; never signs or submits transactions.
    #[clap(long, requires = "recovery_hashi_release_sha256")]
    pub recovery_hashi_witness_dir: Option<std::path::PathBuf>,

    /// Independently admitted outer release-manifest SHA-256 (0x-prefixed).
    #[clap(long, requires = "recovery_hashi_witness_dir")]
    pub recovery_hashi_release_sha256: Option<sp_core::H256>,

    /// Legacy numeric check for an already acknowledged guardian halt.
    ///
    /// This never releases a pending halt. Use the exact observed-halt digest
    /// printed by the node for a new manual release, including after a reorg.
    #[clap(long, value_name = "NUMBER")]
    pub bridge_resume_after_halt: Option<u32>,

    /// Release this node's exact durable observed-halt epoch after review.
    ///
    /// Best and finalized guardian state must agree. A new signal, disappearance
    /// or rewind requires a new digest; keeping an old flag cannot acknowledge it.
    /// This only changes local acknowledgement and never enables worker mode.
    #[clap(long, value_name = "CONTEXT_HASH")]
    pub bridge_resume_after_observed_halt: Option<sp_core::H256>,

    /// Acknowledge this node's completed recovery context before HASHI signing.
    ///
    /// Pass the exact 0x-prefixed context digest shown by this node. Best and
    /// finalized state must agree. This writes a local acknowledgement only;
    /// guardian halts and --offchain-worker mode remain independent.
    #[clap(long, value_name = "CONTEXT_HASH")]
    pub bridge_resume_after_recovery: Option<sp_core::H256>,

    /// Review specific Ethereum deposit blocks alongside the normal historical scan.
    /// SHA-256 of bridge/priority-scan.json. Requires an explicitly enabled worker.
    /// While armed, HASHI signing/retries require the pinned contract and spec 142.
    /// Retire a stale plan by removing this option on a reviewed worker restart.
    #[clap(long, value_name = "SHA256")]
    pub bridge_priority_scan: Option<sp_core::H256>,
}

impl Cli {
    /// Retained validator keys alone must not opt a recovery node into bridge bootstrap.
    pub(crate) fn bridge_worker_options(
        &self,
    ) -> crate::bridge_bootstrap_policy::BridgeWorkerOptions {
        crate::bridge_bootstrap_policy::BridgeWorkerOptions {
            recovery_bootstrap: matches!(
                self.run.offchain_worker_params.enabled,
                sc_cli::OffchainWorkerEnabled::Always
            ),
            resume_after_halt: self.bridge_resume_after_halt,
            resume_after_observed_halt: self.bridge_resume_after_observed_halt,
            resume_after_recovery: self.bridge_resume_after_recovery,
            priority_scan_sha256: self.bridge_priority_scan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn recovery_bridge_bootstrap_requires_explicit_always() {
        for arguments in [
            vec!["framenode"],
            vec!["framenode", "--validator"],
            vec![
                "framenode",
                "--validator",
                "--offchain-worker",
                "when-authority",
            ],
            vec!["framenode", "--validator", "--offchain-worker", "never"],
        ] {
            let cli = Cli::try_parse_from(arguments).expect("valid node arguments");
            assert!(!cli.bridge_worker_options().recovery_bootstrap);
        }
        for arguments in [
            vec!["framenode", "--offchain-worker", "always"],
            vec!["framenode", "--validator", "--offchain-worker=always"],
        ] {
            let cli = Cli::try_parse_from(arguments).expect("valid bridge peer arguments");
            assert!(cli.bridge_worker_options().recovery_bootstrap);
        }
    }

    #[test]
    fn observed_halt_cli_requires_exact_digest_without_enabling_workers() {
        assert!(Cli::try_parse_from(["framenode", "--bridge-resume-after-observed-halt"]).is_err());
        assert!(
            Cli::try_parse_from(["framenode", "--bridge-resume-after-observed-halt", "1"]).is_err()
        );
        let digest = format!("{:#x}", sp_core::H256::repeat_byte(7));
        let cli = Cli::try_parse_from([
            "framenode",
            "--offchain-worker=never",
            "--bridge-resume-after-observed-halt",
            &digest,
        ])
        .unwrap();
        let options = cli.bridge_worker_options();
        assert_eq!(
            options.resume_after_observed_halt,
            Some(sp_core::H256::repeat_byte(7))
        );
        assert_eq!(options.resume_after_halt, None);
        assert_eq!(options.resume_after_recovery, None);
        assert!(!options.recovery_bootstrap);
    }

    #[test]
    fn recovery_acknowledgement_cli_requires_digest_and_does_not_enable_workers() {
        assert!(Cli::try_parse_from(["framenode", "--bridge-resume-after-recovery"]).is_err());
        assert!(
            Cli::try_parse_from(["framenode", "--bridge-resume-after-recovery", "true"]).is_err()
        );
        let digest = format!("{:#x}", sp_core::H256::repeat_byte(7));
        let cli = Cli::try_parse_from([
            "framenode",
            "--offchain-worker=never",
            "--bridge-resume-after-recovery",
            &digest,
        ])
        .unwrap();
        let options = cli.bridge_worker_options();
        assert_eq!(
            options.resume_after_recovery,
            Some(sp_core::H256::repeat_byte(7))
        );
        assert_eq!(options.resume_after_halt, None);
        assert!(!options.recovery_bootstrap);
    }
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Key management cli utilities
    #[clap(subcommand)]
    Key(sc_cli::KeySubcommand),
    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Prepare a separate, manifest-bound recovery specification offline.
    PrepareRecoverySpec(crate::recovery::PrepareRecoverySpecCmd),

    /// Verify persisted recovery state and finality in a stopped owned database copy.
    VerifyRecoveryDatabase(crate::recovery_evidence::VerifyRecoveryDatabaseCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Inspect or explicitly repair the known SORA mainnet BABE epoch cache mismatch offline.
    RepairBabeEpochCache(crate::babe_epoch_repair::RepairBabeEpochCacheCmd),

    /// Generate chainspec for network fork-off
    #[cfg(feature = "private-net")]
    ForkOff(crate::fork_off::ForkOffCmd),

    /// The custom benchmark subcommmand benchmarking runtime pallets.
    #[cfg(feature = "runtime-benchmarks")]
    #[clap(subcommand)]
    Benchmark(Box<frame_benchmarking_cli::BenchmarkCmd>),
}
