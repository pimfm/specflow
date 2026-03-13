use anyhow::{Context, Result};
use std::process::Command;

/// Execute an AppleScript and return stdout
fn run_applescript(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("AppleScript error: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Set the title of a task
pub fn set_title(task_id: &str, title: &str) -> Result<()> {
    let escaped_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set name of t to "{escaped_title}"
end tell"#
    ))?;
    Ok(())
}

/// Set the notes of a task
pub fn set_notes(task_id: &str, notes: &str) -> Result<()> {
    let escaped_notes = notes.replace('\\', "\\\\").replace('"', "\\\"");
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set notes of t to "{escaped_notes}"
end tell"#
    ))?;
    Ok(())
}

/// Set the tag names of a task (comma-separated)
pub fn set_tags(task_id: &str, tags: &[&str]) -> Result<()> {
    let tag_str = tags.join(", ");
    let escaped = tag_str.replace('\\', "\\\\").replace('"', "\\\"");
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set tag names of t to "{escaped}"
end tell"#
    ))?;
    Ok(())
}

/// Move task to a project by project UUID
pub fn set_project(task_id: &str, project_name: &str, area_name: &str) -> Result<()> {
    let escaped_proj = project_name.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_area = area_name.replace('\\', "\\\\").replace('"', "\\\"");
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set targetProject to missing value
  repeat with p in every project
    if name of p is "{escaped_proj}" then
      try
        if name of area of p is "{escaped_area}" then
          set targetProject to p
          exit repeat
        end if
      end try
    end if
  end repeat
  if targetProject is not missing value then
    set project of t to targetProject
  else
    error "Project '{escaped_proj}' not found in area '{escaped_area}'"
  end if
end tell"#
    ))?;
    Ok(())
}

/// Move task directly to the Agents area (no project)
pub fn set_area(task_id: &str, area_name: &str) -> Result<()> {
    let escaped_area = area_name.replace('\\', "\\\\").replace('"', "\\\"");
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set targetArea to missing value
  repeat with a in every area
    if name of a is "{escaped_area}" then
      set targetArea to a
      exit repeat
    end if
  end repeat
  if targetArea is not missing value then
    set area of t to targetArea
  else
    error "Area '{escaped_area}' not found"
  end if
end tell"#
    ))?;
    Ok(())
}

/// Schedule task for today
pub fn schedule_today(task_id: &str) -> Result<()> {
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  schedule t for current date
end tell"#
    ))?;
    Ok(())
}

/// Schedule task for a specific date (YYYY-MM-DD)
pub fn schedule_date(task_id: &str, date: &str) -> Result<()> {
    // Parse date parts
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        anyhow::bail!("Date must be in YYYY-MM-DD format");
    }

    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set targetDate to current date
  set year of targetDate to {year}
  set month of targetDate to {month}
  set day of targetDate to {day}
  set hours of targetDate to 0
  set minutes of targetDate to 0
  set seconds of targetDate to 0
  schedule t for targetDate
end tell"#,
        year = parts[0],
        month = parts[1],
        day = parts[2],
    ))?;
    Ok(())
}

/// Complete a task
pub fn complete_task(task_id: &str) -> Result<()> {
    run_applescript(&format!(
        r#"tell application "Things3"
  set t to to do id "{task_id}"
  set status of t to completed
end tell"#
    ))?;
    Ok(())
}

/// Add checklist items to a task using the things:///update URL scheme
/// This is the only reliable way to add checklist items programmatically
pub fn add_checklist_items(task_id: &str, items: &[String]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    // Use things:///add-json approach via AppleScript
    let json_items: Vec<String> = items
        .iter()
        .map(|item| {
            let escaped = item.replace('\\', "\\\\").replace('"', "\\\"");
            format!("{{\"title\":\"{}\"}}", escaped)
        })
        .collect();

    let json_array = format!("[{}]", json_items.join(","));

    let json_payload = format!(
        r#"[{{"type":"to-do","operation":"update","id":"{}","checklist-items":{}}}]"#,
        task_id, json_array
    );

    let escaped_json = json_payload.replace('\\', "\\\\").replace('"', "\\\"");

    // Use shell script for URL encoding within AppleScript
    run_applescript(&format!(
        r#"set jsonPayload to "{escaped_json}"
set encodedJSON to do shell script "python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1]))' " & quoted form of jsonPayload
open location "things:///json?data=" & encodedJSON"#
    ))?;

    // Give Things a moment to process
    std::thread::sleep(std::time::Duration::from_millis(500));
    Ok(())
}

