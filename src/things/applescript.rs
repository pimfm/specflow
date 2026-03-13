use anyhow::{Context, Result};
use serde_json::json;
use std::io::Write;
use std::process::{Command, Stdio};

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

/// Add checklist items to a task using the things:///json URL scheme.
/// Uses serde_json for correct serialization and pipes JSON via stdin
/// to avoid AppleScript/shell string escaping issues.
pub fn add_checklist_items(task_id: &str, items: &[String]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    let checklist_items: Vec<serde_json::Value> = items
        .iter()
        .map(|item| json!({"title": item}))
        .collect();

    let payload = json!([{
        "type": "to-do",
        "operation": "update",
        "id": task_id,
        "checklist-items": checklist_items,
    }]);

    let json_payload = serde_json::to_string(&payload)
        .context("Failed to serialize checklist items to JSON")?;

    // Pipe JSON via stdin to python3 for URL encoding, then open the Things URL.
    // This avoids all AppleScript/shell string escaping issues.
    let mut child = Command::new("python3")
        .arg("-c")
        .arg("import urllib.parse, sys, subprocess; data = sys.stdin.read().strip(); encoded = urllib.parse.quote(data); subprocess.run(['open', 'things:///json?data=' + encoded])")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn python3 for URL encoding")?;

    child
        .stdin
        .take()
        .context("Failed to open stdin")?
        .write_all(json_payload.as_bytes())
        .context("Failed to write JSON to python3 stdin")?;

    let output = child.wait_with_output().context("Failed to wait for python3")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to add checklist items: {}", stderr.trim());
    }

    // Give Things a moment to process
    std::thread::sleep(std::time::Duration::from_millis(500));
    Ok(())
}

