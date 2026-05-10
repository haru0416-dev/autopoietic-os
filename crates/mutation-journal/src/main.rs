use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use autopoietic_core::{
    DecayStatus, DigestRef, EffectRecord, EffectRisk, EvidenceBundle, GenerationRecord,
    LineageStatus, MutationPromotionRecord, MutationRecord, MutationStatus, OrganRecord,
    OrganRegistrySuggestion, OrganRegistrySuggestionOutput, OrganReviewFinding, OrganReviewOutput,
    OrganReviewStatus, OrganType, PromotionStatus, ProvenanceRef,
};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use sha2::{Digest, Sha256};
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
    #[command(subcommand)]
    Organ(OrganCommand),
}

#[derive(Debug, Subcommand)]
enum OrganCommand {
    Add(OrganAddArgs),
    List(OrganListArgs),
    Review(OrganReviewArgs),
    Suggest(OrganSuggestArgs),
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OrganTypeArg {
    TmpTool,
    Cli,
    DevShell,
    Package,
    HomeManagerModule,
    NixosModule,
    SystemdService,
    SystemdTimer,
    Overlay,
}

impl From<OrganTypeArg> for OrganType {
    fn from(value: OrganTypeArg) -> Self {
        match value {
            OrganTypeArg::TmpTool => Self::TmpTool,
            OrganTypeArg::Cli => Self::Cli,
            OrganTypeArg::DevShell => Self::DevShell,
            OrganTypeArg::Package => Self::Package,
            OrganTypeArg::HomeManagerModule => Self::HomeManagerModule,
            OrganTypeArg::NixosModule => Self::NixosModule,
            OrganTypeArg::SystemdService => Self::SystemdService,
            OrganTypeArg::SystemdTimer => Self::SystemdTimer,
            OrganTypeArg::Overlay => Self::Overlay,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DecayStatusArg {
    Active,
    Candidate,
    Stale,
    Duplicate,
    Failed,
}

impl From<DecayStatusArg> for DecayStatus {
    fn from(value: DecayStatusArg) -> Self {
        match value {
            DecayStatusArg::Active => Self::Active,
            DecayStatusArg::Candidate => Self::Candidate,
            DecayStatusArg::Stale => Self::Stale,
            DecayStatusArg::Duplicate => Self::Duplicate,
            DecayStatusArg::Failed => Self::Failed,
        }
    }
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
    #[arg(long, value_enum, default_value_t = LineageStatusArg::Installed)]
    lineage_status: LineageStatusArg,
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
    #[arg(long)]
    verification_id: Option<String>,
    #[arg(long)]
    promotion_id: Option<String>,
    #[arg(long)]
    target_root: Option<String>,
    #[arg(long)]
    target_configuration: Option<String>,
    #[arg(long = "metadata")]
    metadata: Vec<String>,
}

#[derive(Debug, Parser)]
struct OrganAddArgs {
    #[arg(long, default_value = "memory/organs.jsonl")]
    path: PathBuf,
    #[arg(long)]
    name: String,
    #[arg(long = "type", value_enum)]
    organ_type: OrganTypeArg,
    #[arg(long)]
    source: String,
    #[arg(long)]
    purpose: String,
    #[arg(long)]
    created_by: Option<String>,
    #[arg(long)]
    usage_count: Option<u64>,
    #[arg(long)]
    failure_count: Option<u64>,
    #[arg(long = "related-goal")]
    related_goals: Vec<String>,
    #[arg(long, value_enum)]
    decay_status: Option<DecayStatusArg>,
}

#[derive(Debug, Parser)]
struct OrganListArgs {
    #[arg(long, default_value = "memory/organs.jsonl")]
    path: PathBuf,
}

#[derive(Debug, Parser)]
struct OrganReviewArgs {
    #[arg(long, default_value = "memory/organs.jsonl")]
    path: PathBuf,
    #[arg(long)]
    evidence_bundle: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct OrganSuggestArgs {
    #[arg(long, default_value = "memory/organs.jsonl")]
    registry: PathBuf,
    #[arg(long, default_value = "memory/mutation-promotions.jsonl")]
    promotions: PathBuf,
    #[arg(long, default_value = "memory/generations.jsonl")]
    generations: PathBuf,
    #[arg(long)]
    evidence_bundle: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LineageStatusArg {
    Planned,
    Installed,
    Failed,
}

impl From<LineageStatusArg> for LineageStatus {
    fn from(value: LineageStatusArg) -> Self {
        match value {
            LineageStatusArg::Planned => Self::Planned,
            LineageStatusArg::Installed => Self::Installed,
            LineageStatusArg::Failed => Self::Failed,
        }
    }
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

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(line)
            .with_context(|| format!("failed to parse {} line {}", path.display(), index + 1))?;
        records.push(record);
    }
    Ok(records)
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("organ {field} must not be empty");
    }
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
        lineage_status: args.lineage_status.into(),
        generation: args.generation,
        mutation_id: args.mutation_id,
        goal: args.goal,
        changed_organs: args.changed_organs,
        parent_generation: args.parent_generation,
        activation_result: args.activation_result,
        verification_id: args.verification_id,
        promotion_id: args.promotion_id,
        target_root: args.target_root,
        target_configuration: args.target_configuration,
        metadata: parse_metadata(&args.metadata)?,
    };
    append_jsonl(&args.path, &record)?;
    Ok(record)
}

fn add_organ(args: OrganAddArgs) -> Result<OrganRecord> {
    require_non_empty("name", &args.name)?;
    require_non_empty("source", &args.source)?;
    require_non_empty("purpose", &args.purpose)?;
    let record = OrganRecord {
        name: args.name,
        organ_type: args.organ_type.into(),
        source: args.source,
        purpose: args.purpose,
        created_by: args.created_by,
        usage_count: args.usage_count,
        failure_count: args.failure_count,
        related_goals: args.related_goals,
        decay_status: args.decay_status.map(Into::into),
    };
    append_jsonl(&args.path, &record)?;
    Ok(record)
}

fn list_organs(args: OrganListArgs) -> Result<Vec<OrganRecord>> {
    read_jsonl(&args.path)
}

fn review_organs(args: OrganReviewArgs) -> Result<OrganReviewOutput> {
    let records: Vec<OrganRecord> = read_jsonl(&args.path)?;
    let duplicate_names = duplicate_values(records.iter().map(|record| record.name.as_str()));
    let duplicate_sources = duplicate_values(records.iter().map(|record| record.source.as_str()));
    let mut active = Vec::new();
    let mut candidates = Vec::new();
    let mut stale = Vec::new();
    let mut duplicate = Vec::new();
    let mut failed = Vec::new();
    let mut unknown = Vec::new();
    let mut findings = Vec::new();

    for record in &records {
        let finding = review_organ_record(record, &duplicate_names, &duplicate_sources);
        match finding.status {
            OrganReviewStatus::Active => active.push(record.name.clone()),
            OrganReviewStatus::Candidate => candidates.push(record.name.clone()),
            OrganReviewStatus::Stale => stale.push(record.name.clone()),
            OrganReviewStatus::Duplicate => duplicate.push(record.name.clone()),
            OrganReviewStatus::Failed => failed.push(record.name.clone()),
            OrganReviewStatus::Unknown => unknown.push(record.name.clone()),
        }
        findings.push(finding);
    }

    let output = OrganReviewOutput {
        schema_version: "0.1.0".to_owned(),
        reviewed_at: now_iso(),
        source: args.path.display().to_string(),
        total_organs: records.len(),
        findings,
        active,
        candidates,
        stale,
        duplicate,
        failed,
        unknown,
    };
    if let Some(evidence_bundle) = &args.evidence_bundle {
        write_evidence_bundle(
            evidence_bundle.as_path(),
            &[args.path.as_path()],
            "organ-review-output",
            format!("organ review:{}", args.path.display()),
            &output,
            |raw_ref| output.to_evidence_bundle(raw_ref),
        )?;
    }
    Ok(output)
}

fn suggest_organs(args: OrganSuggestArgs) -> Result<OrganRegistrySuggestionOutput> {
    let registry_exists = args.registry.exists();
    let promotions_exists = args.promotions.exists();
    let generations_exists = args.generations.exists();
    let registry: Vec<OrganRecord> = read_jsonl(&args.registry)?;
    let promotions: Vec<MutationPromotionRecord> = read_jsonl(&args.promotions)?;
    let generations: Vec<GenerationRecord> = read_jsonl(&args.generations)?;
    let source_statuses = BTreeMap::from([
        (
            "registry".to_owned(),
            source_status(registry_exists, registry.len()),
        ),
        (
            "promotions".to_owned(),
            source_status(promotions_exists, promotions.len()),
        ),
        (
            "generations".to_owned(),
            source_status(generations_exists, generations.len()),
        ),
    ]);
    let limits = suggestion_limits(&source_statuses);
    let registered_organs: Vec<String> = registry.iter().map(|organ| organ.name.clone()).collect();
    let mut observations = BTreeMap::<String, OrganObservation>::new();

    for promotion in &promotions {
        if promotion.status != PromotionStatus::Promoted {
            continue;
        }
        for organ in &promotion.changed_organs {
            observations
                .entry(organ.clone())
                .or_default()
                .observe_promotion(promotion);
        }
    }
    for generation in &generations {
        for organ in &generation.changed_organs {
            observations
                .entry(organ.clone())
                .or_default()
                .observe_generation(generation);
        }
    }

    let observed_changed_organs: Vec<String> = observations.keys().cloned().collect();
    let candidates = observations
        .into_iter()
        .filter(|(name, _)| {
            !registered_organs
                .iter()
                .any(|registered| registered == name)
        })
        .map(|(name, observation)| observation.into_suggestion(name))
        .collect();

    let output = OrganRegistrySuggestionOutput {
        schema_version: "0.1.0".to_owned(),
        reviewed_at: now_iso(),
        registry_source: args.registry.display().to_string(),
        promotion_source: args.promotions.display().to_string(),
        generation_source: args.generations.display().to_string(),
        source_statuses,
        registered_organs,
        observed_changed_organs,
        candidates,
        limits,
    };
    if let Some(evidence_bundle) = &args.evidence_bundle {
        write_evidence_bundle(
            evidence_bundle.as_path(),
            &[
                args.registry.as_path(),
                args.promotions.as_path(),
                args.generations.as_path(),
            ],
            "organ-registry-suggestion-output",
            format!("organ suggest:{}", args.registry.display()),
            &output,
            |raw_ref| output.to_evidence_bundle(raw_ref),
        )?;
    }
    Ok(output)
}

fn write_evidence_bundle<T, F>(
    path: &Path,
    protected_sources: &[&Path],
    kind: &str,
    source: String,
    entry: &T,
    to_bundle: F,
) -> Result<()>
where
    T: serde::Serialize,
    F: FnOnce(ProvenanceRef) -> EvidenceBundle,
{
    ensure_sidecar_path(path, protected_sources)?;
    let raw_ref = evidence_provenance(kind, source, "0.1.0", entry)?;
    let bundle = to_bundle(raw_ref);
    write_json(path, &bundle)
}

fn write_json<T: serde::Serialize>(path: &Path, entry: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open output file {}", path.display()))?;
    serde_json::to_writer_pretty(&mut file, entry).context("failed to serialize output file")?;
    file.write_all(b"\n")
        .context("failed to terminate output file")?;
    Ok(())
}

fn evidence_provenance<T: serde::Serialize>(
    kind: &str,
    source: String,
    schema_version: &str,
    entry: &T,
) -> Result<ProvenanceRef> {
    let bytes = serde_json::to_vec(entry).context("failed to serialize evidence source")?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(ProvenanceRef {
        kind: kind.to_owned(),
        source,
        digest: DigestRef {
            algorithm: "sha256".to_owned(),
            value: format!("sha256:{}", to_hex(&hasher.finalize())),
        },
        schema_version: schema_version.to_owned(),
        metadata: BTreeMap::new(),
    })
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn ensure_sidecar_path(path: &Path, protected_sources: &[&Path]) -> Result<()> {
    let requested = path_key(path)?;
    for source in protected_sources {
        let source = path_key(source)?;
        if requested == source || requested.starts_with(&source) {
            bail!(
                "evidence bundle path must not overwrite input source {}",
                source.display()
            );
        }
    }
    Ok(())
}

fn path_key(path: &Path) -> Result<PathBuf> {
    let absolute = absolute_normalized_path(path)?;
    let parent = absolute.parent().unwrap_or_else(|| Path::new("/"));
    let parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    let Some(file_name) = absolute.file_name() else {
        bail!("path must include a file name: {}", path.display());
    };
    Ok(parent.join(file_name))
}

fn absolute_normalized_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)
    };
    Ok(normalize_path(&absolute))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn source_status(exists: bool, records: usize) -> String {
    if exists {
        format!("loaded:{records}")
    } else {
        "missing:treated-as-empty".to_owned()
    }
}

fn suggestion_limits(source_statuses: &BTreeMap<String, String>) -> Vec<String> {
    source_statuses
        .iter()
        .filter(|(_, status)| status.starts_with("missing:"))
        .map(|(source, _)| {
            format!("{source} source was missing, so absence of candidates is not proof that every changed organ is registered")
        })
        .collect()
}

#[derive(Debug, Default)]
struct OrganObservation {
    promotion_ids: Vec<String>,
    generation_ids: Vec<String>,
    mutation_ids: Vec<String>,
}

impl OrganObservation {
    fn observe_promotion(&mut self, record: &MutationPromotionRecord) {
        push_unique(&mut self.promotion_ids, record.promotion_id.clone());
        push_unique(&mut self.mutation_ids, record.mutation_id.clone());
    }

    fn observe_generation(&mut self, record: &GenerationRecord) {
        push_unique(&mut self.generation_ids, record.generation.clone());
        push_unique(&mut self.mutation_ids, record.mutation_id.clone());
    }

    fn into_suggestion(self, name: String) -> OrganRegistrySuggestion {
        let mut observed_in = Vec::new();
        observed_in.extend(
            self.promotion_ids
                .iter()
                .map(|id| format!("promotion:{id}")),
        );
        observed_in.extend(
            self.generation_ids
                .iter()
                .map(|id| format!("generation:{id}")),
        );

        let mut evidence = BTreeMap::new();
        if !self.promotion_ids.is_empty() {
            evidence.insert("promotion_ids".to_owned(), self.promotion_ids.join(","));
        }
        if !self.generation_ids.is_empty() {
            evidence.insert("generation_ids".to_owned(), self.generation_ids.join(","));
        }
        if !self.mutation_ids.is_empty() {
            evidence.insert("mutation_ids".to_owned(), self.mutation_ids.join(","));
        }

        OrganRegistrySuggestion {
            name,
            reason: "changed_organs mentions this organ, but the registry has no matching name"
                .to_owned(),
            observed_in,
            evidence,
        }
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn duplicate_values<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(value.to_owned()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(value, count)| (count > 1).then_some(value))
        .collect()
}

fn review_organ_record(
    record: &OrganRecord,
    duplicate_names: &[String],
    duplicate_sources: &[String],
) -> OrganReviewFinding {
    let mut evidence = BTreeMap::new();
    evidence.insert("source".to_owned(), record.source.clone());
    if let Some(usage_count) = record.usage_count {
        evidence.insert("usage_count".to_owned(), usage_count.to_string());
    }
    if let Some(failure_count) = record.failure_count {
        evidence.insert("failure_count".to_owned(), failure_count.to_string());
    }
    evidence.insert(
        "related_goals_count".to_owned(),
        record.related_goals.len().to_string(),
    );
    if let Some(status) = record.decay_status {
        evidence.insert("declared_decay_status".to_owned(), format!("{status:?}"));
    }

    if duplicate_names.iter().any(|name| name == &record.name) {
        return review_finding(
            record,
            OrganReviewStatus::Duplicate,
            "duplicate organ name",
            evidence,
        );
    }
    if duplicate_sources
        .iter()
        .any(|source| source == &record.source)
    {
        return review_finding(
            record,
            OrganReviewStatus::Duplicate,
            "duplicate organ source",
            evidence,
        );
    }
    if record.failure_count.is_some_and(|count| count > 0) {
        return review_finding(
            record,
            OrganReviewStatus::Failed,
            "failure_count is greater than zero",
            evidence,
        );
    }
    if let Some(status) = record.decay_status {
        return review_finding(
            record,
            organ_review_status(status),
            "explicit decay_status",
            evidence,
        );
    }
    if record.usage_count == Some(0) {
        return review_finding(
            record,
            OrganReviewStatus::Stale,
            "usage_count is zero",
            evidence,
        );
    }
    if record.related_goals.is_empty() {
        return review_finding(
            record,
            OrganReviewStatus::Candidate,
            "no related goals recorded",
            evidence,
        );
    }
    review_finding(
        record,
        OrganReviewStatus::Unknown,
        "insufficient review evidence",
        evidence,
    )
}

fn organ_review_status(status: DecayStatus) -> OrganReviewStatus {
    match status {
        DecayStatus::Active => OrganReviewStatus::Active,
        DecayStatus::Candidate => OrganReviewStatus::Candidate,
        DecayStatus::Stale => OrganReviewStatus::Stale,
        DecayStatus::Duplicate => OrganReviewStatus::Duplicate,
        DecayStatus::Failed => OrganReviewStatus::Failed,
    }
}

fn review_finding(
    record: &OrganRecord,
    status: OrganReviewStatus,
    reason: &str,
    evidence: BTreeMap<String, String>,
) -> OrganReviewFinding {
    OrganReviewFinding {
        name: record.name.clone(),
        status,
        reason: reason.to_owned(),
        evidence,
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let entry = match args.command {
        Command::Append(args) => serde_json::to_value(append_mutation(args)?)?,
        Command::Effect(args) => serde_json::to_value(append_effect(args)?)?,
        Command::Generation(args) => serde_json::to_value(append_generation(args)?)?,
        Command::Organ(OrganCommand::Add(args)) => serde_json::to_value(add_organ(args)?)?,
        Command::Organ(OrganCommand::List(args)) => serde_json::to_value(list_organs(args)?)?,
        Command::Organ(OrganCommand::Review(args)) => serde_json::to_value(review_organs(args)?)?,
        Command::Organ(OrganCommand::Suggest(args)) => serde_json::to_value(suggest_organs(args)?)?,
    };
    println!("{}", serde_json::to_string_pretty(&entry)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopoietic_core::{PromotionStatus, VerificationCheckResult, VerificationCheckStatus};

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "autopoietic-journal-{name}-{}",
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&root).expect("test temp root should be created");
        root
    }

    fn organ_args(path: PathBuf, name: &str, status: Option<DecayStatusArg>) -> OrganAddArgs {
        OrganAddArgs {
            path,
            name: name.to_owned(),
            organ_type: OrganTypeArg::Cli,
            source: format!("crates/{name}"),
            purpose: format!("test organ {name}"),
            created_by: Some("test".to_owned()),
            usage_count: Some(1),
            failure_count: Some(0),
            related_goals: vec!["p4-test".to_owned()],
            decay_status: status,
        }
    }

    fn check_result() -> VerificationCheckResult {
        VerificationCheckResult {
            name: "test-check".to_owned(),
            command: "true".to_owned(),
            args: Vec::new(),
            status: VerificationCheckStatus::Passed,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    fn promotion_record(id: &str, changed_organs: Vec<String>) -> MutationPromotionRecord {
        MutationPromotionRecord {
            promotion_id: id.to_owned(),
            timestamp: "2026-05-11T00:00:00Z".to_owned(),
            mutation_id: format!("mut-{id}"),
            goal: format!("promote {id}"),
            phase: "P2".to_owned(),
            status: PromotionStatus::Promoted,
            reason: "test promotion".to_owned(),
            verification_id: Some(format!("ver-{id}")),
            proposal_fingerprint:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            verified_root_fingerprint: Some(
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_owned(),
            ),
            promotion_root_fingerprint:
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
            parent_genome: "git:parent".to_owned(),
            target_configuration: "iso".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            changed_organs,
            checks: vec![check_result()],
            metadata: BTreeMap::new(),
        }
    }

    fn generation_record(id: &str, changed_organs: Vec<String>) -> GenerationRecord {
        GenerationRecord {
            timestamp: "2026-05-11T00:00:01Z".to_owned(),
            lineage_status: LineageStatus::Planned,
            generation: id.to_owned(),
            mutation_id: format!("mut-{id}"),
            goal: format!("plan {id}"),
            changed_organs,
            parent_generation: Some("gen-parent".to_owned()),
            activation_result: "planned-install".to_owned(),
            verification_id: Some(format!("ver-{id}")),
            promotion_id: Some(format!("pro-{id}")),
            target_root: Some("/mnt/autopoietic".to_owned()),
            target_configuration: Some("iso".to_owned()),
            metadata: BTreeMap::new(),
        }
    }

    fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> T {
        let contents = fs::read_to_string(path).expect("test json file should be readable");
        serde_json::from_str(&contents).expect("test json file should parse")
    }

    #[test]
    fn add_organ_appends_registry_record() {
        let root = temp_root("organ-add");
        let path = root.join("organs.jsonl");

        let record = add_organ(organ_args(
            path.clone(),
            "mutation-journal",
            Some(DecayStatusArg::Active),
        ))
        .expect("organ should be added");

        assert_eq!(record.name, "mutation-journal");
        assert_eq!(record.decay_status, Some(DecayStatus::Active));
        let records: Vec<OrganRecord> = read_jsonl(&path).expect("registry should read");
        assert_eq!(records, vec![record]);
    }

    #[test]
    fn list_organs_reads_registry_records() {
        let root = temp_root("organ-list");
        let path = root.join("organs.jsonl");
        add_organ(organ_args(
            path.clone(),
            "first",
            Some(DecayStatusArg::Active),
        ))
        .expect("first organ should be added");
        add_organ(organ_args(
            path.clone(),
            "second",
            Some(DecayStatusArg::Candidate),
        ))
        .expect("second organ should be added");

        let records = list_organs(OrganListArgs { path }).expect("organs should list");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "first");
        assert_eq!(records[1].name, "second");
    }

    #[test]
    fn review_organs_groups_by_decay_status_without_writing() {
        let root = temp_root("organ-review");
        let path = root.join("organs.jsonl");
        add_organ(organ_args(
            path.clone(),
            "active",
            Some(DecayStatusArg::Active),
        ))
        .expect("active organ should be added");
        add_organ(organ_args(
            path.clone(),
            "candidate",
            Some(DecayStatusArg::Candidate),
        ))
        .expect("candidate organ should be added");
        add_organ(organ_args(path.clone(), "unknown", None))
            .expect("unknown organ should be added");
        let before = fs::read_to_string(&path).expect("registry should be readable");

        let review = review_organs(OrganReviewArgs {
            path: path.clone(),
            evidence_bundle: None,
        })
        .expect("review should be generated");

        assert_eq!(review.total_organs, 3);
        assert_eq!(review.findings.len(), 3);
        assert_eq!(review.active, vec!["active".to_owned()]);
        assert_eq!(review.candidates, vec!["candidate".to_owned()]);
        assert_eq!(review.unknown, vec!["unknown".to_owned()]);
        assert_eq!(
            fs::read_to_string(&path).expect("registry should remain readable"),
            before
        );
    }

    #[test]
    fn review_organs_detects_duplicate_names_without_manual_status() {
        let root = temp_root("organ-review-duplicate-name");
        let path = root.join("organs.jsonl");
        add_organ(organ_args(path.clone(), "dup", None)).expect("first organ should be added");
        let mut second = organ_args(path.clone(), "dup", None);
        second.source = "crates/other".to_owned();
        add_organ(second).expect("second organ should be added");

        let review = review_organs(OrganReviewArgs {
            path,
            evidence_bundle: None,
        })
        .expect("review should be generated");

        assert_eq!(review.duplicate, vec!["dup".to_owned(), "dup".to_owned()]);
        assert!(
            review
                .findings
                .iter()
                .all(|finding| finding.reason == "duplicate organ name")
        );
    }

    #[test]
    fn review_organs_detects_failure_count_without_manual_status() {
        let root = temp_root("organ-review-failure-count");
        let path = root.join("organs.jsonl");
        let mut args = organ_args(path.clone(), "failing", None);
        args.failure_count = Some(2);
        add_organ(args).expect("failing organ should be added");

        let review = review_organs(OrganReviewArgs {
            path,
            evidence_bundle: None,
        })
        .expect("review should be generated");

        assert_eq!(review.failed, vec!["failing".to_owned()]);
        assert_eq!(
            review.findings[0].reason,
            "failure_count is greater than zero"
        );
    }

    #[test]
    fn review_organs_detects_zero_usage_as_stale_without_manual_status() {
        let root = temp_root("organ-review-zero-usage");
        let path = root.join("organs.jsonl");
        let mut args = organ_args(path.clone(), "unused", None);
        args.usage_count = Some(0);
        add_organ(args).expect("unused organ should be added");

        let review = review_organs(OrganReviewArgs {
            path,
            evidence_bundle: None,
        })
        .expect("review should be generated");

        assert_eq!(review.stale, vec!["unused".to_owned()]);
        assert_eq!(review.findings[0].reason, "usage_count is zero");
    }

    #[test]
    fn add_organ_rejects_empty_required_fields() {
        let root = temp_root("organ-empty");
        let path = root.join("organs.jsonl");
        let mut args = organ_args(path, "", Some(DecayStatusArg::Active));
        args.name = " ".to_owned();

        let error = add_organ(args).expect_err("empty organ name should fail");

        assert!(error.to_string().contains("name"));
    }

    #[test]
    fn suggest_organs_reports_changed_organs_missing_from_registry() {
        let root = temp_root("organ-suggest");
        let registry = root.join("organs.jsonl");
        let promotions = root.join("promotions.jsonl");
        let generations = root.join("generations.jsonl");
        add_organ(organ_args(
            registry.clone(),
            "registered-cli",
            Some(DecayStatusArg::Active),
        ))
        .expect("registered organ should be added");
        append_jsonl(
            &promotions,
            &promotion_record(
                "pro-one",
                vec!["registered-cli".to_owned(), "new-service".to_owned()],
            ),
        )
        .expect("promotion should be written");
        append_jsonl(
            &generations,
            &generation_record(
                "gen-one",
                vec!["new-service".to_owned(), "new-module".to_owned()],
            ),
        )
        .expect("generation should be written");

        let suggestions = suggest_organs(OrganSuggestArgs {
            registry,
            promotions,
            generations,
            evidence_bundle: None,
        })
        .expect("suggestions should be generated");

        assert_eq!(
            suggestions.registered_organs,
            vec!["registered-cli".to_owned()]
        );
        assert_eq!(
            suggestions
                .source_statuses
                .get("promotions")
                .map(String::as_str),
            Some("loaded:1")
        );
        assert_eq!(
            suggestions
                .source_statuses
                .get("generations")
                .map(String::as_str),
            Some("loaded:1")
        );
        assert_eq!(suggestions.candidates.len(), 2);
        assert_eq!(suggestions.candidates[0].name, "new-module");
        assert_eq!(suggestions.candidates[1].name, "new-service");
        assert!(
            suggestions.candidates[1]
                .observed_in
                .contains(&"promotion:pro-one".to_owned())
        );
        assert!(
            suggestions.candidates[1]
                .observed_in
                .contains(&"generation:gen-one".to_owned())
        );
    }

    #[test]
    fn suggest_organs_is_read_only_and_treats_missing_sources_as_empty() {
        let root = temp_root("organ-suggest-read-only");
        let registry = root.join("organs.jsonl");
        let promotions = root.join("promotions.jsonl");
        let generations = root.join("generations.jsonl");
        add_organ(organ_args(
            registry.clone(),
            "registered-cli",
            Some(DecayStatusArg::Active),
        ))
        .expect("registered organ should be added");
        let before = fs::read_to_string(&registry).expect("registry should be readable");

        let suggestions = suggest_organs(OrganSuggestArgs {
            registry: registry.clone(),
            promotions,
            generations,
            evidence_bundle: None,
        })
        .expect("suggestions should be generated");

        assert!(suggestions.observed_changed_organs.is_empty());
        assert!(suggestions.candidates.is_empty());
        assert_eq!(
            suggestions
                .source_statuses
                .get("promotions")
                .map(String::as_str),
            Some("missing:treated-as-empty")
        );
        assert!(
            suggestions
                .limits
                .iter()
                .any(|limit| limit.contains("promotions source was missing"))
        );
        assert_eq!(
            fs::read_to_string(&registry).expect("registry should remain readable"),
            before
        );
    }

    #[test]
    fn organ_review_can_write_p4_evidence_bundle_sidecar() {
        let root = temp_root("organ-review-evidence");
        let registry = root.join("organs.jsonl");
        let evidence = root.join("evidence/review.json");
        add_organ(organ_args(
            registry.clone(),
            "mutation-journal",
            Some(DecayStatusArg::Active),
        ))
        .expect("organ should be added");
        let before = fs::read_to_string(&registry).expect("registry should be readable");

        let review = review_organs(OrganReviewArgs {
            path: registry.clone(),
            evidence_bundle: Some(evidence.clone()),
        })
        .expect("review should be generated");
        let bundle: EvidenceBundle = read_json(&evidence);

        assert_eq!(review.total_organs, 1);
        assert_eq!(bundle.phase, "P4");
        assert_eq!(bundle.claims[0].claim, "organ decay review generated");
        assert!(
            bundle.claims[0]
                .limits
                .iter()
                .any(|limit| limit.contains("read-only"))
        );
        assert_eq!(
            fs::read_to_string(&registry).expect("registry should remain readable"),
            before
        );
    }

    #[test]
    fn organ_suggest_can_write_p4_evidence_bundle_sidecar() {
        let root = temp_root("organ-suggest-evidence");
        let registry = root.join("organs.jsonl");
        let promotions = root.join("promotions.jsonl");
        let generations = root.join("generations.jsonl");
        let evidence = root.join("evidence/suggest.json");
        append_jsonl(
            &promotions,
            &promotion_record("pro-one", vec!["new-service".to_owned()]),
        )
        .expect("promotion should be written");

        let suggestions = suggest_organs(OrganSuggestArgs {
            registry,
            promotions,
            generations,
            evidence_bundle: Some(evidence.clone()),
        })
        .expect("suggestions should be generated");
        let bundle: EvidenceBundle = read_json(&evidence);

        assert_eq!(suggestions.candidates.len(), 1);
        assert_eq!(bundle.phase, "P4");
        assert_eq!(
            bundle.claims[0].claim,
            "organ registry suggestions generated"
        );
        assert!(
            bundle
                .canonical_facts
                .iter()
                .any(|fact| fact.fact_id == "fact:organ-suggest:candidate-organs")
        );
    }

    #[test]
    fn organ_evidence_bundle_must_not_overwrite_input_sources() {
        let root = temp_root("organ-evidence-overwrite");
        let registry = root.join("organs.jsonl");
        add_organ(organ_args(
            registry.clone(),
            "mutation-journal",
            Some(DecayStatusArg::Active),
        ))
        .expect("organ should be added");

        let error = review_organs(OrganReviewArgs {
            path: registry.clone(),
            evidence_bundle: Some(registry),
        })
        .expect_err("evidence bundle should not overwrite registry");

        assert!(
            error
                .to_string()
                .contains("must not overwrite input source")
        );
    }

    #[test]
    fn organ_evidence_bundle_rejects_absolute_input_alias() {
        let relative = PathBuf::from("memory/organs.jsonl");
        let absolute = std::env::current_dir()
            .expect("test current directory should be available")
            .join("memory/organs.jsonl");

        let error = ensure_sidecar_path(&absolute, &[relative.as_path()])
            .expect_err("absolute alias should be rejected");

        assert!(
            error
                .to_string()
                .contains("must not overwrite input source")
        );
    }

    #[test]
    fn organ_evidence_bundle_does_not_follow_existing_symlink_sidecar() {
        let root = temp_root("organ-evidence-symlink");
        let registry = root.join("organs.jsonl");
        let symlink = root.join("review-evidence.json");
        add_organ(organ_args(
            registry.clone(),
            "mutation-journal",
            Some(DecayStatusArg::Active),
        ))
        .expect("organ should be added");
        std::os::unix::fs::symlink(&registry, &symlink).expect("test symlink should be created");
        let before = fs::read_to_string(&registry).expect("registry should be readable");

        let error = review_organs(OrganReviewArgs {
            path: registry.clone(),
            evidence_bundle: Some(symlink),
        })
        .expect_err("existing symlink evidence path should fail");

        assert!(error.to_string().contains("failed to open output file"));
        assert_eq!(
            fs::read_to_string(&registry).expect("registry should remain readable"),
            before
        );
    }

    #[test]
    fn organ_evidence_bundle_rejects_descendant_of_missing_input_source() {
        let root = temp_root("organ-evidence-descendant");
        let missing_registry = root.join("missing-organs.jsonl");
        let evidence = missing_registry.join("evidence.json");

        let error = review_organs(OrganReviewArgs {
            path: missing_registry.clone(),
            evidence_bundle: Some(evidence),
        })
        .expect_err("evidence bundle should not create a directory at the input path");

        assert!(
            error
                .to_string()
                .contains("must not overwrite input source")
        );
        assert!(!missing_registry.exists());
    }

    #[test]
    fn suggest_organs_ignores_rejected_promotions() {
        let root = temp_root("organ-suggest-rejected");
        let registry = root.join("organs.jsonl");
        let promotions = root.join("promotions.jsonl");
        let generations = root.join("generations.jsonl");
        let mut rejected = promotion_record("pro-rejected", vec!["rejected-organ".to_owned()]);
        rejected.status = PromotionStatus::Rejected;
        append_jsonl(&promotions, &rejected).expect("promotion should be written");

        let suggestions = suggest_organs(OrganSuggestArgs {
            registry,
            promotions,
            generations,
            evidence_bundle: None,
        })
        .expect("suggestions should be generated");

        assert!(suggestions.observed_changed_organs.is_empty());
        assert!(suggestions.candidates.is_empty());
    }

    #[test]
    fn organ_suggest_args_parse_paths() {
        let args = Args::parse_from([
            "mutation-journal",
            "organ",
            "suggest",
            "--registry",
            "custom/organs.jsonl",
            "--promotions",
            "custom/promotions.jsonl",
            "--generations",
            "custom/generations.jsonl",
            "--evidence-bundle",
            "custom/evidence.json",
        ]);

        let Command::Organ(OrganCommand::Suggest(args)) = args.command else {
            panic!("organ suggest command should parse");
        };
        assert_eq!(args.registry, PathBuf::from("custom/organs.jsonl"));
        assert_eq!(args.promotions, PathBuf::from("custom/promotions.jsonl"));
        assert_eq!(args.generations, PathBuf::from("custom/generations.jsonl"));
        assert_eq!(
            args.evidence_bundle,
            Some(PathBuf::from("custom/evidence.json"))
        );
    }
}
