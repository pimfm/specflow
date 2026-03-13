use anyhow::{Context, Result};
use serde::Deserialize;
use std::process::Command;

use crate::things::model::Task;

#[derive(Debug, Clone, Deserialize)]
pub struct TaskEnhancement {
    pub title: String,
    pub description: String,
    pub project: Option<String>,
    pub steps: Vec<String>,
}

const SYSTEM_PROMPT: &str = r#"You are a task triage assistant for a software development workflow. You receive raw task titles and notes from a personal inbox and must produce a structured enhancement.

Your output MUST be valid JSON with exactly these fields:
{
  "title": "...",
  "description": "...",
  "project": "...",
  "steps": ["..."]
}

Rules for each field:

**title**: A concise, actionable title (max 80 chars). Start with a verb. Remove filler words. Make it specific enough that someone reading a task list knows exactly what to do. Examples:
- "maybe look into auth stuff" → "Implement JWT authentication for API endpoints"
- "tests broken" → "Fix failing integration tests in payment module"
- "k8s" → "Set up Kubernetes deployment manifests"

**description**: A clear, agent-ready description (2-5 sentences). Include:
- What needs to be done and why
- Key constraints or requirements if apparent from the input
- Expected outcome
Write in imperative mood. No fluff.

**project**: Classify into exactly one of these categories, or null if unclear:
- "Development" — code, features, bugs, infrastructure, CI/CD, APIs, databases, testing
- "Business" — finance, legal, strategy, meetings, contracts, compliance
- "Marketing" — campaigns, content, social media, SEO, branding, analytics
- "Customer Service" — support tickets, user feedback, onboarding, help docs

**steps**: An ordered checklist of 4-8 concrete implementation steps. Each step should be:
- Actionable (starts with a verb)
- Specific to THIS task (not generic)
- Small enough to complete in one sitting
- Written so an agent or developer can execute without ambiguity

For development tasks, always include steps for: understanding the codebase, implementation, testing, and creating a merge request.
For bugs, include: reproduction, root cause analysis, fix, verification.

Respond with ONLY the JSON object, no markdown fences, no extra text."#;

/// Call the Claude CLI to enhance a task's title, description, project classification, and steps.
/// Uses the `claude` CLI with print mode, which inherits the user's console login authentication.
pub async fn enhance_task(task: &Task) -> Result<TaskEnhancement> {
    let user_prompt = format!(
        "Task title: {}\nTask notes: {}\nExisting checklist items: {}",
        task.title,
        if task.notes.is_empty() {
            "(none)"
        } else {
            &task.notes
        },
        if task.checklist_items.is_empty() {
            "(none)".to_string()
        } else {
            task.checklist_items
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    // Run claude CLI in print mode (non-interactive, single prompt/response)
    // Unset CLAUDECODE env var to allow nested invocation
    let output = tokio::task::spawn_blocking(move || {
        Command::new("claude")
            .arg("-p")
            .arg(&user_prompt)
            .arg("--system-prompt")
            .arg(SYSTEM_PROMPT)
            .arg("--model")
            .arg("sonnet")
            .env_remove("CLAUDECODE")
            .output()
            .context("Failed to run claude CLI. Is it installed?")
    })
    .await??;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("claude CLI error: {}", stderr.trim());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();

    // Strip markdown fences if present
    let json_str = text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let enhancement: TaskEnhancement =
        serde_json::from_str(json_str).with_context(|| {
            format!(
                "Failed to parse Claude's response as JSON. Raw output:\n{}",
                text
            )
        })?;

    Ok(enhancement)
}
