#![deny(clippy::correctness)]
#![warn(clippy::suspicious, clippy::style, clippy::complexity, clippy::perf)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationRecord {
    pub timestamp: String,
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
