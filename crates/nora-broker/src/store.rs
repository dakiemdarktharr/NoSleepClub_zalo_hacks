use crate::{
    ActionPreview, AppGrant, ApprovalToken, ApprovedLaunch, BrokerError, Disclosure,
    ExecutionContext, Reminder, Result, RootGrant, RootHandle, RootHandles, RootPermissions,
    ToolAction, ToolReceipt, arithmetic, bounded_text, digest, files, fingerprint, fresh_id,
    identifier,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use std::{
    fs::{self, File},
    path::{Component, Path, PathBuf},
    time::Duration,
};

const APPLICATION_ID: i64 = 0x4e42_524b;
const MAX_ROOTS: i64 = 32;
const MAX_APPS: i64 = 32;
const MAX_PENDING: i64 = 128;
const MAX_REMINDERS: i64 = 256;
const PREVIEW_TTL_MS: i64 = 120_000;

/// Own this value in one trusted supervisor. The lock prevents two live broker
/// instances from independently authorizing/consuming effects in one state dir.
pub struct Broker {
    connection: Connection,
    roots: RootHandles,
    protected_paths: Vec<PathBuf>,
    _ownership: File,
}

struct Prepared {
    action: ToolAction,
    goal_id: String,
    session_id: String,
    grant_revision: u64,
    fingerprint: String,
    created_ms: i64,
    expires_ms: i64,
    status: String,
    nonce_hash: Option<String>,
    disclose_to_llm: bool,
}

impl Broker {
    pub fn open(state_dir: impl AsRef<Path>) -> Result<Self> {
        let state_dir = state_dir.as_ref();
        if state_dir.exists() && fs::symlink_metadata(state_dir)?.file_type().is_symlink() {
            return Err(BrokerError::ForeignState);
        }
        fs::create_dir_all(state_dir)?;
        let lock_path = state_dir.join(".nora-broker.lock");
        if lock_path.exists() && fs::symlink_metadata(&lock_path)?.file_type().is_symlink() {
            return Err(BrokerError::ForeignState);
        }
        let ownership = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        ownership.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => BrokerError::AlreadyOwned,
            std::fs::TryLockError::Error(error) => BrokerError::Io(error),
        })?;
        let database = state_dir.join("broker.sqlite");
        if database.exists() && fs::symlink_metadata(&database)?.file_type().is_symlink() {
            return Err(BrokerError::ForeignState);
        }
        if database.exists() && fs::metadata(&database)?.len() > 32_000_000 {
            return Err(BrokerError::ForeignState);
        }
        let connection = Connection::open(database)?;
        let app: i64 = connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )?;
        if !((app == 0 && version == 0 && count == 0) || (app == APPLICATION_ID && version == 1)) {
            return Err(BrokerError::ForeignState);
        }
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "cache_size", -1024)?;
        connection.pragma_update(None, "journal_size_limit", 4_000_000)?;
        let page: i64 = connection.pragma_query_value(None, "page_size", |r| r.get(0))?;
        if page <= 0 || page > 16_000_000 {
            return Err(BrokerError::ForeignState);
        }
        connection.pragma_update(None, "max_page_count", 16_000_000 / page)?;
        if app == 0 {
            connection.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS roots(id TEXT PRIMARY KEY, json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS apps(id TEXT PRIMARY KEY, json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS previews(id TEXT PRIMARY KEY, action TEXT NOT NULL, goal_id TEXT NOT NULL, session_id TEXT NOT NULL,
                grant_revision INTEGER NOT NULL, fingerprint TEXT NOT NULL, created_ms INTEGER NOT NULL, expires_ms INTEGER NOT NULL, status TEXT NOT NULL,
                nonce_hash TEXT, disclose_to_llm INTEGER NOT NULL CHECK(disclose_to_llm IN (0,1)), receipt TEXT);
            CREATE TABLE IF NOT EXISTS reminders(id TEXT PRIMARY KEY, goal_id TEXT NOT NULL, text TEXT NOT NULL,
                due_at_ms INTEGER NOT NULL, dismissed INTEGER NOT NULL CHECK(dismissed IN (0,1)), cloud_disclosure INTEGER NOT NULL CHECK(cloud_disclosure IN (0,1)));
            ")?;
            connection.pragma_update(None, "application_id", APPLICATION_ID)?;
            connection.pragma_update(None, "user_version", 1)?;
            connection.execute_batch("COMMIT;")?;
        }
        validate_schema(&connection)?;
        // In-flight writes are never retried on open. Readback reconciliation
        // requires an explicit trusted runtime/UI request and current grants.
        connection.execute(
            "UPDATE previews SET status='uncertain' WHERE status='dispatched'",
            [],
        )?;
        let mut broker = Self {
            connection,
            roots: RootHandles::new(),
            protected_paths: vec![fs::canonicalize(state_dir)?],
            _ownership: ownership,
        };
        for grant in broker.roots()? {
            if !grant.revoked
                && !broker.is_protected_path(&grant.path, false)
                && let Ok((canonical, dir)) = files::open_root(&grant.path)
                && canonical == grant.path
            {
                broker
                    .roots
                    .insert(grant.id.clone(), RootHandle { grant, dir });
            }
        }
        broker.apps()?;
        Ok(broker)
    }

    /// Trusted UI operation. A model must never be allowed to grant itself a root.
    pub fn grant_root(
        &mut self,
        label: &str,
        path: &Path,
        permissions: RootPermissions,
        now_ms: i64,
    ) -> Result<RootGrant> {
        bounded_text(label, 128, "root label")?;
        timestamp(now_ms)?;
        if !permissions.read && !permissions.create_new {
            return Err(BrokerError::Invalid("empty root permissions"));
        }
        let (canonical, dir) = files::open_root(path)?;
        if self.is_protected_path(&canonical, false) {
            return Err(BrokerError::PermissionDenied);
        }
        let current = self.roots()?.into_iter().find(|r| r.path == canonical);
        let (id, revision, created_ms) = if let Some(previous) = current {
            if now_ms < previous.updated_ms {
                return Err(BrokerError::Invalid("timestamp regression"));
            }
            (
                previous.id,
                previous
                    .revision
                    .checked_add(1)
                    .ok_or(BrokerError::Limit("grant revision"))?,
                previous.created_ms,
            )
        } else {
            let count: i64 = self
                .connection
                .query_row("SELECT COUNT(*) FROM roots", [], |r| r.get(0))?;
            if count >= MAX_ROOTS {
                return Err(BrokerError::Limit("root grants"));
            }
            (fresh_id("root_")?, 1, now_ms)
        };
        let grant = RootGrant {
            id,
            label: label.into(),
            path: canonical,
            permissions,
            revision,
            revoked: false,
            created_ms,
            updated_ms: now_ms,
        };
        self.connection.execute("INSERT INTO roots(id,json) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![grant.id,encode(&grant)?])?;
        self.roots.insert(
            grant.id.clone(),
            RootHandle {
                grant: grant.clone(),
                dir,
            },
        );
        Ok(grant)
    }

    pub fn roots(&self) -> Result<Vec<RootGrant>> {
        let roots: Vec<RootGrant> =
            self.records("SELECT id,json FROM roots ORDER BY id", MAX_ROOTS)?;
        for root in &roots {
            if root.revision == 0
                || root.created_ms < 0
                || root.updated_ms < root.created_ms
                || !root.path.is_absolute()
                || root.path.as_os_str().len() > 4096
                || (!root.permissions.read && !root.permissions.create_new)
            {
                return Err(BrokerError::ForeignState);
            }
            bounded_text(&root.label, 128, "root label").map_err(|_| BrokerError::ForeignState)?;
        }
        Ok(roots)
    }

    /// Trusted runtime startup policy: protect NORA state/config/catalog paths
    /// even when a user grants a containing project directory. No unprotect API.
    pub fn protect_path(&mut self, path: &Path) -> Result<()> {
        let canonical = fs::canonicalize(path)?;
        if self.protected_paths.contains(&canonical) {
            return Ok(());
        }
        if self.protected_paths.len() >= 64 {
            return Err(BrokerError::Limit("protected paths"));
        }
        self.protected_paths.push(canonical);
        Ok(())
    }
    pub fn apps(&self) -> Result<Vec<AppGrant>> {
        let apps: Vec<AppGrant> = self.records("SELECT id,json FROM apps ORDER BY id", MAX_APPS)?;
        for app in &apps {
            if app.revision == 0
                || app.created_ms < 0
                || app.updated_ms < app.created_ms
                || !app.executable.is_absolute()
                || app.executable.as_os_str().len() > 4096
                || app.sha256.len() != 64
                || !app.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(BrokerError::ForeignState);
            }
            bounded_text(&app.label, 128, "app label").map_err(|_| BrokerError::ForeignState)?;
        }
        Ok(apps)
    }

    pub fn revoke_root(&mut self, id: &str, now_ms: i64) -> Result<()> {
        identifier(id)?;
        timestamp(now_ms)?;
        let mut grant = self
            .roots()?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or(BrokerError::PermissionDenied)?;
        if now_ms < grant.updated_ms {
            return Err(BrokerError::Invalid("timestamp regression"));
        }
        grant.revoked = true;
        grant.updated_ms = now_ms;
        grant.revision = grant
            .revision
            .checked_add(1)
            .ok_or(BrokerError::Limit("grant revision"))?;
        self.connection.execute(
            "UPDATE roots SET json=?1 WHERE id=?2",
            params![encode(&grant)?, id],
        )?;
        self.roots.remove(id);
        Ok(())
    }

    pub fn register_app(
        &mut self,
        label: &str,
        executable: &Path,
        now_ms: i64,
    ) -> Result<AppGrant> {
        bounded_text(label, 128, "app label")?;
        timestamp(now_ms)?;
        let (canonical, _file, sha256) = files::open_executable(executable)?;
        let previous = self.apps()?.into_iter().find(|a| a.executable == canonical);
        let (id, revision, created_ms) = if let Some(app) = previous {
            if now_ms < app.updated_ms {
                return Err(BrokerError::Invalid("timestamp regression"));
            }
            (
                app.id,
                app.revision
                    .checked_add(1)
                    .ok_or(BrokerError::Limit("app revision"))?,
                app.created_ms,
            )
        } else {
            let count: i64 = self
                .connection
                .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))?;
            if count >= MAX_APPS {
                return Err(BrokerError::Limit("app grants"));
            }
            (fresh_id("app_")?, 1, now_ms)
        };
        let grant = AppGrant {
            id,
            label: label.into(),
            executable: canonical,
            sha256,
            revision,
            revoked: false,
            created_ms,
            updated_ms: now_ms,
        };
        self.connection.execute("INSERT INTO apps(id,json) VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",params![grant.id,encode(&grant)?])?;
        Ok(grant)
    }

    pub fn revoke_app(&mut self, id: &str, now_ms: i64) -> Result<()> {
        identifier(id)?;
        timestamp(now_ms)?;
        let mut app = self.app(id)?;
        if now_ms < app.updated_ms {
            return Err(BrokerError::Invalid("timestamp regression"));
        }
        app.revoked = true;
        app.updated_ms = now_ms;
        app.revision = app
            .revision
            .checked_add(1)
            .ok_or(BrokerError::Limit("app revision"))?;
        self.connection.execute(
            "UPDATE apps SET json=?1 WHERE id=?2",
            params![encode(&app)?, id],
        )?;
        Ok(())
    }

    pub fn preview(
        &mut self,
        action: &ToolAction,
        context: &ExecutionContext,
    ) -> Result<ActionPreview> {
        action.validate()?;
        context.validate()?;
        if !action.requires_approval() {
            return Err(BrokerError::Invalid(
                "read-only action does not need a preview",
            ));
        }
        let revision = self.check_action(action, context)?;
        self.connection.execute(
            "DELETE FROM previews WHERE expires_ms<?1 AND status IN ('prepared','approved')",
            [context.now_ms],
        )?;
        let pending: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM previews", [], |r| r.get(0))?;
        if pending >= MAX_PENDING {
            return Err(BrokerError::Limit(
                "pending/retained previews; reconcile uncertain actions",
            ));
        }
        let id = fresh_id("preview_")?;
        let hash = fingerprint(action, context, revision)?;
        let expires_at_ms = context
            .now_ms
            .checked_add(PREVIEW_TTL_MS)
            .ok_or(BrokerError::Invalid("preview expiry"))?;
        let (summary, details) = match action {
            ToolAction::CreateNote {
                root_id,
                path,
                content,
            } => (
                format!("Tạo ghi chú mới: {path}"),
                json!({"operation":"create_note","root_id":root_id,"path":path,"content_bytes":content.len(),"content_sha256":digest(content.as_bytes()),"content_preview":content,"content_preview_truncated":false,"overwrite":false}),
            ),
            ToolAction::OpenApp { app_id } => {
                let app = self.app(app_id)?;
                (
                    format!("Mở ứng dụng: {}", app.label),
                    json!({"operation":"open_app","app_id":app.id,"label":app.label,"executable":app.executable,"executable_sha256":app.sha256,"arguments":[]}),
                )
            }
            ToolAction::CreateReminder { text, due_at_ms } => (
                "Tạo lời nhắc".into(),
                json!({"operation":"create_reminder","text":text,"due_at_ms":due_at_ms}),
            ),
            _ => return Err(BrokerError::Invalid("preview action")),
        };
        let sql_revision =
            i64::try_from(revision).map_err(|_| BrokerError::Limit("grant revision"))?;
        self.connection.execute("INSERT INTO previews(id,action,goal_id,session_id,grant_revision,fingerprint,created_ms,expires_ms,status,nonce_hash,disclose_to_llm) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'prepared',NULL,?9)",params![id,encode(action)?,context.goal_id,context.session_id,sql_revision,hash,context.now_ms,expires_at_ms,context.disclose_to_llm])?;
        Ok(ActionPreview {
            id,
            goal_id: context.goal_id.clone(),
            goal_revision: context.goal_revision,
            session_id: context.session_id.clone(),
            summary,
            fingerprint: hash,
            expires_at_ms,
            grant_revision: revision,
            details,
        })
    }

    /// Trusted UI operation only. Do not register this method as an LLM tool.
    pub fn approve(
        &mut self,
        preview_id: &str,
        context: &ExecutionContext,
    ) -> Result<ApprovalToken> {
        context.validate()?;
        let prepared = self.prepared(preview_id)?;
        if prepared.status != "prepared" {
            return Err(BrokerError::ApprovalMismatch);
        }
        self.match_prepared(&prepared, &prepared.action, context)?;
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(|_| BrokerError::Invalid("system entropy"))?;
        let changed = self.connection.execute(
            "UPDATE previews SET nonce_hash=?1,status='approved' WHERE id=?2 AND status='prepared'",
            params![digest(&nonce), preview_id],
        )?;
        if changed != 1 {
            return Err(BrokerError::ApprovalMismatch);
        }
        Ok(ApprovalToken {
            preview_id: preview_id.into(),
            nonce,
        })
    }

    pub fn execute(
        &mut self,
        action: &ToolAction,
        context: &ExecutionContext,
        approval: Option<ApprovalToken>,
    ) -> Result<ToolReceipt> {
        action.validate()?;
        context.validate()?;
        self.check_action(action, context)?;
        if !action.requires_approval() {
            if approval.is_some() {
                return Err(BrokerError::ApprovalMismatch);
            }
            return self.read_action(action, context);
        }
        let token = approval.ok_or(BrokerError::ApprovalRequired)?;
        let prepared = self.prepared(&token.preview_id)?;
        self.match_prepared(&prepared, action, context)?;
        if prepared.status != "approved"
            || prepared.nonce_hash.as_deref() != Some(digest(&token.nonce).as_str())
        {
            return Err(BrokerError::ApprovalMismatch);
        }
        let changed=self.connection.execute("UPDATE previews SET status='dispatched',nonce_hash=NULL WHERE id=?1 AND status='approved'",[&token.preview_id])?;
        if changed != 1 {
            return Err(BrokerError::ApprovalMismatch);
        }
        let result = self.effect_action(action, context, &token.preview_id);
        match result {
            Ok(receipt) => {
                let status = if receipt.verified {
                    "verified"
                } else {
                    "authorized"
                };
                self.connection.execute(
                    "UPDATE previews SET status=?1,receipt=?2 WHERE id=?3 AND status='dispatched'",
                    params![status, encode(&receipt.data)?, token.preview_id],
                )?;
                Ok(receipt)
            }
            Err(error) => {
                self.connection.execute(
                    "UPDATE previews SET status='uncertain' WHERE id=?1 AND status='dispatched'",
                    [token.preview_id],
                )?;
                Err(error)
            }
        }
    }

    /// Readback-only reconciliation; never writes/retries the target. This is a
    /// trusted supervisor/UI operation, not a tool available to model content.
    pub fn reconcile_note(
        &mut self,
        preview_id: &str,
        context: &ExecutionContext,
    ) -> Result<ToolReceipt> {
        context.validate()?;
        let prepared = self.prepared(preview_id)?;
        if prepared.goal_id != context.goal_id
            || !matches!(
                prepared.status.as_str(),
                "uncertain" | "dispatched" | "verified"
            )
        {
            return Err(BrokerError::ApprovalMismatch);
        }
        let ToolAction::CreateNote {
            root_id,
            path,
            content,
        } = &prepared.action
        else {
            return Err(BrokerError::Invalid("reconcilable note action"));
        };
        let root = self.checked_root(root_id, path, context)?;
        if prepared.session_id != context.session_id
            || prepared.grant_revision != root.grant.revision
            || prepared.disclose_to_llm != context.disclose_to_llm
            || prepared.fingerprint != fingerprint(&prepared.action, context, root.grant.revision)?
        {
            return Err(BrokerError::ApprovalMismatch);
        }
        if !root.grant.permissions.read {
            return Err(BrokerError::PermissionDenied);
        }
        let (_, hash, bytes) = files::read_text(&root.dir, path)?;
        if hash != digest(content.as_bytes()) || bytes != content.len() as u64 {
            return Err(BrokerError::NeedsReconciliation);
        }
        let receipt = ToolReceipt {
            data: json!({"root_id":root_id,"path":path,"sha256":hash,"bytes":bytes,"readback_verified":true,"reconciled_without_write":true}),
            verified: true,
            disclosure: disclosure(context),
            approval_id: Some(preview_id.into()),
            launch: None,
        };
        self.connection.execute(
            "UPDATE previews SET status='verified',receipt=?1 WHERE id=?2",
            params![encode(&receipt.data)?, preview_id],
        )?;
        Ok(receipt)
    }

    pub fn preview_status(&self, id: &str) -> Result<String> {
        Ok(self.prepared(id)?.status)
    }

    /// Trusted maintenance only, AFTER the goal/effect store durably verifies
    /// the corresponding receipt. Never expose this as a model tool. Until this
    /// acknowledgement, verified broker proof survives approval TTL/restart.
    pub fn acknowledge_effect(
        &mut self,
        preview_id: &str,
        expected_fingerprint: &str,
    ) -> Result<()> {
        if expected_fingerprint.len() != 64
            || !expected_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(BrokerError::Invalid("effect fingerprint"));
        }
        let prepared = self.prepared(preview_id)?;
        if !matches!(prepared.status.as_str(), "verified" | "authorized")
            || prepared.fingerprint != expected_fingerprint
        {
            return Err(BrokerError::ApprovalMismatch);
        }
        let changed=self.connection.execute("DELETE FROM previews WHERE id=?1 AND fingerprint=?2 AND status IN ('verified','authorized')",params![preview_id,expected_fingerprint])?;
        if changed != 1 {
            return Err(BrokerError::ApprovalMismatch);
        }
        Ok(())
    }

    pub fn reminders(&self) -> Result<Vec<Reminder>> {
        let mut statement=self.connection.prepare("SELECT id,goal_id,text,due_at_ms,dismissed,cloud_disclosure FROM reminders ORDER BY due_at_ms,id LIMIT 257")?;
        let out = statement
            .query_map([], |row| {
                Ok(Reminder {
                    id: row.get(0)?,
                    goal_id: row.get(1)?,
                    text: row.get(2)?,
                    due_at_ms: row.get(3)?,
                    dismissed: row.get(4)?,
                    cloud_disclosure: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if out.len() > MAX_REMINDERS as usize {
            return Err(BrokerError::ForeignState);
        }
        Ok(out)
    }

    pub fn due_reminders(&self, now_ms: i64) -> Result<Vec<Reminder>> {
        timestamp(now_ms)?;
        Ok(self
            .reminders()?
            .into_iter()
            .filter(|r| !r.dismissed && r.due_at_ms <= now_ms)
            .collect())
    }

    pub fn dismiss_reminder(&mut self, id: &str) -> Result<()> {
        identifier(id)?;
        let changed = self
            .connection
            .execute("UPDATE reminders SET dismissed=1 WHERE id=?1", [id])?;
        if changed != 1 {
            return Err(BrokerError::Invalid("reminder ID"));
        }
        Ok(())
    }

    fn read_action(&self, action: &ToolAction, context: &ExecutionContext) -> Result<ToolReceipt> {
        let data = match action {
            ToolAction::ListFiles { root_id, path } => {
                let root = self.checked_root(root_id, path, context)?;
                let mut data = files::list(&root.dir, path)?;
                if let Some(entries) = data["entries"].as_array_mut() {
                    entries.retain(|entry| {
                        entry["name"].as_str().is_some_and(|name| {
                            !self.is_protected_path(&root.grant.path.join(path).join(name), true)
                        })
                    });
                }
                data["root_id"] = root_id.clone().into();
                data["path"] = path.clone().into();
                data["untrusted_content"] = true.into();
                data
            }
            ToolAction::ReadText { root_id, path } => {
                let root = self.checked_root(root_id, path, context)?;
                let (text, sha256, bytes) = files::read_text(&root.dir, path)?;
                json!({"root_id":root_id,"path":path,"text":text,"sha256":sha256,"bytes":bytes,"untrusted_content":true})
            }
            ToolAction::SearchText {
                root_id,
                path,
                query,
            } => {
                let root = self.checked_root(root_id, path, context)?;
                let mut data = files::search(&root.dir, path, query)?;
                data["root_id"] = root_id.clone().into();
                data["path"] = path.clone().into();
                data
            }
            ToolAction::Calculate { expression } => {
                json!({"expression":expression,"result":arithmetic::calculate(expression)?})
            }
            ToolAction::CurrentTime => json!({"unix_ms":context.now_ms,"timezone":"UTC"}),
            ToolAction::ListReminders => {
                json!({"scope":if context.disclose_to_llm { "cloud_allowed_reminders" } else { "local_reminders" },"reminders":self.reminders()?.into_iter().filter(|r|!context.disclose_to_llm || r.cloud_disclosure).collect::<Vec<_>>()})
            }
            _ => return Err(BrokerError::ApprovalRequired),
        };
        Ok(ToolReceipt {
            data,
            verified: true,
            disclosure: disclosure(context),
            approval_id: None,
            launch: None,
        })
    }

    fn effect_action(
        &mut self,
        action: &ToolAction,
        context: &ExecutionContext,
        approval_id: &str,
    ) -> Result<ToolReceipt> {
        match action {
            ToolAction::CreateNote {
                root_id,
                path,
                content,
            } => {
                let root = self.checked_root(root_id, path, context)?;
                let mut data = files::create_note(&root.dir, path, content)?;
                data["root_id"] = root_id.clone().into();
                Ok(ToolReceipt {
                    data,
                    verified: true,
                    disclosure: disclosure(context),
                    approval_id: Some(approval_id.into()),
                    launch: None,
                })
            }
            ToolAction::OpenApp { app_id } => {
                let app = self.app(app_id)?;
                let (path, file, hash) = files::open_executable(&app.executable)?;
                if hash != app.sha256 {
                    return Err(BrokerError::PermissionDenied);
                }
                Ok(ToolReceipt {
                    data: json!({"app_id":app.id,"launch_authorized":true,"process_started":false,"launch_verified":false}),
                    verified: false,
                    disclosure: Disclosure::CloudAllowed,
                    approval_id: Some(approval_id.into()),
                    launch: Some(ApprovedLaunch {
                        executable: path,
                        expected_sha256: hash,
                        approval_id: approval_id.into(),
                        _file: file,
                    }),
                })
            }
            ToolAction::CreateReminder { text, due_at_ms } => {
                let tx = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)?;
                let count: i64 =
                    tx.query_row("SELECT COUNT(*) FROM reminders", [], |r| r.get(0))?;
                if count >= MAX_REMINDERS {
                    return Err(BrokerError::Limit("reminders"));
                }
                tx.execute("INSERT INTO reminders(id,goal_id,text,due_at_ms,dismissed,cloud_disclosure) VALUES (?1,?2,?3,?4,0,?5)",params![approval_id,context.goal_id,text,due_at_ms,context.disclose_to_llm])?;
                let reminder:Reminder=tx.query_row("SELECT id,goal_id,text,due_at_ms,dismissed,cloud_disclosure FROM reminders WHERE id=?1",[approval_id],|r|Ok(Reminder {id:r.get(0)?,goal_id:r.get(1)?,text:r.get(2)?,due_at_ms:r.get(3)?,dismissed:r.get(4)?,cloud_disclosure:r.get(5)?}))?;
                if reminder.text != *text || reminder.due_at_ms != *due_at_ms {
                    return Err(BrokerError::NeedsReconciliation);
                }
                tx.commit()?;
                Ok(ToolReceipt {
                    data: json!({"reminder":reminder,"state_readback_verified":true}),
                    verified: true,
                    disclosure: disclosure(context),
                    approval_id: Some(approval_id.into()),
                    launch: None,
                })
            }
            _ => Err(BrokerError::Invalid("effect action")),
        }
    }

    fn root(&self, id: &str, context: &ExecutionContext) -> Result<&RootHandle> {
        let root = self
            .roots
            .get(id)
            .filter(|r| !r.grant.revoked)
            .ok_or(BrokerError::PermissionDenied)?;
        if context.disclose_to_llm && !root.grant.permissions.cloud_disclosure {
            return Err(BrokerError::DisclosureDenied);
        }
        Ok(root)
    }

    fn app(&self, id: &str) -> Result<AppGrant> {
        self.apps()?
            .into_iter()
            .find(|a| a.id == id && !a.revoked)
            .ok_or(BrokerError::PermissionDenied)
    }

    fn checked_root(
        &self,
        id: &str,
        path: &str,
        context: &ExecutionContext,
    ) -> Result<&RootHandle> {
        let root = self.root(id, context)?;
        if self.is_protected_path(&root.grant.path.join(path), false) {
            return Err(BrokerError::PermissionDenied);
        }
        Ok(root)
    }

    fn is_protected_path(&self, target: &Path, hide_ancestors: bool) -> bool {
        fn key(path: &Path) -> Vec<String> {
            path.components()
                .filter(|part| !matches!(part, Component::CurDir))
                .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
                .collect()
        }
        let target = key(target);
        self.protected_paths.iter().any(|path| {
            let protected = key(path);
            target.starts_with(&protected) || (hide_ancestors && protected.starts_with(&target))
        })
    }

    fn check_action(&self, action: &ToolAction, context: &ExecutionContext) -> Result<u64> {
        match action {
            ToolAction::ListFiles { root_id, path }
            | ToolAction::ReadText { root_id, path }
            | ToolAction::SearchText { root_id, path, .. } => {
                let root = self.checked_root(root_id, path, context)?;
                if !root.grant.permissions.read {
                    return Err(BrokerError::PermissionDenied);
                }
                Ok(root.grant.revision)
            }
            ToolAction::CreateNote { root_id, path, .. } => {
                let root = self.checked_root(root_id, path, context)?;
                if !root.grant.permissions.create_new {
                    return Err(BrokerError::PermissionDenied);
                }
                files::check_create(&root.dir, path)?;
                Ok(root.grant.revision)
            }
            ToolAction::OpenApp { app_id } => {
                let app = self.app(app_id)?;
                let (_, _, hash) = files::open_executable(&app.executable)?;
                if hash != app.sha256 {
                    return Err(BrokerError::PermissionDenied);
                }
                Ok(app.revision)
            }
            ToolAction::CreateReminder { due_at_ms, .. } => {
                if *due_at_ms < context.now_ms {
                    return Err(BrokerError::Invalid("reminder time is in the past"));
                }
                let max = context
                    .now_ms
                    .checked_add(366 * 24 * 60 * 60 * 1000)
                    .ok_or(BrokerError::Invalid("reminder time"))?;
                if *due_at_ms > max {
                    return Err(BrokerError::Limit("reminder horizon (366 days)"));
                }
                Ok(0)
            }
            ToolAction::Calculate { .. } | ToolAction::CurrentTime | ToolAction::ListReminders => {
                Ok(0)
            }
        }
    }

    fn match_prepared(
        &self,
        prepared: &Prepared,
        action: &ToolAction,
        context: &ExecutionContext,
    ) -> Result<()> {
        let revision = self.check_action(action, context)?;
        if prepared.goal_id != context.goal_id
            || prepared.session_id != context.session_id
            || prepared.grant_revision != revision
            || prepared.created_ms > context.now_ms
            || prepared.expires_ms < context.now_ms
            || prepared.disclose_to_llm != context.disclose_to_llm
            || prepared.fingerprint != fingerprint(action, context, revision)?
        {
            return Err(BrokerError::ApprovalMismatch);
        }
        Ok(())
    }

    fn prepared(&self, id: &str) -> Result<Prepared> {
        identifier(id)?;
        let row=self.connection.query_row("SELECT action,goal_id,session_id,grant_revision,fingerprint,created_ms,expires_ms,status,nonce_hash,disclose_to_llm FROM previews WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get::<_,i64>(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?))).optional()?.ok_or(BrokerError::ApprovalMismatch)?;
        if row.0.len() > 100_000 {
            return Err(BrokerError::ForeignState);
        }
        let action: ToolAction =
            serde_json::from_str(&row.0).map_err(|_| BrokerError::ForeignState)?;
        action.validate().map_err(|_| BrokerError::ForeignState)?;
        Ok(Prepared {
            action,
            goal_id: row.1,
            session_id: row.2,
            grant_revision: u64::try_from(row.3).map_err(|_| BrokerError::ForeignState)?,
            fingerprint: row.4,
            created_ms: row.5,
            expires_ms: row.6,
            status: row.7,
            nonce_hash: row.8,
            disclose_to_llm: row.9,
        })
    }

    fn records<T: DeserializeOwned + RecordIdentity>(&self, sql: &str, max: i64) -> Result<Vec<T>> {
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            if out.len() >= max as usize {
                return Err(BrokerError::ForeignState);
            }
            let id: String = row.get(0)?;
            identifier(&id).map_err(|_| BrokerError::ForeignState)?;
            let encoded: String = row.get(1)?;
            if encoded.len() > 8192 {
                return Err(BrokerError::ForeignState);
            }
            let record: T =
                serde_json::from_str(&encoded).map_err(|_| BrokerError::ForeignState)?;
            if record.record_id() != id {
                return Err(BrokerError::ForeignState);
            }
            out.push(record);
        }
        Ok(out)
    }
}

fn timestamp(now: i64) -> Result<()> {
    if now < 0 {
        Err(BrokerError::Invalid("timestamp"))
    } else {
        Ok(())
    }
}
fn encode(value: &impl Serialize) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}
fn disclosure(context: &ExecutionContext) -> Disclosure {
    if context.disclose_to_llm {
        Disclosure::CloudAllowed
    } else {
        Disclosure::LocalOnly
    }
}

trait RecordIdentity {
    fn record_id(&self) -> &str;
}
impl RecordIdentity for RootGrant {
    fn record_id(&self) -> &str {
        &self.id
    }
}
impl RecordIdentity for AppGrant {
    fn record_id(&self) -> &str {
        &self.id
    }
}

fn validate_schema(connection: &Connection) -> Result<()> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
        [],
        |r| r.get(0),
    )?;
    if count != 4 {
        return Err(BrokerError::ForeignState);
    }
    for (table, expected) in [
        ("roots", &["id:TEXT:0:1", "json:TEXT:1:0"][..]),
        ("apps", &["id:TEXT:0:1", "json:TEXT:1:0"][..]),
        (
            "previews",
            &[
                "id:TEXT:0:1",
                "action:TEXT:1:0",
                "goal_id:TEXT:1:0",
                "session_id:TEXT:1:0",
                "grant_revision:INTEGER:1:0",
                "fingerprint:TEXT:1:0",
                "created_ms:INTEGER:1:0",
                "expires_ms:INTEGER:1:0",
                "status:TEXT:1:0",
                "nonce_hash:TEXT:0:0",
                "disclose_to_llm:INTEGER:1:0",
                "receipt:TEXT:0:0",
            ][..],
        ),
        (
            "reminders",
            &[
                "id:TEXT:0:1",
                "goal_id:TEXT:1:0",
                "text:TEXT:1:0",
                "due_at_ms:INTEGER:1:0",
                "dismissed:INTEGER:1:0",
                "cloud_disclosure:INTEGER:1:0",
            ][..],
        ),
    ] {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
        let actual = statement
            .query_map([], |r| {
                Ok(format!(
                    "{}:{}:{}:{}",
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(5)?
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if actual != expected {
            return Err(BrokerError::ForeignState);
        }
    }
    let integrity: String = connection.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(BrokerError::ForeignState);
    }
    Ok(())
}
