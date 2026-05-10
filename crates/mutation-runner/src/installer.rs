use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use anyhow::{Context, Result, bail};
use autopoietic_core::{
    EffectRecord, EffectRisk, GenerationRecord, InstallPlanOutput, InstallSeedFilePlan,
    InstallSeedFileStatus, InstallSeedFileVerification, InstallSeedManifest, InstallVerifyOutput,
    LineageStatus, MutationPromotionRecord, MutationVerificationRecord, PlannedEffect,
    PromotionStatus, VerificationCheckStatus,
};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::verifier::{
    append_jsonl, evidence_provenance, optional_evidence_bundle_path, warn_evidence_bundle_failure,
    write_json,
};

#[derive(Debug, Clone)]
pub(crate) struct InstallPlanConfig {
    pub(crate) promotion_journal_path: PathBuf,
    pub(crate) verification_journal_path: PathBuf,
    pub(crate) generation_journal_path: PathBuf,
    pub(crate) promotion_id: Option<String>,
    pub(crate) mutation_id: Option<String>,
    pub(crate) target_root: PathBuf,
    pub(crate) parent_generation: String,
    pub(crate) resulting_generation: String,
    pub(crate) record: bool,
    pub(crate) evidence_bundle_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub(crate) struct InstallVerifyConfig {
    pub(crate) plan_path: PathBuf,
    pub(crate) evidence_bundle_path: Option<PathBuf>,
}

pub(crate) fn install_plan_and_record(config: InstallPlanConfig) -> Result<InstallPlanOutput> {
    let evidence_bundle_path = optional_p3_evidence_bundle_path(
        "install-plan",
        config.evidence_bundle_path.as_deref(),
        &[
            &config.promotion_journal_path,
            &config.verification_journal_path,
            &config.generation_journal_path,
        ],
        &config.target_root,
    );
    let promotion = read_selected_promotion(&config)?;
    validate_install_plan_input(&config, &promotion)?;
    let verification = read_selected_verification(&config, &promotion)?;
    validate_verification_seed(&promotion, &verification)?;
    let record = generation_record(&config, &promotion);
    let seed_manifest = seed_manifest(&config, &promotion, &verification, &record)?;
    let output = InstallPlanOutput {
        generation: record,
        seed_manifest,
    };
    if config.record {
        append_jsonl(&config.generation_journal_path, &output.generation)?;
    }
    if let Some(path) = &evidence_bundle_path {
        let write_result = evidence_provenance(
            "install-plan-output",
            "mutation-runner install-plan output".to_owned(),
            "0.1.0",
            &output,
        )
        .and_then(|provenance| write_json(path, &output.to_evidence_bundle(provenance)));
        if let Err(error) = write_result {
            warn_evidence_bundle_failure("install-plan", error);
        }
    }
    Ok(output)
}

pub(crate) fn verify_install_plan(config: InstallVerifyConfig) -> Result<InstallVerifyOutput> {
    let plan = read_install_plan_output(&config.plan_path)?;
    validate_install_verify_plan(&plan)?;
    let evidence_bundle_path = optional_p3_evidence_bundle_path(
        "install-verify",
        config.evidence_bundle_path.as_deref(),
        &[&config.plan_path],
        Path::new(&plan.seed_manifest.target_root),
    );
    let files = plan
        .seed_manifest
        .files
        .iter()
        .map(verify_seed_file)
        .collect::<Vec<_>>();
    let all_matched = files
        .iter()
        .all(|file| file.status == InstallSeedFileStatus::Matched);
    let output = InstallVerifyOutput {
        verified_at: Utc::now().to_rfc3339(),
        target_root: plan.seed_manifest.target_root,
        mutation_id: plan.seed_manifest.mutation_id,
        promotion_id: plan.seed_manifest.promotion_id,
        all_matched,
        files,
    };
    if let Some(path) = &evidence_bundle_path {
        let write_result = evidence_provenance(
            "install-verify-output",
            "mutation-runner install-verify output".to_owned(),
            "0.1.0",
            &output,
        )
        .and_then(|provenance| write_json(path, &output.to_evidence_bundle(provenance)));
        if let Err(error) = write_result {
            warn_evidence_bundle_failure("install-verify", error);
        }
    }
    Ok(output)
}

fn optional_p3_evidence_bundle_path(
    context: &str,
    evidence_bundle_path: Option<&Path>,
    protected_paths: &[&Path],
    target_root: &Path,
) -> Option<PathBuf> {
    let path = optional_evidence_bundle_path(context, evidence_bundle_path, protected_paths)?;
    match evidence_bundle_is_inside_target_root(&path, target_root) {
        Ok(true) => {
            warn_evidence_bundle_failure(
                context,
                anyhow::anyhow!("EvidenceBundle output must not be inside the P3 target root"),
            );
            None
        }
        Ok(false) => Some(path),
        Err(error) => {
            warn_evidence_bundle_failure(context, error);
            None
        }
    }
}

fn evidence_bundle_is_inside_target_root(path: &Path, target_root: &Path) -> Result<bool> {
    let path = absolute_path(path).context("failed to resolve EvidenceBundle output path")?;
    let target_root = absolute_path(target_root).context("failed to resolve P3 target root")?;
    Ok(normalized_starts_with(&path, &target_root))
}

fn validate_install_verify_plan(plan: &InstallPlanOutput) -> Result<()> {
    if plan.seed_manifest.schema_version != "0.1.0" {
        bail!(
            "unsupported install seed manifest schema_version: {}",
            plan.seed_manifest.schema_version
        );
    }
    if plan.seed_manifest.files.is_empty() {
        bail!("install seed manifest contains no files to verify");
    }
    let target_root = Path::new(&plan.seed_manifest.target_root);
    if !target_root.is_absolute() {
        bail!("install seed manifest target_root must be absolute");
    }
    for file in &plan.seed_manifest.files {
        let installed_path = Path::new(&file.installed_path);
        if !installed_path.is_absolute() {
            bail!(
                "install seed installed_path must be absolute: {}",
                file.installed_path
            );
        }
        if path_has_parent_component(installed_path) {
            bail!(
                "install seed installed_path must not contain parent components: {}",
                file.installed_path
            );
        }
        if !is_sha256_uri(&file.content_sha256) {
            bail!(
                "install seed content_sha256 must match sha256:<64 lowercase hex>: {}",
                file.content_sha256
            );
        }
        let target_path = Path::new(&file.target_path);
        if !target_path.is_absolute() {
            bail!(
                "install seed target_path must be absolute: {}",
                file.target_path
            );
        }
        if !normalized_starts_with(target_path, target_root) {
            bail!(
                "install seed target_path is outside target_root: {}",
                file.target_path
            );
        }
        let expected_target_path = expected_target_path(target_root, installed_path)?;
        if normalize_components(target_path) != normalize_components(&expected_target_path) {
            bail!(
                "install seed target_path does not match target_root plus installed_path: {}",
                file.target_path
            );
        }
    }
    Ok(())
}

fn read_install_plan_output(path: &Path) -> Result<InstallPlanOutput> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read install plan {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse install plan {}", path.display()))
}

fn verify_seed_file(file: &InstallSeedFilePlan) -> InstallSeedFileVerification {
    match read_seed_file_without_symlink_traversal(Path::new(&file.target_path)) {
        Ok(bytes) => {
            let actual_sha256 = sha256(&bytes);
            if actual_sha256 == file.content_sha256 {
                seed_verification(
                    file,
                    Some(actual_sha256),
                    InstallSeedFileStatus::Matched,
                    "content hash matched".to_owned(),
                )
            } else {
                seed_verification(
                    file,
                    Some(actual_sha256),
                    InstallSeedFileStatus::Mismatched,
                    "content hash did not match install seed manifest".to_owned(),
                )
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => seed_verification(
            file,
            None,
            InstallSeedFileStatus::Missing,
            "target file is missing".to_owned(),
        ),
        Err(error) => seed_verification(
            file,
            None,
            InstallSeedFileStatus::Error,
            format!("failed to read target file: {error}"),
        ),
    }
}

fn read_seed_file_without_symlink_traversal(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    if path_has_symlink_ancestor_for_seed(path)? {
        return Err(std::io::Error::other(
            "target path traverses a symlink before read",
        ));
    }

    ensure_path_is_regular_seed_file(path)?;

    let mut file = open_seed_file(path)?;

    if path_has_symlink_ancestor_for_seed(path)? {
        return Err(std::io::Error::other(
            "target path traverses a symlink after read opened the file",
        ));
    }

    ensure_opened_file_still_matches_path(&file, path)?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    ensure_opened_file_still_matches_path(&file, path)?;
    Ok(bytes)
}

fn ensure_path_is_regular_seed_file(path: &Path) -> Result<(), std::io::Error> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(std::io::Error::other(
            "target path is not a regular seed file",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn open_seed_file(path: &Path) -> Result<File, std::io::Error> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_seed_file(path: &Path) -> Result<File, std::io::Error> {
    File::open(path)
}

fn path_has_symlink_ancestor_for_seed(path: &Path) -> Result<bool, std::io::Error> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

#[cfg(unix)]
fn ensure_opened_file_still_matches_path(file: &File, path: &Path) -> Result<(), std::io::Error> {
    let open_metadata = file.metadata()?;
    if !open_metadata.file_type().is_file() {
        return Err(std::io::Error::other(
            "opened target path is not a regular seed file",
        ));
    }
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_file() {
        return Err(std::io::Error::other(
            "current target path is not a regular seed file",
        ));
    }
    if open_metadata.dev() != path_metadata.dev() || open_metadata.ino() != path_metadata.ino() {
        return Err(std::io::Error::other(
            "target path changed while verification was opening the file",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_opened_file_still_matches_path(_file: &File, _path: &Path) -> Result<(), std::io::Error> {
    Ok(())
}

fn is_sha256_uri(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn path_has_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

fn expected_target_path(target_root: &Path, installed_path: &Path) -> Result<PathBuf> {
    let relative_installed_path =
        installed_path
            .strip_prefix(Path::new("/"))
            .with_context(|| {
                format!(
                    "failed to relativize installed_path {}",
                    installed_path.display()
                )
            })?;
    Ok(target_root.join(relative_installed_path))
}

fn seed_verification(
    file: &InstallSeedFilePlan,
    actual_sha256: Option<String>,
    status: InstallSeedFileStatus,
    reason: String,
) -> InstallSeedFileVerification {
    InstallSeedFileVerification {
        installed_path: file.installed_path.clone(),
        target_path: file.target_path.clone(),
        expected_sha256: file.content_sha256.clone(),
        actual_sha256,
        status,
        reason,
    }
}

fn read_selected_promotion(config: &InstallPlanConfig) -> Result<MutationPromotionRecord> {
    if config.promotion_id.is_none() && config.mutation_id.is_none() {
        bail!("install plan requires --promotion-id or --mutation-id");
    }
    let contents = fs::read_to_string(&config.promotion_journal_path).with_context(|| {
        format!(
            "failed to read promotion journal {}",
            config.promotion_journal_path.display()
        )
    })?;
    let mut selected = None;
    let mut matched = 0usize;
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: MutationPromotionRecord = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse promotion journal {} line {}",
                config.promotion_journal_path.display(),
                index + 1
            )
        })?;
        if matches_selector(config, &record) {
            matched += 1;
            selected = Some(record);
        }
    }
    if matched > 1 {
        bail!(
            "multiple promotion records matched selector {}; rerun with a unique --promotion-id",
            selector_summary(config)
        );
    }
    selected.with_context(|| "no matching promotion evidence found".to_owned())
}

fn read_selected_verification(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
) -> Result<MutationVerificationRecord> {
    let verification_id = promotion
        .verification_id
        .as_deref()
        .context("promoted P2 evidence is missing verification_id")?;
    let contents = fs::read_to_string(&config.verification_journal_path).with_context(|| {
        format!(
            "failed to read verification journal {}",
            config.verification_journal_path.display()
        )
    })?;
    let mut selected = None;
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: MutationVerificationRecord = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse verification journal {} line {}",
                config.verification_journal_path.display(),
                index + 1
            )
        })?;
        if record.verification_id == verification_id && record.mutation_id == promotion.mutation_id
        {
            if selected.is_some() {
                bail!("multiple verification records matched promotion evidence");
            }
            selected = Some(record);
        }
    }
    selected.with_context(|| "no matching P1 verification evidence found".to_owned())
}

fn selector_summary(config: &InstallPlanConfig) -> String {
    match (&config.promotion_id, &config.mutation_id) {
        (Some(promotion_id), Some(mutation_id)) => {
            format!("promotion_id={promotion_id}, mutation_id={mutation_id}")
        }
        (Some(promotion_id), None) => format!("promotion_id={promotion_id}"),
        (None, Some(mutation_id)) => format!("mutation_id={mutation_id}"),
        (None, None) => "<none>".to_owned(),
    }
}

fn matches_selector(config: &InstallPlanConfig, record: &MutationPromotionRecord) -> bool {
    let promotion_matches = config
        .promotion_id
        .as_ref()
        .is_none_or(|id| record.promotion_id == *id);
    let mutation_matches = config
        .mutation_id
        .as_ref()
        .is_none_or(|id| record.mutation_id == *id);
    promotion_matches && mutation_matches
}

fn validate_install_plan_input(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
) -> Result<()> {
    if promotion.status != PromotionStatus::Promoted {
        bail!(
            "P3 install plan requires promoted P2 evidence, got {:?}",
            promotion.status
        );
    }
    if promotion.verification_id.is_none() {
        bail!("promoted P2 evidence is missing verification_id");
    }
    if promotion.verified_root_fingerprint.is_none() {
        bail!("promoted P2 evidence is missing verified_root_fingerprint");
    }
    if promotion.checks.is_empty() {
        bail!("promoted P2 evidence is missing VM check evidence");
    }
    if !promotion
        .checks
        .iter()
        .any(|check| check.name.starts_with("vm-check:"))
    {
        bail!("promoted P2 evidence is missing VM check evidence");
    }
    if promotion
        .checks
        .iter()
        .any(|check| check.status != VerificationCheckStatus::Passed)
    {
        bail!("promoted P2 evidence contains non-passing checks");
    }
    if !config.target_root.is_absolute() {
        bail!("install plan target root must be an absolute path");
    }
    if config.record && generation_journal_is_inside_target_root(config)? {
        bail!("generation journal for --record must not be inside the install target root");
    }
    if config.record && generation_journal_uses_symlink(config)? {
        bail!("generation journal for --record must not traverse symlinks");
    }
    if config.record && target_root_uses_symlink(config)? {
        bail!("target root for --record must not traverse symlinks");
    }
    if config.parent_generation.trim().is_empty() {
        bail!("install plan requires a non-empty parent generation");
    }
    if config.resulting_generation.trim().is_empty() {
        bail!("install plan requires a non-empty resulting generation");
    }
    Ok(())
}

fn validate_verification_seed(
    promotion: &MutationPromotionRecord,
    verification: &MutationVerificationRecord,
) -> Result<()> {
    if verification.status != autopoietic_core::VerificationStatus::Verified {
        bail!("seeded P1 verification evidence is not verified");
    }
    if verification
        .checks
        .iter()
        .any(|check| check.status != VerificationCheckStatus::Passed)
    {
        bail!("seeded P1 verification evidence contains non-passing checks");
    }
    if verification.proposal_fingerprint != promotion.proposal_fingerprint {
        bail!("seeded P1 verification proposal fingerprint does not match P2 promotion");
    }
    if promotion.verified_root_fingerprint.as_deref()
        != Some(verification.root_fingerprint.as_str())
    {
        bail!("seeded P1 verification root fingerprint does not match P2 promotion");
    }
    Ok(())
}

fn generation_journal_is_inside_target_root(config: &InstallPlanConfig) -> Result<bool> {
    let journal_path = absolute_path(&config.generation_journal_path)
        .context("failed to resolve generation journal path")?;
    Ok(normalized_starts_with(&journal_path, &config.target_root))
}

fn generation_journal_uses_symlink(config: &InstallPlanConfig) -> Result<bool> {
    path_has_symlink_ancestor(&absolute_path(&config.generation_journal_path)?)
}

fn target_root_uses_symlink(config: &InstallPlanConfig) -> Result<bool> {
    path_has_symlink_ancestor(&absolute_path(&config.target_root)?)
}

fn path_has_symlink_ancestor(path: &Path) -> Result<bool> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect journal path {}", current.display())
                });
            }
        }
    }
    Ok(false)
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("failed to read current directory")?
            .join(path))
    }
}

fn normalized_starts_with(path: &Path, prefix: &Path) -> bool {
    normalize_components(path).starts_with(normalize_components(prefix))
}

fn normalize_components(path: &Path) -> PathBuf {
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

fn generation_record(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
) -> GenerationRecord {
    let mut metadata = BTreeMap::new();
    metadata.insert("parent_genome".to_owned(), promotion.parent_genome.clone());
    metadata.insert(
        "proposal_fingerprint".to_owned(),
        promotion.proposal_fingerprint.clone(),
    );
    metadata.insert(
        "promotion_root_fingerprint".to_owned(),
        promotion.promotion_root_fingerprint.clone(),
    );
    if let Some(fingerprint) = &promotion.verified_root_fingerprint {
        metadata.insert("verified_root_fingerprint".to_owned(), fingerprint.clone());
    }

    GenerationRecord {
        timestamp: Utc::now().to_rfc3339(),
        lineage_status: LineageStatus::Planned,
        generation: config.resulting_generation.clone(),
        mutation_id: promotion.mutation_id.clone(),
        goal: promotion.goal.clone(),
        changed_organs: promotion.changed_organs.clone(),
        parent_generation: Some(config.parent_generation.clone()),
        activation_result: "planned-install".to_owned(),
        verification_id: promotion.verification_id.clone(),
        promotion_id: Some(promotion.promotion_id.clone()),
        target_root: Some(config.target_root.display().to_string()),
        target_configuration: Some(promotion.target_configuration.clone()),
        metadata,
    }
}

fn seed_manifest(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
    verification: &MutationVerificationRecord,
    generation: &GenerationRecord,
) -> Result<InstallSeedManifest> {
    let promotion_id = generation
        .promotion_id
        .clone()
        .context("generation record is missing promotion_id")?;
    Ok(InstallSeedManifest {
        schema_version: "0.1.0".to_owned(),
        generated_at: Utc::now().to_rfc3339(),
        target_root: config.target_root.display().to_string(),
        mutation_id: promotion.mutation_id.clone(),
        promotion_id,
        lineage_status: generation.lineage_status,
        files: seed_files(config, promotion, verification, generation)?,
    })
}

fn seed_files(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
    verification: &MutationVerificationRecord,
    generation: &GenerationRecord,
) -> Result<Vec<InstallSeedFilePlan>> {
    let identity_seed = serde_json::json!({
        "host": promotion.target_configuration,
        "roles": ["autopoietic", "installed-seed"]
    });
    let planned_effects = planned_effect_records(config, promotion);

    Ok(vec![
        seed_file(
            config,
            "/etc/autopoietic/identity.json",
            "synthetic:p3-install-plan:identity",
            json_bytes(&identity_seed)?,
        )?,
        seed_file(
            config,
            "/var/lib/autopoietic/mutation-results.jsonl",
            "evidence:p1-verification-record",
            jsonl_bytes(std::slice::from_ref(verification))?,
        )?,
        seed_file(
            config,
            "/var/lib/autopoietic/mutation-promotions.jsonl",
            "evidence:p2-promotion-record",
            jsonl_bytes(std::slice::from_ref(promotion))?,
        )?,
        seed_file(
            config,
            "/var/lib/autopoietic/generations.jsonl",
            "evidence:p3-generation-lineage-record",
            jsonl_bytes(std::slice::from_ref(generation))?,
        )?,
        seed_file(
            config,
            "/var/lib/autopoietic/effects.jsonl",
            "synthetic:p3-planned-effect-ledger",
            jsonl_bytes(&planned_effects)?,
        )?,
    ])
}

fn planned_effect_records(
    config: &InstallPlanConfig,
    promotion: &MutationPromotionRecord,
) -> Vec<EffectRecord> {
    let now = Utc::now().to_rfc3339();
    planned_seed_installed_paths()
        .iter()
        .map(|installed_path| {
            let target = target_path_for_installed_path(&config.target_root, installed_path)
                .expect("static installed paths are absolute");
            EffectRecord {
                effect_id: format!(
                    "eff-plan-{}",
                    sha256(format!("{}:{}", promotion.promotion_id, target.display()).as_bytes())
                ),
                timestamp: now.clone(),
                mutation_id: promotion.mutation_id.clone(),
                effect_type: "planned-seed-file-write".to_owned(),
                target: target.display().to_string(),
                reversible: false,
                compensation: "inspect target state before applying; do not remove blindly"
                    .to_owned(),
                verified_by: "mutation-runner install-plan".to_owned(),
                risk: EffectRisk::Medium,
                metadata: BTreeMap::from([(
                    "installed_path".to_owned(),
                    (*installed_path).to_owned(),
                )]),
            }
        })
        .collect()
}

fn planned_seed_installed_paths() -> [&'static str; 5] {
    [
        "/etc/autopoietic/identity.json",
        "/var/lib/autopoietic/mutation-results.jsonl",
        "/var/lib/autopoietic/mutation-promotions.jsonl",
        "/var/lib/autopoietic/generations.jsonl",
        "/var/lib/autopoietic/effects.jsonl",
    ]
}

fn seed_file(
    config: &InstallPlanConfig,
    installed_path: &str,
    source: &str,
    content_bytes: Vec<u8>,
) -> Result<InstallSeedFilePlan> {
    let target_path = target_path_for_installed_path(&config.target_root, installed_path)?;
    Ok(InstallSeedFilePlan {
        installed_path: installed_path.to_owned(),
        target_path: target_path.display().to_string(),
        source: source.to_owned(),
        content_sha256: sha256(&content_bytes),
        effect: PlannedEffect {
            effect_type: "planned-seed-file-write".to_owned(),
            target: target_path.display().to_string(),
            reversible: false,
            compensation: "inspect target state before applying; do not remove blindly".to_owned(),
            verified_by: "mutation-runner install-plan".to_owned(),
            risk: EffectRisk::Medium,
            metadata: BTreeMap::from([("installed_path".to_owned(), installed_path.to_owned())]),
        },
    })
}

fn json_bytes<T: Serialize>(content: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(content).context("failed to serialize seed content")
}

fn jsonl_bytes<T: Serialize>(items: &[T]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for item in items {
        serde_json::to_writer(&mut bytes, item).context("failed to serialize JSONL seed entry")?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn target_path_for_installed_path(target_root: &Path, installed_path: &str) -> Result<PathBuf> {
    let relative = installed_path
        .strip_prefix('/')
        .with_context(|| format!("installed seed path must be absolute: {installed_path}"))?;
    Ok(target_root.join(relative))
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", to_hex(&hasher.finalize()))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopoietic_core::{
        MutationVerificationRecord, VerificationCheckResult, VerificationStatus,
    };
    use std::fs;
    use uuid::Uuid;

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("autopoietic-{name}-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("test temp root should be created");
        root
    }

    fn promotion(status: PromotionStatus) -> MutationPromotionRecord {
        MutationPromotionRecord {
            promotion_id: "pro-test".to_owned(),
            timestamp: Utc::now().to_rfc3339(),
            mutation_id: "mut-test".to_owned(),
            goal: "test install plan".to_owned(),
            phase: "P2-test".to_owned(),
            status,
            reason: "test evidence".to_owned(),
            verification_id: Some("ver-test".to_owned()),
            proposal_fingerprint:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            verified_root_fingerprint: Some(
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_owned(),
            ),
            promotion_root_fingerprint:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            parent_genome: "git:parent".to_owned(),
            target_configuration: "iso".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            changed_organs: vec!["docs".to_owned()],
            checks: vec![VerificationCheckResult {
                name: "vm-check:iso-boot-basic".to_owned(),
                command: "nix".to_owned(),
                args: Vec::new(),
                status: VerificationCheckStatus::Passed,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            }],
            metadata: BTreeMap::new(),
        }
    }

    fn write_promotion(path: &Path, record: &MutationPromotionRecord) {
        fs::write(
            path,
            format!(
                "{}\n",
                serde_json::to_string(record).expect("promotion should serialize")
            ),
        )
        .expect("promotion journal should be written");
    }

    fn verification() -> MutationVerificationRecord {
        MutationVerificationRecord {
            verification_id: "ver-test".to_owned(),
            timestamp: Utc::now().to_rfc3339(),
            mutation_id: "mut-test".to_owned(),
            goal: "test install plan".to_owned(),
            phase: "P1-test".to_owned(),
            status: VerificationStatus::Verified,
            reason: "test verification evidence".to_owned(),
            proposal_fingerprint:
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            root_fingerprint:
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            checks: vec![VerificationCheckResult {
                name: "verification-smoke".to_owned(),
                command: "true".to_owned(),
                args: Vec::new(),
                status: VerificationCheckStatus::Passed,
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            }],
            side_effects: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn write_verification(path: &Path, record: &MutationVerificationRecord) {
        fs::write(
            path,
            format!(
                "{}\n",
                serde_json::to_string(record).expect("verification should serialize")
            ),
        )
        .expect("verification journal should be written");
    }

    fn verify_plan(root: &Path, content_sha256: String) -> InstallPlanOutput {
        let target_path = root.join("installed-root/etc/autopoietic/identity.json");
        InstallPlanOutput {
            generation: GenerationRecord {
                timestamp: Utc::now().to_rfc3339(),
                lineage_status: LineageStatus::Planned,
                generation: "gen-child".to_owned(),
                mutation_id: "mut-test".to_owned(),
                goal: "test install verify".to_owned(),
                changed_organs: Vec::new(),
                parent_generation: Some("gen-parent".to_owned()),
                activation_result: "planned-install".to_owned(),
                verification_id: Some("ver-test".to_owned()),
                promotion_id: Some("pro-test".to_owned()),
                target_root: Some(root.join("installed-root").display().to_string()),
                target_configuration: Some("iso".to_owned()),
                metadata: BTreeMap::new(),
            },
            seed_manifest: InstallSeedManifest {
                schema_version: "0.1.0".to_owned(),
                generated_at: Utc::now().to_rfc3339(),
                target_root: root.join("installed-root").display().to_string(),
                mutation_id: "mut-test".to_owned(),
                promotion_id: "pro-test".to_owned(),
                lineage_status: LineageStatus::Planned,
                files: vec![InstallSeedFilePlan {
                    installed_path: "/etc/autopoietic/identity.json".to_owned(),
                    target_path: target_path.display().to_string(),
                    source: "test".to_owned(),
                    content_sha256,
                    effect: PlannedEffect {
                        effect_type: "planned-seed-file-write".to_owned(),
                        target: target_path.display().to_string(),
                        reversible: false,
                        compensation: "test".to_owned(),
                        verified_by: "test".to_owned(),
                        risk: EffectRisk::Low,
                        metadata: BTreeMap::new(),
                    },
                }],
            },
        }
    }

    fn write_install_plan(path: &Path, plan: &InstallPlanOutput) {
        fs::write(
            path,
            serde_json::to_vec(plan).expect("install plan should serialize"),
        )
        .expect("install plan should be written");
    }

    fn install_verify_config(plan_path: PathBuf) -> InstallVerifyConfig {
        InstallVerifyConfig {
            plan_path,
            evidence_bundle_path: None,
        }
    }

    fn config(root: &Path) -> InstallPlanConfig {
        let verification_journal_path = root.join("verifications.jsonl");
        if !verification_journal_path.exists() {
            write_verification(&verification_journal_path, &verification());
        }
        InstallPlanConfig {
            promotion_journal_path: root.join("promotions.jsonl"),
            verification_journal_path,
            generation_journal_path: root.join("generations.jsonl"),
            promotion_id: Some("pro-test".to_owned()),
            mutation_id: None,
            target_root: root.join("installed-root"),
            parent_generation: "gen-parent".to_owned(),
            resulting_generation: "gen-child".to_owned(),
            record: false,
            evidence_bundle_path: None,
        }
    }

    #[test]
    fn promoted_evidence_produces_dry_run_generation_record_without_journal_write() {
        let root = temp_root("install-plan-dry-run");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));

        let output = install_plan_and_record(config(&root)).expect("install plan should be built");
        let record = output.generation;

        assert_eq!(record.activation_result, "planned-install");
        assert_eq!(record.lineage_status, LineageStatus::Planned);
        assert_eq!(record.mutation_id, "mut-test");
        assert_eq!(record.parent_generation.as_deref(), Some("gen-parent"));
        assert_eq!(record.generation, "gen-child");
        assert_eq!(record.verification_id.as_deref(), Some("ver-test"));
        assert_eq!(record.promotion_id.as_deref(), Some("pro-test"));
        assert_eq!(record.target_configuration.as_deref(), Some("iso"));
        assert_eq!(output.seed_manifest.lineage_status, LineageStatus::Planned);
        assert_eq!(output.seed_manifest.files.len(), 5);
        assert!(
            output
                .seed_manifest
                .files
                .iter()
                .all(|file| file.content_sha256.starts_with("sha256:"))
        );
        assert!(
            output
                .seed_manifest
                .files
                .iter()
                .any(|file| { file.installed_path == "/var/lib/autopoietic/generations.jsonl" })
        );
        assert!(!root.join("generations.jsonl").exists());
    }

    #[test]
    fn install_plan_can_write_evidence_bundle() {
        let root = temp_root("install-plan-evidence-bundle");
        let promotion_journal = root.join("promotions.jsonl");
        let bundle_path = root.join("evidence/install-plan.json");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.evidence_bundle_path = Some(bundle_path.clone());

        let output = install_plan_and_record(config).expect("install plan should be built");

        let bundle: autopoietic_core::EvidenceBundle = serde_json::from_slice(
            &fs::read(bundle_path).expect("install-plan bundle should be written"),
        )
        .expect("install-plan bundle should parse");
        assert_eq!(bundle.phase, "P3");
        assert_eq!(bundle.subject.generation_id.as_deref(), Some("gen-child"));
        assert_eq!(bundle.claims[0].claim, "install planned");
        assert_eq!(output.generation.generation, "gen-child");
    }

    #[test]
    fn install_plan_skips_evidence_bundle_overwriting_generation_journal_without_changing_gate() {
        let root = temp_root("install-plan-evidence-overwrite-generation");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.evidence_bundle_path = Some(config.generation_journal_path.clone());

        let output =
            install_plan_and_record(config).expect("unsafe bundle path should not gate P3");

        assert_eq!(output.generation.lineage_status, LineageStatus::Planned);
        assert!(!root.join("generations.jsonl").exists());
    }

    #[test]
    fn install_plan_skips_evidence_bundle_inside_target_root_without_changing_gate() {
        let root = temp_root("install-plan-evidence-inside-target-root");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        let bundle_path = config.target_root.join("memory/evidence/install-plan.json");
        config.evidence_bundle_path = Some(bundle_path.clone());

        let output =
            install_plan_and_record(config).expect("target-root bundle path should not gate P3");

        assert_eq!(output.generation.lineage_status, LineageStatus::Planned);
        assert!(!bundle_path.exists());
    }

    #[test]
    fn record_flag_appends_generation_lineage_journal() {
        let root = temp_root("install-plan-record");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.record = true;

        let output = install_plan_and_record(config).expect("install plan should be recorded");
        let record = output.generation;

        let journal = fs::read_to_string(root.join("generations.jsonl"))
            .expect("generation journal should exist");
        assert!(journal.contains(&record.generation));
        assert!(journal.contains("planned"));
        assert!(journal.contains("pro-test"));
        assert!(journal.contains("planned-install"));
    }

    #[test]
    fn mutation_id_selector_rejects_multiple_matching_promotions() {
        let root = temp_root("install-plan-ambiguous-mutation");
        let promotion_journal = root.join("promotions.jsonl");
        let mut first = promotion(PromotionStatus::Promoted);
        first.promotion_id = "pro-first".to_owned();
        let mut second = promotion(PromotionStatus::Promoted);
        second.promotion_id = "pro-second".to_owned();
        fs::write(
            &promotion_journal,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first).expect("first promotion should serialize"),
                serde_json::to_string(&second).expect("second promotion should serialize")
            ),
        )
        .expect("promotion journal should be written");
        let mut config = config(&root);
        config.promotion_id = None;
        config.mutation_id = Some("mut-test".to_owned());

        let error = install_plan_and_record(config).expect_err("ambiguous mutation selector fails");

        assert!(error.to_string().contains("multiple promotion records"));
    }

    #[test]
    fn promotion_id_selector_allows_matching_promotion_among_multiple_records() {
        let root = temp_root("install-plan-promotion-id-selector");
        let promotion_journal = root.join("promotions.jsonl");
        let mut first = promotion(PromotionStatus::Promoted);
        first.promotion_id = "pro-first".to_owned();
        let mut second = promotion(PromotionStatus::Promoted);
        second.promotion_id = "pro-second".to_owned();
        fs::write(
            &promotion_journal,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first).expect("first promotion should serialize"),
                serde_json::to_string(&second).expect("second promotion should serialize")
            ),
        )
        .expect("promotion journal should be written");
        let mut config = config(&root);
        config.promotion_id = Some("pro-first".to_owned());
        config.mutation_id = Some("mut-test".to_owned());

        let record = install_plan_and_record(config)
            .expect("promotion id disambiguates")
            .generation;

        assert_eq!(record.promotion_id.as_deref(), Some("pro-first"));
    }

    #[test]
    fn promotion_id_selector_rejects_duplicate_matching_promotions() {
        let root = temp_root("install-plan-duplicate-promotion-id");
        let promotion_journal = root.join("promotions.jsonl");
        let mut first = promotion(PromotionStatus::Promoted);
        first.promotion_id = "pro-duplicate".to_owned();
        let mut second = promotion(PromotionStatus::Promoted);
        second.promotion_id = "pro-duplicate".to_owned();
        fs::write(
            &promotion_journal,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&first).expect("first promotion should serialize"),
                serde_json::to_string(&second).expect("second promotion should serialize")
            ),
        )
        .expect("promotion journal should be written");
        let mut config = config(&root);
        config.promotion_id = Some("pro-duplicate".to_owned());

        let error = install_plan_and_record(config).expect_err("duplicate promotion id fails");

        assert!(error.to_string().contains("multiple promotion records"));
    }

    #[test]
    fn seed_manifest_effects_are_planned_not_written() {
        let root = temp_root("install-plan-seed-manifest");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));

        let output = install_plan_and_record(config(&root)).expect("install plan should be built");

        for file in &output.seed_manifest.files {
            assert!(file.target_path.starts_with(root.to_str().unwrap()));
            assert_eq!(file.effect.effect_type, "planned-seed-file-write");
            assert!(!file.effect.reversible);
            assert_eq!(file.effect.verified_by, "mutation-runner install-plan");
            assert!(!Path::new(&file.target_path).exists());
        }
    }

    #[test]
    fn install_verify_succeeds_when_seed_files_match_manifest_hashes() {
        let root = temp_root("install-verify-matched");
        let content = br#"{"host":"iso"}"#;
        let plan = verify_plan(&root, sha256(content));
        let target_path = PathBuf::from(&plan.seed_manifest.files[0].target_path);
        fs::create_dir_all(target_path.parent().expect("target file has parent"))
            .expect("target parent should be created");
        fs::write(&target_path, content).expect("target file should be written");
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(install_verify_config(plan_path))
            .expect("install verification should run");

        assert!(output.all_matched);
        assert_eq!(output.files[0].status, InstallSeedFileStatus::Matched);
        assert_eq!(
            output.files[0].actual_sha256.as_deref(),
            Some(sha256(content).as_str())
        );
    }

    #[test]
    fn install_verify_can_write_evidence_bundle() {
        let root = temp_root("install-verify-evidence-bundle");
        let content = br#"{"host":"iso"}"#;
        let plan = verify_plan(&root, sha256(content));
        let target_path = PathBuf::from(&plan.seed_manifest.files[0].target_path);
        fs::create_dir_all(target_path.parent().expect("target file has parent"))
            .expect("target parent should be created");
        fs::write(&target_path, content).expect("target file should be written");
        let plan_path = root.join("plan.json");
        let bundle_path = root.join("evidence/install-verify.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(InstallVerifyConfig {
            plan_path,
            evidence_bundle_path: Some(bundle_path.clone()),
        })
        .expect("install verification should run");

        assert!(output.all_matched);
        let bundle: autopoietic_core::EvidenceBundle = serde_json::from_slice(
            &fs::read(bundle_path).expect("install-verify bundle should be written"),
        )
        .expect("install-verify bundle should parse");
        assert_eq!(bundle.claims[0].claim, "seed files verified");
        assert_eq!(
            bundle.comparisons[0].status,
            autopoietic_core::ComparisonStatus::Matched
        );
        assert_eq!(
            bundle.observations[0].raw_ref.source,
            "mutation-runner install-verify output"
        );
    }

    #[test]
    fn install_verify_skips_evidence_bundle_overwriting_plan_without_changing_gate() {
        let root = temp_root("install-verify-evidence-overwrite-plan");
        let content = br#"{"host":"iso"}"#;
        let plan = verify_plan(&root, sha256(content));
        let target_path = PathBuf::from(&plan.seed_manifest.files[0].target_path);
        fs::create_dir_all(target_path.parent().expect("target file has parent"))
            .expect("target parent should be created");
        fs::write(&target_path, content).expect("target file should be written");
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(InstallVerifyConfig {
            plan_path: plan_path.clone(),
            evidence_bundle_path: Some(plan_path.clone()),
        })
        .expect("unsafe bundle path should not gate install verification");

        assert!(output.all_matched);
        let preserved: InstallPlanOutput =
            serde_json::from_slice(&fs::read(plan_path).expect("plan should remain readable"))
                .expect("plan should remain an install plan");
        assert_eq!(preserved.seed_manifest.promotion_id, "pro-test");
    }

    #[test]
    fn install_verify_skips_evidence_bundle_inside_target_root_without_changing_gate() {
        let root = temp_root("install-verify-evidence-inside-target-root");
        let content = br#"{"host":"iso"}"#;
        let plan = verify_plan(&root, sha256(content));
        let target_path = PathBuf::from(&plan.seed_manifest.files[0].target_path);
        fs::create_dir_all(target_path.parent().expect("target file has parent"))
            .expect("target parent should be created");
        fs::write(&target_path, content).expect("target file should be written");
        let bundle_path = PathBuf::from(&plan.seed_manifest.target_root)
            .join("memory/evidence/install-verify.json");
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(InstallVerifyConfig {
            plan_path,
            evidence_bundle_path: Some(bundle_path.clone()),
        })
        .expect("target-root bundle path should not gate install verification");

        assert!(output.all_matched);
        assert!(!bundle_path.exists());
    }

    #[test]
    fn install_verify_reports_missing_seed_files() {
        let root = temp_root("install-verify-missing");
        let plan = verify_plan(&root, sha256(b"expected"));
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(install_verify_config(plan_path))
            .expect("install verification should run");

        assert!(!output.all_matched);
        assert_eq!(output.files[0].status, InstallSeedFileStatus::Missing);
    }

    #[test]
    fn install_verify_reports_mismatched_seed_files() {
        let root = temp_root("install-verify-mismatched");
        let plan = verify_plan(&root, sha256(b"expected"));
        let target_path = PathBuf::from(&plan.seed_manifest.files[0].target_path);
        fs::create_dir_all(target_path.parent().expect("target file has parent"))
            .expect("target parent should be created");
        fs::write(&target_path, b"actual").expect("target file should be written");
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(install_verify_config(plan_path))
            .expect("install verification should run");

        assert!(!output.all_matched);
        assert_eq!(output.files[0].status, InstallSeedFileStatus::Mismatched);
        assert_eq!(
            output.files[0].actual_sha256.as_deref(),
            Some(sha256(b"actual").as_str())
        );
    }

    #[test]
    fn install_verify_reports_error_for_non_regular_target_paths() {
        let root = temp_root("install-verify-non-regular-target");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].installed_path = "/etc/autopoietic".to_owned();
        plan.seed_manifest.files[0].target_path = root
            .join("installed-root/etc/autopoietic")
            .display()
            .to_string();
        fs::create_dir_all(&plan.seed_manifest.files[0].target_path)
            .expect("target directory should be created");
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(install_verify_config(plan_path))
            .expect("non-regular target path produces a verification report");

        assert!(!output.all_matched);
        assert_eq!(output.files[0].status, InstallSeedFileStatus::Error);
        assert!(output.files[0].reason.contains("not a regular seed file"));
    }

    #[test]
    fn install_verify_rejects_empty_seed_manifest() {
        let root = temp_root("install-verify-empty-manifest");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files = Vec::new();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("empty manifest fails");

        assert!(error.to_string().contains("no files"));
    }

    #[test]
    fn install_verify_rejects_unsupported_seed_manifest_schema_version() {
        let root = temp_root("install-verify-schema-version");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.schema_version = "9.9.9".to_owned();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("unsupported schema version fails");

        assert!(error.to_string().contains("schema_version"));
    }

    #[test]
    fn install_verify_rejects_malformed_seed_manifest_hashes() {
        let root = temp_root("install-verify-malformed-hash");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].content_sha256 = "not-a-sha256-uri".to_owned();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("malformed content hash fails");

        assert!(error.to_string().contains("content_sha256"));
    }

    #[test]
    fn install_verify_rejects_relative_installed_paths() {
        let root = temp_root("install-verify-relative-installed-path");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].installed_path = "etc/autopoietic/identity.json".to_owned();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("relative installed path fails");

        assert!(error.to_string().contains("installed_path"));
    }

    #[test]
    fn install_verify_rejects_installed_paths_with_parent_components() {
        let root = temp_root("install-verify-parent-installed-path");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].installed_path = "/etc/../identity.json".to_owned();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("parent components in installed path fail");

        assert!(error.to_string().contains("parent components"));
    }

    #[test]
    fn install_verify_rejects_relative_target_paths() {
        let root = temp_root("install-verify-relative-target");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].target_path = "relative/path".to_owned();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("relative target path fails");

        assert!(error.to_string().contains("must be absolute"));
    }

    #[test]
    fn install_verify_rejects_target_paths_outside_target_root() {
        let root = temp_root("install-verify-outside-target");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].target_path = root.join("outside-file").display().to_string();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("outside target path fails");

        assert!(error.to_string().contains("outside target_root"));
    }

    #[test]
    fn install_verify_rejects_target_paths_that_do_not_match_installed_paths() {
        let root = temp_root("install-verify-target-binding");
        let mut plan = verify_plan(&root, sha256(b"expected"));
        plan.seed_manifest.files[0].target_path = root
            .join("installed-root/var/lib/autopoietic/identity.json")
            .display()
            .to_string();
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let error = verify_install_plan(install_verify_config(plan_path))
            .expect_err("target path must match installed path");

        assert!(error.to_string().contains("installed_path"));
    }

    #[cfg(unix)]
    #[test]
    fn install_verify_reports_error_for_target_paths_through_symlinks() {
        let root = temp_root("install-verify-symlink-target");
        fs::create_dir_all(root.join("outside"))
            .expect("outside dir should be created for symlink test");
        fs::create_dir_all(root.join("installed-root/etc"))
            .expect("target root parent should be created for symlink test");
        std::os::unix::fs::symlink(
            root.join("outside"),
            root.join("installed-root/etc/autopoietic"),
        )
        .expect("target path symlink should be created");
        let plan = verify_plan(&root, sha256(b"expected"));
        let plan_path = root.join("plan.json");
        write_install_plan(&plan_path, &plan);

        let output = verify_install_plan(install_verify_config(plan_path))
            .expect("symlink target path produces a verification report");

        assert!(!output.all_matched);
        assert_eq!(output.files[0].status, InstallSeedFileStatus::Error);
        assert!(output.files[0].reason.contains("symlink"));
    }

    #[cfg(unix)]
    #[test]
    fn install_verify_rejects_target_path_swapped_to_symlink_after_open() {
        let root = temp_root("install-verify-symlink-after-open");
        let target_path = root.join("target.json");
        let replacement_path = root.join("replacement.json");
        fs::write(&target_path, b"original").expect("target file should be written");
        fs::write(&replacement_path, b"replacement").expect("replacement file should be written");
        let file = File::open(&target_path).expect("target file should open");
        fs::remove_file(&target_path).expect("target file should be removed");
        std::os::unix::fs::symlink(&replacement_path, &target_path)
            .expect("target path should be swapped to a symlink");

        let error = ensure_opened_file_still_matches_path(&file, &target_path)
            .expect_err("current symlink path should fail stability check");

        assert!(error.to_string().contains("regular seed file"));
    }

    #[test]
    fn mismatched_verification_seed_is_rejected() {
        let root = temp_root("install-plan-mismatched-verification");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let verification_journal = root.join("verifications.jsonl");
        let mut verification = verification();
        verification.proposal_fingerprint =
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned();
        write_verification(&verification_journal, &verification);

        let error = install_plan_and_record(config(&root)).expect_err("mismatched P1 seed fails");

        assert!(error.to_string().contains("proposal fingerprint"));
    }

    #[test]
    fn verification_seed_with_failed_checks_is_rejected() {
        let root = temp_root("install-plan-failed-verification-check");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let verification_journal = root.join("verifications.jsonl");
        let mut verification = verification();
        verification.checks[0].status = VerificationCheckStatus::Failed;
        write_verification(&verification_journal, &verification);

        let error = install_plan_and_record(config(&root)).expect_err("failed P1 check fails");

        assert!(error.to_string().contains("non-passing checks"));
    }

    #[test]
    fn rejected_promotion_evidence_cannot_enter_p3_lineage() {
        let root = temp_root("install-plan-rejected");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Rejected));

        let error = install_plan_and_record(config(&root)).expect_err("rejected promotion fails");

        assert!(error.to_string().contains("requires promoted P2 evidence"));
    }

    #[test]
    fn promoted_evidence_without_checks_is_rejected() {
        let root = temp_root("install-plan-no-checks");
        let promotion_journal = root.join("promotions.jsonl");
        let mut record = promotion(PromotionStatus::Promoted);
        record.checks = Vec::new();
        write_promotion(&promotion_journal, &record);

        let error = install_plan_and_record(config(&root)).expect_err("missing checks fail");

        assert!(error.to_string().contains("missing VM check evidence"));
    }

    #[test]
    fn promoted_evidence_without_verified_root_fingerprint_is_rejected() {
        let root = temp_root("install-plan-no-verified-root");
        let promotion_journal = root.join("promotions.jsonl");
        let mut record = promotion(PromotionStatus::Promoted);
        record.verified_root_fingerprint = None;
        write_promotion(&promotion_journal, &record);

        let error =
            install_plan_and_record(config(&root)).expect_err("missing root evidence fails");

        assert!(error.to_string().contains("verified_root_fingerprint"));
    }

    #[test]
    fn promoted_evidence_without_vm_check_is_rejected() {
        let root = temp_root("install-plan-no-vm-check");
        let promotion_journal = root.join("promotions.jsonl");
        let mut record = promotion(PromotionStatus::Promoted);
        record.checks[0].name = "non-vm-check".to_owned();
        write_promotion(&promotion_journal, &record);

        let error = install_plan_and_record(config(&root)).expect_err("missing vm check fails");

        assert!(error.to_string().contains("missing VM check evidence"));
    }

    #[test]
    fn relative_target_root_is_rejected() {
        let root = temp_root("install-plan-relative-root");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.target_root = PathBuf::from("relative-root");

        let error = install_plan_and_record(config).expect_err("relative target root fails");

        assert!(error.to_string().contains("absolute path"));
    }

    #[test]
    fn record_journal_inside_target_root_is_rejected() {
        let root = temp_root("install-plan-journal-in-target");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.record = true;
        config.generation_journal_path = config.target_root.join("memory/generations.jsonl");

        let error = install_plan_and_record(config).expect_err("target-root journal fails");

        assert!(error.to_string().contains("must not be inside"));
    }

    #[test]
    fn relative_record_journal_inside_target_root_is_rejected() {
        let root = temp_root("install-plan-relative-journal-in-target");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let relative_target = format!("autopoietic-target-{}", Uuid::new_v4().simple());
        let mut config = config(&root);
        config.record = true;
        config.target_root = std::env::current_dir()
            .expect("current dir should exist")
            .join(&relative_target);
        config.generation_journal_path =
            PathBuf::from(relative_target).join("memory/generations.jsonl");

        let error =
            install_plan_and_record(config).expect_err("relative target-root journal fails");

        assert!(error.to_string().contains("must not be inside"));
    }

    #[cfg(unix)]
    #[test]
    fn record_journal_symlink_into_target_root_is_rejected() {
        let root = temp_root("install-plan-symlink-journal");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        fs::create_dir_all(root.join("installed-root/memory"))
            .expect("target memory dir should be created for symlink test");
        std::os::unix::fs::symlink(root.join("installed-root"), root.join("journal-link"))
            .expect("symlink should be created");
        let mut config = config(&root);
        config.record = true;
        config.generation_journal_path = root.join("journal-link/memory/generations.jsonl");

        let error = install_plan_and_record(config).expect_err("symlink journal fails");

        assert!(error.to_string().contains("must not traverse symlinks"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_target_root_is_rejected_for_record() {
        let root = temp_root("install-plan-symlink-target-root");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        fs::create_dir_all(root.join("real-target/memory"))
            .expect("real target dir should be created for symlink test");
        std::os::unix::fs::symlink(root.join("real-target"), root.join("target-link"))
            .expect("target symlink should be created");
        let mut config = config(&root);
        config.record = true;
        config.target_root = root.join("target-link");
        config.generation_journal_path = root.join("real-target/memory/generations.jsonl");

        let error = install_plan_and_record(config).expect_err("symlink target root fails");

        assert!(error.to_string().contains("target root"));
    }
}
