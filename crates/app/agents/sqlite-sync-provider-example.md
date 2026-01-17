# SQLite-Based Sync Provider Example

This document shows how to implement a sync provider where jobs are persisted to SQLite and executed by worker processes pulling from the queue.

## Architecture

- **Provider**: Inserts jobs into SQLite table
- **Workers**: Poll SQLite for pending jobs and execute them
- **Benefits**:
  - Jobs survive restarts
  - Multiple workers can pull from same queue
  - Built-in job history and retry logic
  - Can prioritize/order jobs with SQL queries

## Database Schema

```sql
CREATE TABLE sync_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type TEXT NOT NULL,  -- 'SyncBucket', 'DownloadPins', 'PingPeer'
    payload TEXT NOT NULL,   -- JSON-serialized job data
    status TEXT NOT NULL,    -- 'pending', 'running', 'completed', 'failed'
    attempts INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    error TEXT,
    priority INTEGER DEFAULT 0  -- Higher = more urgent
);

CREATE INDEX idx_sync_jobs_status ON sync_jobs(status, priority DESC, created_at);
```

## Provider Implementation

```rust
use anyhow::Result;
use async_trait::async_trait;
use common::peer::{SyncJob, SyncProvider, Peer};
use common::bucket_log::BucketLogProvider;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct SqliteSyncProvider {
    database: Database,
}

impl SqliteSyncProvider {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl<L> SyncProvider<L> for SqliteSyncProvider
where
    L: BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    async fn execute(&self, _peer: &Peer<L>, job: SyncJob) -> Result<()> {
        // Serialize job to JSON
        let job_type = match &job {
            SyncJob::SyncBucket(_) => "SyncBucket",
            SyncJob::DownloadPins(_) => "DownloadPins",
            SyncJob::PingPeer(_) => "PingPeer",
        };

        let payload = serde_json::to_string(&job)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        // Insert into database
        sqlx::query(
            r#"
            INSERT INTO sync_jobs (job_type, payload, status, created_at)
            VALUES (?, ?, 'pending', ?)
            "#
        )
        .bind(job_type)
        .bind(payload)
        .bind(now)
        .execute(self.database.pool())
        .await?;

        tracing::debug!("Enqueued {} job to database", job_type);
        Ok(())
    }
}
```

## Worker Implementation

```rust
use anyhow::Result;
use common::peer::{Peer, sync::{SyncJob, execute_job}};
use common::bucket_log::BucketLogProvider;
use tokio::time::{sleep, Duration};

pub struct SqliteWorkerConfig {
    /// How long to sleep when no jobs are available
    pub poll_interval: Duration,
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// How long to wait before retrying failed jobs
    pub retry_delay: Duration,
}

impl Default for SqliteWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            max_attempts: 3,
            retry_delay: Duration::from_secs(60),
        }
    }
}

/// Run a worker that pulls jobs from SQLite and executes them
///
/// This can be run in multiple processes for horizontal scaling.
pub async fn run_sqlite_worker<L>(
    peer: Peer<L>,
    database: Database,
    config: SqliteWorkerConfig,
) -> Result<()>
where
    L: BucketLogProvider + Clone + Send + Sync + 'static,
    L::Error: std::error::Error + Send + Sync + 'static,
{
    use tokio::time::{interval, Duration};

    tracing::info!("Starting SQLite sync worker for peer {}", peer.id());

    // Create interval timer for periodic pings (every 5 seconds)
    let mut ping_interval = interval(Duration::from_secs(5));
    ping_interval.tick().await; // Skip first immediate tick

    loop {
        tokio::select! {
            // Poll for next job
            job_result = poll_next_job(&database) => {
                match job_result {
                    Ok(Some((job_id, job))) => {
                        tracing::info!("Processing job {}: {:?}", job_id, job);

                        // Mark as running
                        if let Err(e) = mark_job_running(&database, job_id).await {
                            tracing::error!("Failed to mark job {} as running: {}", job_id, e);
                            continue;
                        }

                        // Execute the job
                        match execute_job(&peer, job).await {
                            Ok(()) => {
                                tracing::info!("Job {} completed successfully", job_id);
                                if let Err(e) = mark_job_completed(&database, job_id).await {
                                    tracing::error!("Failed to mark job {} as completed: {}", job_id, e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Job {} failed: {}", job_id, e);
                                if let Err(e) = mark_job_failed(&database, job_id, &e.to_string(), config.max_attempts).await {
                                    tracing::error!("Failed to mark job {} as failed: {}", job_id, e);
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // No jobs available, sleep
                        sleep(config.poll_interval).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to poll for jobs: {}", e);
                        sleep(config.poll_interval).await;
                    }
                }
            }

            // Periodic ping scheduler
            _ = ping_interval.tick() => {
                tracing::info!("Running periodic ping scheduler");
                peer.schedule_periodic_pings().await;
            }
        }
    }
}

/// Poll for the next pending job
async fn poll_next_job(database: &Database) -> Result<Option<(i64, SyncJob)>> {
    let row = sqlx::query(
        r#"
        SELECT id, payload
        FROM sync_jobs
        WHERE status = 'pending'
        ORDER BY priority DESC, created_at ASC
        LIMIT 1
        "#
    )
    .fetch_optional(database.pool())
    .await?;

    match row {
        Some(row) => {
            let id: i64 = row.get("id");
            let payload: String = row.get("payload");
            let job: SyncJob = serde_json::from_str(&payload)?;
            Ok(Some((id, job)))
        }
        None => Ok(None),
    }
}

async fn mark_job_running(database: &Database, job_id: i64) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET status = 'running', started_at = ?
        WHERE id = ?
        "#
    )
    .bind(now)
    .bind(job_id)
    .execute(database.pool())
    .await?;

    Ok(())
}

async fn mark_job_completed(database: &Database, job_id: i64) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET status = 'completed', completed_at = ?
        WHERE id = ?
        "#
    )
    .bind(now)
    .bind(job_id)
    .execute(database.pool())
    .await?;

    Ok(())
}

async fn mark_job_failed(
    database: &Database,
    job_id: i64,
    error: &str,
    max_attempts: u32,
) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    // Increment attempts and check if we should retry
    let row = sqlx::query(
        r#"
        UPDATE sync_jobs
        SET attempts = attempts + 1, error = ?, completed_at = ?
        WHERE id = ?
        RETURNING attempts
        "#
    )
    .bind(error)
    .bind(now)
    .bind(job_id)
    .fetch_one(database.pool())
    .await?;

    let attempts: i32 = row.get("attempts");

    // If we've exceeded max attempts, mark as failed
    // Otherwise, reset to pending for retry
    let status = if attempts >= max_attempts as i32 {
        "failed"
    } else {
        "pending"
    };

    sqlx::query("UPDATE sync_jobs SET status = ? WHERE id = ?")
        .bind(status)
        .bind(job_id)
        .execute(database.pool())
        .await?;

    Ok(())
}
```

## Usage Example

```rust
use crate::daemon::database::Database;
use crate::daemon::sync_provider::{SqliteSyncProvider, run_sqlite_worker, SqliteWorkerConfig};

// In State::from_config
let database = Database::connect(&sqlite_database_url).await?;

// Create SQLite-based sync provider
let sync_provider = SqliteSyncProvider::new(database.clone());

let peer = PeerBuilder::new()
    .with_sync_provider(std::sync::Arc::new(sync_provider))
    .log_provider(database.clone())
    .build()
    .await;

// Spawn worker(s) - can spawn multiple for parallel processing
let peer_for_worker = peer.clone();
let db_for_worker = database.clone();
tokio::spawn(async move {
    if let Err(e) = run_sqlite_worker(
        peer_for_worker,
        db_for_worker,
        SqliteWorkerConfig::default(),
    ).await {
        tracing::error!("Worker failed: {}", e);
    }
});

// Can spawn additional workers in other processes/machines
// by connecting to the same database
```

## Advanced Features

### Priority Queue
```sql
-- High priority sync jobs get processed first
UPDATE sync_jobs SET priority = 10 WHERE job_type = 'SyncBucket';
```

### Job Monitoring
```sql
-- See job queue status
SELECT status, COUNT(*) as count, AVG(attempts) as avg_attempts
FROM sync_jobs
GROUP BY status;

-- Find stuck jobs (running too long)
SELECT id, job_type, started_at
FROM sync_jobs
WHERE status = 'running'
  AND started_at < unixepoch('now') - 300; -- 5 minutes ago
```

### Cleanup Old Jobs
```sql
-- Delete completed jobs older than 7 days
DELETE FROM sync_jobs
WHERE status = 'completed'
  AND completed_at < unixepoch('now') - 604800;
```

## Comparison with Queued Provider

| Feature | QueuedSyncProvider (flume) | SqliteSyncProvider |
|---------|---------------------------|-------------------|
| Persistence | No (lost on restart) | Yes (survives restarts) |
| Scaling | Single process | Multiple processes/machines |
| Job History | No | Yes (queryable) |
| Retry Logic | Manual | Built-in with attempts tracking |
| Ordering | FIFO | Customizable (SQL ORDER BY) |
| Memory Usage | Jobs in RAM | Jobs on disk |
| Complexity | Simple | More complex |
| Performance | Faster (in-memory) | Slower (disk I/O) |

## When to Use Each

- **QueuedSyncProvider**: Simple deployments, single instance, high throughput, jobs can be lost
- **SqliteSyncProvider**: Need persistence, multiple workers, job history/monitoring, retry logic
