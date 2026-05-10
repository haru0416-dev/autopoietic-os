#![deny(clippy::correctness)]
#![warn(clippy::suspicious, clippy::style, clippy::complexity, clippy::perf)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub host: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NixFile {
    pub path: String,
    pub bytes: u64,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenomeState {
    pub has_flake: bool,
    pub has_lock: bool,
    pub inputs: Vec<String>,
    pub nix_files: Vec<NixFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformState {
    pub system: String,
    pub release: String,
    pub machine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitState {
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemdState {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub units: Vec<UnitState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledPackages {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemGeneration {
    pub generation: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationsState {
    pub current_system: Option<String>,
    pub system_generations: Vec<SystemGeneration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalState {
    pub included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummary {
    pub path: String,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampled_files: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub top_suffixes: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellHistoryState {
    pub included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BodyState {
    pub platform: PlatformState,
    pub systemd: SystemdState,
    pub installed_packages: InstalledPackages,
    pub generations: GenerationsState,
    pub journal: JournalState,
    pub projects: Vec<ProjectSummary>,
    pub shell_history: ShellHistoryState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryState {
    pub root: String,
    pub mutation_log: String,
    pub effect_log: String,
    pub generation_log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PainPoint {
    pub signal: String,
    pub evidence: Vec<String>,
    pub candidate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelfState {
    pub schema_version: String,
    pub observed_at: String,
    pub identity: Identity,
    pub genome: GenomeState,
    pub body: BodyState,
    pub memory: MemoryState,
    pub pain_points: Vec<PainPoint>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MutationStatus {
    Pending,
    Accepted,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EffectRisk {
    Low,
    Medium,
    High,
    Irreversible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationRecord {
    pub mutation_id: String,
    pub timestamp: String,
    pub goal: String,
    pub status: MutationStatus,
    pub phase: String,
    pub reason: String,
    pub changed_paths: Vec<String>,
    pub next_hypothesis: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectRecord {
    pub effect_id: String,
    pub timestamp: String,
    pub mutation_id: String,
    #[serde(rename = "type")]
    pub effect_type: String,
    pub target: String,
    pub reversible: bool,
    pub compensation: String,
    pub verified_by: String,
    pub risk: EffectRisk,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposalCheck {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SideEffectDeclaration {
    #[serde(rename = "type")]
    pub effect_type: String,
    pub target: String,
    pub reversible: bool,
    pub compensation: String,
    pub risk: EffectRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationProposal {
    pub schema_version: String,
    pub mutation_id: String,
    pub goal: String,
    pub phase: String,
    pub changed_paths: Vec<String>,
    pub expected_checks: Vec<ProposalCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<SideEffectDeclaration>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationStatus {
    Verified,
    Rejected,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationCheckStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationCheckResult {
    pub name: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    pub status: VerificationCheckStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationVerificationRecord {
    pub verification_id: String,
    pub timestamp: String,
    pub mutation_id: String,
    pub goal: String,
    pub phase: String,
    pub status: VerificationStatus,
    pub reason: String,
    pub proposal_fingerprint: String,
    pub root_fingerprint: String,
    pub changed_paths: Vec<String>,
    pub checks: Vec<VerificationCheckResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<SideEffectDeclaration>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PromotionStatus {
    Promoted,
    Rejected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationPromotionRecord {
    pub promotion_id: String,
    pub timestamp: String,
    pub mutation_id: String,
    pub goal: String,
    pub phase: String,
    pub status: PromotionStatus,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_id: Option<String>,
    pub proposal_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_root_fingerprint: Option<String>,
    pub promotion_root_fingerprint: String,
    pub parent_genome: String,
    pub target_configuration: String,
    pub changed_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_organs: Vec<String>,
    pub checks: Vec<VerificationCheckResult>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LineageStatus {
    Planned,
    Installed,
    Failed,
}

fn default_lineage_status() -> LineageStatus {
    LineageStatus::Installed
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationRecord {
    pub timestamp: String,
    #[serde(default = "default_lineage_status")]
    pub lineage_status: LineageStatus,
    pub generation: String,
    pub mutation_id: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_organs: Vec<String>,
    pub parent_generation: Option<String>,
    pub activation_result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_configuration: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedEffect {
    #[serde(rename = "type")]
    pub effect_type: String,
    pub target: String,
    pub reversible: bool,
    pub compensation: String,
    pub verified_by: String,
    pub risk: EffectRisk,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallSeedFilePlan {
    pub installed_path: String,
    pub target_path: String,
    pub source: String,
    pub content_sha256: String,
    pub effect: PlannedEffect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallSeedManifest {
    pub schema_version: String,
    pub generated_at: String,
    pub target_root: String,
    pub mutation_id: String,
    pub promotion_id: String,
    pub lineage_status: LineageStatus,
    pub files: Vec<InstallSeedFilePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallPlanOutput {
    pub generation: GenerationRecord,
    pub seed_manifest: InstallSeedManifest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InstallSeedFileStatus {
    Matched,
    Missing,
    Mismatched,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallSeedFileVerification {
    pub installed_path: String,
    pub target_path: String,
    pub expected_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_sha256: Option<String>,
    pub status: InstallSeedFileStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallVerifyOutput {
    pub verified_at: String,
    pub target_root: String,
    pub mutation_id: String,
    pub promotion_id: String,
    pub all_matched: bool,
    pub files: Vec<InstallSeedFileVerification>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonStatus {
    Matched,
    Mismatched,
    Missing,
    Incomparable,
    Stale,
    Ambiguous,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DataQuality {
    Raw,
    Observed,
    Canonicalized,
    Verified,
    Derived,
    Stale,
    Ambiguous,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DigestRef {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceRef {
    pub kind: String,
    pub source: String,
    pub digest: DigestRef,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceSubject {
    pub mutation_id: String,
    pub proposal_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceInputRef {
    pub input_id: String,
    pub kind: String,
    pub provenance: ProvenanceRef,
    pub quality: DataQuality,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceObservation {
    pub observation_id: String,
    pub kind: String,
    pub summary: String,
    pub quality: DataQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    pub raw_ref: ProvenanceRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalFact {
    pub fact_id: String,
    pub kind: String,
    pub value: Value,
    pub quality: DataQuality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComparisonReport {
    pub comparison_id: String,
    pub left_ref: String,
    pub right_ref: String,
    pub status: ComparisonStatus,
    pub reason: String,
    pub quality: DataQuality,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceClaim {
    pub claim_id: String,
    pub claim: String,
    pub quality: DataQuality,
    pub backing: Vec<String>,
    pub limits: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceBundle {
    pub schema_version: String,
    pub bundle_id: String,
    pub phase: String,
    pub subject: EvidenceSubject,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<EvidenceInputRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<EvidenceObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub canonical_facts: Vec<CanonicalFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comparisons: Vec<ComparisonReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<EvidenceClaim>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl MutationVerificationRecord {
    pub fn to_evidence_bundle(&self, raw_ref: ProvenanceRef) -> EvidenceBundle {
        let record_ref = format!("observation:{}:record", self.verification_id);
        let status_fact_ref = format!("fact:{}:status", self.verification_id);
        EvidenceBundle {
            schema_version: "0.1.0".to_owned(),
            bundle_id: format!("evidence:{}", self.verification_id),
            phase: self.phase.clone(),
            subject: EvidenceSubject {
                mutation_id: self.mutation_id.clone(),
                proposal_fingerprint: self.proposal_fingerprint.clone(),
                root_fingerprint: Some(self.root_fingerprint.clone()),
                generation_id: None,
                metadata: BTreeMap::new(),
            },
            inputs: vec![evidence_input(
                format!("input:{}:verification-record", self.verification_id),
                "mutation-verification-record",
                raw_ref.clone(),
            )],
            observations: vec![EvidenceObservation {
                observation_id: record_ref.clone(),
                kind: "mutation-verification-record".to_owned(),
                summary: self.reason.clone(),
                quality: DataQuality::Observed,
                observed_at: Some(self.timestamp.clone()),
                raw_ref,
                metadata: BTreeMap::new(),
            }],
            canonical_facts: verification_canonical_facts(self),
            comparisons: Vec::new(),
            claims: vec![EvidenceClaim {
                claim_id: format!("claim:{}:status", self.verification_id),
                claim: verification_claim(self.status).to_owned(),
                quality: DataQuality::Derived,
                backing: vec![record_ref, status_fact_ref],
                limits: vec![
                    "P1 does not boot a VM".to_owned(),
                    "P1 does not install, activate, or write to the live system".to_owned(),
                    "P1 verified is not P2 promoted and not generation accepted".to_owned(),
                ],
                metadata: BTreeMap::new(),
            }],
            metadata: BTreeMap::new(),
        }
    }
}

impl MutationPromotionRecord {
    pub fn to_evidence_bundle(&self, raw_ref: ProvenanceRef) -> EvidenceBundle {
        let record_ref = format!("observation:{}:record", self.promotion_id);
        let status_fact_ref = format!("fact:{}:status", self.promotion_id);
        let comparisons = promotion_comparisons(self);
        let mut backing = vec![record_ref.clone(), status_fact_ref];
        backing.extend(
            comparisons
                .iter()
                .map(|comparison| comparison.comparison_id.clone()),
        );
        EvidenceBundle {
            schema_version: "0.1.0".to_owned(),
            bundle_id: format!("evidence:{}", self.promotion_id),
            phase: self.phase.clone(),
            subject: EvidenceSubject {
                mutation_id: self.mutation_id.clone(),
                proposal_fingerprint: self.proposal_fingerprint.clone(),
                root_fingerprint: Some(self.promotion_root_fingerprint.clone()),
                generation_id: None,
                metadata: BTreeMap::new(),
            },
            inputs: vec![evidence_input(
                format!("input:{}:promotion-record", self.promotion_id),
                "mutation-promotion-record",
                raw_ref.clone(),
            )],
            observations: vec![EvidenceObservation {
                observation_id: record_ref,
                kind: "mutation-promotion-record".to_owned(),
                summary: self.reason.clone(),
                quality: DataQuality::Observed,
                observed_at: Some(self.timestamp.clone()),
                raw_ref,
                metadata: BTreeMap::new(),
            }],
            canonical_facts: promotion_canonical_facts(self),
            comparisons,
            claims: vec![EvidenceClaim {
                claim_id: format!("claim:{}:status", self.promotion_id),
                claim: promotion_claim(self.status).to_owned(),
                quality: DataQuality::Derived,
                backing,
                limits: vec![
                    "P2 does not accept the mutation into generation lineage".to_owned(),
                    "P2 does not write installed memory".to_owned(),
                    "P2 does not run live nixos-rebuild switch".to_owned(),
                ],
                metadata: BTreeMap::new(),
            }],
            metadata: BTreeMap::new(),
        }
    }
}

fn evidence_input(input_id: String, kind: &str, provenance: ProvenanceRef) -> EvidenceInputRef {
    EvidenceInputRef {
        input_id,
        kind: kind.to_owned(),
        provenance,
        quality: DataQuality::Raw,
        metadata: BTreeMap::new(),
    }
}

fn verification_canonical_facts(record: &MutationVerificationRecord) -> Vec<CanonicalFact> {
    let prefix = &record.verification_id;
    vec![
        canonical_fact(prefix, "mutation-id", record.mutation_id.clone(), &[]),
        canonical_fact(
            prefix,
            "proposal-fingerprint",
            record.proposal_fingerprint.clone(),
            &[],
        ),
        canonical_fact(
            prefix,
            "root-fingerprint",
            record.root_fingerprint.clone(),
            &[],
        ),
        canonical_fact(prefix, "status", record.status, &[]),
        canonical_fact(prefix, "changed-paths", record.changed_paths.clone(), &[]),
        canonical_fact(
            prefix,
            "check-statuses",
            check_statuses(&record.checks),
            &[],
        ),
    ]
}

fn promotion_canonical_facts(record: &MutationPromotionRecord) -> Vec<CanonicalFact> {
    let prefix = &record.promotion_id;
    vec![
        canonical_fact(prefix, "mutation-id", record.mutation_id.clone(), &[]),
        canonical_fact(
            prefix,
            "proposal-fingerprint",
            record.proposal_fingerprint.clone(),
            &[],
        ),
        canonical_fact(
            prefix,
            "promotion-root-fingerprint",
            record.promotion_root_fingerprint.clone(),
            &[],
        ),
        canonical_fact(
            prefix,
            "verified-root-fingerprint",
            record.verified_root_fingerprint.clone(),
            &[],
        ),
        canonical_fact(prefix, "status", record.status, &[]),
        canonical_fact(prefix, "changed-paths", record.changed_paths.clone(), &[]),
        canonical_fact(
            prefix,
            "check-statuses",
            check_statuses(&record.checks),
            &[],
        ),
    ]
}

fn canonical_fact(
    prefix: &str,
    kind: &str,
    value: impl Serialize,
    derived_from: &[String],
) -> CanonicalFact {
    CanonicalFact {
        fact_id: format!("fact:{prefix}:{kind}"),
        kind: kind.to_owned(),
        value: serde_json::to_value(value)
            .expect("serializing core evidence fact value should not fail"),
        quality: DataQuality::Canonicalized,
        derived_from: derived_from.to_vec(),
        metadata: BTreeMap::new(),
    }
}

fn check_statuses(checks: &[VerificationCheckResult]) -> BTreeMap<String, VerificationCheckStatus> {
    checks
        .iter()
        .map(|check| (check.name.clone(), check.status))
        .collect()
}

fn promotion_comparisons(record: &MutationPromotionRecord) -> Vec<ComparisonReport> {
    let (status, reason) = compare_root_fingerprints(
        record.verified_root_fingerprint.as_deref(),
        &record.promotion_root_fingerprint,
    );
    vec![ComparisonReport {
        comparison_id: format!(
            "comparison:{}:verified-root-vs-promotion-root",
            record.promotion_id
        ),
        left_ref: format!("fact:{}:verified-root-fingerprint", record.promotion_id),
        right_ref: format!("fact:{}:promotion-root-fingerprint", record.promotion_id),
        status,
        reason,
        quality: DataQuality::Verified,
        metadata: BTreeMap::new(),
    }]
}

fn compare_root_fingerprints(
    verified_root_fingerprint: Option<&str>,
    promotion_root_fingerprint: &str,
) -> (ComparisonStatus, String) {
    let Some(verified_root_fingerprint) = verified_root_fingerprint else {
        return (
            ComparisonStatus::Missing,
            "missing P1 verified root fingerprint".to_owned(),
        );
    };
    if verified_root_fingerprint.starts_with("unavailable:")
        || promotion_root_fingerprint.starts_with("unavailable:")
    {
        return (
            ComparisonStatus::Incomparable,
            "root fingerprint comparison is unavailable for this promotion record".to_owned(),
        );
    }
    if verified_root_fingerprint == promotion_root_fingerprint {
        (
            ComparisonStatus::Matched,
            "P1 verified root fingerprint matches P2 promotion root fingerprint".to_owned(),
        )
    } else {
        (
            ComparisonStatus::Stale,
            "P1 verified root fingerprint differs from P2 promotion root fingerprint".to_owned(),
        )
    }
}

fn verification_claim(status: VerificationStatus) -> &'static str {
    match status {
        VerificationStatus::Verified => "mutation verified",
        VerificationStatus::Rejected => "mutation rejected",
        VerificationStatus::Error => "verification errored",
    }
}

fn promotion_claim(status: PromotionStatus) -> &'static str {
    match status {
        PromotionStatus::Promoted => "mutation promoted",
        PromotionStatus::Rejected => "promotion rejected",
        PromotionStatus::Error => "promotion errored",
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OrganType {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DecayStatus {
    Active,
    Candidate,
    Stale,
    Duplicate,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub organ_type: OrganType,
    pub source: String,
    pub purpose: String,
    pub created_by: Option<String>,
    pub usage_count: Option<u64>,
    pub failure_count: Option<u64>,
    pub related_goals: Vec<String>,
    pub decay_status: Option<DecayStatus>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provenance(source: &str) -> ProvenanceRef {
        ProvenanceRef {
            kind: "jsonl-record".to_owned(),
            source: source.to_owned(),
            digest: DigestRef {
                algorithm: "sha256".to_owned(),
                value: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_owned(),
            },
            schema_version: "0.1.0".to_owned(),
            metadata: BTreeMap::new(),
        }
    }

    fn check(status: VerificationCheckStatus) -> VerificationCheckResult {
        VerificationCheckResult {
            name: "nix-flake-check".to_owned(),
            command: "nix".to_owned(),
            args: vec!["flake".to_owned(), "check".to_owned()],
            status,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        }
    }

    fn verification_record(status: VerificationStatus) -> MutationVerificationRecord {
        MutationVerificationRecord {
            verification_id: "ver-test".to_owned(),
            timestamp: "2026-05-10T00:00:00Z".to_owned(),
            mutation_id: "mut-test".to_owned(),
            goal: "test evidence mapping".to_owned(),
            phase: "P1".to_owned(),
            status,
            reason: "test verification".to_owned(),
            proposal_fingerprint:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            root_fingerprint:
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            checks: vec![check(VerificationCheckStatus::Passed)],
            side_effects: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn promotion_record(status: PromotionStatus) -> MutationPromotionRecord {
        MutationPromotionRecord {
            promotion_id: "pro-test".to_owned(),
            timestamp: "2026-05-10T00:00:00Z".to_owned(),
            mutation_id: "mut-test".to_owned(),
            goal: "test evidence mapping".to_owned(),
            phase: "P2".to_owned(),
            status,
            reason: "test promotion".to_owned(),
            verification_id: Some("ver-test".to_owned()),
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
            changed_organs: vec!["docs".to_owned()],
            checks: vec![check(VerificationCheckStatus::Passed)],
            metadata: BTreeMap::new(),
        }
    }

    #[test]
    fn verification_record_maps_to_backed_evidence_bundle() {
        let bundle = verification_record(VerificationStatus::Verified)
            .to_evidence_bundle(provenance("memory/mutation-results.jsonl:1"));

        assert_eq!(bundle.schema_version, "0.1.0");
        assert_eq!(bundle.subject.mutation_id, "mut-test");
        assert_eq!(bundle.subject.generation_id, None);
        assert_eq!(bundle.claims[0].claim, "mutation verified");
        assert!(
            bundle.claims[0]
                .backing
                .contains(&"observation:ver-test:record".to_owned())
        );
        assert!(
            bundle
                .canonical_facts
                .iter()
                .any(|fact| fact.fact_id == "fact:ver-test:root-fingerprint")
        );
    }

    #[test]
    fn promotion_record_maps_root_fingerprint_comparison() {
        let bundle = promotion_record(PromotionStatus::Promoted)
            .to_evidence_bundle(provenance("memory/mutation-promotions.jsonl:1"));

        assert_eq!(
            bundle.subject.root_fingerprint.as_deref(),
            Some("sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        );
        assert_eq!(bundle.claims[0].claim, "mutation promoted");
        assert_eq!(bundle.comparisons[0].status, ComparisonStatus::Matched);
        assert!(
            bundle.claims[0]
                .backing
                .contains(&"comparison:pro-test:verified-root-vs-promotion-root".to_owned())
        );
    }

    #[test]
    fn promotion_record_marks_root_fingerprint_drift_as_stale() {
        let mut record = promotion_record(PromotionStatus::Rejected);
        record.promotion_root_fingerprint =
            "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_owned();

        let bundle = record.to_evidence_bundle(provenance("memory/mutation-promotions.jsonl:2"));

        assert_eq!(bundle.comparisons[0].status, ComparisonStatus::Stale);
        assert_eq!(bundle.claims[0].claim, "promotion rejected");
    }

    #[test]
    fn promotion_record_marks_missing_verified_root_as_missing() {
        let mut record = promotion_record(PromotionStatus::Rejected);
        record.verified_root_fingerprint = None;
        record.promotion_root_fingerprint = "unavailable:no-verification-evidence".to_owned();

        let bundle = record.to_evidence_bundle(provenance("memory/mutation-promotions.jsonl:3"));

        assert_eq!(bundle.comparisons[0].status, ComparisonStatus::Missing);
        assert!(bundle.comparisons[0].reason.contains("missing P1"));
    }

    #[test]
    fn promotion_record_marks_unavailable_root_as_incomparable() {
        let mut record = promotion_record(PromotionStatus::Rejected);
        record.promotion_root_fingerprint = "unavailable:not-verified".to_owned();

        let bundle = record.to_evidence_bundle(provenance("memory/mutation-promotions.jsonl:4"));

        assert_eq!(bundle.comparisons[0].status, ComparisonStatus::Incomparable);
        assert!(bundle.comparisons[0].reason.contains("unavailable"));
    }
}
