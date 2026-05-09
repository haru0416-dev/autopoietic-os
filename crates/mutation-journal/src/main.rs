use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use autopoietic_core::{
    EffectRecord, EffectRisk, GenerationRecord, MutationRecord, MutationStatus,
};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(about = "Append Autopoietic OS journal entries")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Append(MutationArgs),
    Effect(EffectArgs),
    Generation(GenerationArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MutationStatusArg {
    Pending,
    Accepted,
    Failed,
    Reverted,
}

impl From<MutationStatusArg> for MutationStatus {
    fn from(value: MutationStatusArg) -> Self {
        match value {
            MutationStatusArg::Pending => Self::Pending,
            MutationStatusArg::Accepted => Self::Accepted,
            MutationStatusArg::Failed => Self::Failed,
            MutationStatusArg::Reverted => Self::Reverted,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EffectRiskArg {
    Low,
    Medium,
    High,
    Irreversible,
}

impl From<EffectRiskArg> for EffectRisk {
    fn from(value: EffectRiskArg) -> Self {
        match value {
            EffectRiskArg::Low => Self::Low,
            EffectRiskArg::Medium => Self::Medium,
            EffectRiskArg::High => Self::High,
            EffectRiskArg::Irreversible => Self::Irreversible,
        }
    }
}

#[derive(Debug, Parser)]
struct MutationArgs {
    #[arg(long, default_value = "memory/mutations.jsonl")]
    path: PathBuf,
    #[arg(long)]
    mutation_id: Option<String>,
    #[arg(long)]
    goal: String,
    #[arg(long, value_enum, default_value_t = MutationStatusArg::Pending)]
    status: MutationStatusArg,
    #[arg(long)]
    phase: String,
    #[arg(long, default_value = "")]
    reason: String,
    #[arg(long = "changed-path")]
    changed_paths: Vec<String>,
    #[arg(long, default_value = "")]
    next_hypothesis: String,
    #[arg(long = "metadata")]
    metadata: Vec<String>,
}

#[derive(Debug, Parser)]
struct EffectArgs {
    #[arg(long, default_value = "memory/effects.jsonl")]
    path: PathBuf,
    #[arg(long)]
    effect_id: Option<String>,
    #[arg(long)]
    mutation_id: String,
    #[arg(long = "type")]
    effect_type: String,
    #[arg(long)]
    target: String,
    #[arg(long, default_value_t = false)]
    reversible: bool,
    #[arg(long, default_value = "")]
    compensation: String,
    #[arg(long, default_value = "")]
    verified_by: String,
    #[arg(long, value_enum, default_value_t = EffectRiskArg::Low)]
    risk: EffectRiskArg,
    #[arg(long = "metadata")]
    metadata: Vec<String>,
}

#[derive(Debug, Parser)]
struct GenerationArgs {
    #[arg(long, default_value = "memory/generations.jsonl")]
    path: PathBuf,
    #[arg(long)]
    generation: String,
    #[arg(long)]
    mutation_id: String,
    #[arg(long)]
    goal: String,
    #[arg(long = "changed-organ")]
    changed_organs: Vec<String>,
    #[arg(long)]
    parent_generation: Option<String>,
    #[arg(long, default_value = "unknown")]
    activation_result: String,
    #[arg(long = "metadata")]
    metadata: Vec<String>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn id(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    )
}

fn parse_metadata(items: &[String]) -> Result<BTreeMap<String, String>> {
    let mut values = BTreeMap::new();
    for item in items {
        let Some((key, value)) = item.split_once('=') else {
            bail!("metadata must be key=value, got: {item}");
        };
        values.insert(key.to_owned(), value.to_owned());
    }
    Ok(values)
}

fn append_jsonl<T: serde::Serialize>(path: &PathBuf, entry: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create journal directory {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open journal {}", path.display()))?;
    serde_json::to_writer(&mut file, entry).context("failed to serialize journal entry")?;
    file.write_all(b"\n")
        .context("failed to terminate journal entry")?;
    Ok(())
}

fn append_mutation(args: MutationArgs) -> Result<MutationRecord> {
    let record = MutationRecord {
        mutation_id: args.mutation_id.unwrap_or_else(|| id("mut")),
        timestamp: now_iso(),
        goal: args.goal,
        status: args.status.into(),
        phase: args.phase,
        reason: args.reason,
        changed_paths: args.changed_paths,
        next_hypothesis: args.next_hypothesis,
        metadata: parse_metadata(&args.metadata)?,
    };
    append_jsonl(&args.path, &record)?;
    Ok(record)
}

fn append_effect(args: EffectArgs) -> Result<EffectRecord> {
    let record = EffectRecord {
        effect_id: args.effect_id.unwrap_or_else(|| id("eff")),
        timestamp: now_iso(),
        mutation_id: args.mutation_id,
        effect_type: args.effect_type,
        target: args.target,
        reversible: args.reversible,
        compensation: args.compensation,
        verified_by: args.verified_by,
        risk: args.risk.into(),
        metadata: parse_metadata(&args.metadata)?,
    };
    append_jsonl(&args.path, &record)?;
    Ok(record)
}

fn append_generation(args: GenerationArgs) -> Result<GenerationRecord> {
    let record = GenerationRecord {
        timestamp: now_iso(),
        generation: args.generation,
        mutation_id: args.mutation_id,
        goal: args.goal,
        changed_organs: args.changed_organs,
        parent_generation: args.parent_generation,
        activation_result: args.activation_result,
        metadata: parse_metadata(&args.metadata)?,
    };
    append_jsonl(&args.path, &record)?;
    Ok(record)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let entry = match args.command {
        Command::Append(args) => serde_json::to_value(append_mutation(args)?)?,
        Command::Effect(args) => serde_json::to_value(append_effect(args)?)?,
        Command::Generation(args) => serde_json::to_value(append_generation(args)?)?,
    };
    println!("{}", serde_json::to_string_pretty(&entry)?);
    Ok(())
}
