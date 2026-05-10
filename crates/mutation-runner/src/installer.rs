use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use autopoietic_core::{
    GenerationRecord, LineageStatus, MutationPromotionRecord, PromotionStatus,
    VerificationCheckStatus,
};
use chrono::Utc;

use crate::verifier::append_jsonl;

#[derive(Debug, Clone)]
pub(crate) struct InstallPlanConfig {
    pub(crate) promotion_journal_path: PathBuf,
    pub(crate) generation_journal_path: PathBuf,
    pub(crate) promotion_id: Option<String>,
    pub(crate) mutation_id: Option<String>,
    pub(crate) target_root: PathBuf,
    pub(crate) parent_generation: String,
    pub(crate) resulting_generation: String,
    pub(crate) record: bool,
}

pub(crate) fn install_plan_and_record(config: InstallPlanConfig) -> Result<GenerationRecord> {
    let promotion = read_selected_promotion(&config)?;
    validate_install_plan_input(&config, &promotion)?;
    let record = generation_record(&config, &promotion);
    if config.record {
        append_jsonl(&config.generation_journal_path, &record)?;
    }
    Ok(record)
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
    if config.parent_generation.trim().is_empty() {
        bail!("install plan requires a non-empty parent generation");
    }
    if config.resulting_generation.trim().is_empty() {
        bail!("install plan requires a non-empty resulting generation");
    }
    Ok(())
}

fn generation_journal_is_inside_target_root(config: &InstallPlanConfig) -> Result<bool> {
    let journal_path = absolute_path(&config.generation_journal_path)
        .context("failed to resolve generation journal path")?;
    Ok(normalized_starts_with(&journal_path, &config.target_root))
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

#[cfg(test)]
mod tests {
    use super::*;
    use autopoietic_core::VerificationCheckResult;
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

    fn config(root: &Path) -> InstallPlanConfig {
        InstallPlanConfig {
            promotion_journal_path: root.join("promotions.jsonl"),
            generation_journal_path: root.join("generations.jsonl"),
            promotion_id: Some("pro-test".to_owned()),
            mutation_id: None,
            target_root: root.join("installed-root"),
            parent_generation: "gen-parent".to_owned(),
            resulting_generation: "gen-child".to_owned(),
            record: false,
        }
    }

    #[test]
    fn promoted_evidence_produces_dry_run_generation_record_without_journal_write() {
        let root = temp_root("install-plan-dry-run");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));

        let record = install_plan_and_record(config(&root)).expect("install plan should be built");

        assert_eq!(record.activation_result, "planned-install");
        assert_eq!(record.lineage_status, LineageStatus::Planned);
        assert_eq!(record.mutation_id, "mut-test");
        assert_eq!(record.parent_generation.as_deref(), Some("gen-parent"));
        assert_eq!(record.generation, "gen-child");
        assert_eq!(record.verification_id.as_deref(), Some("ver-test"));
        assert_eq!(record.promotion_id.as_deref(), Some("pro-test"));
        assert_eq!(record.target_configuration.as_deref(), Some("iso"));
        assert!(!root.join("generations.jsonl").exists());
    }

    #[test]
    fn record_flag_appends_generation_lineage_journal() {
        let root = temp_root("install-plan-record");
        let promotion_journal = root.join("promotions.jsonl");
        write_promotion(&promotion_journal, &promotion(PromotionStatus::Promoted));
        let mut config = config(&root);
        config.record = true;

        let record = install_plan_and_record(config).expect("install plan should be recorded");

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

        let record = install_plan_and_record(config).expect("promotion id disambiguates");

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
}
