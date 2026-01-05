//! Scheduler error types

use thiserror::Error;

/// Scheduler error types
#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    #[error("job not found: {0}")]
    JobNotFound(String),

    #[error("job already exists: {0}")]
    JobAlreadyExists(String),

    #[error("scheduler not running")]
    NotRunning,

    #[error("scheduler already running")]
    AlreadyRunning,

    #[error("job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("parse error: {0}")]
    ParseError(String),
}

/// Result type for scheduler operations
pub type SchedulerResult<T> = Result<T, SchedulerError>;
