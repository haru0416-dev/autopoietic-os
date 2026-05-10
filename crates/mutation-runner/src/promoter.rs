use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use autopoietic_core::{
    MutationPromotionRecord, MutationProposal, MutationVerificationRecord, PromotionStatus,
    ProposalCheck, VerificationCheckStatus, VerificationStatus,
};
use chrono::Utc;
use uuid::Uuid;

use crate::verifier::{
    PatchInputError, VerifyConfig, append_jsonl, apply_patch_to_worktree, create_worktree,
    evidence_provenance, optional_evidence_bundle_path, proposal_fingerprint, read_proposal,
    read_proposal_patch, root_fingerprint, run_command, validate_proposal, write_json,
};

#[derive(Debug, Clone)]
pub(crate) struct PromoteConfig {
    pub(crate) proposal_path: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) verification_journal_path: PathBuf,
    pub(crate) journal_path: PathBuf,
    pub(crate) work_dir: Option<PathBuf>,
    pub(crate) keep_worktree: bool,
    pub(crate) system: String,
    pub(crate) target_configuration: String,
    pub(crate) vm_checks: Vec<String>,
    pub(crate) parent_genome: String,
    pub(crate) changed_organs: Vec<String>,
    pub(crate) extra_checks: Vec<ProposalCheck>,
    pub(crate) skip_default_checks: bool,
    pub(crate) evidence_bundle_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct PromotionEvidence {
    verification_id: Option<String>,
    proposal_fingerprint: String,
    verified_root_fingerprint: Option<String>,
    promotion_root_fingerprint: String,
}

impl PromotionEvidence {
    fn unavailable(reason: &str) -> Self {
        let unavailable = format!("unavailable:{reason}");
        Self {
            verification_id: None,
            proposal_fingerprint: unavailable.clone(),
            verified_root_fingerprint: None,
            promotion_root_fingerprint: unavailable,
        }
    }
}

pub(crate) fn promote_and_record(config: PromoteConfig) -> Result<MutationPromotionRecord> {
    let evidence_bundle_path = optional_evidence_bundle_path(
        "promotion",
        config.evidence_bundle_path.as_deref(),
        &[&config.journal_path, &config.verification_journal_path],
    );
    let proposal = read_proposal(&config.proposal_path)?;
    let record = promote_proposal(&proposal, &config);
    append_jsonl(&config.journal_path, &record)?;
    if let Some(path) = &evidence_bundle_path {
        let write_result = evidence_provenance(
            "mutation-promotion-record",
            format!("{}#{}", config.journal_path.display(), record.promotion_id),
            "0.1.0",
            &record,
        )
        .and_then(|provenance| write_json(path, &record.to_evidence_bundle(provenance)));
        if let Err(error) = write_result {
            eprintln!("warning: skipped promotion EvidenceBundle output: {error:#}");
        }
    }
    Ok(record)
}

fn promote_proposal(
    proposal: &MutationProposal,
    config: &PromoteConfig,
) -> MutationPromotionRecord {
    let mut checks = Vec::new();

    let verification =
        match read_latest_verification(&config.verification_journal_path, &proposal.mutation_id) {
            Ok(Some(record)) => record,
            Ok(None) => {
                return record(
                    proposal,
                    config,
                    PromotionStatus::Rejected,
                    format!(
                        "no P1 verification evidence found for mutation {}",
                        proposal.mutation_id
                    ),
                    PromotionEvidence::unavailable("no-verification-evidence"),
                    checks,
                );
            }
            Err(error) => {
                return record(
                    proposal,
                    config,
                    PromotionStatus::Error,
                    format!("failed to read P1 verification evidence: {error:#}"),
                    PromotionEvidence::unavailable("verification-journal-error"),
                    checks,
                );
            }
        };

    if verification.status != VerificationStatus::Verified {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            format!(
                "P1 verification {} is not verified",
                verification.verification_id
            ),
            PromotionEvidence {
                verification_id: Some(verification.verification_id.clone()),
                proposal_fingerprint: verification.proposal_fingerprint,
                verified_root_fingerprint: Some(verification.root_fingerprint),
                promotion_root_fingerprint: "unavailable:not-verified".to_owned(),
            },
            checks,
        );
    }

    let verification_id = Some(verification.verification_id.clone());
    let patch = match read_proposal_patch(proposal, &config.proposal_path) {
        Ok(patch) => patch,
        Err(PatchInputError::Rejected(reason)) => {
            return record(
                proposal,
                config,
                PromotionStatus::Rejected,
                reason,
                PromotionEvidence {
                    verification_id,
                    proposal_fingerprint: "unavailable:patch-input-rejected".to_owned(),
                    verified_root_fingerprint: Some(verification.root_fingerprint),
                    promotion_root_fingerprint: "unavailable:patch-input-rejected".to_owned(),
                },
                checks,
            );
        }
        Err(PatchInputError::Error(reason)) => {
            return record(
                proposal,
                config,
                PromotionStatus::Error,
                reason,
                PromotionEvidence {
                    verification_id,
                    proposal_fingerprint: "unavailable:patch-input-error".to_owned(),
                    verified_root_fingerprint: Some(verification.root_fingerprint),
                    promotion_root_fingerprint: "unavailable:patch-input-error".to_owned(),
                },
                checks,
            );
        }
    };
    if let Err(reason) = validate_proposal(proposal, &patch) {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            reason,
            PromotionEvidence {
                verification_id,
                proposal_fingerprint: proposal_fingerprint(proposal, &patch),
                verified_root_fingerprint: Some(verification.root_fingerprint),
                promotion_root_fingerprint: "unavailable:validation-rejected".to_owned(),
            },
            checks,
        );
    }
    let fingerprint = proposal_fingerprint(proposal, &patch);
    if verification.proposal_fingerprint != fingerprint {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            "P1 verification evidence does not match the current proposal fingerprint".to_owned(),
            PromotionEvidence {
                verification_id,
                proposal_fingerprint: fingerprint,
                verified_root_fingerprint: Some(verification.root_fingerprint),
                promotion_root_fingerprint: "unavailable:fingerprint-mismatch".to_owned(),
            },
            checks,
        );
    }
    let verified_root_fingerprint = verification.root_fingerprint;
    let promotion_root_fingerprint =
        match root_fingerprint(&config.root, &config.proposal_path, proposal) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                return record(
                    proposal,
                    config,
                    PromotionStatus::Error,
                    format!("failed to fingerprint promotion root: {error:#}"),
                    PromotionEvidence {
                        verification_id,
                        proposal_fingerprint: fingerprint,
                        verified_root_fingerprint: Some(verified_root_fingerprint),
                        promotion_root_fingerprint: format!("unavailable:{error:#}"),
                    },
                    checks,
                );
            }
        };
    if promotion_root_fingerprint != verified_root_fingerprint {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            "promotion root fingerprint does not match P1 verification root fingerprint".to_owned(),
            PromotionEvidence {
                verification_id,
                proposal_fingerprint: fingerprint,
                verified_root_fingerprint: Some(verified_root_fingerprint),
                promotion_root_fingerprint,
            },
            checks,
        );
    }

    let evidence = PromotionEvidence {
        verification_id,
        proposal_fingerprint: fingerprint,
        verified_root_fingerprint: Some(verified_root_fingerprint),
        promotion_root_fingerprint,
    };

    if let Err(reason) = validate_target_checks(config) {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            reason,
            evidence,
            checks,
        );
    }

    let verify_config = VerifyConfig {
        proposal_path: config.proposal_path.clone(),
        root: config.root.clone(),
        journal_path: config.verification_journal_path.clone(),
        work_dir: config.work_dir.clone(),
        keep_worktree: config.keep_worktree,
        skip_default_checks: true,
        evidence_bundle_path: None,
    };
    let worktree = match create_worktree(&verify_config) {
        Ok(worktree) => worktree,
        Err(error) => {
            return record(
                proposal,
                config,
                PromotionStatus::Error,
                format!("failed to create isolated promotion worktree: {error:#}"),
                evidence,
                checks,
            );
        }
    };

    let apply = apply_patch_to_worktree(&worktree.path, &patch);
    let apply_passed = apply.status == VerificationCheckStatus::Passed;
    checks.push(apply);
    if !apply_passed {
        return record(
            proposal,
            config,
            PromotionStatus::Rejected,
            "patch application failed during promotion replay".to_owned(),
            evidence,
            checks,
        );
    }

    let mut promotion_checks = Vec::new();
    if !config.skip_default_checks {
        let vm_checks = if config.vm_checks.is_empty() {
            vec!["iso-boot-basic".to_owned()]
        } else {
            config.vm_checks.clone()
        };
        for vm_check in vm_checks {
            promotion_checks.push(nix_vm_check(&worktree.path, &config.system, &vm_check));
        }
    }
    promotion_checks.extend(config.extra_checks.iter().cloned());

    for check in &promotion_checks {
        checks.push(run_command(&worktree.path, check));
    }

    if checks
        .iter()
        .all(|check| check.status == VerificationCheckStatus::Passed)
    {
        record(
            proposal,
            config,
            PromotionStatus::Promoted,
            "all promotion checks passed".to_owned(),
            evidence,
            checks,
        )
    } else {
        record(
            proposal,
            config,
            PromotionStatus::Rejected,
            "one or more promotion checks failed".to_owned(),
            evidence,
            checks,
        )
    }
}

fn validate_target_checks(config: &PromoteConfig) -> Result<(), String> {
    let prefix = format!("{}-", config.target_configuration);
    let vm_checks = if config.vm_checks.is_empty() {
        vec!["iso-boot-basic".to_owned()]
    } else {
        config.vm_checks.clone()
    };
    for vm_check in vm_checks {
        if !vm_check.starts_with(&prefix) {
            return Err(format!(
                "VM check '{vm_check}' does not match target_configuration '{}'",
                config.target_configuration
            ));
        }
    }
    Ok(())
}

fn read_latest_verification(
    path: &Path,
    mutation_id: &str,
) -> Result<Option<MutationVerificationRecord>> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read verification journal {}", path.display()))?;
    let mut latest = None;
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: MutationVerificationRecord = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse verification journal {} line {}",
                path.display(),
                index + 1
            )
        })?;
        if record.mutation_id == mutation_id {
            latest = Some(record);
        }
    }
    Ok(latest)
}

fn nix_vm_check(worktree: &Path, system: &str, vm_check: &str) -> ProposalCheck {
    ProposalCheck {
        name: format!("vm-check:{vm_check}"),
        command: "nix".to_owned(),
        args: vec![
            "build".to_owned(),
            "--no-link".to_owned(),
            "--print-out-paths".to_owned(),
            "--no-write-lock-file".to_owned(),
            format!("path:{}#checks.{system}.{vm_check}", worktree.display()),
        ],
    }
}

fn record(
    proposal: &MutationProposal,
    config: &PromoteConfig,
    status: PromotionStatus,
    reason: String,
    evidence: PromotionEvidence,
    checks: Vec<autopoietic_core::VerificationCheckResult>,
) -> MutationPromotionRecord {
    MutationPromotionRecord {
        promotion_id: format!(
            "pro-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            Uuid::new_v4().simple()
        ),
        timestamp: Utc::now().to_rfc3339(),
        mutation_id: proposal.mutation_id.clone(),
        goal: proposal.goal.clone(),
        phase: proposal.phase.clone(),
        status,
        reason,
        verification_id: evidence.verification_id,
        proposal_fingerprint: evidence.proposal_fingerprint,
        verified_root_fingerprint: evidence.verified_root_fingerprint,
        promotion_root_fingerprint: evidence.promotion_root_fingerprint,
        parent_genome: config.parent_genome.clone(),
        target_configuration: config.target_configuration.clone(),
        changed_paths: proposal.changed_paths.clone(),
        changed_organs: config.changed_organs.clone(),
        checks,
        metadata: proposal.metadata.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use autopoietic_core::{
        MutationVerificationRecord, VerificationCheckResult, VerificationCheckStatus,
    };

    fn temp_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("autopoietic-{name}-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root).expect("test temp root should be created");
        root
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("test parent directory should be created");
        }
        fs::write(path, contents).expect("test file should be written");
    }

    fn patch_one_file(path: &str, old: &str, new: &str) -> String {
        format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-{old}\n+{new}\n"
        )
    }

    fn proposal(patch: &str) -> MutationProposal {
        MutationProposal {
            schema_version: "0.1.0".to_owned(),
            mutation_id: "mut-promote".to_owned(),
            goal: "promote proposal".to_owned(),
            phase: "P2-test".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            expected_checks: Vec::new(),
            patch: Some(patch.to_owned()),
            patch_path: None,
            side_effects: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn write_proposal(path: &Path, proposal: &MutationProposal) {
        write_file(
            path,
            &serde_json::to_string(proposal).expect("proposal should serialize"),
        );
    }

    fn verification(
        root: &Path,
        proposal_path: &Path,
        proposal: &MutationProposal,
        status: VerificationStatus,
    ) -> MutationVerificationRecord {
        let patch = proposal
            .patch
            .as_deref()
            .expect("test proposal has inline patch");
        MutationVerificationRecord {
            verification_id: "ver-test".to_owned(),
            timestamp: Utc::now().to_rfc3339(),
            mutation_id: proposal.mutation_id.clone(),
            goal: proposal.goal.clone(),
            phase: "P1-test".to_owned(),
            status,
            reason: "test evidence".to_owned(),
            proposal_fingerprint: proposal_fingerprint(proposal, patch),
            root_fingerprint: root_fingerprint(root, proposal_path, proposal)
                .expect("test root should fingerprint"),
            changed_paths: proposal.changed_paths.clone(),
            checks: vec![VerificationCheckResult {
                name: "test".to_owned(),
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
        write_file(
            path,
            &format!(
                "{}\n",
                serde_json::to_string(record).expect("verification should serialize")
            ),
        );
    }

    fn config(
        root: &Path,
        proposal_path: &Path,
        verification_journal_path: &Path,
    ) -> PromoteConfig {
        PromoteConfig {
            proposal_path: proposal_path.to_path_buf(),
            root: root.to_path_buf(),
            verification_journal_path: verification_journal_path.to_path_buf(),
            journal_path: root.join("promotions.jsonl"),
            work_dir: Some(temp_root("promote-work")),
            keep_worktree: false,
            system: "x86_64-linux".to_owned(),
            target_configuration: "iso".to_owned(),
            vm_checks: Vec::new(),
            parent_genome: "test-parent".to_owned(),
            changed_organs: vec!["docs".to_owned()],
            extra_checks: vec![ProposalCheck {
                name: "promotion-smoke".to_owned(),
                command: "true".to_owned(),
                args: Vec::new(),
            }],
            skip_default_checks: true,
            evidence_bundle_path: None,
        }
    }

    #[test]
    fn verified_mutation_is_promoted_and_journaled() {
        let root = temp_root("promoted-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );

        let record = promote_and_record(config(&root, &proposal_path, &verification_journal_path))
            .expect("promotion should produce a record");

        assert_eq!(record.status, PromotionStatus::Promoted);
        assert_eq!(record.verification_id.as_deref(), Some("ver-test"));
        assert!(record.proposal_fingerprint.starts_with("sha256:"));
        assert!(
            record
                .verified_root_fingerprint
                .as_deref()
                .is_some_and(|value| value.starts_with("sha256:"))
        );
        assert!(record.promotion_root_fingerprint.starts_with("sha256:"));
        assert!(root.join("promotions.jsonl").exists());
        assert_eq!(fs::read_to_string(root.join("README.md")).unwrap(), "old\n");
    }

    #[test]
    fn promotion_can_write_evidence_bundle() {
        let root = temp_root("promotion-evidence-bundle");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let bundle_path = root.join("evidence/promotion.json");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.evidence_bundle_path = Some(bundle_path.clone());

        let record = promote_and_record(config).expect("promotion should produce a record");

        assert_eq!(record.status, PromotionStatus::Promoted);
        let bundle: autopoietic_core::EvidenceBundle = serde_json::from_slice(
            &fs::read(bundle_path).expect("evidence bundle should be written"),
        )
        .expect("evidence bundle should parse");
        assert_eq!(bundle.subject.mutation_id, "mut-promote");
        assert_eq!(bundle.claims[0].claim, "mutation promoted");
        assert_eq!(
            bundle.comparisons[0].status,
            autopoietic_core::ComparisonStatus::Matched
        );
    }

    #[test]
    fn promotion_skips_evidence_bundle_overwriting_journal_without_changing_gate() {
        let root = temp_root("promotion-evidence-overwrite");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.evidence_bundle_path = Some(config.journal_path.clone());

        let record = promote_and_record(config).expect("unsafe evidence path should not gate P2");

        assert_eq!(record.status, PromotionStatus::Promoted);
        assert!(
            fs::read_to_string(root.join("promotions.jsonl"))
                .expect("primary promotion journal should be written")
                .contains("promoted")
        );
    }

    #[test]
    fn promotion_skips_evidence_bundle_overwriting_verification_journal_without_changing_gate() {
        let root = temp_root("promotion-evidence-overwrite-verification");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let before = fs::read_to_string(&verification_journal_path)
            .expect("verification journal should be readable before promotion");
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.evidence_bundle_path = Some(verification_journal_path.clone());

        let record = promote_and_record(config).expect("unsafe evidence path should not gate P2");

        assert_eq!(record.status, PromotionStatus::Promoted);
        assert_eq!(
            fs::read_to_string(&verification_journal_path)
                .expect("verification journal should remain readable"),
            before
        );
        assert!(root.join("promotions.jsonl").exists());
    }

    #[cfg(unix)]
    #[test]
    fn promotion_skips_symlinked_journal_alias_without_corrupting_jsonl() {
        let root = temp_root("promotion-evidence-symlink-journal-alias");
        fs::create_dir_all(root.join("memory")).expect("memory directory should be created");
        std::os::unix::fs::symlink(root.join("memory"), root.join("link"))
            .expect("journal path symlink should be created");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let bundle_path = root.join("memory/promotions.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.journal_path = root.join("link/promotions.jsonl");
        config.evidence_bundle_path = Some(bundle_path.clone());

        let record = promote_and_record(config).expect("symlinked journal should not gate P2");

        assert_eq!(record.status, PromotionStatus::Promoted);
        let journal = fs::read_to_string(&bundle_path).expect("journal target should be written");
        let parsed: MutationPromotionRecord = serde_json::from_str(journal.trim())
            .expect("aliased target should remain a JSONL promotion record, not a bundle");
        assert_eq!(parsed.status, PromotionStatus::Promoted);
    }

    #[test]
    fn promotion_does_not_copy_memory_evidence_into_worktree() {
        let root = temp_root("promotion-evidence-not-copied");
        write_file(&root.join("README.md"), "old\n");
        write_file(
            &root.join("memory/evidence/p1.json"),
            "{\"volatile\":true}\n",
        );
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.extra_checks = vec![ProposalCheck {
            name: "evidence-sidecar-absent".to_owned(),
            command: "test".to_owned(),
            args: vec![
                "!".to_owned(),
                "-e".to_owned(),
                "memory/evidence/p1.json".to_owned(),
            ],
        }];

        let record = promote_and_record(config).expect("evidence sidecar should not affect P2");

        assert_eq!(record.status, PromotionStatus::Promoted);
        assert_eq!(record.checks[1].name, "evidence-sidecar-absent");
        assert_eq!(record.checks[1].status, VerificationCheckStatus::Passed);
    }

    #[test]
    fn missing_verified_evidence_is_rejected() {
        let root = temp_root("no-evidence-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        write_proposal(
            &proposal_path,
            &proposal(&patch_one_file("README.md", "old", "new")),
        );
        write_file(&verification_journal_path, "");

        let record = promote_and_record(config(&root, &proposal_path, &verification_journal_path))
            .expect("rejection should still be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert!(record.reason.contains("no P1 verification evidence"));
    }

    #[test]
    fn rejected_p1_evidence_is_not_promoted() {
        let root = temp_root("rejected-evidence-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Rejected,
            ),
        );

        let record = promote_and_record(config(&root, &proposal_path, &verification_journal_path))
            .expect("rejection should still be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert!(record.reason.contains("is not verified"));
    }

    #[test]
    fn failed_promotion_check_is_rejected_and_journaled() {
        let root = temp_root("failed-promotion-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.extra_checks = vec![ProposalCheck {
            name: "promotion-smoke".to_owned(),
            command: "false".to_owned(),
            args: Vec::new(),
        }];

        let record = promote_and_record(config).expect("failed promotion should be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert_eq!(record.reason, "one or more promotion checks failed");
        assert!(root.join("promotions.jsonl").exists());
    }

    #[test]
    fn changed_proposal_after_verification_is_rejected() {
        let root = temp_root("changed-proposal-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let verified_proposal = proposal(&patch_one_file("README.md", "old", "new"));
        let changed_proposal = proposal(&patch_one_file("README.md", "old", "other"));
        write_proposal(&proposal_path, &changed_proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &verified_proposal,
                VerificationStatus::Verified,
            ),
        );

        let record = promote_and_record(config(&root, &proposal_path, &verification_journal_path))
            .expect("fingerprint mismatch should be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert!(record.reason.contains("does not match"));
    }

    #[test]
    fn changed_root_after_verification_is_rejected() {
        let root = temp_root("changed-root-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        write_file(
            &root.join("new-base-file.txt"),
            "changed after verification\n",
        );

        let record = promote_and_record(config(&root, &proposal_path, &verification_journal_path))
            .expect("root fingerprint mismatch should be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert!(record.reason.contains("root fingerprint does not match"));
    }

    #[test]
    fn mismatched_target_configuration_is_rejected() {
        let root = temp_root("target-mismatch-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let verification_journal_path = root.join("mutation-results.jsonl");
        let proposal = proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        write_verification(
            &verification_journal_path,
            &verification(
                &root,
                &proposal_path,
                &proposal,
                VerificationStatus::Verified,
            ),
        );
        let mut config = config(&root, &proposal_path, &verification_journal_path);
        config.target_configuration = "aion".to_owned();
        config.vm_checks = vec!["iso-boot-basic".to_owned()];

        let record = promote_and_record(config).expect("target mismatch should be journaled");

        assert_eq!(record.status, PromotionStatus::Rejected);
        assert!(
            record
                .reason
                .contains("does not match target_configuration")
        );
    }
}
