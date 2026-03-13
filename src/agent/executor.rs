use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

use crate::things::{applescript, model::Task};
use crate::spec::{writer, gherkin};

/// Execute the full agent pipeline for a single task
pub async fn execute_task(task: &Task) -> Result<ExecutionResult> {
    let task_id = extract_task_id(&task.title)?;
    let branch_name = format!("feature/{}-{}", task_id, slugify(&task.title));

    info!("Starting agent execution for task #{}: {}", task_id, task.title);

    // Mark task as running
    applescript::set_tags(&task.uuid, &["agent-running"])?;

    // Step 1: Find the affected repository
    let repo_path = find_repository(task)?;
    info!("Found repository at: {:?}", repo_path);

    // Step 2: Create worktree
    let worktree_path = create_worktree(&repo_path, &branch_name)?;
    info!("Created worktree at: {:?}", worktree_path);

    // Step 3: Write specification
    let spec_path = writer::write_spec(&worktree_path, task)?;
    info!("Wrote specification to: {:?}", spec_path);

    // Step 4: Write Gherkin tests
    let feature_path = gherkin::write_gherkin(&worktree_path, task)?;
    info!("Wrote Gherkin tests to: {:?}", feature_path);

    // Step 5: Run tests (they should initially fail)
    let initial_test = run_gradle_tests(&worktree_path);
    info!("Initial test run: {:?}", initial_test.is_ok());

    // Step 6: Commit spec and tests
    git_add_and_commit(
        &worktree_path,
        &format!("feat(#{}): add specification and Gherkin tests", task_id),
    )?;

    // Step 7: Implementation would be done by Claude Code agent
    // For now, we create the structure and leave implementation to the agent

    // Step 8: Push and create MR
    git_push(&worktree_path, &branch_name)?;
    let mr_url = create_gitlab_mr(&worktree_path, &branch_name, task)?;
    info!("Created MR: {}", mr_url);

    // Step 9: Add Review tag and MR link to task
    let updated_notes = format!("MR: {}\n\n{}", mr_url, task.notes);
    applescript::set_notes(&task.uuid, &updated_notes)?;
    applescript::set_tags(&task.uuid, &["agent-done"])?;

    // Step 10: Complete the task
    applescript::complete_task(&task.uuid)?;
    info!("Completed task #{}", task_id);

    Ok(ExecutionResult {
        task_id: task_id.to_string(),
        branch_name,
        mr_url,
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

/// Find the repository path based on task context
fn find_repository(task: &Task) -> Result<PathBuf> {
    // Look for repository references in the task notes
    // Common patterns: repo name, URL, path

    // Check for GitLab URLs in notes
    for word in task.notes.split_whitespace() {
        if word.contains("gitlab") && word.contains("/") {
            // Extract repo name from URL and look for it locally
            if let Some(repo_name) = word.split('/').last() {
                let repo_name = repo_name.trim_end_matches(".git");
                let candidate = find_local_repo(repo_name);
                if let Some(path) = candidate {
                    return Ok(path);
                }
            }
        }
    }

    // Try to infer from task title keywords
    let home = std::env::var("HOME").context("HOME not set")?;
    let dev_dirs = [
        format!("{}/projects", home),
        format!("{}/dev", home),
        format!("{}/code", home),
        format!("{}/vibe", home),
        format!("{}/work", home),
    ];

    // Search for a matching repo
    for dir in &dev_dirs {
        let dir_path = Path::new(dir);
        if dir_path.exists() {
            if let Ok(entries) = std::fs::read_dir(dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join(".git").exists() {
                        // Check if repo name matches something in the task
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

    anyhow::bail!(
        "Could not find repository for task '{}'. Add a repo path or GitLab URL to the task notes.",
        task.title
    )
}

fn find_local_repo(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let search_dirs = [
        format!("{}/projects", home),
        format!("{}/dev", home),
        format!("{}/code", home),
        format!("{}/vibe", home),
        format!("{}/work", home),
    ];

    for dir in &search_dirs {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.exists() && candidate.join(".git").exists() {
            return Some(candidate);
        }
    }
    None
}

fn create_worktree(repo_path: &Path, branch_name: &str) -> Result<PathBuf> {
    // First fetch latest
    run_git(repo_path, &["fetch", "origin"])?;

    // Create worktree
    let worktree_dir = repo_path.join(".worktrees").join(branch_name.replace('/', "-"));
    std::fs::create_dir_all(worktree_dir.parent().unwrap())?;

    run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            branch_name,
            worktree_dir.to_str().unwrap(),
            "origin/main",
        ],
    )?;

    Ok(worktree_dir)
}

fn run_gradle_tests(worktree_path: &Path) -> Result<()> {
    let output = Command::new("./gradlew")
        .arg("test")
        .current_dir(worktree_path)
        .output()
        .context("Failed to run gradle tests")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Tests failed: {}", stderr);
    }

    Ok(())
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

fn create_gitlab_mr(worktree_path: &Path, branch_name: &str, task: &Task) -> Result<String> {
    let task_id = extract_task_id(&task.title)?;
    let title = format!("#{} {}", task_id, task.title);

    let output = Command::new("glab")
        .args([
            "mr",
            "create",
            "--title",
            &title,
            "--description",
            &task.notes,
            "--source-branch",
            branch_name,
            "--target-branch",
            "main",
            "--no-editor",
        ])
        .current_dir(worktree_path)
        .output()
        .context("Failed to create GitLab MR. Is glab CLI installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create MR: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Extract MR URL from glab output
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
    pub mr_url: String,
    pub worktree_path: PathBuf,
    pub spec_path: PathBuf,
    pub feature_path: PathBuf,
}
