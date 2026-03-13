use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use super::model::{ChecklistItem, Project, Task, TaskStatus};

pub struct ThingsDb {
    path: PathBuf,
}

impl ThingsDb {
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let path = PathBuf::from(format!(
            "{}/Library/Group Containers/JLMPQHK86H.com.culturedcode.ThingsMac/ThingsData-04REQ/Things Database.thingsdatabase/main.sqlite",
            home
        ));

        if !path.exists() {
            anyhow::bail!("Things 3 database not found at {:?}", path);
        }

        Ok(Self { path })
    }

    fn conn(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(conn)
    }

    /// Read all tasks from the Inbox (type=0, project=null, area=null, trashed=0, status=0, start=0)
    pub fn inbox_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.uuid, t.title, t.notes, t.status, t.project, t.area, t.start, t.startDate, t.creationDate
             FROM TMTask t
             WHERE t.type = 0
               AND t.project IS NULL
               AND t.area IS NULL
               AND t.trashed = 0
               AND t.status = 0
               AND t.start = 0
             ORDER BY t.\"index\" ASC"
        )?;

        let tasks = self.read_tasks(&conn, &mut stmt)?;
        Ok(tasks)
    }

    /// Read all tasks in the Agents area that are scheduled for today, have an agent tag, and do NOT have agent-done tag
    pub fn agent_today_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.conn()?;

        // Get today's date as integer (YYYYMMDD format used by Things internally)
        // Things stores startDate as days since 2001-01-01 (Core Data reference date)
        // Actually, let's check: Things startDate is an integer in format like 132890
        // It's actually the number of days since the Core Data reference date (2001-01-01)
        let today = chrono::Local::now().date_naive();
        let core_data_epoch = chrono::NaiveDate::from_ymd_opt(2001, 1, 1).unwrap();
        let today_int = (today - core_data_epoch).num_days();

        // Get the Agents area UUID
        let agents_area_uuid: Option<String> = conn
            .query_row(
                "SELECT uuid FROM TMArea WHERE title = 'Agents'",
                [],
                |row| row.get(0),
            )
            .ok();

        let agents_uuid = match agents_area_uuid {
            Some(uuid) => uuid,
            None => return Ok(vec![]),
        };

        // Get all project UUIDs in the Agents area
        let mut proj_stmt = conn.prepare(
            "SELECT uuid FROM TMTask WHERE type = 1 AND area = ?1 AND trashed = 0 AND status = 0"
        )?;
        let _project_uuids: Vec<String> = proj_stmt
            .query_map([&agents_uuid], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Build query for tasks in Agents area or its projects, scheduled for today
        // start=1 means "scheduled", start=2 means "someday"
        // A task is "today" when startDate = today_int OR start=2 with todayIndex set
        let mut all_tasks = Vec::new();

        // Tasks directly in Agents area
        let mut stmt = conn.prepare(
            "SELECT t.uuid, t.title, t.notes, t.status, t.project, t.area, t.start, t.startDate, t.creationDate
             FROM TMTask t
             WHERE t.type = 0
               AND t.trashed = 0
               AND t.status = 0
               AND (t.area = ?1 OR t.project IN (SELECT uuid FROM TMTask WHERE type = 1 AND area = ?1 AND trashed = 0))
               AND t.start = 1
               AND t.startDate = ?2"
        )?;

        let tasks = self.read_tasks_with_params(&conn, &mut stmt, rusqlite::params![&agents_uuid, today_int])?;
        all_tasks.extend(tasks);

        // Also get tasks that appear in Today view (start=2 means "today" in Things)
        let mut stmt2 = conn.prepare(
            "SELECT t.uuid, t.title, t.notes, t.status, t.project, t.area, t.start, t.startDate, t.creationDate
             FROM TMTask t
             WHERE t.type = 0
               AND t.trashed = 0
               AND t.status = 0
               AND (t.area = ?1 OR t.project IN (SELECT uuid FROM TMTask WHERE type = 1 AND area = ?1 AND trashed = 0))
               AND t.start = 2"
        )?;

        let tasks2 = self.read_tasks_with_params(&conn, &mut stmt2, rusqlite::params![&agents_uuid])?;
        all_tasks.extend(tasks2);

        // Filter: must have an agent- tag AND must NOT have agent-done tag
        let filtered: Vec<Task> = all_tasks
            .into_iter()
            .filter(|t| t.is_agent_task() && !t.has_review_tag())
            .collect();

        Ok(filtered)
    }

    pub fn get_projects_in_agents(&self) -> Result<Vec<Project>> {
        let conn = self.conn()?;

        let agents_area_uuid: String = conn.query_row(
            "SELECT uuid FROM TMArea WHERE title = 'Agents'",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT t.uuid, t.title, t.area
             FROM TMTask t
             WHERE t.type = 1
               AND t.area = ?1
               AND t.trashed = 0
               AND t.status = 0
             ORDER BY t.title"
        )?;

        let projects = stmt
            .query_map([&agents_area_uuid], |row| {
                Ok(Project {
                    uuid: row.get(0)?,
                    title: row.get(1)?,
                    area_uuid: row.get(2)?,
                    area_name: Some("Agents".to_string()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(projects)
    }

    pub fn get_checklist_items(&self, task_uuid: &str) -> Result<Vec<ChecklistItem>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT uuid, title, status FROM TMChecklistItem WHERE task = ?1 ORDER BY \"index\" ASC"
        )?;

        let items = stmt
            .query_map([task_uuid], |row| {
                Ok(ChecklistItem {
                    uuid: row.get(0)?,
                    title: row.get(1)?,
                    completed: row.get::<_, i32>(2)? == 3,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    pub fn get_tags_for_task(&self, task_uuid: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT tg.title FROM TMTag tg
             INNER JOIN TMTaskTag tt ON tt.tags = tg.uuid
             WHERE tt.tasks = ?1
             ORDER BY tg.title"
        )?;

        let tags = stmt
            .query_map([task_uuid], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tags)
    }

    fn read_tasks(&self, conn: &Connection, stmt: &mut rusqlite::Statement) -> Result<Vec<Task>> {
        self.read_tasks_with_params(conn, stmt, rusqlite::params![])
    }

    fn read_tasks_with_params(
        &self,
        conn: &Connection,
        stmt: &mut rusqlite::Statement,
        params: impl rusqlite::Params,
    ) -> Result<Vec<Task>> {
        let rows = stmt.query_map(params, |row| {
            let uuid: String = row.get(0)?;
            let title: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
            let notes: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            let status_int: i32 = row.get(3)?;
            let project_uuid: Option<String> = row.get(4)?;
            let area_uuid: Option<String> = row.get(5)?;
            let _start: Option<i32> = row.get(6)?;
            let start_date: Option<i64> = row.get(7)?;
            let creation_date: Option<f64> = row.get(8)?;

            let status = match status_int {
                0 => TaskStatus::Open,
                3 => TaskStatus::Completed,
                2 => TaskStatus::Cancelled,
                _ => TaskStatus::Open,
            };

            Ok(Task {
                uuid,
                title,
                notes,
                tags: vec![], // filled in later
                status,
                project: None,    // resolved later
                project_uuid,
                area: None,       // resolved later
                area_uuid,
                checklist_items: vec![], // filled in later
                start_date,
                creation_date,
            })
        })?;

        let mut tasks: Vec<Task> = rows.filter_map(|r| r.ok()).collect();

        // Enrich with tags and checklist items
        for task in &mut tasks {
            task.tags = self.get_tags_for_task(&task.uuid).unwrap_or_default();
            task.checklist_items = self.get_checklist_items(&task.uuid).unwrap_or_default();

            // Resolve project name
            if let Some(ref proj_uuid) = task.project_uuid {
                if let Ok(name) = conn.query_row(
                    "SELECT title FROM TMTask WHERE uuid = ?1",
                    [proj_uuid],
                    |row| row.get::<_, String>(0),
                ) {
                    task.project = Some(name);
                }
            }

            // Resolve area name
            if let Some(ref area_uuid) = task.area_uuid {
                if let Ok(name) = conn.query_row(
                    "SELECT title FROM TMArea WHERE uuid = ?1",
                    [area_uuid],
                    |row| row.get::<_, String>(0),
                ) {
                    task.area = Some(name);
                }
            }
        }

        Ok(tasks)
    }
}
