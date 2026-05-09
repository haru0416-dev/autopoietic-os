#![deny(clippy::correctness)]
#![warn(clippy::suspicious, clippy::style, clippy::complexity, clippy::perf)]

mod promoter;
mod verifier;

use std::path::PathBuf;

use anyhow::{Result, bail};
use autopoietic_core::{PromotionStatus, VerificationStatus};
use clap::{Parser, Subcommand};
use promoter::{PromoteConfig, promote_and_record};
use verifier::{VerifyConfig, verify_and_record};

#[derive(Debug, Parser)]
#[command(about = "Verify Autopoietic OS mutation proposals without live mutation")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Verify(VerifyArgs),
    Promote(PromoteArgs),
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

#[derive(Debug, Parser)]
struct PromoteArgs {
    #[arg(long)]
    proposal: PathBuf,
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[arg(long, default_value = "memory/mutation-results.jsonl")]
    verification_journal: PathBuf,
    #[arg(long, default_value = "memory/mutation-promotions.jsonl")]
    journal: PathBuf,
    #[arg(long)]
    work_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    keep_worktree: bool,
    #[arg(long, default_value = "x86_64-linux")]
    system: String,
    #[arg(long, default_value = "iso")]
    target_configuration: String,
    #[arg(long = "vm-check")]
    vm_checks: Vec<String>,
    #[arg(long)]
    parent_genome: String,
    #[arg(long = "changed-organ")]
    changed_organs: Vec<String>,
}

impl From<PromoteArgs> for PromoteConfig {
    fn from(value: PromoteArgs) -> Self {
        Self {
            proposal_path: value.proposal,
            root: value.root,
            verification_journal_path: value.verification_journal,
            journal_path: value.journal,
            work_dir: value.work_dir,
            keep_worktree: value.keep_worktree,
            system: value.system,
            target_configuration: value.target_configuration,
            vm_checks: value.vm_checks,
            parent_genome: value.parent_genome,
            changed_organs: value.changed_organs,
            extra_checks: Vec::new(),
            skip_default_checks: false,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Verify(args) => {
            let record = verify_and_record(args.into())?;
            println!("{}", serde_json::to_string_pretty(&record)?);
            if record.status == VerificationStatus::Verified {
                Ok(())
            } else {
                bail!("proposal {}", record.status_reason())
            }
        }
        Command::Promote(args) => {
            let record = promote_and_record(args.into())?;
            println!("{}", serde_json::to_string_pretty(&record)?);
            if record.status == PromotionStatus::Promoted {
                Ok(())
            } else {
                bail!("proposal {}", record.status_reason())
            }
        }
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

impl StatusReason for autopoietic_core::MutationPromotionRecord {
    fn status_reason(&self) -> &'static str {
        match self.status {
            PromotionStatus::Promoted => "promoted",
            PromotionStatus::Rejected => "rejected",
            PromotionStatus::Error => "errored",
        }
    }
}
