use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use anyhow::{Context, Result, bail};
use autopoietic_core::{
    DigestRef, MutationProposal, MutationVerificationRecord, ProposalCheck, ProvenanceRef,
    VerificationCheckResult, VerificationCheckStatus, VerificationStatus,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct VerifyConfig {
    pub(crate) proposal_path: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) journal_path: PathBuf,
    pub(crate) work_dir: Option<PathBuf>,
    pub(crate) keep_worktree: bool,
    pub(crate) skip_default_checks: bool,
    pub(crate) evidence_bundle_path: Option<PathBuf>,
}

pub(crate) struct Worktree {
    pub(crate) path: PathBuf,
    keep: bool,
}

impl Drop for Worktree {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn verify_and_record(config: VerifyConfig) -> Result<MutationVerificationRecord> {
    let evidence_bundle_path = optional_evidence_bundle_path(
        "verification",
        config.evidence_bundle_path.as_deref(),
        &[&config.journal_path],
    );
    let proposal = read_proposal(&config.proposal_path)?;
    let record = verify_proposal(&proposal, &config);
    append_jsonl(&config.journal_path, &record)?;
    if let Some(path) = &evidence_bundle_path {
        let write_result = evidence_provenance(
            "mutation-verification-record",
            format!(
                "{}#{}",
                config.journal_path.display(),
                record.verification_id
            ),
            "0.1.0",
            &record,
        )
        .and_then(|provenance| write_json(path, &record.to_evidence_bundle(provenance)));
        if let Err(error) = write_result {
            warn_evidence_bundle_failure("verification", error);
        }
    }
    Ok(record)
}

pub(crate) fn read_proposal(path: &Path) -> Result<MutationProposal> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read proposal {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse proposal {}", path.display()))
}

fn verify_proposal(
    proposal: &MutationProposal,
    config: &VerifyConfig,
) -> MutationVerificationRecord {
    let mut checks = Vec::new();
    let root_fingerprint = root_fingerprint(&config.root, &config.proposal_path, proposal)
        .unwrap_or_else(|error| format!("unavailable:{error:#}"));
    let patch = match read_proposal_patch(proposal, &config.proposal_path) {
        Ok(patch) => patch,
        Err(PatchInputError::Rejected(reason)) => {
            return record(
                proposal,
                VerificationStatus::Rejected,
                reason,
                "unavailable:patch-input-rejected".to_owned(),
                root_fingerprint,
                checks,
            );
        }
        Err(PatchInputError::Error(reason)) => {
            return record(
                proposal,
                VerificationStatus::Error,
                reason,
                "unavailable:patch-input-error".to_owned(),
                root_fingerprint,
                checks,
            );
        }
    };
    if let Err(reason) = validate_proposal(proposal, &patch) {
        return record(
            proposal,
            VerificationStatus::Rejected,
            reason,
            proposal_fingerprint(proposal, &patch),
            root_fingerprint,
            checks,
        );
    }
    let fingerprint = proposal_fingerprint(proposal, &patch);

    let worktree = match create_worktree(config) {
        Ok(worktree) => worktree,
        Err(error) => {
            return record(
                proposal,
                VerificationStatus::Error,
                format!("failed to create isolated worktree: {error:#}"),
                fingerprint,
                root_fingerprint,
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
            VerificationStatus::Rejected,
            "patch application failed".to_owned(),
            fingerprint,
            root_fingerprint,
            checks,
        );
    }

    let mut expected_checks = Vec::new();
    if !config.skip_default_checks {
        expected_checks.push(ProposalCheck {
            name: "nix-flake-check".to_owned(),
            command: "nix".to_owned(),
            args: vec![
                "flake".to_owned(),
                "check".to_owned(),
                "--no-write-lock-file".to_owned(),
                format!("path:{}", worktree.path.display()),
            ],
        });
    }
    expected_checks.extend(proposal.expected_checks.iter().cloned());

    for check in &expected_checks {
        if !is_allowed_check(check, &worktree.path) {
            return record(
                proposal,
                VerificationStatus::Rejected,
                format!(
                    "check command '{}' is not allowed in P1 offline verification",
                    check.command
                ),
                fingerprint,
                root_fingerprint,
                checks,
            );
        }
        checks.push(run_command(&worktree.path, check));
    }

    if checks
        .iter()
        .all(|check| check.status == VerificationCheckStatus::Passed)
    {
        record(
            proposal,
            VerificationStatus::Verified,
            "all verifier checks passed".to_owned(),
            fingerprint,
            root_fingerprint,
            checks,
        )
    } else {
        record(
            proposal,
            VerificationStatus::Rejected,
            "one or more verifier checks failed".to_owned(),
            fingerprint,
            root_fingerprint,
            checks,
        )
    }
}

pub(crate) enum PatchInputError {
    Rejected(String),
    Error(String),
}

pub(crate) fn read_proposal_patch(
    proposal: &MutationProposal,
    proposal_path: &Path,
) -> Result<String, PatchInputError> {
    match (&proposal.patch, &proposal.patch_path) {
        (Some(_), Some(_)) => Err(PatchInputError::Rejected(
            "proposal must set only one of patch or patch_path".to_owned(),
        )),
        (None, None) => Err(PatchInputError::Rejected(
            "proposal must set patch or patch_path".to_owned(),
        )),
        (Some(patch), None) => Ok(patch.clone()),
        (None, Some(patch_path)) => {
            if patch_path.trim().is_empty() {
                return Err(PatchInputError::Rejected(
                    "proposal patch_path is required when patch is absent".to_owned(),
                ));
            }
            validate_relative_path(patch_path).map_err(PatchInputError::Rejected)?;
            let proposal_dir = proposal_directory(proposal_path).map_err(PatchInputError::Error)?;
            let patch_file = proposal_dir.join(patch_path);
            let proposal_dir = resolve_path_for_guard(proposal_dir).map_err(|error| {
                PatchInputError::Error(format!(
                    "failed to resolve proposal directory {}: {error:#}",
                    proposal_dir.display()
                ))
            })?;
            let guarded_patch_file = resolve_path_for_guard(&patch_file).map_err(|error| {
                PatchInputError::Error(format!(
                    "failed to resolve proposal patch {}: {error:#}",
                    patch_file.display()
                ))
            })?;
            if !guarded_patch_file.starts_with(&proposal_dir) {
                return Err(PatchInputError::Rejected(format!(
                    "proposal patch_path '{}' must stay inside the proposal directory",
                    patch_path
                )));
            }
            fs::read_to_string(&patch_file).map_err(|error| {
                PatchInputError::Error(format!(
                    "failed to read proposal patch {}: {error}",
                    patch_file.display()
                ))
            })
        }
    }
}

fn proposal_directory(proposal_path: &Path) -> Result<&Path, String> {
    let parent = proposal_path.parent().ok_or_else(|| {
        format!(
            "failed to resolve proposal directory for {}",
            proposal_path.display()
        )
    })?;
    if parent.as_os_str().is_empty() {
        Ok(Path::new("."))
    } else {
        Ok(parent)
    }
}

pub(crate) fn validate_proposal(proposal: &MutationProposal, patch: &str) -> Result<(), String> {
    if proposal.schema_version != "0.1.0" {
        return Err(format!(
            "unsupported proposal schema_version '{}'",
            proposal.schema_version
        ));
    }
    if proposal.goal.trim().is_empty() {
        return Err("proposal goal is required".to_owned());
    }
    if proposal.phase.trim().is_empty() {
        return Err("proposal phase is required".to_owned());
    }
    if patch.trim().is_empty() {
        return Err("proposal patch is required".to_owned());
    }
    for path in &proposal.changed_paths {
        validate_relative_path(path)?;
    }
    let patch_paths = patch_paths(patch)?;
    for path in &patch_paths {
        if validate_relative_path(path).is_err() {
            let declared = proposal
                .side_effects
                .iter()
                .any(|effect| effect.target == *path);
            if !declared {
                return Err(format!(
                    "patch path '{path}' requires an explicit side-effect declaration"
                ));
            }
            return Err(format!(
                "patch path '{path}' is outside the isolated worktree and cannot be applied in P1"
            ));
        }
        if !proposal.changed_paths.iter().any(|changed| changed == path) {
            return Err(format!(
                "patch path '{path}' is not listed in changed_paths"
            ));
        }
    }
    Ok(())
}

pub(crate) fn proposal_fingerprint(proposal: &MutationProposal, patch: &str) -> String {
    fn update(hasher: &mut Sha256, value: &str) {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    update(&mut hasher, "autopoietic-proposal-v1");
    update(&mut hasher, &proposal.schema_version);
    update(&mut hasher, &proposal.mutation_id);
    update(&mut hasher, &proposal.goal);
    update(&mut hasher, &proposal.phase);
    for path in &proposal.changed_paths {
        update(&mut hasher, path);
    }
    for check in &proposal.expected_checks {
        update(&mut hasher, &check.name);
        update(&mut hasher, &check.command);
        for arg in &check.args {
            update(&mut hasher, arg);
        }
    }
    update(&mut hasher, proposal.patch_path.as_deref().unwrap_or(""));
    update(&mut hasher, patch);
    for effect in &proposal.side_effects {
        update(&mut hasher, &effect.effect_type);
        update(&mut hasher, &effect.target);
        update(&mut hasher, &effect.reversible.to_string());
        update(&mut hasher, &effect.compensation);
        update(&mut hasher, &format!("{:?}", effect.risk));
    }
    for (key, value) in &proposal.metadata {
        update(&mut hasher, key);
        update(&mut hasher, value);
    }
    format!("sha256:{}", to_hex(&hasher.finalize()))
}

pub(crate) fn root_fingerprint(
    root: &Path,
    proposal_path: &Path,
    proposal: &MutationProposal,
) -> Result<String> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    let proposal_path = resolve_path_for_guard(proposal_path)?;
    let patch_path = proposal
        .patch_path
        .as_deref()
        .and_then(|path| {
            proposal_directory(proposal_path.as_path())
                .ok()
                .map(|dir| dir.join(path))
        })
        .map(|path| resolve_path_for_guard(&path))
        .transpose()?;
    let mut files = Vec::new();
    collect_fingerprint_files(
        &root,
        &root,
        &proposal_path,
        patch_path.as_deref(),
        &mut files,
    )?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    hasher.update(b"autopoietic-root-v1");
    for (relative, absolute) in files {
        hasher.update((relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        let bytes = fs::read(&absolute).with_context(|| {
            format!(
                "failed to read root fingerprint file {}",
                absolute.display()
            )
        })?;
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format!("sha256:{}", to_hex(&hasher.finalize())))
}

fn collect_fingerprint_files(
    root: &Path,
    current: &Path,
    proposal_path: &Path,
    patch_path: Option<&Path>,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read directory {}", current.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", current.display()))?;
        let path = entry.path();
        let file_name = entry.file_name();
        if should_skip(&file_name) || is_memory_evidence_path(&path) {
            continue;
        }
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to stat {}", path.display()))?;
        if metadata.is_dir() {
            collect_fingerprint_files(root, &path, proposal_path, patch_path, files)?;
        } else if metadata.is_file() {
            let guarded = resolve_path_for_guard(&path)?;
            if guarded == proposal_path || patch_path.is_some_and(|patch| guarded == patch) {
                continue;
            }
            let relative = guarded
                .strip_prefix(root)
                .with_context(|| format!("failed to relativize {}", guarded.display()))?
                .to_string_lossy()
                .replace('\\', "/");
            if is_volatile_evidence(&relative) {
                continue;
            }
            files.push((relative, guarded));
        }
    }
    Ok(())
}

fn is_volatile_evidence(relative: &str) -> bool {
    relative.ends_with(".jsonl") || relative.starts_with("memory/evidence/")
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

fn validate_relative_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(format!("path '{path}' must be relative"));
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!("path '{path}' must stay inside the worktree"));
    }
    Ok(())
}

fn patch_paths(patch: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let lines: Vec<&str> = patch.lines().collect();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].starts_with("--- ") {
            index += 1;
            continue;
        }
        let old_path = patch_file_path(lines[index], "--- ")?;
        index += 1;
        if index >= lines.len() || !lines[index].starts_with("+++ ") {
            return Err("patch file header is missing target path".to_owned());
        }
        let new_path = patch_file_path(lines[index], "+++ ")?;
        let normalized = if new_path == "/dev/null" {
            old_path
        } else {
            new_path
        };
        if !values.contains(&normalized) {
            values.push(normalized);
        }
        index += 1;
    }
    if values.is_empty() {
        return Err("proposal patch does not contain a target path".to_owned());
    }
    Ok(values)
}

pub(crate) fn apply_patch_to_worktree(worktree: &Path, patch: &str) -> VerificationCheckResult {
    match apply_unified_patch(worktree, patch) {
        Ok(()) => VerificationCheckResult {
            name: "apply-patch".to_owned(),
            command: "internal-unified-diff".to_owned(),
            args: Vec::new(),
            status: VerificationCheckStatus::Passed,
            exit_code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        },
        Err(error) => VerificationCheckResult {
            name: "apply-patch".to_owned(),
            command: "internal-unified-diff".to_owned(),
            args: Vec::new(),
            status: VerificationCheckStatus::Failed,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: error,
        },
    }
}

fn apply_unified_patch(worktree: &Path, patch: &str) -> Result<(), String> {
    let lines: Vec<&str> = patch.lines().collect();
    let mut index = 0;
    let mut applied = false;

    while index < lines.len() {
        if !lines[index].starts_with("--- ") {
            index += 1;
            continue;
        }

        let old_path = patch_file_path(lines[index], "--- ")?;
        index += 1;
        if index >= lines.len() || !lines[index].starts_with("+++ ") {
            return Err("patch file header is missing target path".to_owned());
        }
        let new_path = patch_file_path(lines[index], "+++ ")?;
        index += 1;

        let deletes_file = new_path == "/dev/null";
        let target = if deletes_file {
            old_path.clone()
        } else {
            new_path.clone()
        };
        validate_relative_path(&target)?;
        let path = worktree.join(&target);
        let original = fs::read_to_string(&path).unwrap_or_default();
        let original_lines = split_lines(&original);
        let mut output = Vec::new();
        let mut old_index = 0usize;
        let mut file_had_hunk = false;

        while index < lines.len() && lines[index].starts_with("@@ ") {
            file_had_hunk = true;
            let old_start = parse_hunk_old_start(lines[index])?;
            index += 1;
            let copy_until = old_start.saturating_sub(1);
            while old_index < copy_until && old_index < original_lines.len() {
                output.push(original_lines[old_index].clone());
                old_index += 1;
            }

            while index < lines.len()
                && !lines[index].starts_with("@@ ")
                && !lines[index].starts_with("diff --git ")
                && !lines[index].starts_with("--- ")
            {
                let line = lines[index];
                if line == r"\ No newline at end of file" {
                    index += 1;
                    continue;
                }
                let Some((tag, body)) = line.split_at_checked(1) else {
                    return Err("malformed patch hunk line".to_owned());
                };
                match tag {
                    " " => {
                        let expected = with_newline(body);
                        ensure_old_line(&original_lines, old_index, &expected)?;
                        output.push(original_lines[old_index].clone());
                        old_index += 1;
                    }
                    "-" => {
                        let expected = with_newline(body);
                        ensure_old_line(&original_lines, old_index, &expected)?;
                        old_index += 1;
                    }
                    "+" => output.push(with_newline(body)),
                    _ => return Err(format!("unsupported patch hunk line '{line}'")),
                }
                index += 1;
            }
        }

        if !file_had_hunk {
            return Err(format!("patch for '{target}' does not contain a hunk"));
        }

        while old_index < original_lines.len() {
            output.push(original_lines[old_index].clone());
            old_index += 1;
        }

        if deletes_file {
            if !output.is_empty() {
                return Err(format!(
                    "deletion patch for '{target}' leaves undeleted content"
                ));
            }
            fs::remove_file(&path).map_err(|error| error.to_string())?;
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(&path, output.concat()).map_err(|error| error.to_string())?;
        }
        applied = true;
    }

    if applied {
        Ok(())
    } else {
        Err("patch does not contain an applicable file header".to_owned())
    }
}

fn patch_file_path(line: &str, prefix: &str) -> Result<String, String> {
    let path = line
        .strip_prefix(prefix)
        .ok_or_else(|| format!("patch line '{line}' is missing prefix '{prefix}'"))?
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("patch line '{line}' is missing a path"))?;
    Ok(path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_owned())
}

fn parse_hunk_old_start(line: &str) -> Result<usize, String> {
    let old_range = line
        .strip_prefix("@@ -")
        .and_then(|rest| rest.split_once(' '))
        .map(|(range, _)| range)
        .ok_or_else(|| format!("malformed hunk header '{line}'"))?;
    let start = old_range
        .split_once(',')
        .map_or(old_range, |(start, _)| start);
    start
        .parse::<usize>()
        .map_err(|error| format!("malformed hunk start '{start}': {error}"))
}

fn split_lines(contents: &str) -> Vec<String> {
    contents.split_inclusive('\n').map(str::to_owned).collect()
}

fn with_newline(value: &str) -> String {
    let mut line = value.to_owned();
    line.push('\n');
    line
}

fn ensure_old_line(lines: &[String], index: usize, expected: &str) -> Result<(), String> {
    let Some(actual) = lines.get(index) else {
        return Err("patch hunk refers past end of file".to_owned());
    };
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "patch context mismatch: expected {:?}, got {:?}",
            expected, actual
        ))
    }
}

pub(crate) fn create_worktree(config: &VerifyConfig) -> Result<Worktree> {
    let root = config
        .root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", config.root.display()))?;
    let parent = config.work_dir.clone().unwrap_or_else(std::env::temp_dir);
    let parent = if parent.is_absolute() {
        parent
    } else {
        std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(parent)
    };
    let guarded_parent = resolve_path_for_guard(&parent)?;
    if guarded_parent.starts_with(&root) {
        anyhow::bail!(
            "work_dir {} must not be inside root {}",
            guarded_parent.display(),
            root.display()
        );
    }
    let base = parent.join(format!(
        "autopoietic-mutation-{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&base)
        .with_context(|| format!("failed to create worktree root {}", base.display()))?;
    copy_tree(&root, &base)
        .with_context(|| format!("failed to copy {} to {}", root.display(), base.display()))?;
    Ok(Worktree {
        path: base,
        keep: config.keep_worktree,
    })
}

fn resolve_path_for_guard(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()));
    }

    let mut missing = Vec::new();
    let mut current = path;
    while !current.exists() {
        let name = current
            .file_name()
            .with_context(|| format!("failed to resolve parent for {}", path.display()))?;
        missing.push(name.to_owned());
        current = current
            .parent()
            .with_context(|| format!("failed to resolve parent for {}", path.display()))?;
    }

    let mut resolved = current
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", current.display()))?;
    for part in missing.iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", source.display()))?;
        let source_path = entry.path();
        let file_name = entry.file_name();
        if should_skip(&file_name) || is_memory_evidence_path(&source_path) {
            continue;
        }
        let destination_path = destination.join(&file_name);
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to stat {}", source_path.display()))?;
        if metadata.is_dir() {
            fs::create_dir_all(&destination_path)
                .with_context(|| format!("failed to create {}", destination_path.display()))?;
            copy_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn should_skip(file_name: &OsStr) -> bool {
    matches!(file_name.to_str(), Some(".git" | "target" | "result"))
        || file_name
            .to_str()
            .is_some_and(|name| name.starts_with("result-") || name.ends_with(".jsonl"))
}

fn is_memory_evidence_path(path: &Path) -> bool {
    let mut previous_was_memory = false;
    for component in path.components() {
        let name = component.as_os_str();
        if previous_was_memory && name == OsStr::new("evidence") {
            return true;
        }
        previous_was_memory = name == OsStr::new("memory");
    }
    false
}

fn is_allowed_check(check: &ProposalCheck, worktree: &Path) -> bool {
    match check.command.as_str() {
        "true" | "false" => check.args.is_empty(),
        "test" => check.args.iter().all(|arg| is_safe_test_arg(arg)),
        "nix" => {
            check.name == "nix-flake-check"
                && check.args
                    == [
                        "flake".to_owned(),
                        "check".to_owned(),
                        "--no-write-lock-file".to_owned(),
                        format!("path:{}", worktree.display()),
                    ]
        }
        _ => false,
    }
}

fn is_safe_test_arg(arg: &str) -> bool {
    validate_relative_path(arg).is_ok()
}

pub(crate) fn run_command(worktree: &Path, check: &ProposalCheck) -> VerificationCheckResult {
    let output = Command::new(&check.command)
        .args(&check.args)
        .current_dir(worktree)
        .output();
    match output {
        Ok(output) => check_result(check, output),
        Err(error) => VerificationCheckResult {
            name: check.name.clone(),
            command: check.command.clone(),
            args: check.args.clone(),
            status: VerificationCheckStatus::Error,
            exit_code: None,
            stdout: String::new(),
            stderr: error.to_string(),
        },
    }
}

fn check_result(check: &ProposalCheck, output: Output) -> VerificationCheckResult {
    VerificationCheckResult {
        name: check.name.clone(),
        command: check.command.clone(),
        args: check.args.clone(),
        status: if output.status.success() {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn record(
    proposal: &MutationProposal,
    status: VerificationStatus,
    reason: String,
    proposal_fingerprint: String,
    root_fingerprint: String,
    checks: Vec<VerificationCheckResult>,
) -> MutationVerificationRecord {
    MutationVerificationRecord {
        verification_id: format!(
            "ver-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            Uuid::new_v4().simple()
        ),
        timestamp: Utc::now().to_rfc3339(),
        mutation_id: proposal.mutation_id.clone(),
        goal: proposal.goal.clone(),
        phase: proposal.phase.clone(),
        status,
        reason,
        proposal_fingerprint,
        root_fingerprint,
        changed_paths: proposal.changed_paths.clone(),
        checks,
        side_effects: proposal.side_effects.clone(),
        metadata: proposal.metadata.clone(),
    }
}

pub(crate) fn append_jsonl<T: serde::Serialize>(path: &Path, entry: &T) -> Result<()> {
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

pub(crate) fn write_json<T: serde::Serialize>(path: &Path, entry: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create JSON output directory {}",
                parent.display()
            )
        })?;
    }
    let bytes = serde_json::to_vec_pretty(entry).context("failed to serialize JSON output")?;
    fs::write(path, bytes)
        .with_context(|| format!("failed to write JSON output {}", path.display()))
}

pub(crate) fn validate_evidence_bundle_path(
    evidence_bundle_path: Option<&Path>,
    journal_path: &Path,
) -> Result<Option<PathBuf>> {
    let Some(evidence_bundle_path) = evidence_bundle_path else {
        return Ok(None);
    };
    let evidence_bundle_path = normalized_absolute_path(evidence_bundle_path)?;
    let journal_path = normalized_absolute_path(journal_path)?;
    if path_has_symlink_component(&evidence_bundle_path)? {
        bail!("evidence bundle path must not traverse symlinks");
    }
    if path_has_symlink_component(&journal_path)? {
        bail!(
            "primary JSONL journal path must not traverse symlinks when evidence bundle output is requested"
        );
    }
    if evidence_bundle_path == journal_path {
        bail!("evidence bundle path must not replace the primary JSONL journal");
    }
    if existing_paths_refer_to_same_file(&evidence_bundle_path, &journal_path)? {
        bail!("evidence bundle path must not replace the primary JSONL journal");
    }
    Ok(Some(evidence_bundle_path))
}

pub(crate) fn optional_evidence_bundle_path(
    context: &str,
    evidence_bundle_path: Option<&Path>,
    journal_paths: &[&Path],
) -> Option<PathBuf> {
    let mut resolved = evidence_bundle_path.map(Path::to_path_buf);
    for journal_path in journal_paths {
        match validate_evidence_bundle_path(resolved.as_deref(), journal_path) {
            Ok(path) => resolved = path,
            Err(error) => {
                warn_evidence_bundle_failure(context, error);
                return None;
            }
        }
    }
    resolved
}

fn warn_evidence_bundle_failure(context: &str, error: anyhow::Error) {
    eprintln!("warning: skipped {context} EvidenceBundle output: {error:#}");
}

fn path_has_symlink_component(path: &Path) -> Result<bool> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to inspect evidence bundle path {}",
                        current.display()
                    )
                });
            }
        }
    }
    Ok(false)
}

#[cfg(unix)]
fn existing_paths_refer_to_same_file(left: &Path, right: &Path) -> Result<bool> {
    let left_metadata = match fs::metadata(left) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to stat {}", left.display()));
        }
    };
    let right_metadata = match fs::metadata(right) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to stat {}", right.display()));
        }
    };
    Ok(left_metadata.dev() == right_metadata.dev() && left_metadata.ino() == right_metadata.ino())
}

#[cfg(not(unix))]
fn existing_paths_refer_to_same_file(left: &Path, right: &Path) -> Result<bool> {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => Ok(left == right),
        _ => Ok(false),
    }
}

fn normalized_absolute_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)
    };
    Ok(normalize_path_components(&absolute))
}

fn normalize_path_components(path: &Path) -> PathBuf {
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

pub(crate) fn evidence_provenance<T: serde::Serialize>(
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
        metadata: std::collections::BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    use autopoietic_core::{EffectRisk, SideEffectDeclaration, VerificationStatus};

    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

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

    fn base_proposal(patch: &str) -> MutationProposal {
        MutationProposal {
            schema_version: "0.1.0".to_owned(),
            mutation_id: "mut-test".to_owned(),
            goal: "verify proposal".to_owned(),
            phase: "P1-test".to_owned(),
            changed_paths: vec!["README.md".to_owned()],
            expected_checks: vec![ProposalCheck {
                name: "readme-exists".to_owned(),
                command: "test".to_owned(),
                args: vec!["-f".to_owned(), "README.md".to_owned()],
            }],
            patch: Some(patch.to_owned()),
            patch_path: None,
            side_effects: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn config(root: &Path, proposal_path: &Path, journal_path: &Path) -> VerifyConfig {
        VerifyConfig {
            proposal_path: proposal_path.to_path_buf(),
            root: root.to_path_buf(),
            journal_path: journal_path.to_path_buf(),
            work_dir: Some(temp_root("work")),
            keep_worktree: false,
            skip_default_checks: true,
            evidence_bundle_path: None,
        }
    }

    fn write_proposal(path: &Path, proposal: &MutationProposal) {
        let bytes = serde_json::to_vec(proposal).expect("test proposal should serialize");
        write_file(
            path,
            &String::from_utf8(bytes).expect("test proposal should be utf8"),
        );
    }

    fn patch_one_file(path: &str, old: &str, new: &str) -> String {
        format!(
            "diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1 @@\n-{old}\n+{new}\n"
        )
    }

    #[test]
    fn valid_docs_proposal_is_verified_and_journaled() {
        let root = temp_root("valid-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("valid proposal should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert!(record.proposal_fingerprint.starts_with("sha256:"));
        assert!(record.root_fingerprint.starts_with("sha256:"));
        assert!(journal_path.exists());
        assert!(
            fs::read_to_string(journal_path)
                .expect("journal should be readable")
                .contains("verified")
        );
    }

    #[test]
    fn patch_path_proposal_is_verified_and_journaled() {
        let root = temp_root("patch-path-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        write_file(
            &root.join("patch.diff"),
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = Some("patch.diff".to_owned());
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("patch_path proposal should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert!(
            fs::read_to_string(journal_path)
                .expect("journal should be readable")
                .contains("verified")
        );
    }

    #[test]
    fn verification_can_write_evidence_bundle() {
        let root = temp_root("verify-evidence-bundle");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let bundle_path = root.join("evidence/verification.json");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(bundle_path.clone());

        let record = verify_and_record(config).expect("valid proposal should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
        let bundle: autopoietic_core::EvidenceBundle = serde_json::from_slice(
            &fs::read(bundle_path).expect("evidence bundle should be written"),
        )
        .expect("evidence bundle should parse");
        assert_eq!(bundle.subject.mutation_id, "mut-test");
        assert_eq!(bundle.claims[0].claim, "mutation verified");
        assert_eq!(bundle.observations[0].raw_ref.digest.algorithm, "sha256");
    }

    #[test]
    fn verification_skips_evidence_bundle_overwriting_journal_without_changing_gate() {
        let root = temp_root("verify-evidence-overwrite");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(journal_path.clone());

        let record = verify_and_record(config).expect("unsafe evidence path should not gate P1");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert!(
            fs::read_to_string(&journal_path)
                .expect("primary journal should be written")
                .contains("verified")
        );
    }

    #[cfg(unix)]
    #[test]
    fn verification_skips_symlinked_evidence_bundle_path_without_changing_gate() {
        let root = temp_root("verify-evidence-symlink-overwrite");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        write_file(&journal_path, "existing journal\n");
        let bundle_path = root.join("bundle.json");
        std::os::unix::fs::symlink(&journal_path, &bundle_path)
            .expect("bundle path symlink should be created");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(bundle_path);

        let record = verify_and_record(config).expect("symlinked bundle path should not gate P1");

        assert_eq!(record.status, VerificationStatus::Verified);
        let journal = fs::read_to_string(&journal_path).expect("journal should remain readable");
        assert!(journal.starts_with("existing journal\n"));
        assert!(journal.contains("verified"));
    }

    #[cfg(unix)]
    #[test]
    fn verification_skips_symlinked_journal_alias_without_corrupting_jsonl() {
        let root = temp_root("verify-evidence-symlink-journal-alias");
        fs::create_dir_all(root.join("memory")).expect("memory directory should be created");
        std::os::unix::fs::symlink(root.join("memory"), root.join("link"))
            .expect("journal path symlink should be created");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("link/results.jsonl");
        let bundle_path = root.join("memory/results.jsonl");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(bundle_path.clone());

        let record = verify_and_record(config).expect("symlinked journal should not gate P1");

        assert_eq!(record.status, VerificationStatus::Verified);
        let journal = fs::read_to_string(&bundle_path).expect("journal target should be written");
        let parsed: MutationVerificationRecord = serde_json::from_str(journal.trim())
            .expect("aliased target should remain a JSONL verification record, not a bundle");
        assert_eq!(parsed.status, VerificationStatus::Verified);
    }

    #[test]
    fn verification_does_not_copy_memory_evidence_into_worktree() {
        let root = temp_root("verify-evidence-not-copied");
        write_file(&root.join("README.md"), "old\n");
        write_file(
            &root.join("memory/evidence/p1.json"),
            "{\"volatile\":true}\n",
        );
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        proposal.expected_checks = vec![ProposalCheck {
            name: "evidence-sidecar-absent".to_owned(),
            command: "test".to_owned(),
            args: vec![
                "!".to_owned(),
                "-e".to_owned(),
                "memory/evidence/p1.json".to_owned(),
            ],
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("evidence sidecar should not affect P1 checks");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert_eq!(record.checks[1].name, "evidence-sidecar-absent");
        assert_eq!(record.checks[1].status, VerificationCheckStatus::Passed);
    }

    #[cfg(unix)]
    #[test]
    fn verification_skips_normalized_symlinked_evidence_bundle_path_without_changing_gate() {
        let root = temp_root("verify-evidence-normalized-symlink-overwrite");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        write_file(&journal_path, "existing journal\n");
        std::os::unix::fs::symlink(&journal_path, root.join("bundle.json"))
            .expect("bundle path symlink should be created");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(root.join("missing/../bundle.json"));

        let record = verify_and_record(config).expect("normalized symlink should not gate P1");

        assert_eq!(record.status, VerificationStatus::Verified);
        let journal = fs::read_to_string(&journal_path).expect("journal should remain readable");
        assert!(journal.starts_with("existing journal\n"));
        assert!(journal.contains("verified"));
    }

    #[cfg(unix)]
    #[test]
    fn verification_writes_normalized_evidence_bundle_path() {
        let root = temp_root("verify-evidence-normalized-write");
        let outside = temp_root("verify-evidence-normalized-write-outside");
        fs::create_dir_all(outside.join("target")).expect("outside target should be created");
        std::os::unix::fs::symlink(outside.join("target"), root.join("link"))
            .expect("link should be created");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(root.join("link/../safe/bundle.json"));

        let record = verify_and_record(config).expect("normalized bundle path should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert!(root.join("safe/bundle.json").exists());
        assert!(!outside.join("safe/bundle.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn verification_skips_hardlinked_evidence_bundle_path_without_changing_gate() {
        let root = temp_root("verify-evidence-hardlink-overwrite");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        write_file(&journal_path, "existing journal\n");
        let bundle_path = root.join("bundle.json");
        fs::hard_link(&journal_path, &bundle_path).expect("bundle hardlink should be created");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.evidence_bundle_path = Some(bundle_path);

        let record = verify_and_record(config).expect("hardlinked bundle path should not gate P1");

        assert_eq!(record.status, VerificationStatus::Verified);
        let journal = fs::read_to_string(&journal_path).expect("journal should remain readable");
        assert!(journal.starts_with("existing journal\n"));
        assert!(journal.contains("verified"));
    }

    #[test]
    fn root_fingerprint_ignores_memory_evidence_bundles() {
        let root = temp_root("root-fingerprint-evidence-bundle");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let proposal = base_proposal(&patch_one_file("README.md", "old", "new"));
        write_proposal(&proposal_path, &proposal);
        let before = root_fingerprint(&root, &proposal_path, &proposal)
            .expect("root fingerprint should compute before evidence bundle");

        write_file(
            &root.join("memory/evidence/p1.json"),
            "{\"volatile\":true}\n",
        );
        let after = root_fingerprint(&root, &proposal_path, &proposal)
            .expect("root fingerprint should compute after evidence bundle");

        assert_eq!(before, after);
    }

    #[test]
    fn bare_relative_proposal_path_resolves_sibling_patch_path() {
        let _guard = CWD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("cwd lock should not be poisoned");
        let original_cwd = std::env::current_dir().expect("cwd should be readable");
        let root = temp_root("bare-relative-root");
        std::env::set_current_dir(&root).expect("test cwd should change");

        write_file(&root.join("README.md"), "old\n");
        write_file(
            &root.join("patch.diff"),
            &patch_one_file("README.md", "old", "new"),
        );
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = Some("patch.diff".to_owned());
        write_proposal(Path::new("proposal.json"), &proposal);
        let config = VerifyConfig {
            proposal_path: PathBuf::from("proposal.json"),
            root: PathBuf::from("."),
            journal_path: PathBuf::from("results.jsonl"),
            work_dir: Some(temp_root("work")),
            keep_worktree: false,
            skip_default_checks: true,
            evidence_bundle_path: None,
        };

        let result = verify_and_record(config);
        std::env::set_current_dir(original_cwd).expect("cwd should be restored");
        let record = result.expect("relative proposal path should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
    }

    #[test]
    fn multi_file_patch_is_verified() {
        let root = temp_root("multi-file-root");
        write_file(&root.join("README.md"), "old readme\n");
        write_file(&root.join("docs.txt"), "old docs\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(&format!(
            "{}{}",
            patch_one_file("README.md", "old readme", "new readme"),
            patch_one_file("docs.txt", "old docs", "new docs")
        ));
        proposal.changed_paths = vec!["README.md".to_owned(), "docs.txt".to_owned()];
        proposal.expected_checks = vec![
            ProposalCheck {
                name: "readme-exists".to_owned(),
                command: "test".to_owned(),
                args: vec!["-f".to_owned(), "README.md".to_owned()],
            },
            ProposalCheck {
                name: "docs-exists".to_owned(),
                command: "test".to_owned(),
                args: vec!["-f".to_owned(), "docs.txt".to_owned()],
            },
        ];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("multi-file proposal should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
    }

    #[test]
    fn proposal_with_patch_and_patch_path_is_rejected() {
        let root = temp_root("both-patch-root");
        write_file(&root.join("README.md"), "old\n");
        write_file(&root.join("patch.diff"), "unused\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        proposal.patch_path = Some("patch.diff".to_owned());
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("rejected proposal should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("only one of patch or patch_path"));
    }

    #[test]
    fn proposal_without_patch_or_patch_path_is_rejected() {
        let root = temp_root("no-patch-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = None;
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("rejected proposal should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("patch or patch_path"));
    }

    #[test]
    fn absolute_patch_path_is_rejected() {
        let root = temp_root("absolute-patch-path-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = Some("/tmp/patch.diff".to_owned());
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("absolute patch_path rejection should be journaled");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("must be relative"));
    }

    #[test]
    fn parent_directory_patch_path_is_rejected() {
        let root = temp_root("parent-patch-path-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = Some("../patch.diff".to_owned());
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("parent patch_path rejection should be journaled");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("must stay inside"));
    }

    #[test]
    fn missing_patch_path_file_is_journaled_as_error() {
        let root = temp_root("missing-patch-path-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal("");
        proposal.patch = None;
        proposal.patch_path = Some("missing.diff".to_owned());
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("missing patch_path should still produce a record");

        assert_eq!(record.status, VerificationStatus::Error);
        assert!(record.reason.contains("proposal patch"));
    }

    #[test]
    fn malformed_patch_is_rejected_and_journaled() {
        let root = temp_root("malformed-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal("+++ b/README.md\n@@ not-a-valid-hunk\n");
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("rejected proposal should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("patch"));
        assert!(
            fs::read_to_string(journal_path)
                .expect("journal should be readable")
                .contains("rejected")
        );
    }

    #[test]
    fn undeclared_side_effect_command_is_rejected() {
        let root = temp_root("side-effect-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        proposal.expected_checks = vec![ProposalCheck {
            name: "write-outside".to_owned(),
            command: "touch".to_owned(),
            args: vec!["/tmp/autopoietic-side-effect".to_owned()],
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("side-effect rejection should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("not allowed in P1"));
    }

    #[test]
    fn declared_side_effect_command_is_still_rejected_in_p1() {
        let root = temp_root("declared-side-effect-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        proposal.side_effects = vec![SideEffectDeclaration {
            effect_type: "file-write".to_owned(),
            target: "/tmp/autopoietic-side-effect".to_owned(),
            reversible: true,
            compensation: "remove file".to_owned(),
            risk: EffectRisk::Low,
        }];
        proposal.expected_checks = vec![ProposalCheck {
            name: "write-outside".to_owned(),
            command: "touch".to_owned(),
            args: vec!["/tmp/autopoietic-side-effect".to_owned()],
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("side-effect rejection should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("not allowed in P1"));
    }

    #[test]
    fn allowlisted_test_command_rejects_absolute_paths() {
        let root = temp_root("absolute-test-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        proposal.expected_checks = vec![ProposalCheck {
            name: "absolute-host-path".to_owned(),
            command: "test".to_owned(),
            args: vec!["-f".to_owned(), "/tmp/autopoietic-host-path".to_owned()],
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("absolute path rejection should be journaled");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert!(record.reason.contains("not allowed in P1"));
    }

    #[test]
    fn failed_check_is_rejected_and_journaled() {
        let root = temp_root("failed-check-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        proposal.expected_checks = vec![ProposalCheck {
            name: "intent-check".to_owned(),
            command: "false".to_owned(),
            args: Vec::new(),
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("failed check should still produce a record");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert_eq!(record.reason, "one or more verifier checks failed");
        let journal = fs::read_to_string(journal_path).expect("journal should be readable");
        assert!(journal.contains("intent-check"));
        assert!(journal.contains("failed"));
    }

    #[test]
    fn work_dir_inside_root_is_rejected_before_copying() {
        let root = temp_root("workdir-root");
        write_file(&root.join("README.md"), "old\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.work_dir = Some(root.join("work"));

        let record = verify_and_record(config).expect("workdir rejection should be journaled");

        assert_eq!(record.status, VerificationStatus::Error);
        assert!(record.reason.contains("must not be inside root"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_work_dir_inside_root_is_rejected() {
        let root = temp_root("symlink-workdir-root");
        write_file(&root.join("README.md"), "old\n");
        let outside = temp_root("symlink-outside");
        let link = outside.join("link-to-root");
        std::os::unix::fs::symlink(&root, &link).expect("test symlink should be created");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let proposal = base_proposal(
            "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n",
        );
        write_proposal(&proposal_path, &proposal);
        let mut config = config(&root, &proposal_path, &journal_path);
        config.work_dir = Some(link.join("work"));

        let record = verify_and_record(config).expect("symlink workdir rejection should journal");

        assert_eq!(record.status, VerificationStatus::Error);
        assert!(record.reason.contains("must not be inside root"));
    }

    #[test]
    fn deletion_patch_removes_file_in_isolated_worktree() {
        let root = temp_root("delete-root");
        write_file(&root.join("obsolete.txt"), "remove me\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/obsolete.txt b/obsolete.txt\n--- a/obsolete.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-remove me\n",
        );
        proposal.changed_paths = vec!["obsolete.txt".to_owned()];
        proposal.expected_checks = vec![ProposalCheck {
            name: "obsolete-removed".to_owned(),
            command: "test".to_owned(),
            args: vec!["!".to_owned(), "-f".to_owned(), "obsolete.txt".to_owned()],
        }];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("deletion proposal should verify");

        assert_eq!(record.status, VerificationStatus::Verified);
        assert!(root.join("obsolete.txt").exists());
    }

    #[test]
    fn malformed_partial_deletion_patch_is_rejected() {
        let root = temp_root("partial-delete-root");
        write_file(&root.join("obsolete.txt"), "remove me\nkeep me\n");
        let proposal_path = root.join("proposal.json");
        let journal_path = root.join("results.jsonl");
        let mut proposal = base_proposal(
            "diff --git a/obsolete.txt b/obsolete.txt\n--- a/obsolete.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-remove me\n",
        );
        proposal.changed_paths = vec!["obsolete.txt".to_owned()];
        write_proposal(&proposal_path, &proposal);

        let record = verify_and_record(config(&root, &proposal_path, &journal_path))
            .expect("partial deletion rejection should be journaled");

        assert_eq!(record.status, VerificationStatus::Rejected);
        assert_eq!(record.reason, "patch application failed");
        assert!(root.join("obsolete.txt").exists());
    }
}
