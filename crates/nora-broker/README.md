# nora-broker

Bounded local grants, typed tools and approval-bound writes for a single trusted
Rust supervisor. No network client, shell executor, package installer, deletion,
overwrite or external message sending is exposed by this crate. It does not
authenticate IPC callers or provide an OS sandbox; the runtime must separate
trusted UI methods from model tool execution.

`Broker::open(state_dir)` obtains an exclusive std file lock and opens its own
application/version-marked SQLite database. Foreign schemas, unexpected objects,
malformed grant records and incompatible state are rejected. WAL/FULL transactions
persist grants, previews, effect acknowledgements and reminders. The main DB has
a configured16 MB page limit and1 MB SQLite cache; the4 MB journal setting is not
a hard cap on an active WAL. The app must monitor disk/checkpoint health.

Trusted setup/UI methods: `grant_root`, `revoke_root`, `register_app`, `revoke_app`,
`protect_path`, `approve`, `reconcile_note`, reminder dismissal. None belongs in
the LLM tool catalog. Root/app paths returned by `roots()`/`apps()` are UI state,
not automatically permitted LLM context. Root permissions distinguish read,
create-new and cloud disclosure. Local-only grants reject even listing metadata
when `ExecutionContext.disclose_to_llm=true`; local-only receipts must never be
forwarded to a model by the caller.

Model tools deserialize only the strict `ToolAction` enum: bounded file listing,
UTF-8 read/search, create-new `.md`/`.txt` notes, approved application opening,
bounded arithmetic, UTC epoch time, durable reminders. Files use directory
capabilities, portable relative paths and checked bounds. Traversal, ADS, Windows
devices, DOS short-name aliases (`~`), sensitive paths, format controls in filenames and symlink components are
rejected. Standard `.env`, SSH/cloud credentials, secret key files and internal
DB/state names are denied. Broker state is protected even inside a granted
project; the runtime should additionally `protect_path` its complete state/config
directories before accepting requests. These controls are not comprehensive DLP:
explicitly cloud-enabled user documents may contain sensitive text.

Every effect needs `preview` -> trusted UI `approve` -> `execute`. Preview holds
the exact bounded text (up to65,536 bytes), size/hash, goal ID/revision, session,
grant revision and two-minute expiry. UI must allow complete scroll review and
must not silently truncate parameters. A private non-deserializable one-use token
binds the approval; cancellation/revocation/replan/changed arguments invalidate it.
Notes never overwrite and return hashes plus actual readback evidence. Failures
after dispatch remain uncertain; restart marks interrupted dispatches uncertain.
`reconcile_note` reads back exact expected content without writing or blind retry.
It also covers the crash gap where this broker saved verification but the goal
store did not. The original session/proposal fingerprint and current grant
revision remain bound; approval expiry does not authorize a new effect and is
irrelevant to readback of an already dispatched one. Verified/authorized proof is
retained until the trusted runner calls `acknowledge_effect` after its own durable
effect verification. Never expose acknowledgement/receipt deletion as a model
tool or call it before that durable goal-store commit.

Executable grants come only from a trusted native file picker. Only local `.exe`
files with an MZ header are accepted, shell/interpreter names are denied, hashes
are pinned, and model-provided arguments are impossible. `OpenApp` returns
`ApprovedLaunch`, not an executed process. `verified=false` and
`process_started=false` are deliberate: the trusted launcher must keep the result
alive during spawn (Windows read-only sharing prevents executable replacement)
and record actual process verification separately. No tests launch user apps; a
fixed hidden Windows PowerShell helper creates a synthetic junction fixture with
an empty environment and temp paths supplied as separate arguments.

An OS account able to alter granted files, replace this application or read its
memory is outside this library's isolation boundary. Live directory handles keep
file operations rooted, but this is not a claim that all hostile OS-level races,
hardlink/content changes or native parser vulnerabilities are defeated. AppContainer
worker isolation and whole-process resource enforcement belong to the platform
layer. Windows junction tests run without changing Developer Mode or privileges;
Unix symlink coverage is a separate platform test.
