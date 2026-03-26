use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;
use uuid::Uuid;

use crate::{
    error::SchedulerError,
    task::{ScheduledTask, TaskState},
};

pub struct CronScheduler {
    db: sled::Db,
}

impl CronScheduler {
    pub fn open(path: &str) -> Result<Self, SchedulerError> {
        let db = sled::open(path).map_err(|e| SchedulerError::BackendError { source: e.into() })?;
        Ok(Self { db })
    }

    fn parse_schedule(expr: &str) -> Result<Schedule, SchedulerError> {
        // Try as-is (6-field with sec), then prepend "0" for standard 5-field cron.
        Schedule::from_str(expr)
            .or_else(|_| Schedule::from_str(&format!("0 {expr}")))
            .map_err(|e| SchedulerError::InvalidSchedule {
                reason: e.to_string(),
            })
    }

    fn next_run(schedule: &Schedule) -> Option<DateTime<Utc>> {
        schedule.upcoming(Utc).next()
    }

    fn key(id: Uuid) -> [u8; 16] {
        *id.as_bytes()
    }

    fn save(&self, task: &ScheduledTask) -> Result<(), SchedulerError> {
        let bytes = serde_json::to_vec(task)
            .map_err(|e| SchedulerError::BackendError { source: e.into() })?;
        self.db
            .insert(Self::key(task.id), bytes)
            .map_err(|e| SchedulerError::BackendError { source: e.into() })?;
        Ok(())
    }

    fn load(&self, id: Uuid) -> Result<ScheduledTask, SchedulerError> {
        let raw = self
            .db
            .get(Self::key(id))
            .map_err(|e| SchedulerError::BackendError { source: e.into() })?
            .ok_or(SchedulerError::TaskNotFound { id })?;
        serde_json::from_slice(&raw).map_err(|e| SchedulerError::BackendError { source: e.into() })
    }

    /// Add a cron-scheduled task. Returns the new task's [`Uuid`].
    pub fn add_task(
        &self,
        agent: &str,
        cron_expr: &str,
        input: &str,
    ) -> Result<Uuid, SchedulerError> {
        let schedule = Self::parse_schedule(cron_expr)?;
        let id = Uuid::new_v4();
        let task = ScheduledTask {
            id,
            agent: agent.to_owned(),
            cron_expr: cron_expr.to_owned(),
            input: input.to_owned(),
            state: TaskState::Pending,
            last_run: None,
            next_run: Self::next_run(&schedule),
        };
        self.save(&task)?;
        Ok(id)
    }

    /// Return all persisted tasks.
    pub fn list_tasks(&self) -> Result<Vec<ScheduledTask>, SchedulerError> {
        let mut tasks = Vec::new();
        for result in self.db.iter() {
            let (_, v) = result.map_err(|e| SchedulerError::BackendError { source: e.into() })?;
            let task: ScheduledTask = serde_json::from_slice(&v)
                .map_err(|e| SchedulerError::BackendError { source: e.into() })?;
            tasks.push(task);
        }
        Ok(tasks)
    }

    /// Return tasks that are `Pending` and whose `next_run` is in the past.
    pub fn due_tasks(&self) -> Result<Vec<ScheduledTask>, SchedulerError> {
        let now = Utc::now();
        Ok(self
            .list_tasks()?
            .into_iter()
            .filter(|t| t.state == TaskState::Pending && t.next_run.is_some_and(|nr| nr <= now))
            .collect())
    }

    /// Update the state of a task. Setting `Running` also records `last_run`.
    pub fn update_state(&self, id: Uuid, state: TaskState) -> Result<(), SchedulerError> {
        let mut task = self.load(id)?;
        if state == TaskState::Running {
            task.last_run = Some(Utc::now());
        }
        task.state = state;
        self.save(&task)
    }

    /// Recompute `next_run` from the cron expression and reset state to `Pending`.
    pub fn reschedule(&self, id: Uuid) -> Result<(), SchedulerError> {
        let mut task = self.load(id)?;
        let schedule = Self::parse_schedule(&task.cron_expr)?;
        task.next_run = Self::next_run(&schedule);
        task.state = TaskState::Pending;
        self.save(&task)
    }

    /// Remove a task from the store.
    pub fn remove_task(&self, id: Uuid) -> Result<(), SchedulerError> {
        self.db
            .remove(Self::key(id))
            .map_err(|e| SchedulerError::BackendError { source: e.into() })?;
        Ok(())
    }

    #[cfg(test)]
    fn add_task_with_next_run(
        &self,
        agent: &str,
        cron_expr: &str,
        input: &str,
        next_run: DateTime<Utc>,
    ) -> Result<Uuid, SchedulerError> {
        Self::parse_schedule(cron_expr)?;
        let id = Uuid::new_v4();
        let task = ScheduledTask {
            id,
            agent: agent.to_owned(),
            cron_expr: cron_expr.to_owned(),
            input: input.to_owned(),
            state: TaskState::Pending,
            last_run: None,
            next_run: Some(next_run),
        };
        self.save(&task)?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use tempfile::TempDir;

    fn open(tmp: &TempDir) -> CronScheduler {
        CronScheduler::open(tmp.path().to_str().unwrap()).unwrap()
    }

    #[test]
    fn add_task_persists() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let id = s.add_task("agent1", "* * * * *", "run").unwrap();
        let tasks = s.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, id);
        assert_eq!(tasks[0].agent, "agent1");
        assert_eq!(tasks[0].state, TaskState::Pending);
    }

    #[test]
    fn add_invalid_cron_errors() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let result = s.add_task("agent1", "not-a-valid-cron-expr", "run");
        assert!(matches!(
            result,
            Err(SchedulerError::InvalidSchedule { .. })
        ));
    }

    #[test]
    fn five_field_cron_accepted() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        // Standard 5-field: min hour dom month dow
        let result = s.add_task("agent1", "30 8 * * 1", "run");
        assert!(result.is_ok(), "5-field cron should be accepted");
    }

    #[test]
    fn due_tasks_returns_past() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let past = Utc::now() - Duration::seconds(30);
        let id = s
            .add_task_with_next_run("agent1", "* * * * *", "run", past)
            .unwrap();
        let due = s.due_tasks().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, id);
    }

    #[test]
    fn due_tasks_excludes_future() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        // "0 0 1 1 *" = midnight Jan 1 every year — always in the future from now
        s.add_task("agent1", "0 0 1 1 *", "run").unwrap();
        let due = s.due_tasks().unwrap();
        assert!(due.is_empty());
    }

    #[test]
    fn update_state_changes_state() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let id = s.add_task("agent1", "* * * * *", "run").unwrap();
        s.update_state(id, TaskState::Running).unwrap();
        let tasks = s.list_tasks().unwrap();
        assert_eq!(tasks[0].state, TaskState::Running);
        assert!(tasks[0].last_run.is_some());
    }

    #[test]
    fn reschedule_resets_to_pending() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let id = s.add_task("agent1", "* * * * *", "run").unwrap();
        s.update_state(id, TaskState::Completed).unwrap();
        s.reschedule(id).unwrap();
        let tasks = s.list_tasks().unwrap();
        assert_eq!(tasks[0].state, TaskState::Pending);
        assert!(tasks[0].next_run.is_some());
    }

    #[test]
    fn remove_task_deletes() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let id = s.add_task("agent1", "* * * * *", "run").unwrap();
        s.remove_task(id).unwrap();
        assert!(s.list_tasks().unwrap().is_empty());
    }

    #[test]
    fn task_not_found_error() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let result = s.update_state(Uuid::new_v4(), TaskState::Running);
        assert!(matches!(result, Err(SchedulerError::TaskNotFound { .. })));
    }
}
