//! Scheduled job definitions and types

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

use crate::error::{SchedulerError, SchedulerResult};

/// Job identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub String);

impl JobId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for JobId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for JobId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Schedule definition - either cron or interval
#[derive(Debug, Clone)]
pub enum Schedule {
    /// Cron expression (e.g., "0 */5 * * * *" for every 5 minutes)
    Cron(cron::Schedule),
    /// Fixed interval
    Interval(Duration),
    /// One-time execution at a specific time
    Once(DateTime<Utc>),
}

impl Schedule {
    /// Parse a cron expression
    pub fn cron(expr: &str) -> SchedulerResult<Self> {
        let schedule = expr
            .parse::<cron::Schedule>()
            .map_err(|e| SchedulerError::InvalidCron(e.to_string()))?;
        Ok(Self::Cron(schedule))
    }

    /// Create an interval schedule
    pub fn interval(duration: Duration) -> Self {
        Self::Interval(duration)
    }

    /// Create an interval from a human-readable string like "5m", "1h", "30s"
    pub fn interval_from_str(s: &str) -> SchedulerResult<Self> {
        let duration = humantime::parse_duration(s)
            .map_err(|e| SchedulerError::ParseError(e.to_string()))?;
        Ok(Self::Interval(Duration::from_std(duration).unwrap_or(Duration::seconds(0))))
    }

    /// Schedule a one-time execution
    pub fn once(at: DateTime<Utc>) -> Self {
        Self::Once(at)
    }

    /// Get the next execution time after the given time
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Schedule::Cron(cron_schedule) => cron_schedule.after(&after).next(),
            Schedule::Interval(duration) => Some(after + *duration),
            Schedule::Once(at) => {
                if *at > after {
                    Some(*at)
                } else {
                    None
                }
            }
        }
    }

    /// Get the next execution time from now
    pub fn next(&self) -> Option<DateTime<Utc>> {
        self.next_after(Utc::now())
    }
}

/// Result of a job execution
#[derive(Debug, Clone)]
pub struct JobResult {
    /// Whether the job succeeded
    pub success: bool,
    /// Output message
    pub message: Option<String>,
    /// Duration of execution
    pub duration: std::time::Duration,
    /// Any data produced
    pub data: Option<serde_json::Value>,
}

impl JobResult {
    /// Create a success result
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
            duration: std::time::Duration::ZERO,
            data: None,
        }
    }

    /// Create a success result with message
    pub fn success_with_message(msg: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(msg.into()),
            duration: std::time::Duration::ZERO,
            data: None,
        }
    }

    /// Create a failure result
    pub fn failure(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(msg.into()),
            duration: std::time::Duration::ZERO,
            data: None,
        }
    }

    /// Set the execution duration
    pub fn with_duration(mut self, duration: std::time::Duration) -> Self {
        self.duration = duration;
        self
    }
}

/// Context passed to job execution
#[derive(Debug, Clone)]
pub struct JobContext {
    /// Job ID
    pub job_id: JobId,
    /// Scheduled execution time
    pub scheduled_time: DateTime<Utc>,
    /// Actual execution start time
    pub execution_time: DateTime<Utc>,
    /// Number of previous executions
    pub run_count: u64,
    /// Last execution result (if any)
    pub last_result: Option<JobResult>,
}

/// Trait for implementing scheduled jobs
#[async_trait]
pub trait Job: Send + Sync {
    /// Get the job ID
    fn id(&self) -> &JobId;

    /// Get the job name (for display/logging)
    fn name(&self) -> &str;

    /// Get the schedule for this job
    fn schedule(&self) -> &Schedule;

    /// Execute the job
    async fn execute(&self, ctx: JobContext) -> JobResult;

    /// Called when the job is added to the scheduler
    fn on_added(&self) {
        debug!(job_id = %self.id(), name = %self.name(), "Job added to scheduler");
    }

    /// Called when the job is removed from the scheduler
    fn on_removed(&self) {
        debug!(job_id = %self.id(), name = %self.name(), "Job removed from scheduler");
    }

    /// Whether to continue running after a failure
    fn continue_on_failure(&self) -> bool {
        true
    }

    /// Maximum number of retries on failure
    fn max_retries(&self) -> u32 {
        0
    }
}

/// A simple job backed by a closure
pub struct FunctionJob<F>
where
    F: Fn(JobContext) -> JobResult + Send + Sync + 'static,
{
    id: JobId,
    name: String,
    schedule: Schedule,
    executor: F,
}

impl<F> FunctionJob<F>
where
    F: Fn(JobContext) -> JobResult + Send + Sync + 'static,
{
    pub fn new(id: impl Into<JobId>, name: impl Into<String>, schedule: Schedule, executor: F) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            schedule,
            executor,
        }
    }
}

#[async_trait]
impl<F> Job for FunctionJob<F>
where
    F: Fn(JobContext) -> JobResult + Send + Sync + 'static,
{
    fn id(&self) -> &JobId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    async fn execute(&self, ctx: JobContext) -> JobResult {
        (self.executor)(ctx)
    }
}

/// An async job backed by an async closure
pub struct AsyncFunctionJob<F, Fut>
where
    F: Fn(JobContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = JobResult> + Send + 'static,
{
    id: JobId,
    name: String,
    schedule: Schedule,
    executor: F,
}

impl<F, Fut> AsyncFunctionJob<F, Fut>
where
    F: Fn(JobContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = JobResult> + Send + 'static,
{
    pub fn new(id: impl Into<JobId>, name: impl Into<String>, schedule: Schedule, executor: F) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            schedule,
            executor,
        }
    }
}

#[async_trait]
impl<F, Fut> Job for AsyncFunctionJob<F, Fut>
where
    F: Fn(JobContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = JobResult> + Send + 'static,
{
    fn id(&self) -> &JobId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    async fn execute(&self, ctx: JobContext) -> JobResult {
        (self.executor)(ctx).await
    }
}

/// Builder for creating jobs
pub struct JobBuilder {
    id: JobId,
    name: Option<String>,
    schedule: Option<Schedule>,
}

impl JobBuilder {
    pub fn new(id: impl Into<JobId>) -> Self {
        let id = id.into();
        Self {
            id,
            name: None,
            schedule: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn cron(mut self, expr: &str) -> SchedulerResult<Self> {
        self.schedule = Some(Schedule::cron(expr)?);
        Ok(self)
    }

    pub fn interval(mut self, duration: chrono::Duration) -> Self {
        self.schedule = Some(Schedule::interval(duration));
        self
    }

    pub fn interval_str(mut self, s: &str) -> SchedulerResult<Self> {
        self.schedule = Some(Schedule::interval_from_str(s)?);
        Ok(self)
    }

    pub fn once_at(mut self, at: DateTime<Utc>) -> Self {
        self.schedule = Some(Schedule::once(at));
        self
    }

    pub fn build_sync<F>(self, executor: F) -> SchedulerResult<FunctionJob<F>>
    where
        F: Fn(JobContext) -> JobResult + Send + Sync + 'static,
    {
        let schedule = self
            .schedule
            .ok_or_else(|| SchedulerError::Internal("schedule not set".to_string()))?;

        Ok(FunctionJob {
            id: self.id.clone(),
            name: self.name.unwrap_or_else(|| self.id.to_string()),
            schedule,
            executor,
        })
    }

    pub fn build_async<F, Fut>(self, executor: F) -> SchedulerResult<AsyncFunctionJob<F, Fut>>
    where
        F: Fn(JobContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = JobResult> + Send + 'static,
    {
        let schedule = self
            .schedule
            .ok_or_else(|| SchedulerError::Internal("schedule not set".to_string()))?;

        Ok(AsyncFunctionJob {
            id: self.id.clone(),
            name: self.name.unwrap_or_else(|| self.id.to_string()),
            schedule,
            executor,
        })
    }
}

/// Heartbeat job that sends periodic messages
pub struct HeartbeatJob {
    id: JobId,
    schedule: Schedule,
    conversation_id: String,
    message: String,
    sender: tokio::sync::mpsc::Sender<HeartbeatEvent>,
}

/// Event emitted by heartbeat job
#[derive(Debug, Clone)]
pub struct HeartbeatEvent {
    pub conversation_id: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

impl HeartbeatJob {
    pub fn new(
        id: impl Into<JobId>,
        schedule: Schedule,
        conversation_id: impl Into<String>,
        message: impl Into<String>,
        sender: tokio::sync::mpsc::Sender<HeartbeatEvent>,
    ) -> Self {
        Self {
            id: id.into(),
            schedule,
            conversation_id: conversation_id.into(),
            message: message.into(),
            sender,
        }
    }
}

#[async_trait]
impl Job for HeartbeatJob {
    fn id(&self) -> &JobId {
        &self.id
    }

    fn name(&self) -> &str {
        "heartbeat"
    }

    fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    async fn execute(&self, _ctx: JobContext) -> JobResult {
        let event = HeartbeatEvent {
            conversation_id: self.conversation_id.clone(),
            message: self.message.clone(),
            timestamp: Utc::now(),
        };

        match self.sender.send(event).await {
            Ok(_) => JobResult::success_with_message("heartbeat sent"),
            Err(e) => JobResult::failure(format!("failed to send heartbeat: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_cron() {
        let schedule = Schedule::cron("0 0 * * * *").unwrap();
        let next = schedule.next();
        assert!(next.is_some());
    }

    #[test]
    fn test_schedule_interval() {
        let schedule = Schedule::interval(Duration::minutes(5));
        let now = Utc::now();
        let next = schedule.next_after(now).unwrap();
        assert!(next > now);
        assert!(next <= now + Duration::minutes(6));
    }

    #[test]
    fn test_schedule_interval_from_str() {
        let schedule = Schedule::interval_from_str("30s").unwrap();
        let next = schedule.next();
        assert!(next.is_some());
    }

    #[test]
    fn test_job_builder() {
        let job = JobBuilder::new("test-job")
            .name("Test Job")
            .interval(Duration::seconds(60))
            .build_sync(|_ctx| JobResult::success())
            .unwrap();

        assert_eq!(job.id().as_str(), "test-job");
        assert_eq!(job.name(), "Test Job");
    }
}
