use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

use crate::spec::{gherkin, writer};
use crate::sync::engine::SyncEngine;
use crate::things::{applescript, model::Task};

/// Execute the full agent pipeline for a single task, using Gas Town for dispatch.
pub async fn execute_task(task: &Task, sync: &mut SyncEngine) -> Result<ExecutionResult> {
    let task_id = extract_task_id(&task.title)?;
    let branch_name = format!("feature/{}-{}", task_id, slugify(&task.title));

    info!(
        "Starting execution for task #{}: {}",
        task_id, task.title
    );

    // Mark task as running in Things
    applescript::set_tags(&task.uuid, &["agent-running"])?;

    // Step 1: Find the repository — check GT rigs first, then local dirs
    let repo_path = find_repository(task, sync)?;
    info!("Found repository at: {:?}", repo_path);

    // Step 2: Determine the rig name for GT integration
    let rig_name = sync
        .gt
        .find_rig_for_task(&task.title, &task.notes)?
        .unwrap_or_else(|| "specflow".to_string());

    // Step 3: Create a bead in GT for tracking
    let bead_id = sync.create_bead_for_task(
        &task.uuid,
        &task.title,
        &task.notes,
        &rig_name,
        2, // default priority
    )?;
    info!("Created bead {} in rig {}", bead_id, rig_name);

    // Step 4: Create worktree
    let worktree_path = create_worktree(&repo_path, &branch_name)?;
    info!("Created worktree at: {:?}", worktree_path);

    // Step 5: Write specification
    let spec_path = writer::write_spec(&worktree_path, task)?;
    info!("Wrote specification to: {:?}", spec_path);

    // Step 6: Write Gherkin tests
    let feature_path = gherkin::write_gherkin(&worktree_path, task)?;
    info!("Wrote Gherkin tests to: {:?}", feature_path);

    // Step 7: Commit spec and tests
    git_add_and_commit(
        &worktree_path,
        &format!("feat(#{}): add specification and Gherkin tests", task_id),
    )?;

    // Step 8: Push and create PR (GitHub)
    git_push(&worktree_path, &branch_name)?;
    let pr_url = create_github_pr(&worktree_path, &branch_name, task)?;
    info!("Created PR: {}", pr_url);

    // Step 9: Sling the bead for agent processing
    match sync.sling_bead(&bead_id, &rig_name) {
        Ok(msg) => info!("Slung bead {} to {}: {}", bead_id, rig_name, msg),
        Err(e) => info!("Sling skipped (may need manual dispatch): {}", e),
    }

    // Step 10: Update Things with PR link and bead reference
    let updated_notes = format!(
        "PR: {}\nBead: {}\nRig: {}\n\n{}",
        pr_url, bead_id, rig_name, task.notes
    );
    applescript::set_notes(&task.uuid, &updated_notes)?;
    applescript::set_tags(&task.uuid, &["agent-running"])?;

    info!(
        "Task #{} dispatched — bead {} in rig {}",
        task_id, bead_id, rig_name
    );

    Ok(ExecutionResult {
        task_id: task_id.to_string(),
        branch_name,
        pr_url,
        bead_id,
        rig: rig_name,
        worktree_path,
        spec_path,
        feature_path,
    })
}

fn extract_task_id(title: &str) -> Result<String> {
    if title.starts_with('#') {
        if let Some(pos) = title.find(' ') {
            let id = &title[1..pos];
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Ok(id.to_string());
            }
        }
    }
    anyhow::bail!("No task ID found in title: {}", title)
}

fn slugify(title: &str) -> String {
    let title = if title.starts_with('#') {
        if let Some(pos) = title.find(' ') {
            &title[pos + 1..]
        } else {
            title
        }
    } else {
        title
    };

    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Find the repository — check GT rigs first for git_url, then scan local dirs
fn find_repository(task: &Task, sync: &SyncEngine) -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;

    // Check GT rigs for matching names
    if let Ok(rigs) = sync.gt.load_rigs() {
        for (name, rig) in &rigs.rigs {
            let lower_title = task.title.to_lowercase();
            let lower_notes = task.notes.to_lowercase();

            if lower_title.contains(&name.to_lowercase())
                || lower_notes.contains(&name.to_lowercase())
            {
                // Try ~/vibe/<name> first
                let vibe_path = PathBuf::from(format!("{}/vibe/{}", home, name));
                if vibe_path.join(".git").exists() {
                    return Ok(vibe_path);
                }
                // Try extracting repo name from git URL
                if let Some(repo_name) = rig.git_url.split('/').last() {
                    let repo_name = repo_name.trim_end_matches(".git");
                    let alt_path = PathBuf::from(format!("{}/vibe/{}", home, repo_name));
                    if alt_path.join(".git").exists() {
                        return Ok(alt_path);
                    }
                }
            }
        }
    }

    // Fallback: scan local development directories
    let dev_dirs = [
        format!("{}/vibe", home),
        format!("{}/projects", home),
        format!("{}/dev", home),
        format!("{}/code", home),
    ];

    for dir in &dev_dirs {
        let dir_path = Path::new(dir);
        if dir_path.exists() {
            if let Ok(entries) = std::fs::read_dir(dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join(".git").exists() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let lower_title = task.title.to_lowercase();
                            let lower_notes = task.notes.to_lowercase();
                            if lower_title.contains(&name.to_lowercase())
                                || lower_notes.contains(&name.to_lowercase())
                            {
                                return Ok(path);
                            }
                        }
                    }
                }
            }
        }
    }

    // Check for GitHub URLs in notes
    for word in task.notes.split_whitespace() {
        if word.contains("github.com") && word.contains('/') {
            if let Some(repo_name) = word.split('/').last() {
                let repo_name = repo_name.trim_end_matches(".git");
                let candidate = PathBuf::from(format!("{}/vibe/{}", home, repo_name));
                if candidate.join(".git").exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    anyhow::bail!(
        "Could not find repository for task '{}'. Add a repo name or GitHub URL to the task notes.",
        task.title
    )
}

fn create_worktree(repo_path: &Path, branch_name: &str) -> Result<PathBuf> {
    run_git(repo_path, &["fetch", "origin"])?;

    let worktree_dir = repo_path
        .join(".worktrees")
        .join(branch_name.replace('/', "-"));
    std::fs::create_dir_all(worktree_dir.parent().unwrap())?;

    let default_branch = detect_default_branch(repo_path);

    run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            branch_name,
            worktree_dir.to_str().unwrap(),
            &format!("origin/{}", default_branch),
        ],
    )?;

    Ok(worktree_dir)
}

fn detect_default_branch(repo_path: &Path) -> String {
    if run_git(repo_path, &["rev-parse", "--verify", "origin/main"]).is_ok() {
        "main".to_string()
    } else {
        "master".to_string()
    }
}

fn git_add_and_commit(worktree_path: &Path, message: &str) -> Result<()> {
    run_git(worktree_path, &["add", "-A"])?;
    run_git(worktree_path, &["commit", "-m", message])?;
    Ok(())
}

fn git_push(worktree_path: &Path, branch_name: &str) -> Result<()> {
    run_git(worktree_path, &["push", "-u", "origin", branch_name])?;
    Ok(())
}

fn create_github_pr(worktree_path: &Path, branch_name: &str, task: &Task) -> Result<String> {
    let task_id = extract_task_id(&task.title)?;
    let title = format!("#{} {}", task_id, task.title);

    let output = Command::new("gh")
        .args([
            "pr",
            "create",
            "--title",
            &title,
            "--body",
            &task.notes,
            "--head",
            branch_name,
            "--base",
            "main",
        ])
        .current_dir(worktree_path)
        .output()
        .context("Failed to create GitHub PR. Is gh CLI installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create PR: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("http") {
            return Ok(line.trim().to_string());
        }
    }

    Ok(stdout.trim().to_string())
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to run git {:?}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {:?} failed: {}", args, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ExecutionResult {
    pub task_id: String,
    pub branch_name: String,
    pub pr_url: String,
    pub bead_id: String,
    pub rig: String,
    pub worktree_path: PathBuf,
    pub spec_path: PathBuf,
    pub feature_path: PathBuf,
}
