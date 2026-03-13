use anyhow::Result;
use tracing::{info, warn};

use super::claude;
use super::id_tracker::IdTracker;
use crate::things::{applescript, db::ThingsDb, model::Task};

pub struct TriageProcessor {
    db: ThingsDb,
    id_tracker: IdTracker,
}

impl TriageProcessor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            db: ThingsDb::new()?,
            id_tracker: IdTracker::new()?,
        })
    }

    /// Process all tasks in the inbox
    pub async fn process_inbox(&self) -> Result<Vec<TriagedTask>> {
        let inbox_tasks = self.db.inbox_tasks()?;
        info!("Found {} tasks in inbox to triage", inbox_tasks.len());

        let mut results = Vec::new();

        for task in &inbox_tasks {
            match self.triage_task(task).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Failed to triage task '{}': {}", task.title, e);
                }
            }
        }

        Ok(results)
    }

    async fn triage_task(&self, task: &Task) -> Result<TriagedTask> {
        let task_id = self.id_tracker.next_id()?;
        let id_prefix = format!("#{:03}", task_id);

        info!("Triaging task: {} -> {}", task.title, id_prefix);

        let analysis = self.analyze_task(task).await?;

        // Update the task in Things 3
        let new_title = format!("{} {}", id_prefix, analysis.title);
        applescript::set_title(&task.uuid, &new_title)?;
        applescript::set_notes(&task.uuid, &analysis.description)?;

        if !analysis.steps.is_empty() {
            applescript::add_checklist_items(&task.uuid, &analysis.steps)?;
        }

        if let Some(ref project_name) = analysis.project {
            applescript::set_project(&task.uuid, project_name, "Agents")?;
        } else {
            applescript::set_area(&task.uuid, "Agents")?;
        }

        match &analysis.schedule {
            Schedule::Today => applescript::schedule_today(&task.uuid)?,
            Schedule::Date(date) => applescript::schedule_date(&task.uuid, date)?,
        }

        applescript::set_tags(&task.uuid, &["agent-running"])?;

        Ok(TriagedTask {
            uuid: task.uuid.clone(),
            id: task_id,
            original_title: task.title.clone(),
            new_title,
            project: analysis.project,
            schedule: analysis.schedule,
        })
    }

    async fn analyze_task(&self, task: &Task) -> Result<TaskAnalysis> {
        // Use Claude to enhance the task
        match claude::enhance_task(task).await {
            Ok(enhancement) => {
                info!("Claude enhanced: '{}' -> '{}'", task.title, enhancement.title);

                // Validate project classification against known projects
                let project = enhancement.project.and_then(|p| {
                    match p.as_str() {
                        "Development" | "Business" | "Marketing" | "Customer Service" => Some(p),
                        _ => {
                            warn!("Claude returned unknown project '{}', falling back to keyword classification", p);
                            classify_project(&task.title, &task.notes)
                        }
                    }
                });

                Ok(TaskAnalysis {
                    title: enhancement.title,
                    description: enhancement.description,
                    steps: enhancement.steps,
                    project,
                    schedule: Schedule::Today,
                })
            }
            Err(e) => {
                warn!(
                    "Claude enhancement failed for '{}': {}. Using fallback.",
                    task.title, e
                );
                self.analyze_task_fallback(task)
            }
        }
    }

    /// Fallback analysis when Claude API is unavailable
    fn analyze_task_fallback(&self, task: &Task) -> Result<TaskAnalysis> {
        let project = classify_project(&task.title, &task.notes);
        let clean_title = clean_title(&task.title);

        let mut description = task.notes.clone();
        if description.is_empty() {
            description = format!("Task: {}", clean_title);
        }

        let steps = generate_steps(&clean_title, &task.notes, &task.checklist_items);

        Ok(TaskAnalysis {
            title: clean_title,
            description,
            steps,
            project,
            schedule: Schedule::Today,
        })
    }
}

fn classify_project(title: &str, notes: &str) -> Option<String> {
    let text = format!("{} {}", title, notes).to_lowercase();

    let dev_keywords = [
        "code",
        "implement",
        "build",
        "api",
        "deploy",
        "bug",
        "fix",
        "feature",
        "endpoint",
        "database",
        "migration",
        "refactor",
        "test",
        "ci",
        "pipeline",
        "repo",
        "git",
        "server",
        "frontend",
        "backend",
        "infrastructure",
    ];
    let business_keywords = [
        "invoice",
        "contract",
        "meeting",
        "proposal",
        "budget",
        "finance",
        "accounting",
        "legal",
        "compliance",
        "strategy",
        "plan",
        "review",
        "quarterly",
    ];
    let marketing_keywords = [
        "campaign",
        "social",
        "content",
        "blog",
        "newsletter",
        "seo",
        "analytics",
        "brand",
        "launch",
        "promote",
        "audience",
        "growth",
    ];
    let cs_keywords = [
        "customer", "support", "ticket", "complaint", "feedback", "onboard", "user", "help",
        "issue", "request",
    ];

    let dev_score: usize = dev_keywords.iter().filter(|k| text.contains(*k)).count();
    let biz_score: usize = business_keywords
        .iter()
        .filter(|k| text.contains(*k))
        .count();
    let mkt_score: usize = marketing_keywords
        .iter()
        .filter(|k| text.contains(*k))
        .count();
    let cs_score: usize = cs_keywords.iter().filter(|k| text.contains(*k)).count();

    let max_score = dev_score.max(biz_score).max(mkt_score).max(cs_score);

    if max_score == 0 {
        return None;
    }

    if dev_score == max_score {
        Some("Development".to_string())
    } else if biz_score == max_score {
        Some("Business".to_string())
    } else if mkt_score == max_score {
        Some("Marketing".to_string())
    } else {
        Some("Customer Service".to_string())
    }
}

fn clean_title(title: &str) -> String {
    let title = if title.starts_with('#') {
        if let Some(pos) = title.find(' ') {
            let prefix = &title[1..pos];
            if prefix.chars().all(|c| c.is_ascii_digit()) {
                title[pos..].trim().to_string()
            } else {
                title.to_string()
            }
        } else {
            title.to_string()
        }
    } else {
        title.to_string()
    };

    let mut chars = title.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn generate_steps(
    title: &str,
    notes: &str,
    existing_checklist: &[crate::things::model::ChecklistItem],
) -> Vec<String> {
    if !existing_checklist.is_empty() {
        return existing_checklist
            .iter()
            .map(|c| c.title.clone())
            .collect();
    }

    let text = format!("{} {}", title, notes).to_lowercase();
    let mut steps = Vec::new();

    if text.contains("implement")
        || text.contains("build")
        || text.contains("feature")
        || text.contains("code")
    {
        steps.push("Review requirements and acceptance criteria".to_string());
        steps.push("Identify affected codebase and files".to_string());
        steps.push("Write specification with GIVEN/WHEN/THEN scenarios".to_string());
        steps.push("Implement the changes".to_string());
        steps.push("Write and run tests".to_string());
        steps.push("Create merge request".to_string());
    } else if text.contains("fix") || text.contains("bug") {
        steps.push("Reproduce the issue".to_string());
        steps.push("Identify root cause".to_string());
        steps.push("Implement the fix".to_string());
        steps.push("Verify fix resolves the issue".to_string());
        steps.push("Create merge request".to_string());
    } else {
        steps.push("Review the task requirements".to_string());
        steps.push("Plan the approach".to_string());
        steps.push("Execute the task".to_string());
        steps.push("Verify completion".to_string());
    }

    steps
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TriagedTask {
    pub uuid: String,
    pub id: u32,
    pub original_title: String,
    pub new_title: String,
    pub project: Option<String>,
    pub schedule: Schedule,
}

#[derive(Debug, Clone)]
pub enum Schedule {
    Today,
    #[allow(dead_code)]
    Date(String),
}

struct TaskAnalysis {
    title: String,
    description: String,
    steps: Vec<String>,
    project: Option<String>,
    schedule: Schedule,
}
