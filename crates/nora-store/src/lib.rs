//! Local durable state for a single trusted NORA supervisor.
//!
//! SQLite transactions prevent conflicting transitions. They do not make an
//! external API exactly-once, authenticate callers, or protect files from the OS
//! account. Recovery must run after the supervisor has acquired exclusive app
//! ownership; merely opening another database connection never recovers effects.

use std::{
    io::{self, Write},
    path::Path,
    time::Duration,
};

use nora_core::notifications::DeliveryState;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const APPLICATION_ID: i64 = 0x4e4f_5241;
const MAX_CHECKPOINT_BYTES: usize = 262_144;
const MAX_RECEIPT_BYTES: usize = 16_384;
const MAX_DATABASE_BYTES: i64 = 64_000_000;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid {0}")]
    Invalid(&'static str),
    #[error("database belongs to another application or schema version")]
    ForeignDatabase,
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("revision, identity or state conflict")]
    Conflict,
    #[error("transition is not permitted")]
    Transition,
    #[error("goal still has unverified effects")]
    UnverifiedEffects,
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Queued,
    Running,
    WaitingInput,
    WaitingPermission,
    WaitingApproval,
    Verifying,
    NeedsReconciliation,
    Completed,
    Failed,
    Cancelled,
}

impl GoalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::WaitingInput => "waiting_input",
            Self::WaitingPermission => "waiting_permission",
            Self::WaitingApproval => "waiting_approval",
            Self::Verifying => "verifying",
            Self::NeedsReconciliation => "needs_reconciliation",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn permits(self, next: Self) -> bool {
        use GoalStatus::*;
        match self {
            Queued => matches!(next, Running | Cancelled | Failed),
            Running => matches!(
                next,
                WaitingInput
                    | WaitingPermission
                    | WaitingApproval
                    | Verifying
                    | NeedsReconciliation
                    | Cancelled
                    | Failed
            ),
            WaitingInput | WaitingPermission | WaitingApproval => {
                matches!(next, Running | Cancelled | Failed)
            }
            Verifying => matches!(
                next,
                Completed | Running | NeedsReconciliation | Cancelled | Failed
            ),
            NeedsReconciliation => matches!(next, Running | Verifying | Cancelled | Failed),
            Completed | Cancelled | Failed => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoalRecord {
    pub id: String,
    pub objective: String,
    pub revision: i64,
    pub status: GoalStatus,
    pub checkpoint: serde_json::Value,
    pub created_ms: i64,
    pub updated_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStatus {
    Prepared,
    InFlight,
    NeedsReconciliation,
    Verified,
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRecord {
    pub id: String,
    pub goal_id: String,
    pub goal_revision: i64,
    pub fingerprint: String,
    pub status: EffectStatus,
    pub receipt: Option<String>,
    pub created_ms: i64,
    pub updated_ms: i64,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::configure(Connection::open(path)?)
    }

    pub fn memory() -> Result<Self> {
        Self::configure(Connection::open_in_memory()?)
    }

    fn configure(mut connection: Connection) -> Result<Self> {
        let app: i64 = connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if !matches!(app, 0 | APPLICATION_ID) || !(0..=1).contains(&version) {
            return Err(StoreError::ForeignDatabase);
        }
        if app == 0 {
            if version != 0 {
                return Err(StoreError::ForeignDatabase);
            }
            let foreign: i64 = connection.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' AND NOT ((type='table' AND name='deliveries') OR (type='index' AND tbl_name='deliveries'))",
                [], |r| r.get(0),
            )?;
            if foreign != 0 {
                return Err(StoreError::ForeignDatabase);
            }
            let legacy:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='deliveries')",[],|r|r.get(0))?;
            if legacy && !compatible_legacy_deliveries(&connection)? {
                return Err(StoreError::ForeignDatabase);
            }
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "cache_size", -2048)?;
        connection.pragma_update(None, "journal_size_limit", 8_000_000)?;
        let page_size: i64 = connection.pragma_query_value(None, "page_size", |r| r.get(0))?;
        if page_size <= 0 || page_size > MAX_DATABASE_BYTES {
            return Err(StoreError::ForeignDatabase);
        }
        connection.pragma_update(None, "max_page_count", MAX_DATABASE_BYTES / page_size)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS deliveries (
                goal_id TEXT NOT NULL, notice_key TEXT NOT NULL,
                channel TEXT NOT NULL CHECK(channel IN ('surface','speech')),
                delivered_at REAL NOT NULL, PRIMARY KEY(goal_id,notice_key,channel));
             CREATE TABLE IF NOT EXISTS goals (
                id TEXT PRIMARY KEY, objective TEXT NOT NULL, revision INTEGER NOT NULL,
                status TEXT NOT NULL, checkpoint TEXT NOT NULL,
                created_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS effects (
                id TEXT PRIMARY KEY, goal_id TEXT NOT NULL REFERENCES goals(id),
                goal_revision INTEGER NOT NULL, fingerprint TEXT NOT NULL,
                status TEXT NOT NULL, receipt TEXT,
                created_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL);
             CREATE INDEX IF NOT EXISTS effects_goal ON effects(goal_id,status);",
        )?;
        tx.pragma_update(None, "application_id", APPLICATION_ID)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
        Ok(Self { connection })
    }

    pub fn create_goal(&self, id: &str, objective: &str, now_ms: i64) -> Result<GoalRecord> {
        identifier(id)?;
        nonempty(objective, 4096, "objective")?;
        timestamp(now_ms)?;
        self.connection.execute(
            "INSERT INTO goals VALUES (?1,?2,0,'queued','{}',?3,?3)",
            params![id, objective, now_ms],
        )?;
        self.goal(id)
    }

    pub fn goal(&self, id: &str) -> Result<GoalRecord> {
        identifier(id)?;
        read_goal(&self.connection, id)
    }

    pub fn transition_goal(
        &mut self,
        id: &str,
        revision: i64,
        next: GoalStatus,
        now_ms: i64,
    ) -> Result<GoalRecord> {
        identifier(id)?;
        timestamp(now_ms)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_goal(&tx, id)?;
        if current.revision != revision || now_ms < current.updated_ms {
            return Err(StoreError::Conflict);
        }
        if !current.status.permits(next) {
            return Err(StoreError::Transition);
        }
        if next == GoalStatus::Completed {
            let pending: i64 = tx.query_row("SELECT COUNT(*) FROM effects WHERE goal_id=?1 AND status NOT IN ('verified','abandoned')", [id], |r|r.get(0))?;
            if pending != 0 {
                return Err(StoreError::UnverifiedEffects);
            }
        }
        let new_revision = revision.checked_add(1).ok_or(StoreError::Conflict)?;
        tx.execute(
            "UPDATE goals SET status=?1,revision=?2,updated_ms=?3 WHERE id=?4",
            params![next.as_str(), new_revision, now_ms, id],
        )?;
        let result = read_goal(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn checkpoint(
        &mut self,
        id: &str,
        revision: i64,
        checkpoint: &serde_json::Value,
        now_ms: i64,
    ) -> Result<GoalRecord> {
        identifier(id)?;
        timestamp(now_ms)?;
        if !checkpoint.is_object() {
            return Err(StoreError::Invalid("checkpoint object"));
        }
        let mut writer = BoundedJson::new(MAX_CHECKPOINT_BYTES);
        serde_json::to_writer(&mut writer, checkpoint)?;
        let encoded = writer.bytes;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = read_goal(&tx, id)?;
        if current.revision != revision || now_ms < current.updated_ms {
            return Err(StoreError::Conflict);
        }
        if matches!(
            current.status,
            GoalStatus::Completed | GoalStatus::Failed | GoalStatus::Cancelled
        ) {
            return Err(StoreError::Transition);
        }
        let next = revision.checked_add(1).ok_or(StoreError::Conflict)?;
        let text = std::str::from_utf8(&encoded)
            .map_err(|_| StoreError::Invalid("checkpoint encoding"))?;
        tx.execute(
            "UPDATE goals SET checkpoint=?1,revision=?2,updated_ms=?3 WHERE id=?4",
            params![text, next, now_ms, id],
        )?;
        let result = read_goal(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }

    /// Prepare a bound effect. An existing ID never authorizes another dispatch.
    pub fn prepare_effect(
        &mut self,
        id: &str,
        goal_id: &str,
        revision: i64,
        fingerprint: &str,
        now_ms: i64,
    ) -> Result<EffectRecord> {
        identifier(id)?;
        identifier(goal_id)?;
        timestamp(now_ms)?;
        if fingerprint.len() != 64 || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(StoreError::Invalid("SHA256 fingerprint"));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = read_effect(&tx, id)? {
            if existing.goal_id != goal_id
                || existing.goal_revision != revision
                || existing.fingerprint != fingerprint
            {
                return Err(StoreError::Conflict);
            }
            return Ok(existing);
        }
        let goal = read_goal(&tx, goal_id)?;
        if goal.revision != revision
            || goal.status != GoalStatus::Running
            || now_ms < goal.updated_ms
        {
            return Err(StoreError::Conflict);
        }
        tx.execute(
            "INSERT INTO effects VALUES (?1,?2,?3,?4,'prepared',NULL,?5,?5)",
            params![id, goal_id, revision, fingerprint, now_ms],
        )?;
        let result = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        tx.commit()?;
        Ok(result)
    }

    pub fn effect(&self, id: &str) -> Result<EffectRecord> {
        identifier(id)?;
        read_effect(&self.connection, id)?.ok_or_else(|| StoreError::NotFound(id.into()))
    }

    /// Retire a proposal that was never dispatched, for example after replan.
    /// An in-flight/unknown effect must be reconciled, never abandoned this way.
    pub fn abandon_prepared(
        &mut self,
        id: &str,
        reason: &str,
        now_ms: i64,
    ) -> Result<EffectRecord> {
        identifier(id)?;
        nonempty(reason, MAX_RECEIPT_BYTES, "abandon reason")?;
        timestamp(now_ms)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let effect = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        if effect.status != EffectStatus::Prepared || now_ms < effect.updated_ms {
            return Err(StoreError::Transition);
        }
        tx.execute(
            "UPDATE effects SET status='abandoned',receipt=?1,updated_ms=?2 WHERE id=?3",
            params![reason, now_ms, id],
        )?;
        let result = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        tx.commit()?;
        Ok(result)
    }

    /// Atomically claim a prepared effect after rechecking goal revision/state.
    pub fn begin_effect(&mut self, id: &str, now_ms: i64) -> Result<EffectRecord> {
        identifier(id)?;
        timestamp(now_ms)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let effect = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        let goal = read_goal(&tx, &effect.goal_id)?;
        if effect.status != EffectStatus::Prepared
            || goal.status != GoalStatus::Running
            || goal.revision != effect.goal_revision
            || now_ms < effect.updated_ms
            || now_ms < goal.updated_ms
        {
            return Err(StoreError::Conflict);
        }
        tx.execute(
            "UPDATE effects SET status='in_flight',updated_ms=?1 WHERE id=?2",
            params![now_ms, id],
        )?;
        let result = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        tx.commit()?;
        Ok(result)
    }

    /// Record readback evidence supplied by the trusted adapter, not a model.
    pub fn verify_effect(&mut self, id: &str, receipt: &str, now_ms: i64) -> Result<EffectRecord> {
        identifier(id)?;
        nonempty(receipt, MAX_RECEIPT_BYTES, "receipt")?;
        timestamp(now_ms)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let effect = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        if effect.status == EffectStatus::Verified && effect.receipt.as_deref() == Some(receipt) {
            return Ok(effect);
        }
        if !matches!(
            effect.status,
            EffectStatus::InFlight | EffectStatus::NeedsReconciliation
        ) || now_ms < effect.updated_ms
        {
            return Err(StoreError::Transition);
        }
        tx.execute(
            "UPDATE effects SET status='verified',receipt=?1,updated_ms=?2 WHERE id=?3",
            params![receipt, now_ms, id],
        )?;
        let result = read_effect(&tx, id)?.ok_or_else(|| StoreError::NotFound(id.into()))?;
        tx.commit()?;
        Ok(result)
    }

    /// Call only after obtaining exclusive supervisor ownership following restart.
    /// Unknown effects are never automatically put back into the dispatch queue.
    pub fn recover_interrupted(&mut self, now_ms: i64) -> Result<usize> {
        timestamp(now_ms)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let future: i64 = tx.query_row(
            "SELECT COUNT(*) FROM effects WHERE status='in_flight' AND updated_ms>?1",
            [now_ms],
            |r| r.get(0),
        )?;
        if future != 0 {
            return Err(StoreError::Conflict);
        }
        let invalid_goals:i64=tx.query_row("SELECT COUNT(*) FROM goals WHERE id IN (SELECT goal_id FROM effects WHERE status='in_flight') AND (updated_ms>?1 OR revision=9223372036854775807)",[now_ms],|r|r.get(0))?;
        if invalid_goals != 0 {
            return Err(StoreError::Conflict);
        }
        tx.execute("UPDATE goals SET status='needs_reconciliation',revision=revision+1,updated_ms=?1 WHERE status IN ('running','verifying') AND id IN (SELECT goal_id FROM effects WHERE status='in_flight')",[now_ms])?;
        let count=tx.execute("UPDATE effects SET status='needs_reconciliation',updated_ms=?1 WHERE status='in_flight'",[now_ms])?;
        tx.commit()?;
        Ok(count)
    }

    pub fn delivery_state(&self, goal_id: &str, notice_key: &str) -> Result<DeliveryState> {
        identifier(goal_id)?;
        nonempty(notice_key, 1024, "notice key")?;
        let shown:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM deliveries WHERE goal_id=?1 AND notice_key=?2 AND channel='surface')",params![goal_id,notice_key],|r|r.get(0))?;
        let spoken:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM deliveries WHERE goal_id=?1 AND notice_key=?2 AND channel='speech')",params![goal_id,notice_key],|r|r.get(0))?;
        let last: Option<f64> = self.connection.query_row(
            "SELECT MAX(delivered_at) FROM deliveries WHERE goal_id=?1 AND channel='speech'",
            [goal_id],
            |r| r.get(0),
        )?;
        Ok(DeliveryState {
            surface_delivered: shown,
            speech_delivered: spoken,
            last_speech: last,
        })
    }

    pub fn acknowledge(
        &self,
        goal_id: &str,
        notice_key: &str,
        channel: &str,
        at: f64,
    ) -> Result<()> {
        identifier(goal_id)?;
        nonempty(notice_key, 1024, "notice key")?;
        if !matches!(channel, "surface" | "speech") || !at.is_finite() || at < 0.0 {
            return Err(StoreError::Invalid("delivery channel/time"));
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO deliveries VALUES (?1,?2,?3,?4)",
            params![goal_id, notice_key, channel, at],
        )?;
        Ok(())
    }
}

struct BoundedJson {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedJson {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
}

impl Write for BoundedJson {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::other("checkpoint length overflow"))?;
        if length > self.limit {
            return Err(io::Error::other("serialized checkpoint size limit"));
        }
        self.bytes
            .try_reserve_exact(input.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn compatible_legacy_deliveries(connection: &Connection) -> Result<bool> {
    let mut statement = connection.prepare("PRAGMA table_info(deliveries)")?;
    let mut rows = statement.query([])?;
    let expected = [
        ("goal_id", "TEXT", 1i64),
        ("notice_key", "TEXT", 2),
        ("channel", "TEXT", 3),
        ("delivered_at", "REAL", 0),
    ];
    for (name, kind, pk) in expected {
        let Some(row) = rows.next()? else {
            return Ok(false);
        };
        if row.get::<_, String>(1)? != name
            || row.get::<_, String>(2)?.to_ascii_uppercase() != kind
            || row.get::<_, i64>(3)? != 1
            || row.get::<_, i64>(5)? != pk
        {
            return Ok(false);
        }
    }
    if rows.next()?.is_some() {
        return Ok(false);
    }
    let sql: String = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='deliveries'",
        [],
        |r| r.get(0),
    )?;
    let normalized: String = sql
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    Ok(normalized.contains("check(channelin('surface','speech'))"))
}

fn identifier(value: &str) -> Result<()> {
    nonempty(value, 256, "identifier")
}
fn nonempty(value: &str, max: usize, name: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.contains('\0') {
        Err(StoreError::Invalid(name))
    } else {
        Ok(())
    }
}
fn timestamp(value: i64) -> Result<()> {
    if value < 0 {
        Err(StoreError::Invalid("timestamp"))
    } else {
        Ok(())
    }
}

fn read_goal(connection: &Connection, id: &str) -> Result<GoalRecord> {
    let row=connection.query_row("SELECT id,objective,revision,status,checkpoint,created_ms,updated_ms FROM goals WHERE id=?1",[id],|r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?))
    }).optional()?.ok_or_else(||StoreError::NotFound(id.into()))?;
    if row.4.len() > MAX_CHECKPOINT_BYTES {
        return Err(StoreError::Invalid("stored checkpoint size"));
    }
    Ok(GoalRecord {
        id: row.0,
        objective: row.1,
        revision: row.2,
        status: serde_json::from_value(serde_json::Value::String(row.3))?,
        checkpoint: serde_json::from_str(&row.4)?,
        created_ms: row.5,
        updated_ms: row.6,
    })
}

fn read_effect(connection: &Connection, id: &str) -> Result<Option<EffectRecord>> {
    let row=connection.query_row("SELECT id,goal_id,goal_revision,fingerprint,status,receipt,created_ms,updated_ms FROM effects WHERE id=?1",[id],|r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,i64>(6)?,r.get::<_,i64>(7)?))
    }).optional()?;
    row.map(|r| {
        Ok(EffectRecord {
            id: r.0,
            goal_id: r.1,
            goal_revision: r.2,
            fingerprint: r.3,
            status: serde_json::from_value(serde_json::Value::String(r.4))?,
            receipt: r.5,
            created_ms: r.6,
            updated_ms: r.7,
        })
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn running(store: &mut Store) -> GoalRecord {
        store
            .create_goal("goal", "Tạo bản nháp trong thư mục được cấp quyền", 0)
            .unwrap();
        store
            .transition_goal("goal", 0, GoalStatus::Running, 1)
            .unwrap()
    }

    #[test]
    fn notification_delivery_survives_reopen_and_is_goal_scoped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.sqlite");
        {
            let s = Store::open(&path).unwrap();
            s.acknowledge("a", "finding:1", "speech", 10.0).unwrap();
            s.acknowledge("a", "finding:1", "speech", 11.0).unwrap();
        }
        let s = Store::open(path).unwrap();
        let a = s.delivery_state("a", "finding:1").unwrap();
        assert!(a.speech_delivered);
        assert!(!a.surface_delivered);
        assert_eq!(a.last_speech, Some(10.0));
        assert!(!s.delivery_state("b", "finding:1").unwrap().speech_delivered);
    }

    #[test]
    fn checkpoint_and_transition_reject_stale_revisions() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        let g = s.checkpoint("goal", 1, &json!({"step":2}), 2).unwrap();
        assert_eq!(g.revision, 2);
        assert!(matches!(
            s.transition_goal("goal", 1, GoalStatus::Verifying, 3),
            Err(StoreError::Conflict)
        ));
        assert_eq!(s.goal("goal").unwrap().checkpoint, json!({"step":2}));
    }

    #[test]
    fn effect_identity_cannot_change_and_dispatch_is_claimed_once() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        let digest = "a".repeat(64);
        let a = s.prepare_effect("effect", "goal", 1, &digest, 2).unwrap();
        assert_eq!(
            a,
            s.prepare_effect("effect", "goal", 1, &digest, 2).unwrap()
        );
        assert!(
            s.prepare_effect("effect", "goal", 1, &"b".repeat(64), 2)
                .is_err()
        );
        s.begin_effect("effect", 3).unwrap();
        assert!(s.begin_effect("effect", 4).is_err());
    }

    #[test]
    fn cancellation_invalidates_prepared_dispatch() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        s.prepare_effect("e", "goal", 1, &"a".repeat(64), 2)
            .unwrap();
        s.transition_goal("goal", 1, GoalStatus::Cancelled, 3)
            .unwrap();
        assert!(s.begin_effect("e", 4).is_err());
        assert_eq!(s.effect("e").unwrap().status, EffectStatus::Prepared);
    }

    #[test]
    fn restart_requires_reconciliation_never_blind_retry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.sqlite");
        {
            let mut s = Store::open(&path).unwrap();
            running(&mut s);
            s.prepare_effect("e", "goal", 1, &"a".repeat(64), 2)
                .unwrap();
            s.begin_effect("e", 3).unwrap();
        }
        let mut s = Store::open(&path).unwrap();
        assert_eq!(s.effect("e").unwrap().status, EffectStatus::InFlight);
        assert_eq!(s.recover_interrupted(4).unwrap(), 1);
        assert!(s.begin_effect("e", 5).is_err());
        assert_eq!(
            s.goal("goal").unwrap().status,
            GoalStatus::NeedsReconciliation
        );
        s.verify_effect("e", "provider-readback:receipt-123", 5)
            .unwrap();
        let g = s.goal("goal").unwrap();
        let g = s
            .transition_goal("goal", g.revision, GoalStatus::Verifying, 6)
            .unwrap();
        s.transition_goal("goal", g.revision, GoalStatus::Completed, 7)
            .unwrap();
    }

    #[test]
    fn cannot_complete_while_effect_is_unverified() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        s.prepare_effect("e", "goal", 1, &"a".repeat(64), 2)
            .unwrap();
        let g = s
            .transition_goal("goal", 1, GoalStatus::Verifying, 3)
            .unwrap();
        assert!(matches!(
            s.transition_goal("goal", g.revision, GoalStatus::Completed, 4),
            Err(StoreError::UnverifiedEffects)
        ));
    }

    #[test]
    fn foreign_database_is_not_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("foreign.sqlite");
        let c = Connection::open(&path).unwrap();
        c.execute("CREATE TABLE user_data(secret TEXT)", [])
            .unwrap();
        drop(c);
        assert!(matches!(
            Store::open(&path),
            Err(StoreError::ForeignDatabase)
        ));
        let c = Connection::open(path).unwrap();
        let app: i64 = c
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap();
        assert_eq!(app, 0);
    }

    #[test]
    fn invalid_values_and_oversized_checkpoint_are_rejected() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        assert!(s.acknowledge("g", "k", "speech", f64::NAN).is_err());
        assert!(s.acknowledge("g", "k", "unknown", 0.0).is_err());
        assert!(
            s.checkpoint(
                "goal",
                1,
                &json!({"large":"x".repeat(MAX_CHECKPOINT_BYTES)}),
                2
            )
            .is_err()
        );
        assert!(s.prepare_effect("e", "goal", 1, "not-a-digest", 2).is_err());
        assert!(
            s.transition_goal("goal", 1, GoalStatus::Completed, 2)
                .is_err()
        );
    }
    #[test]
    fn replan_retires_only_undispatched_effect_and_can_complete() {
        let mut s = Store::memory().unwrap();
        running(&mut s);
        s.prepare_effect("old", "goal", 1, &"a".repeat(64), 2)
            .unwrap();
        let g = s.checkpoint("goal", 1, &json!({"replan":true}), 3).unwrap();
        assert!(s.begin_effect("old", 4).is_err());
        s.abandon_prepared("old", "superseded before dispatch", 4)
            .unwrap();
        s.prepare_effect("new", "goal", g.revision, &"b".repeat(64), 5)
            .unwrap();
        s.begin_effect("new", 6).unwrap();
        assert!(
            s.abandon_prepared("new", "cannot discard uncertainty", 7)
                .is_err()
        );
        s.verify_effect("new", "readback:new", 7).unwrap();
        let g = s
            .transition_goal("goal", g.revision, GoalStatus::Verifying, 8)
            .unwrap();
        s.transition_goal("goal", g.revision, GoalStatus::Completed, 9)
            .unwrap();
    }
    #[test]
    fn checkpoint_writer_rejects_without_buffering_large_value() {
        let value = json!({"large":"x".repeat(1_000_000)});
        let mut writer = BoundedJson::new(32);
        assert!(serde_json::to_writer(&mut writer, &value).is_err());
        assert!(writer.bytes.len() <= 32);
        assert!(writer.bytes.capacity() <= 32);
    }
    #[test]
    fn foreign_deliveries_only_database_is_not_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("foreign.sqlite");
        let c = Connection::open(&path).unwrap();
        c.execute("CREATE TABLE deliveries(unrelated TEXT)", [])
            .unwrap();
        drop(c);
        assert!(matches!(
            Store::open(&path),
            Err(StoreError::ForeignDatabase)
        ));
        let c = Connection::open(path).unwrap();
        let app: i64 = c
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap();
        assert_eq!(app, 0);
        let goals: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='goals'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(goals, 0);
    }
    #[test]
    fn known_python_ledger_schema_is_compatible() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.sqlite");
        let c = Connection::open(&path).unwrap();
        c.execute("CREATE TABLE IF NOT EXISTS deliveries (goal_id TEXT NOT NULL, notice_key TEXT NOT NULL, channel TEXT NOT NULL CHECK(channel IN ('surface','speech')), delivered_at REAL NOT NULL, PRIMARY KEY(goal_id,notice_key,channel))",[]).unwrap();
        c.execute("INSERT INTO deliveries VALUES ('g','k','speech',12)", [])
            .unwrap();
        drop(c);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.delivery_state("g", "k").unwrap().last_speech, Some(12.0));
    }
    #[test]
    fn foreign_view_or_legacy_trigger_is_not_adopted() {
        for schema in [
            "CREATE VIEW user_owned_view AS SELECT 42",
            "CREATE TABLE deliveries (goal_id TEXT NOT NULL, notice_key TEXT NOT NULL, channel TEXT NOT NULL CHECK(channel IN ('surface','speech')), delivered_at REAL NOT NULL, PRIMARY KEY(goal_id,notice_key,channel)); CREATE TRIGGER user_owned_trigger AFTER INSERT ON deliveries BEGIN SELECT 42; END",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("foreign.sqlite");
            let c = Connection::open(&path).unwrap();
            c.execute_batch(schema).unwrap();
            let before: i64 = c
                .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
                .unwrap();
            drop(c);
            assert!(matches!(
                Store::open(&path),
                Err(StoreError::ForeignDatabase)
            ));
            let c = Connection::open(path).unwrap();
            let app: i64 = c
                .pragma_query_value(None, "application_id", |r| r.get(0))
                .unwrap();
            let after: i64 = c
                .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get(0))
                .unwrap();
            assert_eq!(app, 0);
            assert_eq!(after, before);
        }
    }
}
