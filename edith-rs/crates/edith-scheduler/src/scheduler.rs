//! Scheduler runtime - manages job execution

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::error::{SchedulerError, SchedulerResult};
use crate::job::{Job, JobContext, JobId, JobResult};

/// State of a scheduled job
struct JobState {
    job: Arc<dyn Job>,
    next_run: Option<DateTime<Utc>>,
    last_run: Option<DateTime<Utc>>,
    last_result: Option<JobResult>,
    run_count: u64,
    enabled: bool,
}

impl std::fmt::Debug for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobState")
            .field("job_id", &self.job.id())
            .field("job_name", &self.job.name())
            .field("next_run", &self.next_run)
            .field("last_run", &self.last_run)
            .field("run_count", &self.run_count)
            .field("enabled", &self.enabled)
            .finish()
    }
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// How often to check for due jobs (ms)
    pub tick_interval_ms: u64,
    /// Maximum concurrent job executions
    pub max_concurrent: usize,
    /// Whether to catch up on missed jobs
    pub catch_up: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval_ms: 1000,
            max_concurrent: 10,
            catch_up: false,
        }
    }
}

/// Event types from the scheduler
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    JobStarted {
        job_id: JobId,
        scheduled_time: DateTime<Utc>,
    },
    JobCompleted {
        job_id: JobId,
        result: JobResult,
    },
    JobFailed {
        job_id: JobId,
        error: String,
    },
    JobAdded {
        job_id: JobId,
    },
    JobRemoved {
        job_id: JobId,
    },
    SchedulerStarted,
    SchedulerStopped,
}

/// The main scheduler
pub struct Scheduler {
    config: SchedulerConfig,
    jobs: Arc<RwLock<HashMap<JobId, JobState>>>,
    running: Arc<RwLock<bool>>,
    task_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
    event_tx: Option<mpsc::Sender<SchedulerEvent>>,
}

impl Scheduler {
    /// Create a new scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            task_handle: Arc::new(RwLock::new(None)),
            event_tx: None,
        }
    }

    /// Create with event channel
    pub fn with_events(mut self) -> (Self, mpsc::Receiver<SchedulerEvent>) {
        let (tx, rx) = mpsc::channel(100);
        self.event_tx = Some(tx);
        (self, rx)
    }

    /// Add a job to the scheduler
    pub fn add_job(&self, job: Arc<dyn Job>) -> SchedulerResult<()> {
        let job_id = job.id().clone();

        let mut jobs = self.jobs.write();
        if jobs.contains_key(&job_id) {
            return Err(SchedulerError::JobAlreadyExists(job_id.to_string()));
        }

        let next_run = job.schedule().next();
        job.on_added();

        jobs.insert(
            job_id.clone(),
            JobState {
                job,
                next_run,
                last_run: None,
                last_result: None,
                run_count: 0,
                enabled: true,
            },
        );

        info!(job_id = %job_id, ?next_run, "Job added to scheduler");

        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(SchedulerEvent::JobAdded { job_id });
        }

        Ok(())
    }

    /// Remove a job from the scheduler
    pub fn remove_job(&self, job_id: &JobId) -> SchedulerResult<()> {
        let mut jobs = self.jobs.write();
        let state = jobs
            .remove(job_id)
            .ok_or_else(|| SchedulerError::JobNotFound(job_id.to_string()))?;

        state.job.on_removed();

        info!(job_id = %job_id, "Job removed from scheduler");

        if let Some(ref tx) = self.event_tx {
            let _ = tx.try_send(SchedulerEvent::JobRemoved {
                job_id: job_id.clone(),
            });
        }

        Ok(())
    }

    /// Enable a job
    pub fn enable_job(&self, job_id: &JobId) -> SchedulerResult<()> {
        let mut jobs = self.jobs.write();
        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| SchedulerError::JobNotFound(job_id.to_string()))?;

        state.enabled = true;
        state.next_run = state.job.schedule().next();

        info!(job_id = %job_id, "Job enabled");
        Ok(())
    }

    /// Disable a job
    pub fn disable_job(&self, job_id: &JobId) -> SchedulerResult<()> {
        let mut jobs = self.jobs.write();
        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| SchedulerError::JobNotFound(job_id.to_string()))?;

        state.enabled = false;
        state.next_run = None;

        info!(job_id = %job_id, "Job disabled");
        Ok(())
    }

    /// Get job information
    pub fn get_job_info(&self, job_id: &JobId) -> Option<JobInfo> {
        let jobs = self.jobs.read();
        jobs.get(job_id).map(|state| JobInfo {
            id: job_id.clone(),
            name: state.job.name().to_string(),
            next_run: state.next_run,
            last_run: state.last_run,
            run_count: state.run_count,
            enabled: state.enabled,
            last_success: state.last_result.as_ref().map(|r| r.success),
        })
    }

    /// List all jobs
    pub fn list_jobs(&self) -> Vec<JobInfo> {
        let jobs = self.jobs.read();
        jobs.iter()
            .map(|(id, state)| JobInfo {
                id: id.clone(),
                name: state.job.name().to_string(),
                next_run: state.next_run,
                last_run: state.last_run,
                run_count: state.run_count,
                enabled: state.enabled,
                last_success: state.last_result.as_ref().map(|r| r.success),
            })
            .collect()
    }

    /// Start the scheduler
    pub fn start(&self) -> SchedulerResult<()> {
        let mut running = self.running.write();
        if *running {
            return Err(SchedulerError::AlreadyRunning);
        }
        *running = true;
        drop(running);

        info!("Starting scheduler");

        let jobs = self.jobs.clone();
        let running_flag = self.running.clone();
        let tick_interval = Duration::from_millis(self.config.tick_interval_ms);
        let event_tx = self.event_tx.clone();

        let handle = tokio::spawn(async move {
            if let Some(ref tx) = event_tx {
                let _ = tx.send(SchedulerEvent::SchedulerStarted).await;
            }

            loop {
                if !*running_flag.read() {
                    break;
                }

                // Check for due jobs
                let due_jobs = {
                    let now = Utc::now();
                    let jobs = jobs.read();
                    jobs.iter()
                        .filter(|(_, state)| {
                            state.enabled
                                && state.next_run.map_or(false, |next| next <= now)
                        })
                        .map(|(id, state)| (id.clone(), state.job.clone(), state.run_count))
                        .collect::<Vec<_>>()
                };

                // Execute due jobs
                for (job_id, job, run_count) in due_jobs {
                    let now = Utc::now();
                    let scheduled_time = {
                        let jobs = jobs.read();
                        jobs.get(&job_id)
                            .and_then(|s| s.next_run)
                            .unwrap_or(now)
                    };

                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(SchedulerEvent::JobStarted {
                                job_id: job_id.clone(),
                                scheduled_time,
                            })
                            .await;
                    }

                    debug!(job_id = %job_id, "Executing job");

                    let ctx = JobContext {
                        job_id: job_id.clone(),
                        scheduled_time,
                        execution_time: now,
                        run_count,
                        last_result: {
                            let jobs = jobs.read();
                            jobs.get(&job_id).and_then(|s| s.last_result.clone())
                        },
                    };

                    let start = std::time::Instant::now();
                    let result = job.execute(ctx).await;
                    let duration = start.elapsed();
                    let result = result.with_duration(duration);

                    // Update job state
                    {
                        let mut jobs = jobs.write();
                        if let Some(state) = jobs.get_mut(&job_id) {
                            state.last_run = Some(now);
                            state.run_count += 1;
                            state.next_run = state.job.schedule().next_after(now);

                            if result.success {
                                debug!(
                                    job_id = %job_id,
                                    duration_ms = duration.as_millis(),
                                    "Job completed successfully"
                                );
                            } else {
                                warn!(
                                    job_id = %job_id,
                                    error = ?result.message,
                                    "Job failed"
                                );

                                if !job.continue_on_failure() {
                                    state.enabled = false;
                                    state.next_run = None;
                                }
                            }

                            state.last_result = Some(result.clone());
                        }
                    }

                    if let Some(ref tx) = event_tx {
                        if result.success {
                            let _ = tx
                                .send(SchedulerEvent::JobCompleted {
                                    job_id: job_id.clone(),
                                    result,
                                })
                                .await;
                        } else {
                            let _ = tx
                                .send(SchedulerEvent::JobFailed {
                                    job_id: job_id.clone(),
                                    error: result.message.unwrap_or_default(),
                                })
                                .await;
                        }
                    }
                }

                tokio::time::sleep(tick_interval).await;
            }

            if let Some(ref tx) = event_tx {
                let _ = tx.send(SchedulerEvent::SchedulerStopped).await;
            }

            info!("Scheduler stopped");
        });

        *self.task_handle.write() = Some(handle);
        Ok(())
    }

    /// Stop the scheduler
    pub fn stop(&self) -> SchedulerResult<()> {
        let mut running = self.running.write();
        if !*running {
            return Err(SchedulerError::NotRunning);
        }
        *running = false;
        drop(running);

        info!("Stopping scheduler");
        Ok(())
    }

    /// Check if scheduler is running
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Wait for scheduler to stop
    pub async fn wait(&self) {
        let handle = self.task_handle.write().take();
        if let Some(h) = handle {
            let _ = h.await;
        }
    }

    /// Trigger a job immediately (out of schedule)
    pub async fn trigger_job(&self, job_id: &JobId) -> SchedulerResult<JobResult> {
        let (job, run_count) = {
            let jobs = self.jobs.read();
            let state = jobs
                .get(job_id)
                .ok_or_else(|| SchedulerError::JobNotFound(job_id.to_string()))?;
            (state.job.clone(), state.run_count)
        };

        let now = Utc::now();
        let ctx = JobContext {
            job_id: job_id.clone(),
            scheduled_time: now,
            execution_time: now,
            run_count,
            last_result: None,
        };

        let start = std::time::Instant::now();
        let result = job.execute(ctx).await;
        let duration = start.elapsed();
        let result = result.with_duration(duration);

        // Update state
        {
            let mut jobs = self.jobs.write();
            if let Some(state) = jobs.get_mut(job_id) {
                state.last_run = Some(now);
                state.run_count += 1;
                state.last_result = Some(result.clone());
            }
        }

        Ok(result)
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

/// Information about a job
#[derive(Debug, Clone)]
pub struct JobInfo {
    pub id: JobId,
    pub name: String,
    pub next_run: Option<DateTime<Utc>>,
    pub last_run: Option<DateTime<Utc>>,
    pub run_count: u64,
    pub enabled: bool,
    pub last_success: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobBuilder, Schedule};
    use chrono::Duration;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_scheduler_add_remove_job() {
        let scheduler = Scheduler::default();

        let job = Arc::new(
            JobBuilder::new("test-job")
                .name("Test")
                .interval(Duration::hours(1))
                .build_sync(|_| JobResult::success())
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();
        assert!(scheduler.get_job_info(&JobId::from("test-job")).is_some());

        scheduler.remove_job(&JobId::from("test-job")).unwrap();
        assert!(scheduler.get_job_info(&JobId::from("test-job")).is_none());
    }

    #[tokio::test]
    async fn test_scheduler_trigger_job() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let scheduler = Scheduler::default();

        let job = Arc::new(
            JobBuilder::new("counter-job")
                .interval(Duration::hours(1))
                .build_sync(move |_| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                    JobResult::success()
                })
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();

        let result = scheduler.trigger_job(&JobId::from("counter-job")).await.unwrap();
        assert!(result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_scheduler_start_stop() {
        let scheduler = Scheduler::default();

        let job = Arc::new(
            JobBuilder::new("test-job")
                .interval(Duration::hours(1))
                .build_sync(|_| JobResult::success())
                .unwrap(),
        );

        scheduler.add_job(job).unwrap();

        scheduler.start().unwrap();
        assert!(scheduler.is_running());

        scheduler.stop().unwrap();
        assert!(!scheduler.is_running());
    }
}
