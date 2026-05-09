#![deny(clippy::correctness)]
#![warn(clippy::suspicious, clippy::style, clippy::complexity, clippy::perf)]

mod verifier;

use std::path::PathBuf;

use anyhow::{Result, bail};
use autopoietic_core::VerificationStatus;
use clap::{Parser, Subcommand};
use verifier::{VerifyConfig, verify_and_record};

#[derive(Debug, Parser)]
#[command(about = "Verify Autopoietic NixOS mutation proposals without live mutation")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Verify(VerifyArgs),
}

#[derive(Debug, Parser)]
struct VerifyArgs {
    #[arg(long)]
    proposal: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, default_value = "memory/mutation-results.jsonl")]
    journal: PathBuf,
    #[arg(long)]
    work_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    keep_worktree: bool,
}

impl From<VerifyArgs> for VerifyConfig {
    fn from(value: VerifyArgs) -> Self {
        Self {
            proposal_path: value.proposal,
            root: value.root,
            journal_path: value.journal,
            work_dir: value.work_dir,
            keep_worktree: value.keep_worktree,
            skip_default_checks: false,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let record = match args.command {
        Command::Verify(args) => verify_and_record(args.into())?,
    };
    println!("{}", serde_json::to_string_pretty(&record)?);
    if record.status == VerificationStatus::Verified {
        Ok(())
    } else {
        bail!("proposal {}", record.status_reason())
    }
}

trait StatusReason {
    fn status_reason(&self) -> &'static str;
}

impl StatusReason for autopoietic_core::MutationVerificationRecord {
    fn status_reason(&self) -> &'static str {
        match self.status {
            VerificationStatus::Verified => "verified",
            VerificationStatus::Rejected => "rejected",
            VerificationStatus::Error => "errored",
        }
    }
}
