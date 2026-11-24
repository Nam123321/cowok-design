// @story 10.1
// @implements AC-02, AC-03 (Single MPSC SQLite Write Actor — WAL, busy_timeout=5000, Bounded Backpressure, Zero Blocking I/O Inside Actor)
use std::fmt;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior};
use thiserror::Error;

use crate::pool::SqlitePool;

/// Default bounded MPSC queue capacity for the single SQLite write actor.
pub const DEFAULT_WRITE_ACTOR_CAPACITY: usize = 256;

/// Mandatory SQLite WAL busy timeout (`5000ms`) configured on the actor connection.
pub const DEFAULT_WRITE_ACTOR_BUSY_TIMEOUT_MS: u32 = 5_000;

/// Default `Retry-After` header value (seconds) returned when backpressure triggers.
pub const DEFAULT_BACKPRESSURE_RETRY_AFTER_SECS: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SqliteActorError {
    #[error(
        "ACTOR_BACKPRESSURE: SQLite write actor queue saturated (capacity={queue_capacity}, retry_after_secs={retry_after_secs})"
    )]
    Backpressure {
        retry_after_secs: u64,
        queue_capacity: usize,
    },
    #[error("ACTOR_DISCONNECTED: SQLite write actor thread is no longer running")]
    ActorDisconnected,
    #[error("ACTOR_INVALID_CAPACITY: SQLite write actor queue capacity must be >= 1")]
    InvalidCapacity,
    #[error("SYMLINK_VIOLATION: refusing symlink SQLite path '{0}'")]
    SymlinkDenied(String),
    #[error("ACTOR_OPEN_FAILED: {0}")]
    OpenFailed(String),
    #[error("ACTOR_TX_FAILED: {0}")]
    TransactionFailed(String),
    #[error("ACTOR_OP_FAILED: {0}")]
    OperationFailed(String),
}

type BoxedWriteJob = Box<dyn FnOnce(&mut Connection, QueueDepthGuard) + Send + 'static>;

struct WriteCommand {
    job: BoxedWriteJob,
    depth_guard: QueueDepthGuard,
}

/// Non-blocking receipt returned by [`SqliteWriteActorHandle::try_enqueue_immediate`].
/// Callers can await or block on [`WriteReceipt::wait`] after enqueuing without holding locks.
pub struct WriteReceipt<T> {
    rx: mpsc::Receiver<Result<T, SqliteActorError>>,
}

impl<T> WriteReceipt<T> {
    /// Waits for the SQLite write actor to complete the enqueued `IMMEDIATE` transaction.
    pub fn wait(self) -> Result<T, SqliteActorError> {
        self.rx
            .recv()
            .map_err(|_| SqliteActorError::ActorDisconnected)?
    }
}

struct QueueDepthGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for QueueDepthGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

struct AliveResetGuard {
    alive: Arc<AtomicBool>,
}

impl Drop for AliveResetGuard {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

struct ActorInner {
    tx: Option<mpsc::SyncSender<WriteCommand>>,
    join_handle: Option<JoinHandle<()>>,
    alive: Arc<AtomicBool>,
    queue_depth: Arc<AtomicUsize>,
    capacity: usize,
    db_path: PathBuf,
}

impl Drop for ActorInner {
    fn drop(&mut self) {
        // Close the bounded MPSC channel first so the actor thread drains and exits cleanly.
        let _ = self.tx.take();
        if let Some(handle) = self.join_handle.take() {
            if handle.thread().id() != thread::current().id() {
                let _ = handle.join();
            }
        }
    }
}

/// Cloneable handle to the Single MPSC SQLite Write Actor.
///
/// Serializes all SQLite WAL write mutations onto one dedicated OS thread owning a single
/// `rusqlite::Connection` (`WAL`, `synchronous=NORMAL`, `busy_timeout=5000`, `foreign_keys=ON`),
/// guaranteeing `0` `SQLITE_BUSY` write contention under concurrent workloads and deterministic
/// bounded backpressure when saturated.
#[derive(Clone)]
pub struct SqliteWriteActorHandle {
    inner: Arc<ActorInner>,
}

impl fmt::Debug for SqliteWriteActorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteWriteActorHandle")
            .field("db_path", &self.inner.db_path)
            .field("capacity", &self.inner.capacity)
            .field("queue_depth", &self.queue_depth())
            .finish()
    }
}

/// Namespace factory for spawning the Single MPSC SQLite Write Actor.
pub struct SqliteWriteActor;

impl SqliteWriteActor {
    /// Spawns a dedicated SQLite Write Actor with default bounded capacity (`256`).
    pub fn spawn(db_path: impl Into<PathBuf>) -> Result<SqliteWriteActorHandle, SqliteActorError> {
        Self::spawn_with_capacity(db_path, DEFAULT_WRITE_ACTOR_CAPACITY)
    }

    /// Spawns a dedicated SQLite Write Actor with an explicit bounded queue `capacity`.
    pub fn spawn_with_capacity(
        db_path: impl Into<PathBuf>,
        capacity: usize,
    ) -> Result<SqliteWriteActorHandle, SqliteActorError> {
        SqliteWriteActorHandle::spawn_with_capacity(db_path, capacity)
    }

    /// Spawns a dedicated SQLite Write Actor bound to an existing [`SqlitePool`]'s database path.
    pub fn from_pool(pool: &SqlitePool) -> Result<SqliteWriteActorHandle, SqliteActorError> {
        SqliteWriteActorHandle::from_pool(pool)
    }
}

impl SqliteWriteActorHandle {
    /// Spawns a dedicated SQLite Write Actor for an existing [`SqlitePool`] (default capacity `256`).
    pub fn from_pool(pool: &SqlitePool) -> Result<Self, SqliteActorError> {
        Self::spawn_with_capacity(pool.db_path().to_path_buf(), DEFAULT_WRITE_ACTOR_CAPACITY)
    }

    /// Spawns a dedicated SQLite Write Actor for an existing [`SqlitePool`] with custom bounded capacity.
    pub fn from_pool_with_capacity(
        pool: &SqlitePool,
        capacity: usize,
    ) -> Result<Self, SqliteActorError> {
        Self::spawn_with_capacity(pool.db_path().to_path_buf(), capacity)
    }

    /// Spawns a dedicated SQLite Write Actor with default bounded capacity (`256`).
    pub fn spawn(db_path: impl Into<PathBuf>) -> Result<Self, SqliteActorError> {
        Self::spawn_with_capacity(db_path, DEFAULT_WRITE_ACTOR_CAPACITY)
    }

    /// Spawns a single OS thread owning one WAL-configured `rusqlite::Connection`.
    /// Fails fast if `capacity == 0`, if `db_path` is a symlink, or if opening/PRAGMA setup fails.
    pub fn spawn_with_capacity(
        db_path: impl Into<PathBuf>,
        capacity: usize,
    ) -> Result<Self, SqliteActorError> {
        if capacity == 0 {
            return Err(SqliteActorError::InvalidCapacity);
        }
        let db_path = db_path.into();
        verify_db_path_not_symlink(&db_path)?;

        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<WriteCommand>(capacity);
        let (init_tx, init_rx) = mpsc::sync_channel::<Result<(), SqliteActorError>>(1);
        let queue_depth = Arc::new(AtomicUsize::new(0));
        let alive = Arc::new(AtomicBool::new(true));
        let alive_thread = Arc::clone(&alive);
        let actor_db_path = db_path.clone();

        let join_handle = thread::Builder::new()
            .name("cowok-sqlite-write-actor".to_string())
            .spawn(move || {
                let _alive_guard = AliveResetGuard {
                    alive: alive_thread,
                };
                let mut conn = match open_actor_wal_connection(&actor_db_path) {
                    Ok(c) => {
                        let _ = init_tx.send(Ok(()));
                        c
                    }
                    Err(err) => {
                        let _ = init_tx.send(Err(err));
                        return;
                    }
                };

                while let Ok(cmd) = cmd_rx.recv() {
                    let WriteCommand { job, depth_guard } = cmd;
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        job(&mut conn, depth_guard);
                    }))
                    .is_err()
                        && !conn.is_autocommit()
                    {
                        let _ = conn.execute_batch("ROLLBACK;");
                    }
                }
            })
            .map_err(|e| {
                SqliteActorError::OpenFailed(format!("Failed spawning SQLite write actor: {e}"))
            })?;

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(ActorInner {
                    tx: Some(cmd_tx),
                    join_handle: Some(join_handle),
                    alive,
                    queue_depth,
                    capacity,
                    db_path,
                }),
            }),
            Ok(Err(err)) => {
                let _ = join_handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = join_handle.join();
                Err(SqliteActorError::ActorDisconnected)
            }
        }
    }

    /// Enqueues a write closure executed inside `TransactionBehavior::Immediate` without blocking.
    ///
    /// When the bounded queue is full, immediately returns `Err(SqliteActorError::Backpressure)`
    /// (`retry_after_secs: 1`) instead of blocking indefinitely, OOMing, or dropping writes.
    /// Callers MUST perform all disk/HTTP I/O strictly before enqueuing.
    pub fn try_enqueue_immediate<T, F>(&self, op: F) -> Result<WriteReceipt<T>, SqliteActorError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> Result<T, rusqlite::Error> + Send + 'static,
    {
        let sender = self
            .inner
            .tx
            .as_ref()
            .ok_or(SqliteActorError::ActorDisconnected)?;
        let (reply_tx, reply_rx) = mpsc::sync_channel::<Result<T, SqliteActorError>>(1);

        let job: BoxedWriteJob =
            Box::new(move |conn: &mut Connection, depth_guard: QueueDepthGuard| {
                let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
                        Ok(tx) => match op(&tx) {
                            Ok(value) => tx
                                .commit()
                                .map(|()| value)
                                .map_err(|e| SqliteActorError::TransactionFailed(e.to_string())),
                            Err(e) => Err(SqliteActorError::OperationFailed(e.to_string())),
                        },
                        Err(e) => Err(SqliteActorError::TransactionFailed(e.to_string())),
                    }
                })) {
                    Ok(res) => res,
                    Err(_) => {
                        if !conn.is_autocommit() {
                            let _ = conn.execute_batch("ROLLBACK;");
                        }
                        Err(SqliteActorError::OperationFailed(
                            "WRITE_ACTOR_PANICKED".to_string(),
                        ))
                    }
                };
                drop(depth_guard);
                let _ = reply_tx.send(outcome);
            });

        self.inner.queue_depth.fetch_add(1, Ordering::AcqRel);
        let cmd = WriteCommand {
            job,
            depth_guard: QueueDepthGuard {
                counter: Arc::clone(&self.inner.queue_depth),
            },
        };
        match sender.try_send(cmd) {
            Ok(()) => Ok(WriteReceipt { rx: reply_rx }),
            Err(mpsc::TrySendError::Full(_)) => Err(SqliteActorError::Backpressure {
                retry_after_secs: DEFAULT_BACKPRESSURE_RETRY_AFTER_SECS,
                queue_capacity: self.inner.capacity,
            }),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(SqliteActorError::ActorDisconnected),
        }
    }

    /// Synchronously enqueues and awaits a `TransactionBehavior::Immediate` write closure.
    pub fn execute_immediate<T, F>(&self, op: F) -> Result<T, SqliteActorError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> Result<T, rusqlite::Error> + Send + 'static,
    {
        self.try_enqueue_immediate(op)?.wait()
    }

    /// Returns `true` while the dedicated SQLite write actor thread and channel remain alive.
    pub fn is_alive(&self) -> bool {
        self.inner.tx.is_some()
            && self.inner.alive.load(Ordering::Acquire)
            && self
                .inner
                .join_handle
                .as_ref()
                .is_some_and(|h| !h.is_finished())
    }

    /// Current number of pending + in-flight write commands in the actor queue.
    pub fn queue_depth(&self) -> usize {
        self.inner.queue_depth.load(Ordering::Acquire)
    }

    /// Configured bounded channel capacity of this write actor.
    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    /// Database file path owned by this write actor.
    pub fn db_path(&self) -> &Path {
        &self.inner.db_path
    }
}

fn verify_db_path_not_symlink(db_path: &Path) -> Result<(), SqliteActorError> {
    if let Ok(meta) = std::fs::symlink_metadata(db_path) {
        if meta.file_type().is_symlink() {
            return Err(SqliteActorError::SymlinkDenied(
                db_path.display().to_string(),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::suspicious_open_options)]
fn open_actor_wal_connection(db_path: &Path) -> Result<Connection, SqliteActorError> {
    verify_db_path_not_symlink(db_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let _probe = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(db_path)
            .map_err(|e| {
                if e.raw_os_error() == Some(libc::ELOOP) {
                    SqliteActorError::SymlinkDenied(db_path.display().to_string())
                } else {
                    SqliteActorError::OpenFailed(format!(
                        "Failed opening '{}' with O_NOFOLLOW: {e}",
                        db_path.display()
                    ))
                }
            })?;
    }

    let conn = Connection::open(db_path).map_err(|e| {
        SqliteActorError::OpenFailed(format!(
            "Connection::open('{}') failed: {e}",
            db_path.display()
        ))
    })?;

    conn.busy_timeout(Duration::from_millis(u64::from(
        DEFAULT_WRITE_ACTOR_BUSY_TIMEOUT_MS,
    )))
    .map_err(|e| SqliteActorError::OpenFailed(format!("busy_timeout failed: {e}")))?;

    let pragma_sql = format!(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = {DEFAULT_WRITE_ACTOR_BUSY_TIMEOUT_MS};
         PRAGMA foreign_keys = ON;"
    );
    conn.execute_batch(&pragma_sql)
        .map_err(|e| SqliteActorError::OpenFailed(format!("PRAGMA batch failed: {e}")))?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn single_actor_handles_parallel_threads_with_zero_busy_and_clean_shutdown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("actor-unit.db");
        let actor = SqliteWriteActor::spawn(&db_path).expect("spawn");

        actor
            .execute_immediate(|tx| {
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, val TEXT NOT NULL)",
                    [],
                )?;
                Ok(())
            })
            .expect("create table");

        let mut threads = Vec::new();
        for idx in 0..24usize {
            let handle = actor.clone();
            threads.push(thread::spawn(move || {
                handle.execute_immediate(move |tx| {
                    tx.execute(
                        "INSERT INTO items (id, val) VALUES (?1, ?2)",
                        rusqlite::params![idx as i64, format!("v-{idx}")],
                    )?;
                    Ok(())
                })
            }));
        }

        for t in threads {
            t.join().expect("thread join").expect("write ok");
        }

        let count: i64 = actor
            .execute_immediate(|tx| tx.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)))
            .expect("count");
        assert_eq!(count, 24);
        assert_eq!(actor.queue_depth(), 0);
    }

    #[test]
    fn bounded_actor_returns_backpressure_deterministically_when_saturated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("actor-bp.db");
        let actor = SqliteWriteActor::spawn_with_capacity(&db_path, 1).expect("spawn cap=1");

        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));

        let entered_c = Arc::clone(&entered);
        let release_c = Arc::clone(&release);
        let r1 = actor
            .try_enqueue_immediate(move |_tx| {
                entered_c.wait();
                release_c.wait();
                Ok(1usize)
            })
            .expect("first job accepted");

        entered.wait();

        // Queue capacity = 1: second job fills the bounded buffer; third MUST fail with Backpressure.
        let r2 = actor
            .try_enqueue_immediate(|_tx| Ok(2usize))
            .expect("second job fits in capacity=1 buffer");

        let third = actor.try_enqueue_immediate(|_tx| Ok(3usize));
        assert!(matches!(
            third,
            Err(SqliteActorError::Backpressure {
                retry_after_secs: 1,
                queue_capacity: 1
            })
        ));

        release.wait();
        assert_eq!(r1.wait().expect("r1"), 1);
        assert_eq!(r2.wait().expect("r2"), 2);
        assert_eq!(actor.queue_depth(), 0);
    }

    #[test]
    fn actor_survives_closure_panic_rolls_back_and_stays_alive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("actor-panic.db");
        let actor = SqliteWriteActor::spawn(&db_path).expect("spawn");
        assert!(actor.is_alive());

        actor
            .execute_immediate(|tx| {
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, val TEXT NOT NULL)",
                    [],
                )?;
                Ok(())
            })
            .expect("create table");

        let panic_res: Result<(), SqliteActorError> = actor.execute_immediate(|tx| {
            tx.execute(
                "INSERT INTO items (id, val) VALUES (1, 'must-rollback')",
                [],
            )?;
            panic!("simulated write closure panic");
        });
        assert_eq!(
            panic_res,
            Err(SqliteActorError::OperationFailed(
                "WRITE_ACTOR_PANICKED".to_string()
            ))
        );
        assert!(actor.is_alive());
        assert_eq!(actor.queue_depth(), 0);

        actor
            .execute_immediate(|tx| {
                tx.execute("INSERT INTO items (id, val) VALUES (2, 'after-panic')", [])?;
                Ok(())
            })
            .expect("subsequent write succeeds");

        let rows: Vec<(i64, String)> = actor
            .execute_immediate(|tx| {
                let mut stmt = tx.prepare("SELECT id, val FROM items ORDER BY id ASC")?;
                let iter = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
                let mut out = Vec::new();
                for row in iter {
                    out.push(row?);
                }
                Ok(out)
            })
            .expect("query items");
        assert_eq!(rows, vec![(2, "after-panic".to_string())]);
        assert_eq!(actor.queue_depth(), 0);
    }
}
