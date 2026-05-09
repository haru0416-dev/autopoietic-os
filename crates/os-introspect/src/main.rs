use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use autopoietic_core::{
    BodyState, GenerationsState, GenomeState, Identity, InstalledPackages, JournalState,
    MemoryState, NixFile, PainPoint, PlatformState, ProjectSummary, SelfState, ShellHistoryState,
    SystemGeneration, SystemdState, UnitState,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(about = "Emit Autopoietic NixOS self-state JSON")]
struct Args {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long)]
    output: Option<PathBuf>,

    #[arg(long = "project-root")]
    project_roots: Vec<PathBuf>,

    #[arg(long, default_value_t = 2000)]
    max_project_files: usize,

    #[arg(long, default_value_t = 0)]
    journal_lines: usize,

    #[arg(long)]
    include_shell_history: bool,

    #[arg(long, default_value_t = 200)]
    shell_history_lines: usize,
}

#[derive(Debug)]
struct CommandOutput {
    available: bool,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn system_time_iso(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn run_command(program: &str, args: &[&str]) -> CommandOutput {
    let output = Command::new(program).args(args).output();
    match output {
        Ok(output) => CommandOutput {
            available: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            error: None,
        },
        Err(error) => CommandOutput {
            available: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(error.to_string()),
        },
    }
}

fn ignored_component(name: &OsStr) -> bool {
    matches!(
        name.to_string_lossy().as_ref(),
        ".git" | ".direnv" | "node_modules" | "target" | "result" | "result-vm"
    )
}

fn discover_nix_files(root: &Path) -> Vec<NixFile> {
    let mut files = Vec::new();
    collect_nix_files(root, root, &mut files);
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

fn collect_nix_files(root: &Path, current: &Path, files: &mut Vec<NixFile>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        if ignored_component(&file_name) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_nix_files(root, &path, files);
        } else if file_type.is_file() && path.extension() == Some(OsStr::new("nix")) {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let modified = metadata.modified().map(system_time_iso).unwrap_or_default();
            files.push(NixFile {
                path: relative.display().to_string(),
                bytes: metadata.len(),
                modified,
            });
        }
    }
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| fs::read_to_string("/proc/sys/kernel/hostname").ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn read_identity(root: &Path) -> Identity {
    let identity_file = Path::new("/etc/autopoietic/identity.json");
    if let Some(identity) = read_json(identity_file) {
        let host = identity
            .get("host")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(hostname);
        let roles = identity
            .get("roles")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        return Identity { host, roles };
    }

    let host_config = root.join("hosts/aion/configuration.nix");
    let mut roles = Vec::new();
    let mut host = hostname();
    if let Ok(text) = fs::read_to_string(host_config) {
        if text.contains("host = \"aion\"") {
            host = "aion".to_owned();
        }
        for role in ["development", "research", "writing", "media"] {
            if text.contains(&format!("\"{role}\"")) {
                roles.push(role.to_owned());
            }
        }
    }
    Identity { host, roles }
}

fn flake_state(root: &Path) -> GenomeState {
    let flake = root.join("flake.nix");
    let lock = root.join("flake.lock");
    let inputs = read_json(&lock)
        .and_then(|lock| lock.get("nodes").and_then(Value::as_object).cloned())
        .map(|nodes| {
            let mut names: Vec<String> = nodes
                .keys()
                .filter(|name| *name != "root")
                .cloned()
                .collect();
            names.sort();
            names
        })
        .unwrap_or_default();

    GenomeState {
        has_flake: flake.exists(),
        has_lock: lock.exists(),
        inputs,
        nix_files: discover_nix_files(root),
    }
}

fn systemd_units() -> SystemdState {
    let output = run_command(
        "systemctl",
        &[
            "list-units",
            "--all",
            "--type=service,timer",
            "--no-legend",
            "--no-pager",
        ],
    );
    if !output.available {
        return SystemdState {
            available: false,
            error: output.error.or(Some(output.stderr)),
            units: Vec::new(),
        };
    }

    let units = output
        .stdout
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().take(4).collect();
            (fields.len() >= 4).then(|| UnitState {
                name: fields[0].to_owned(),
                load: fields[1].to_owned(),
                active: fields[2].to_owned(),
                sub: fields[3].to_owned(),
            })
        })
        .collect();

    SystemdState {
        available: true,
        error: None,
        units,
    }
}

fn installed_packages() -> InstalledPackages {
    let profile = run_command("nix", &["profile", "list", "--json"]);
    if profile.available
        && let Ok(value) = serde_json::from_str::<Value>(&profile.stdout)
        && let Some(elements) = value.get("elements").and_then(Value::as_object)
    {
        let mut packages: Vec<String> = elements.keys().cloned().collect();
        packages.sort();
        return InstalledPackages {
            available: true,
            source: Some("nix profile list --json".to_owned()),
            packages,
        };
    }

    let legacy = run_command("nix-env", &["-q"]);
    if legacy.available {
        let mut packages: Vec<String> = legacy.stdout.lines().map(ToOwned::to_owned).collect();
        packages.sort();
        return InstalledPackages {
            available: true,
            source: Some("nix-env -q".to_owned()),
            packages,
        };
    }

    InstalledPackages {
        available: false,
        source: None,
        packages: Vec::new(),
    }
}

fn generations() -> GenerationsState {
    let mut system_generations = Vec::new();
    let profile_dir = Path::new("/nix/var/nix/profiles");
    if let Ok(entries) = fs::read_dir(profile_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(generation) = name
                .strip_prefix("system-")
                .and_then(|value| value.strip_suffix("-link"))
            {
                system_generations.push(SystemGeneration {
                    generation: generation.to_owned(),
                    path: entry.path().display().to_string(),
                });
            }
        }
    }
    system_generations.sort_by(|left, right| left.generation.cmp(&right.generation));

    let current = Path::new("/run/current-system");
    let current_system = current.exists().then(|| {
        fs::canonicalize(current)
            .unwrap_or_else(|_| current.to_path_buf())
            .display()
            .to_string()
    });

    GenerationsState {
        current_system,
        system_generations,
    }
}

fn journal(limit: usize) -> JournalState {
    if limit == 0 {
        return JournalState {
            included: false,
            error: None,
            entries: Vec::new(),
        };
    }

    let limit_string = limit.to_string();
    let output = run_command(
        "journalctl",
        &["-n", &limit_string, "-o", "short-iso", "--no-pager"],
    );
    if !output.available {
        return JournalState {
            included: false,
            error: output.error.or(Some(output.stderr)),
            entries: Vec::new(),
        };
    }

    JournalState {
        included: true,
        error: None,
        entries: output.stdout.lines().map(ToOwned::to_owned).collect(),
    }
}

fn summarize_projects(paths: &[PathBuf], max_files: usize) -> Vec<ProjectSummary> {
    paths
        .iter()
        .map(|path| summarize_project(path, max_files))
        .collect()
}

fn summarize_project(root: &Path, max_files: usize) -> ProjectSummary {
    if !root.exists() {
        return ProjectSummary {
            path: root.display().to_string(),
            exists: false,
            sampled_files: None,
            top_suffixes: Vec::new(),
        };
    }

    let mut suffixes: BTreeMap<String, usize> = BTreeMap::new();
    let mut sampled_files = 0usize;
    collect_project_suffixes(root, max_files, &mut sampled_files, &mut suffixes);
    let mut top_suffixes: Vec<(String, usize)> = suffixes.into_iter().collect();
    top_suffixes.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    top_suffixes.truncate(12);

    ProjectSummary {
        path: root.display().to_string(),
        exists: true,
        sampled_files: Some(sampled_files),
        top_suffixes,
    }
}

fn collect_project_suffixes(
    current: &Path,
    max_files: usize,
    sampled_files: &mut usize,
    suffixes: &mut BTreeMap<String, usize>,
) {
    if *sampled_files >= max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        if *sampled_files >= max_files {
            break;
        }
        if ignored_component(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_project_suffixes(&path, max_files, sampled_files, suffixes);
        } else if file_type.is_file() {
            *sampled_files += 1;
            let suffix = path
                .extension()
                .and_then(OsStr::to_str)
                .map(|value| format!(".{value}"))
                .unwrap_or_else(|| "<none>".to_owned());
            *suffixes.entry(suffix).or_default() += 1;
        }
    }
}

fn shell_history(include: bool, limit: usize) -> ShellHistoryState {
    if !include {
        return ShellHistoryState {
            included: false,
            reason: Some("disabled".to_owned()),
            entries: Vec::new(),
        };
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        Path::new(&home).join(".zsh_history"),
        Path::new(&home).join(".bash_history"),
    ];
    let mut entries = Vec::new();
    for candidate in candidates {
        if let Ok(text) = fs::read_to_string(candidate) {
            entries.extend(text.lines().rev().take(limit).map(ToOwned::to_owned));
        }
    }
    entries.reverse();
    if entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }

    ShellHistoryState {
        included: true,
        reason: None,
        entries,
    }
}

fn infer_pain_points(history: &ShellHistoryState) -> Vec<PainPoint> {
    let joined = history.entries.join("\n");
    let signals = [
        (
            "repeated manual pdf tooling",
            "create research-papers organ",
            &["pdftotext", "zotero", "ocrmypdf"][..],
        ),
        (
            "repeated Python project setup",
            "create python-dev devShell",
            &["uv ", "pytest", "ruff"][..],
        ),
        (
            "repeated media processing",
            "create media-pipeline organ",
            &["ffmpeg", "imagemagick", "yt-dlp"][..],
        ),
    ];

    signals
        .iter()
        .filter_map(|(signal, candidate, tokens)| {
            let evidence: Vec<String> = tokens
                .iter()
                .filter(|token| joined.contains(**token))
                .map(|token| token.trim().to_owned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            (!evidence.is_empty()).then(|| PainPoint {
                signal: (*signal).to_owned(),
                evidence,
                candidate: (*candidate).to_owned(),
            })
        })
        .collect()
}

fn platform_state() -> PlatformState {
    PlatformState {
        system: std::env::consts::OS.to_owned(),
        release: fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|_| "unknown".to_owned()),
        machine: std::env::consts::ARCH.to_owned(),
    }
}

fn build_state(args: &Args) -> Result<SelfState> {
    let root = fs::canonicalize(&args.root)
        .with_context(|| format!("failed to resolve root {}", args.root.display()))?;
    let project_roots = if args.project_roots.is_empty() {
        vec![root.clone()]
    } else {
        args.project_roots.clone()
    };
    let shell_history = shell_history(args.include_shell_history, args.shell_history_lines);
    let pain_points = infer_pain_points(&shell_history);

    Ok(SelfState {
        schema_version: "0.1.0".to_owned(),
        observed_at: now_iso(),
        identity: read_identity(&root),
        genome: flake_state(&root),
        body: BodyState {
            platform: platform_state(),
            systemd: systemd_units(),
            installed_packages: installed_packages(),
            generations: generations(),
            journal: journal(args.journal_lines),
            projects: summarize_projects(&project_roots, args.max_project_files),
            shell_history,
        },
        memory: MemoryState {
            root: root.join("memory").display().to_string(),
            mutation_log: root.join("memory/mutations.jsonl").display().to_string(),
            effect_log: root.join("memory/effects.jsonl").display().to_string(),
            generation_log: root.join("memory/generations.jsonl").display().to_string(),
        },
        pain_points,
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    let state = build_state(&args)?;
    let rendered =
        serde_json::to_string_pretty(&state).context("failed to render self-state JSON")?;
    if let Some(output) = args.output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory {}", parent.display())
            })?;
        }
        fs::write(&output, format!("{rendered}\n"))
            .with_context(|| format!("failed to write {}", output.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
