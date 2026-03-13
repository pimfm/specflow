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

    /// Read all actionable agent tasks by scanning:
    /// 1. The Today view — any task scheduled for today, regardless of area
    /// 2. The Agents area — any task in the Agents area or its projects, regardless of schedule
    /// Only returns tasks that have an agent- tag and do NOT have the agent-done tag.
    pub fn agent_today_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.conn()?;

        // Things stores startDate as days since 2001-01-01 (Core Data reference date)
        let today = chrono::Local::now().date_naive();
        let core_data_epoch = chrono::NaiveDate::from_ymd_opt(2001, 1, 1).unwrap();
        let today_int = (today - core_data_epoch).num_days();

        let mut all_tasks = Vec::new();

        // 1. Today view: tasks scheduled for today (start=1, startDate=today), any area
        let mut today_stmt = conn.prepare(
            "SELECT t.uuid, t.title, t.notes, t.status, t.project, t.area, t.start, t.startDate, t.creationDate
             FROM TMTask t
             WHERE t.type = 0
               AND t.trashed = 0
               AND t.status = 0
               AND t.start = 1
               AND t.startDate = ?1"
        )?;
        let today_tasks = self.read_tasks_with_params(&conn, &mut today_stmt, rusqlite::params![today_int])?;
        all_tasks.extend(today_tasks);

        // 2. Agents area: all open tasks in Agents area or its projects, regardless of schedule
        let agents_area_uuid: Option<String> = conn
            .query_row(
                "SELECT uuid FROM TMArea WHERE title = 'Agents'",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(ref agents_uuid) = agents_area_uuid {
            let mut agents_stmt = conn.prepare(
                "SELECT t.uuid, t.title, t.notes, t.status, t.project, t.area, t.start, t.startDate, t.creationDate
                 FROM TMTask t
                 WHERE t.type = 0
                   AND t.trashed = 0
                   AND t.status = 0
                   AND (t.area = ?1 OR t.project IN (SELECT uuid FROM TMTask WHERE type = 1 AND area = ?1 AND trashed = 0))"
            )?;
            let agents_tasks = self.read_tasks_with_params(&conn, &mut agents_stmt, rusqlite::params![agents_uuid])?;
            all_tasks.extend(agents_tasks);
        }

        // Deduplicate by UUID (a task may appear in both Today and Agents area)
        let mut seen = std::collections::HashSet::new();
        all_tasks.retain(|t| seen.insert(t.uuid.clone()));

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
