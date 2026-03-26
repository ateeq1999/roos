use std::str::FromStr;

use chrono::{DateTime, Utc};
use cron::Schedule;
use uuid::Uuid;

use crate::{
    error::SchedulerError,
    task::{RetryPolicy, RetryStrategy, ScheduledTask, TaskKind, TaskState},
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

    /// Add a cron-scheduled task with the default retry policy.
    pub fn add_task(
        &self,
        agent: &str,
        cron_expr: &str,
        input: &str,
    ) -> Result<Uuid, SchedulerError> {
        self.add_task_with_retry(agent, cron_expr, input, RetryPolicy::default())
    }

    /// Add a cron-scheduled task with a custom retry policy.
    pub fn add_task_with_retry(
        &self,
        agent: &str,
        cron_expr: &str,
        input: &str,
        policy: RetryPolicy,
    ) -> Result<Uuid, SchedulerError> {
        let schedule = Self::parse_schedule(cron_expr)?;
        let id = Uuid::new_v4();
        let task = ScheduledTask {
            id,
            agent: agent.to_owned(),
            kind: TaskKind::Cron {
                expr: cron_expr.to_owned(),
            },
            input: input.to_owned(),
            state: TaskState::Pending,
            last_run: None,
            next_run: Self::next_run(&schedule),
            retry_policy: policy,
            retry_count: 0,
        };
        self.save(&task)?;
        Ok(id)
    }

    /// Add a one-shot task with the default retry policy.
    pub fn add_one_shot(
        &self,
        agent: &str,
        at: DateTime<Utc>,
        input: &str,
    ) -> Result<Uuid, SchedulerError> {
        self.add_one_shot_with_retry(agent, at, input, RetryPolicy::default())
    }

    /// Add a one-shot task with a custom retry policy.
    pub fn add_one_shot_with_retry(
        &self,
        agent: &str,
        at: DateTime<Utc>,
        input: &str,
        policy: RetryPolicy,
    ) -> Result<Uuid, SchedulerError> {
        let id = Uuid::new_v4();
        let task = ScheduledTask {
            id,
            agent: agent.to_owned(),
            kind: TaskKind::OneShot,
            input: input.to_owned(),
            state: TaskState::Pending,
            last_run: None,
            next_run: Some(at),
            retry_policy: policy,
            retry_count: 0,
        };
        self.save(&task)?;
        Ok(id)
    }

    /// Record a task failure. Schedules a retry if attempts remain;
    /// otherwise marks the task `Failed` and logs the final error.
    pub fn record_failure(&self, id: Uuid, reason: &str) -> Result<(), SchedulerError> {
        let mut task = self.load(id)?;
        task.retry_count += 1;
        if task.retry_count <= task.retry_policy.max_retries {
            let base = task.retry_policy.retry_delay_seconds;
            let delay_secs = match task.retry_policy.strategy {
                RetryStrategy::Fixed => base,
                RetryStrategy::Exponential => {
                    let exp = (task.retry_count - 1).min(62);
                    base.saturating_mul(2u64.pow(exp))
                }
            };
            task.next_run = Some(Utc::now() + chrono::Duration::seconds(delay_secs as i64));
            task.state = TaskState::Pending;
        } else {
            tracing::error!(
                task_id = %id,
                reason = reason,
                retry_count = task.retry_count,
                "task exhausted all retries"
            );
            task.state = TaskState::Failed {
                reason: reason.to_owned(),
            };
            task.next_run = None;
        }
        self.save(&task)
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

    /// For cron tasks: recompute `next_run` and reset to `Pending`.
    /// For one-shot tasks: mark `Completed` (they do not repeat).
    pub fn reschedule(&self, id: Uuid) -> Result<(), SchedulerError> {
        let mut task = self.load(id)?;
        match &task.kind {
            TaskKind::Cron { expr } => {
                let schedule = Self::parse_schedule(expr)?;
                task.next_run = Self::next_run(&schedule);
                task.state = TaskState::Pending;
            }
            TaskKind::OneShot => {
                task.state = TaskState::Completed;
                task.next_run = None;
            }
        }
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
            kind: TaskKind::Cron {
                expr: cron_expr.to_owned(),
            },
            input: input.to_owned(),
            state: TaskState::Pending,
            last_run: None,
            next_run: Some(next_run),
            retry_policy: RetryPolicy::default(),
            retry_count: 0,
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

    // ── one-shot tests ────────────────────────────────────────────────────────

    #[test]
    fn add_one_shot_persists() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let at = Utc::now() + Duration::hours(1);
        let id = s.add_one_shot("agent1", at, "run").unwrap();
        let tasks = s.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, id);
        assert_eq!(tasks[0].kind, TaskKind::OneShot);
        assert_eq!(tasks[0].state, TaskState::Pending);
    }

    #[test]
    fn one_shot_due_when_past() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let past = Utc::now() - Duration::seconds(10);
        let id = s.add_one_shot("agent1", past, "run").unwrap();
        let due = s.due_tasks().unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, id);
    }

    #[test]
    fn one_shot_future_not_due() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let future = Utc::now() + Duration::hours(1);
        s.add_one_shot("agent1", future, "run").unwrap();
        assert!(s.due_tasks().unwrap().is_empty());
    }

    #[test]
    fn reschedule_one_shot_marks_completed() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let past = Utc::now() - Duration::seconds(5);
        let id = s.add_one_shot("agent1", past, "run").unwrap();
        s.update_state(id, TaskState::Running).unwrap();
        s.reschedule(id).unwrap();
        let tasks = s.list_tasks().unwrap();
        assert_eq!(tasks[0].state, TaskState::Completed);
        assert!(tasks[0].next_run.is_none());
    }

    // ── cron tests ────────────────────────────────────────────────────────────

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

    // ── retry policy tests ────────────────────────────────────────────────────

    #[test]
    fn retry_on_failure_reschedules_pending() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let id = s.add_task("agent1", "* * * * *", "run").unwrap();
        s.update_state(id, TaskState::Running).unwrap();
        s.record_failure(id, "timeout").unwrap();
        let task = s
            .list_tasks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 1);
        assert!(task.next_run.is_some());
    }

    #[test]
    fn retry_exhausted_marks_failed() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let policy = RetryPolicy {
            max_retries: 2,
            retry_delay_seconds: 1,
            strategy: RetryStrategy::Fixed,
        };
        let id = s
            .add_task_with_retry("agent1", "* * * * *", "run", policy)
            .unwrap();
        s.record_failure(id, "err").unwrap(); // retry 1
        s.record_failure(id, "err").unwrap(); // retry 2
        s.record_failure(id, "final").unwrap(); // exhausted
        let task = s
            .list_tasks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        assert!(matches!(task.state, TaskState::Failed { .. }));
        assert_eq!(task.retry_count, 3);
        assert!(task.next_run.is_none());
    }

    #[test]
    fn retry_strategy_fixed_constant_delay() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let policy = RetryPolicy {
            max_retries: 3,
            retry_delay_seconds: 30,
            strategy: RetryStrategy::Fixed,
        };
        let id = s
            .add_task_with_retry("agent1", "* * * * *", "run", policy)
            .unwrap();
        let before = Utc::now();
        s.record_failure(id, "err").unwrap();
        let task = s
            .list_tasks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        let nr = task.next_run.unwrap();
        // next_run should be ~30s from now (within a 2s window for test timing)
        assert!(nr >= before + chrono::Duration::seconds(29));
        assert!(nr <= before + chrono::Duration::seconds(31));
    }

    #[test]
    fn retry_strategy_exponential_doubles_delay() {
        let tmp = TempDir::new().unwrap();
        let s = open(&tmp);
        let policy = RetryPolicy {
            max_retries: 3,
            retry_delay_seconds: 10,
            strategy: RetryStrategy::Exponential,
        };
        let id = s
            .add_task_with_retry("agent1", "* * * * *", "run", policy)
            .unwrap();
        // 1st failure → delay = 10 * 2^0 = 10s
        let before1 = Utc::now();
        s.record_failure(id, "err").unwrap();
        let t1 = s
            .list_tasks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        let nr1 = t1.next_run.unwrap();
        assert!(nr1 >= before1 + chrono::Duration::seconds(9));
        assert!(nr1 <= before1 + chrono::Duration::seconds(11));
        // 2nd failure → delay = 10 * 2^1 = 20s
        let before2 = Utc::now();
        s.record_failure(id, "err").unwrap();
        let t2 = s
            .list_tasks()
            .unwrap()
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        let nr2 = t2.next_run.unwrap();
        assert!(nr2 >= before2 + chrono::Duration::seconds(19));
        assert!(nr2 <= before2 + chrono::Duration::seconds(21));
    }
}
