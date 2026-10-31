# nora-desktop

Windows-first native Rust shell using eframe/egui0.37, OpenGL/glow and AccessKit.
No WebView, browser, microphone, API client or tool execution lives in this crate.
It sends typed user commands to the trusted runtime over bounded channels.

`run(UiChannels::new(command_sender,event_receiver), initial_snapshot)` opens a
visible window. `run_with_options` can request a background tray launch; if tray
creation fails it keeps the window visible. Closing to tray emits `WindowHidden`
so the runtime can stop/discard microphone capture. Quit and stop-speech are
separate commands from pause/cancel-goal. The runtime must implement those
semantics; a UI button does not enforce an OS boundary.

Give `UiChannels` a `with_wake_sender` handshake and call the returned `UiWake`
after delivering runtime events to wake an idle/hidden window without continuous
polling. Without a waker, the shell uses bounded250/500ms fallback polling.
Tray callbacks use a bounded16-entry queue and wake the native event loop.
Ctrl+Shift+Space registers for this UI process lifetime as hold-to-talk; a
conflicting registration leaves the explicit Nói button available. Press shows
the window before requesting capture. Releases use a latest-state atomic and
an acceptance latch, so a delayed snapshot or full command queue cannot silently
lose the stop request. The runtime still owns consent/session-lock/duration
enforcement. Running goals reject a concurrent new submission; WaitingInput
accepts a text continuation. PTT remains available during a running goal for the
runtime's local status/pause/cancel commands; it does not itself admit a new goal.

The shell displays runtime-authoritative connection/mic/goal state. It never
displays reasoning or reads the local API-key configuration. Transcript events
only show the last recognized text; the runtime decides whether to submit it.
Folder grants require an explicit dialog; cloud disclosure starts unchecked.
No application executes during an app-file picker. Approval cards preserve exact
preview IDs, check expiry/completeness and disable after a decision is queued;
the broker must independently validate authority, fingerprints and one-use.

Public research is a separate unchecked-by-default opt-in; the UI explains that
each public query still needs an exact preview approval. App registration warns
that app labels/IDs are sent to the LLM while paths/hashes stay local. NoticeShown
is sent after a prior card paint intersects the visible scroll clip, and only
while still visible/not minimized; it is a software surface acknowledgement,
not proof of user attention. Unseen/clipped notices remain unacknowledged.

Histories, snapshots, input, notices, preview contents and per-frame event drains
are bounded. An oversized/truncated approval preview becomes non-approvable.
Plain text is rendered as text; URLs or embedded commands do not execute.
System Segoe UI is loaded locally for Vietnamese glyphs and is not distributed.
`EGUI_INSPECTION`/`EGUI_INSPECTION_ADDR` cause a safe startup error rather than
opening a network inspection service; the inspection feature is not selected.

Portable state tests cover bounds, Unicode, duplicate IDs, approval expiry/replay,
channel backpressure and controls. Native launch, tray behavior, accessibility,
GPU/RAM and actual click screenshots remain integration evidence owned by the
launcher. Non-Windows `run` returns UnsupportedPlatform; no Linux desktop support
claim is made by portable tests.
