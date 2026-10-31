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

During recording or ASR, “Hủy nhận diện” emits `CancelVoice` to discard the
current voice request without submitting a transcript or cancelling its goal.

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

Ended goal cards expose `ShowResult { goal_id }` through “Xem kết quả” and switch
to the conversation after the command is accepted. The runtime retrieves the
durable result locally; the card's bounded summary is not the full result.

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
The dark theme is explicitly selected so a Windows light-theme update cannot
replace the palette; foreground and button states use readable contrast.
`EGUI_INSPECTION`/`EGUI_INSPECTION_ADDR` cause a safe startup error rather than
opening a network inspection service; the inspection feature is not selected.

Portable state tests cover bounds, Unicode, duplicate IDs, approval expiry/replay,
channel backpressure and controls. Native launch, tray behavior, accessibility,
GPU/RAM and actual click screenshots remain integration evidence owned by the
launcher. Non-Windows `run` returns UnsupportedPlatform; no Linux desktop support
claim is made by portable tests.

The trusted launcher's optional `DesktopOptions.diagnostic_screenshot` exports
one NORA framebuffer through egui's native callback after one second of painting.
It never captures the OS desktop or other windows, and opens no inspection
service. The option defaults to `None` and cannot be combined with a hidden
launch. Choose a new absolute local `.png` path; both that file and its sibling
`<path>.status.json` use create-new semantics and are never overwritten. The
status reports `scope: nora_own_framebuffer`, `status: ok` with width/height, or
`status: failed` with a fixed error code. A successful status is written only
after PNG encoding and file sync. Frames are limited to 4096 pixels per side and
8,388,608 pixels in total, with a bounded row buffer for encoding. Closing before
capture, callback timeout, invalid dimensions or write errors also make the
native `run_with_options` result fail. The export contains the app's visible
content; enable it only for deliberate local diagnostic runs.
