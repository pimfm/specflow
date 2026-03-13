use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::things::model::Task;

/// Generate Gherkin feature files that map to the spec requirements
pub fn write_gherkin(repo_path: &Path, task: &Task) -> Result<PathBuf> {
    let test_dir = repo_path.join("src/test/resources/features");
    fs::create_dir_all(&test_dir)?;

    let task_id = extract_id(&task.title).unwrap_or_else(|| "000".to_string());
    let slug = slugify(&task.title);
    let filename = format!("{}-{}.feature", task_id, slug);
    let feature_path = test_dir.join(&filename);

    let feature_content = generate_feature(task);
    fs::write(&feature_path, &feature_content)?;

    Ok(feature_path)
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

fn generate_feature(task: &Task) -> String {
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

    let mut feature = String::new();

    feature.push_str(&format!("Feature: #{} {}\n", task_id, clean_title));

    if !task.notes.is_empty() {
        // Add description, indented
        for line in task.notes.lines() {
            feature.push_str(&format!("  {}\n", line));
        }
    }
    feature.push('\n');

    if !task.checklist_items.is_empty() {
        for (i, item) in task.checklist_items.iter().enumerate() {
            feature.push_str(&format!(
                "  @REQ-{}-{}\n\
                 Scenario: {}\n\
                 Given the system is in its default state\n\
                 When {}\n\
                 Then the expected outcome is achieved\n\n",
                task_id,
                i + 1,
                item.title,
                item.title.to_lowercase()
            ));
        }
    } else {
        feature.push_str(&format!(
            "  @REQ-{}-1\n\
             Scenario: {}\n\
             Given the system is in its default state\n\
             When {}\n\
             Then the expected outcome is achieved\n",
            task_id,
            clean_title,
            clean_title.to_lowercase()
        ));
    }

    feature
}
