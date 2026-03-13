use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::things::model::Task;

/// Write a specification file for a task into the project's spec/ directory
pub fn write_spec(repo_path: &Path, task: &Task) -> Result<PathBuf> {
    let spec_dir = repo_path.join("spec");
    fs::create_dir_all(&spec_dir)?;

    // Extract the task ID from title (e.g., "#001 Some title" -> "001")
    let task_id = extract_id(&task.title).unwrap_or_else(|| "000".to_string());
    let slug = slugify(&task.title);
    let filename = format!("{}-{}.md", task_id, slug);
    let spec_path = spec_dir.join(&filename);

    let spec_content = generate_spec(task);
    fs::write(&spec_path, &spec_content)?;

    Ok(spec_path)
}

fn extract_id(title: &str) -> Option<String> {
    if title.starts_with('#') {
        if let Some(pos) = title.find(' ') {
            let id = &title[1..pos];
            if id.chars().all(|c| c.is_ascii_digit()) {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn slugify(title: &str) -> String {
    // Remove the ID prefix first
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

fn generate_spec(task: &Task) -> String {
    let task_id = extract_id(&task.title).unwrap_or_else(|| "000".to_string());
    let clean_title = if task.title.starts_with('#') {
        if let Some(pos) = task.title.find(' ') {
            task.title[pos + 1..].trim().to_string()
        } else {
            task.title.clone()
        }
    } else {
        task.title.clone()
    };

    let mut spec = String::new();

    // Header
    spec.push_str(&format!("# #{} {}\n\n", task_id, clean_title));

    // Purpose
    spec.push_str("## Purpose\n\n");
    if !task.notes.is_empty() {
        spec.push_str(&task.notes);
    } else {
        spec.push_str(&format!("Implement: {}", clean_title));
    }
    spec.push_str("\n\n");

    // Requirements as GIVEN/WHEN/THEN
    spec.push_str("## Requirements\n\n");

    if !task.checklist_items.is_empty() {
        for (i, item) in task.checklist_items.iter().enumerate() {
            spec.push_str(&format!("### REQ-{}-{}: {}\n\n", task_id, i + 1, item.title));
            spec.push_str(&format!(
                "**GIVEN** the system is in its default state\n\
                 **WHEN** {}\n\
                 **THEN** the expected outcome is achieved\n\n",
                item.title.to_lowercase()
            ));
        }
    } else {
        spec.push_str(&format!("### REQ-{}-1: {}\n\n", task_id, clean_title));
        spec.push_str(&format!(
            "**GIVEN** the system is in its default state\n\
             **WHEN** {}\n\
             **THEN** the expected outcome is achieved\n\n",
            clean_title.to_lowercase()
        ));
    }

    // Acceptance Criteria
    spec.push_str("## Acceptance Criteria\n\n");
    spec.push_str("- [ ] All GIVEN/WHEN/THEN requirements pass as Gherkin tests\n");
    spec.push_str("- [ ] No existing tests are broken\n");
    spec.push_str("- [ ] Code follows hexagonal architecture (ports and adapters)\n");
    spec.push_str("- [ ] All new interfaces are properly defined\n");
    spec.push_str("- [ ] Merge request created and linked\n");

    spec
}
