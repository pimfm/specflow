use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::things::model::Task;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnhancement {
    pub title: String,
    pub description: String,
    pub project: Option<String>,
    pub steps: Vec<String>,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: &'static str,
    max_tokens: u32,
    messages: Vec<Message>,
    system: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

fn api_key() -> Result<String> {
    std::env::var("ANTHROPIC_API_KEY").context(
        "ANTHROPIC_API_KEY not set. Export it to enable Claude-powered triage enhancement.",
    )
}

/// Call the Claude API to enhance a task's title, description, project classification, and steps.
pub async fn enhance_task(task: &Task) -> Result<TaskEnhancement> {
    let api_key = api_key()?;
    let client = reqwest::Client::new();

    let system_prompt = r#"You are a task triage assistant for a software development workflow. You receive raw task titles and notes from a personal inbox and must produce a structured enhancement.

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

    let user_content = format!(
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

    let request = ApiRequest {
        model: MODEL,
        max_tokens: 1024,
        messages: vec![Message {
            role: "user",
            content: user_content,
        }],
        system: system_prompt.to_string(),
    };

    let response = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .context("Failed to call Claude API")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Claude API error ({}): {}", status, body);
    }

    let api_response: ApiResponse = response
        .json()
        .await
        .context("Failed to parse Claude API response")?;

    let text = api_response
        .content
        .first()
        .and_then(|b| b.text.as_ref())
        .context("Empty response from Claude API")?;

    // Parse the JSON response, stripping any markdown fences if present
    let json_str = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let enhancement: TaskEnhancement =
        serde_json::from_str(json_str).context("Failed to parse Claude's JSON response")?;

    Ok(enhancement)
}
