use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError, TrySendError},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

use crate::{
    DesktopError, DesktopOptions,
    contracts::*,
    diagnostic::DiagnosticCapture,
    state::{DesktopState, PushToTalkState},
};

const BACKGROUND: Color32 = Color32::from_rgb(13, 18, 25);
const SURFACE: Color32 = Color32::from_rgb(21, 28, 37);
const BORDER: Color32 = Color32::from_rgb(43, 54, 67);
const ACCENT: Color32 = Color32::from_rgb(119, 226, 193);
const MUTED: Color32 = Color32::from_rgb(152, 167, 185);
const FOREGROUND: Color32 = Color32::from_rgb(230, 237, 245);
const WARNING: Color32 = Color32::from_rgb(245, 195, 111);

pub fn run(
    channels: UiChannels,
    initial: AppSnapshot,
    options: DesktopOptions,
) -> Result<(), DesktopError> {
    // Even if an upstream feature changes later, do not start an inspection
    // socket simply because the launch environment happens to request one.
    if std::env::var_os("EGUI_INSPECTION").is_some()
        || std::env::var_os("EGUI_INSPECTION_ADDR").is_some()
    {
        return Err(DesktopError::InspectionDisabled);
    }
    let diagnostic = options
        .diagnostic_screenshot
        .as_deref()
        .map(DiagnosticCapture::prepare)
        .transpose()?;
    if options.start_hidden && diagnostic.is_some() {
        if let Some(capture) = &diagnostic {
            capture.fail("window_hidden");
        }
        return Err(DesktopError::DiagnosticScreenshot);
    }
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NORA")
            .with_app_id("club.nosleep.nora")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(egui::IconData {
                rgba: icon_pixels(),
                width: 32,
                height: 32,
            }),
        renderer: eframe::Renderer::Glow,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        persist_window: false,
        centered: true,
        ..Default::default()
    };
    let window_diagnostic = diagnostic.clone();
    let result = eframe::run_native(
        "NORA",
        native,
        Box::new(move |cc| {
            Ok(Box::new(NoraWindow::new(
                cc,
                channels,
                initial,
                options,
                window_diagnostic,
            )))
        }),
    );
    if result.is_err() {
        if let Some(capture) = &diagnostic {
            capture.fail("initialization_failed");
        }
        return Err(DesktopError::Initialization);
    }
    diagnostic.map_or(Ok(()), |capture| capture.finish())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chat,
    Goals,
    Permissions,
}

#[derive(Clone, Copy)]
enum TraySignal {
    Show,
    Quit,
}

struct FolderDialog {
    path: PathBuf,
    can_read: bool,
    can_create: bool,
    allow_cloud: bool,
}

struct HotkeyRegistration {
    manager: GlobalHotKeyManager,
    hotkey: HotKey,
    pressed: Arc<AtomicBool>,
}

impl Drop for HotkeyRegistration {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.hotkey);
    }
}

struct NoraWindow {
    channels: UiChannels,
    state: DesktopState,
    page: Page,
    options: DesktopOptions,
    tray: Option<TrayIcon>,
    tray_events: Receiver<TraySignal>,
    hidden: bool,
    wake_connected: bool,
    runtime_disconnected: bool,
    pending_folder: Option<FolderDialog>,
    pending_audio: Option<PathBuf>,
    quit_requested: bool,
    hotkey: Option<HotkeyRegistration>,
    ptt: PushToTalkState,
    notice_candidates: Vec<String>,
    minimized: bool,
    diagnostic: Option<DiagnosticCapture>,
}

impl NoraWindow {
    fn new(
        cc: &eframe::CreationContext<'_>,
        channels: UiChannels,
        initial: AppSnapshot,
        options: DesktopOptions,
        diagnostic: Option<DiagnosticCapture>,
    ) -> Self {
        configure_style(&cc.egui_ctx);
        let wake_context = cc.egui_ctx.clone();
        let wake = UiWake::new(move || wake_context.request_repaint());
        let wake_connected = channels
            .wake_ready
            .as_ref()
            .is_some_and(|ready| ready.try_send(wake).is_ok());
        let (tray, tray_events) = create_tray(&cc.egui_ctx);
        let hotkey = register_hotkey(&cc.egui_ctx);
        let mut value = Self {
            channels,
            state: DesktopState::new(initial),
            page: Page::Chat,
            options,
            tray,
            tray_events,
            hidden: false,
            wake_connected,
            runtime_disconnected: false,
            pending_folder: None,
            pending_audio: None,
            quit_requested: false,
            hotkey,
            ptt: PushToTalkState::default(),
            notice_candidates: Vec::new(),
            minimized: false,
            diagnostic,
        };
        if value.options.start_hidden && value.tray.is_some() {
            value.hide(&cc.egui_ctx);
        } else {
            value.send(UiCommand::WindowShown);
        }
        value
    }

    fn send(&mut self, command: UiCommand) -> bool {
        match self.channels.commands.try_send(command.clone()) {
            Ok(()) => {
                self.state.accepted_command(&command);
                if matches!(
                    command,
                    UiCommand::StopRecording
                        | UiCommand::CancelVoice
                        | UiCommand::WindowHidden
                        | UiCommand::Quit
                ) {
                    self.ptt.stopped();
                }
                true
            }
            Err(TrySendError::Full(_)) => {
                self.state.command_rejected(false);
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                self.state.command_rejected(true);
                false
            }
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.hidden = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.send(UiCommand::WindowShown);
        ctx.request_repaint();
    }

    fn hide(&mut self, ctx: &egui::Context) {
        if self.send(UiCommand::WindowHidden) {
            self.hidden = true;
            self.notice_candidates.clear();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        // If the runtime is already gone, closing the window must remain possible.
        if self.runtime_disconnected || self.send(UiCommand::Quit) {
            self.quit_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if minimized != self.minimized {
            if self.send(if minimized {
                UiCommand::WindowHidden
            } else {
                UiCommand::WindowShown
            }) {
                self.minimized = minimized;
            } else {
                ctx.request_repaint_after(Duration::from_millis(25));
            }
        }
        // Candidates came from the previous visible paint pass. Acknowledging
        // clipped/offscreen/hidden cards would lose a pending outbox message.
        let candidates = std::mem::take(&mut self.notice_candidates);
        if !self.hidden && !minimized {
            for id in candidates {
                if self.state.notice_was_shown(&id) {
                    continue;
                }
                let command = UiCommand::NoticeShown { id: id.clone() };
                match self.channels.commands.try_send(command.clone()) {
                    Ok(()) => self.state.accepted_command(&command),
                    Err(TrySendError::Full(_)) => {
                        self.notice_candidates.push(id);
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
        }
        let mut new_preview = false;
        for _ in 0..EVENT_CAPACITY {
            match self.channels.events.try_recv() {
                Ok(event) => {
                    new_preview |= matches!(&event, RuntimeEvent::Preview(_));
                    self.state.apply(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.runtime_disconnected {
                        self.runtime_disconnected = true;
                        self.state.snapshot.mic = MicState::Off;
                        self.state.snapshot.speaking = false;
                        self.state.snapshot.connection = ConnectionState::Offline;
                        self.state.command_rejected(true);
                    }
                    break;
                }
            }
        }
        for _ in 0..16 {
            match self.tray_events.try_recv() {
                Ok(TraySignal::Show) => self.show(ctx),
                Ok(TraySignal::Quit) => self.quit(ctx),
                Err(_) => break,
            }
        }
        if let Some(hotkey) = &self.hotkey {
            let pressed = hotkey.pressed.load(Ordering::Acquire);
            if let Some(command) = self.ptt.next(pressed, self.state.can_start_recording()) {
                let starting = matches!(command, UiCommand::StartRecording);
                if starting {
                    self.show(ctx);
                }
                if self.send(command) && starting {
                    self.ptt.started();
                }
            }
            if self.ptt.needs_stop() {
                ctx.request_repaint_after(Duration::from_millis(25));
            }
        }
        if new_preview && self.hidden {
            self.show(ctx);
        }
        if self.state.should_close {
            self.quit_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Runtime producers can wake a hidden UI with the supplied callback.
        // Without that handshake, bounded polling keeps integrations responsive.
        if !self.wake_connected && !self.runtime_disconnected {
            ctx.request_repaint_after(Duration::from_millis(if self.hidden { 500 } else { 250 }));
        } else if !self.hidden
            && (!self.state.previews.is_empty() || self.state.snapshot.mic == MicState::Recording)
        {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("N").size(28.0).strong().color(ACCENT));
            ui.vertical(|ui| {
                ui.label(RichText::new("NORA").size(24.0).strong());
                ui.label(RichText::new("Cùng bạn hoàn thành").size(12.0).color(MUTED));
            });
        });
        ui.add_space(24.0);
        for (page, title) in [
            (Page::Chat, "Hội thoại"),
            (Page::Goals, "Mục tiêu"),
            (Page::Permissions, "Quyền truy cập"),
        ] {
            if ui
                .add_sized(
                    [ui.available_width(), 38.0],
                    egui::Button::new(if page == Page::Chat && !self.state.previews.is_empty() {
                        format!("Hội thoại · {}", self.state.previews.len())
                    } else {
                        title.to_owned()
                    })
                    .selected(self.page == page),
                )
                .clicked()
            {
                self.page = page;
            }
            ui.add_space(4.0);
        }
        ui.add_space(22.0);
        section_label(ui, "PHIÊN LÀM VIỆC");
        ui.add_space(8.0);
        let color = match self.state.snapshot.connection {
            ConnectionState::Ready => ACCENT,
            ConnectionState::Busy => WARNING,
            _ => MUTED,
        };
        ui.label(
            RichText::new(format!("●  {}", self.state.snapshot.connection.label())).color(color),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(&self.state.snapshot.model)
                .size(12.0)
                .color(MUTED),
        );
        ui.label(
            RichText::new(format!(
                "Reasoning: {}",
                self.state.snapshot.reasoning_effort
            ))
            .size(11.0)
            .color(MUTED),
        );
        ui.add_space(12.0);
        let mut quiet = self.state.snapshot.quiet;
        if ui.checkbox(&mut quiet, "Chỉ hiện thông báo").changed() {
            self.send(UiCommand::SetQuiet { enabled: quiet });
        }
        ui.label(
            RichText::new("Không đọc các cập nhật qua loa.")
                .size(11.0)
                .color(MUTED),
        );
        ui.add_space(10.0);
        ui.label(
            RichText::new(if self.hotkey.is_some() {
                "Giữ Ctrl+Shift+Space để nói."
            } else {
                "Phím tắt chưa khả dụng. Dùng nút Nói."
            })
            .size(11.0)
            .color(MUTED),
        );
        ui.add_space(12.0);
        if self.state.snapshot.speaking && ui.button("Dừng nói").clicked() {
            self.send(UiCommand::StopSpeech);
        }
        if let Some(resource) = &self.state.snapshot.resource_summary {
            ui.add_space(14.0);
            ui.label(RichText::new(resource).size(11.0).color(MUTED));
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            if ui.button("Thoát NORA").clicked() {
                self.quit(ui.ctx());
            }
            if self.tray.is_some() && ui.button("Thu gọn xuống khay").clicked() {
                self.hide(ui.ctx());
            }
            if ui.button("Kiểm tra lại cấu hình").clicked() {
                self.send(UiCommand::RefreshConfig);
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new("Chỉ làm trong quyền bạn cấp.")
                    .size(11.0)
                    .color(MUTED),
            );
        });
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(match self.page {
                    Page::Chat => "Hội thoại",
                    Page::Goals => "Mục tiêu của bạn",
                    Page::Permissions => "Bạn quyết định quyền truy cập",
                });
                ui.label(RichText::new(&self.state.snapshot.status_text).color(MUTED));
            });
        });
        if self.page != Page::Chat {
            if !self.state.previews.is_empty()
                && ui
                    .button(format!(
                        "Xem yêu cầu xác nhận ({})",
                        self.state.previews.len()
                    ))
                    .clicked()
            {
                self.page = Page::Chat;
            }
            if let Some(error) = self.state.local_error.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(error).color(WARNING));
                    if ui.small_button("Đóng thông báo").clicked() {
                        self.state.local_error = None;
                    }
                });
            }
        }
    }

    fn chat(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("composer")
            .frame(
                egui::Frame::new()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(0, 12)),
            )
            .show(ui, |ui| self.composer(ui));
        egui::ScrollArea::vertical().id_salt("conversation-scroll").stick_to_bottom(true).show(ui, |ui| {
            if self.state.messages.is_empty() {
                ui.add_space(48.0);
                ui.label(RichText::new("Bạn đặt mục tiêu.\nTôi giúp thực hiện.").size(32.0).strong());
                ui.add_space(14.0);
                ui.label(RichText::new("Nói hoặc viết điều bạn cần. Tôi sẽ cập nhật khi có kết quả, cần quyền hoặc gặp trở ngại.").size(16.0).color(MUTED));
                ui.add_space(24.0);
                for text in ["Giúp tôi tìm và tóm tắt một tài liệu", "Tạo ghi chú từ những ý chính của tôi", "Tôi có thể nhờ bạn làm những gì?"] {
                    if ui.button(text).clicked() { self.state.input = text.into(); }
                    ui.add_space(6.0);
                }
            }
            for message in &self.state.messages {
                ui.add_space(10.0);
                let is_user = message.role == MessageRole::User;
                egui::Frame::new()
                    .fill(if is_user { Color32::from_rgb(28, 47, 49) } else { SURFACE })
                    .corner_radius(12)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.label(RichText::new(if is_user { "BẠN" } else { "NORA" }).size(11.0).strong().color(if is_user { ACCENT } else { MUTED }));
                        ui.add_space(6.0);
                        ui.add(egui::Label::new(&message.text).wrap().selectable(true));
                    });
            }
            let previews: Vec<_> = self.state.previews.iter().cloned().collect();
            for preview in previews { self.approval(ui, &preview); }
            for notice in self.state.notices.iter().rev().take(6).rev() {
                ui.add_space(10.0);
                let card=egui::Frame::new().stroke(Stroke::new(1.0, BORDER)).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.label(RichText::new(&notice.title).strong().color(match notice.kind { NoticeKind::Blocker | NoticeKind::Permission | NoticeKind::Question | NoticeKind::Approval => WARNING, _ => ACCENT }));
                    if !notice.detail.is_empty() { ui.add(egui::Label::new(&notice.detail).wrap()); }
                });
                if !self.hidden && ui.clip_rect().contains_rect(card.response.rect) && !self.state.notice_was_shown(&notice.id) && !self.notice_candidates.contains(&notice.id) && self.notice_candidates.len()<MAX_NOTICES {
                    self.notice_candidates.push(notice.id.clone());
                    ui.ctx().request_repaint();
                }
            }
            ui.add_space(12.0);
        });
    }

    fn composer(&mut self, ui: &mut egui::Ui) {
        if self.state.recovered_draft.is_some() {
            egui::Frame::new().fill(SURFACE).corner_radius(8).inner_margin(10).show(ui,|ui| {
                ui.label("Một yêu cầu chưa được gửi đang được giữ lại trong phiên này.");
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.state.input.is_empty(),egui::Button::new("Khôi phục yêu cầu chưa gửi")).on_hover_text("Để tránh ghi đè, hãy giữ lại hoặc xóa bản nháp hiện tại trước khi khôi phục.").clicked() {self.state.restore_recovered_draft();}
                    if ui.small_button("Bỏ bản nháp chưa gửi").clicked() {self.state.recovered_draft=None;}
                });
            });
            ui.add_space(8.0);
        }
        if let Some(error) = self.state.local_error.clone() {
            egui::Frame::new()
                .fill(Color32::from_rgb(62, 39, 31))
                .corner_radius(8)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(error).color(WARNING));
                        if ui.small_button("Đóng").clicked() {
                            self.state.local_error = None;
                        }
                    });
                });
            ui.add_space(8.0);
        }
        if let Some(transcript) = &self.state.last_transcript {
            ui.collapsing("Lời nói vừa nhận", |ui| {
                ui.add(egui::Label::new(transcript).wrap().selectable(true));
            });
        }
        egui::Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(12)
            .inner_margin(12)
            .show(ui, |ui| {
                let editor = ui.add(
                    egui::TextEdit::multiline(&mut self.state.input)
                        .hint_text("Nora ơi, giúp tôi…")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .char_limit(MAX_INPUT_BYTES)
                        .frame(egui::Frame::NONE),
                );
                self.state.bound_input();
                let submit_shortcut = editor.has_focus()
                    && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    match self.state.snapshot.mic {
                        MicState::Recording => {
                            if ui
                                .add(
                                    egui::Button::new("■  Xong, gửi lời nói")
                                        .fill(Color32::from_rgb(109, 49, 43)),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::StopRecording);
                            }
                        }
                        MicState::Transcribing => {
                            ui.add_enabled(false, egui::Button::new("Đang nhận lời nói…"));
                        }
                        MicState::Off | MicState::Unavailable => {
                            if ui
                                .add_enabled(
                                    self.state.can_start_recording(),
                                    egui::Button::new(
                                        if self.state.snapshot.mic == MicState::Unavailable {
                                            "●  Thử micro lại"
                                        } else {
                                            "●  Nói với NORA"
                                        },
                                    ),
                                )
                                .clicked()
                            {
                                self.send(UiCommand::StartRecording);
                            }
                        }
                    }
                    if self.state.can_cancel_voice() && ui.button("Hủy nhận diện").clicked() {
                        self.send(UiCommand::CancelVoice);
                    }
                    if ui
                        .add_enabled(
                            self.state.can_import_audio(),
                            egui::Button::new("Tệp giọng nói…"),
                        )
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_title("Chọn tệp giọng nói WAV")
                            .add_filter("Âm thanh WAV", &["wav"])
                            .pick_file()
                    {
                        self.pending_audio = Some(path);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if (ui
                            .add_enabled(
                                self.state.can_submit(),
                                egui::Button::new(
                                    RichText::new("Gửi  ↑").strong().color(BACKGROUND),
                                )
                                .fill(ACCENT),
                            )
                            .clicked()
                            || submit_shortcut)
                            && self.state.can_submit()
                        {
                            self.send(UiCommand::Submit {
                                text: self.state.input.trim().to_owned(),
                            });
                        }
                    });
                });
                ui.label(
                    RichText::new(&self.state.snapshot.voice_status)
                        .size(11.0)
                        .color(MUTED),
                );
            });
        ui.add_space(5.0);
        ui.label(
            RichText::new("Ctrl+Enter để gửi · Microphone chỉ bật khi bạn yêu cầu ghi âm.")
                .size(11.0)
                .color(MUTED),
        );
    }

    fn approval(&mut self, ui: &mut egui::Ui, preview: &ApprovalPreview) {
        ui.add_space(14.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(40, 37, 29))
            .stroke(Stroke::new(1.0, Color32::from_rgb(104, 84, 45)))
            .corner_radius(12)
            .inner_margin(16)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("CẦN BẠN XÁC NHẬN")
                        .size(11.0)
                        .strong()
                        .color(WARNING),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(&preview.title).size(18.0).strong());
                ui.add(egui::Label::new(&preview.description).wrap());
                ui.add_space(6.0);
                ui.add(egui::Label::new(&preview.target).wrap().selectable(true));
                if let Some(content) = &preview.content {
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt((&preview.id, "content"))
                        .max_height(180.0)
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(content).wrap().selectable(true));
                        });
                }
                ui.add_space(10.0);
                let now = unix_ms();
                if self.state.decision_pending(&preview.id) {
                    ui.label(
                        RichText::new("Đã gửi quyết định, đang kiểm tra kết quả…").color(MUTED),
                    );
                } else {
                    if !preview.can_approve {
                        ui.label(
                            RichText::new(
                                "Bản xem trước chưa đầy đủ. NORA cần chuẩn bị lại yêu cầu này.",
                            )
                            .color(WARNING),
                        );
                    } else if preview.expires_at_ms <= now {
                        ui.label(
                            RichText::new("Yêu cầu đã hết hạn. Hãy yêu cầu NORA chuẩn bị lại.")
                                .color(WARNING),
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                self.state.can_decide(&preview.id, true, now),
                                egui::Button::new(
                                    RichText::new("Cho phép hành động này").color(BACKGROUND),
                                )
                                .fill(ACCENT),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Approve {
                                preview_id: preview.id.clone(),
                            });
                        }
                        if ui
                            .add_enabled(
                                self.state.can_decide(&preview.id, false, now),
                                egui::Button::new("Từ chối"),
                            )
                            .clicked()
                        {
                            self.send(UiCommand::Reject {
                                preview_id: preview.id.clone(),
                            });
                        }
                    });
                }
            });
    }

    fn goals(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.state.snapshot.goals.is_empty() {
                ui.label(
                    RichText::new("Chưa có mục tiêu. Bắt đầu từ một yêu cầu trong Hội thoại.")
                        .color(MUTED),
                );
            }
            for goal in self.state.snapshot.goals.clone().iter().rev() {
                ui.add_space(10.0);
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(12)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.label(RichText::new(&goal.title).size(17.0).strong());
                        ui.label(RichText::new(goal.status.label()).color(
                            if goal.status == GoalPhase::Completed {
                                ACCENT
                            } else {
                                WARNING
                            },
                        ));
                        if !goal.detail.is_empty() {
                            ui.add(egui::Label::new(&goal.detail).wrap());
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if goal.status.terminal()
                                && ui.button("Xem kết quả").clicked()
                                && self.send(UiCommand::ShowResult {
                                    goal_id: goal.id.clone(),
                                })
                            {
                                self.page = Page::Chat;
                            }
                            if goal.status.can_pause() && ui.button("Tạm dừng").clicked() {
                                self.send(UiCommand::PauseGoal {
                                    goal_id: goal.id.clone(),
                                });
                            }
                            if goal.status.can_resume()
                                && ui
                                    .button(if goal.status == GoalPhase::NeedsReconciliation {
                                        "Kiểm tra lại trạng thái"
                                    } else {
                                        "Tiếp tục"
                                    })
                                    .clicked()
                            {
                                self.send(UiCommand::ResumeGoal {
                                    goal_id: goal.id.clone(),
                                });
                            }
                            if !goal.status.terminal() && ui.button("Hủy mục tiêu").clicked() {
                                self.send(UiCommand::CancelGoal {
                                    goal_id: goal.id.clone(),
                                });
                            }
                        });
                    });
            }
        });
    }

    fn permissions(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new("NORA chỉ được dùng các thư mục và ứng dụng bạn chọn. Cấp quyền đọc không tự cho phép gửi nội dung lên LLM.").color(MUTED)).wrap());
            ui.add_space(12.0);
            let mut public_research=self.state.snapshot.public_research;
            if ui.checkbox(&mut public_research,"Cho phép nghiên cứu web công khai").changed() {self.send(UiCommand::SetPublicResearch {enabled:public_research});}
            ui.add(egui::Label::new(RichText::new("Chỉ tra cứu thông tin công khai. Mỗi câu truy vấn vẫn cần bạn xác nhận riêng trước khi gửi.").size(12.0).color(MUTED)).wrap());
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.heading("Thư mục");
                if ui.add_enabled(self.state.snapshot.mic != MicState::Recording, egui::Button::new("+ Thêm thư mục")).clicked()
                    && let Some(path) = rfd::FileDialog::new().set_title("Chọn thư mục cho NORA").pick_folder()
                {
                    self.pending_folder = Some(FolderDialog { path, can_read: true, can_create: false, allow_cloud: false });
                }
            });
            if self.state.snapshot.grants.is_empty() { ui.label(RichText::new("Chưa cấp quyền thư mục nào.").color(MUTED)); }
            for grant in self.state.snapshot.grants.clone() {
                ui.add_space(8.0);
                egui::Frame::new().fill(SURFACE).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.label(RichText::new(&grant.label).strong());
                    ui.add(egui::Label::new(&grant.path).wrap().selectable(true));
                    let rights: Vec<_> = [(grant.can_read, "Đọc"), (grant.can_create, "Tạo tệp mới"), (grant.allow_cloud, "Gửi nội dung được chọn tới LLM")].into_iter().filter_map(|(yes, text)| yes.then_some(text)).collect();
                    ui.label(RichText::new(rights.join(" · ")).size(12.0).color(ACCENT));
                    if ui.small_button("Thu hồi quyền").clicked() { self.send(UiCommand::RevokeFolder { id: grant.id }); }
                });
            }
            ui.add_space(24.0);
            ui.heading("Ứng dụng");
            ui.add(egui::Label::new(RichText::new("Tên hiển thị và ID ứng dụng được gửi tới LLM để chọn công cụ; đường dẫn và hash không được gửi.").size(12.0).color(MUTED)).wrap());
            ui.horizontal(|ui| {
                if ui.add_enabled(self.state.snapshot.mic != MicState::Recording, egui::Button::new("+ Thêm ứng dụng")).clicked()
                    && let Some(path) = rfd::FileDialog::new().set_title("Chọn ứng dụng cho phép NORA mở").add_filter("Ứng dụng Windows", &["exe"]).pick_file()
                {
                    let label = path.file_stem().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "Ứng dụng".into());
                    self.send(UiCommand::RegisterApp { path, label });
                }
            });
            ui.label(RichText::new("Mỗi lần mở ứng dụng đều có bản xem trước để bạn xác nhận.").size(12.0).color(MUTED));
            for app in self.state.snapshot.apps.clone() {
                ui.add_space(8.0);
                egui::Frame::new().fill(SURFACE).corner_radius(10).inner_margin(12).show(ui, |ui| {
                    ui.label(RichText::new(app.label).strong());
                    ui.add(egui::Label::new(app.path).wrap());
                    if ui.small_button("Thu hồi quyền mở ứng dụng").clicked() { self.send(UiCommand::RevokeApp { id: app.id }); }
                });
            }
        });
    }

    fn folder_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.pending_folder.take() else {
            return;
        };
        let mut keep = true;
        egui::Window::new("Cho phép truy cập thư mục").collapsible(false).resizable(false).default_width(470.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.add(egui::Label::new(dialog.path.to_string_lossy()).wrap().selectable(true));
            ui.add_space(12.0);
            ui.checkbox(&mut dialog.can_read, "Đọc và tìm tài liệu trong thư mục");
            ui.checkbox(&mut dialog.can_create, "Tạo tệp mới, không ghi đè tệp có sẵn");
            ui.add_space(8.0);
            ui.checkbox(&mut dialog.allow_cloud, "Cho phép gửi nội dung được chọn tới LLM");
            ui.add(egui::Label::new(RichText::new("Nếu bật, các nội dung được dùng để xử lý yêu cầu có thể được gửi tới nhà cung cấp LLM. Bạn có thể thu hồi quyền bất cứ lúc nào.").color(MUTED)).wrap());
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(dialog.can_read || dialog.can_create, egui::Button::new("Cấp các quyền đã chọn")).clicked()
                    && self.send(UiCommand::AddFolder { path: dialog.path.clone(), can_read: dialog.can_read, can_create: dialog.can_create, allow_cloud: dialog.allow_cloud })
                { keep = false; }
                if ui.button("Hủy").clicked() { keep = false; }
            });
        });
        if keep {
            self.pending_folder = Some(dialog);
        }
    }

    fn audio_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_audio.take() else {
            return;
        };
        let mut keep = true;
        egui::Window::new("Gửi yêu cầu từ tệp giọng nói").collapsible(false).resizable(false).default_width(470.0).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui| {
            ui.add(egui::Label::new(path.to_string_lossy()).wrap().selectable(true));
            ui.add_space(12.0);
            ui.label("Âm thanh được nhận diện trên máy và không tải lên nhà cung cấp LLM.");
            ui.add(egui::Label::new("Bản chép lời có thể được gửi tới LLM để xử lý yêu cầu. Nội dung tệp được xem là dữ liệu; việc cấp quyền và xác nhận hành động vẫn do bạn quyết định.").wrap());
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.state.can_import_audio(),egui::Button::new("Nhận diện và gửi yêu cầu")).clicked()
                    && self.send(UiCommand::ImportAudio {path:path.clone()}) {keep=false;}
                if ui.button("Hủy").clicked() {keep=false;}
            });
        });
        if keep {
            self.pending_audio = Some(path);
        }
    }
}

impl eframe::App for NoraWindow {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_requested {
            if self.options.close_to_tray && self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide(&ctx);
            } else {
                if self.runtime_disconnected || self.send(UiCommand::Quit) {
                    self.quit_requested = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
        }
        egui::Panel::left("navigation")
            .exact_size(240.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(SURFACE).inner_margin(18))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(24))
            .show(ui, |ui| {
                egui::Panel::top("page-title")
                    .resizable(false)
                    .frame(egui::Frame::new().inner_margin(egui::Margin {
                        bottom: 18,
                        ..Default::default()
                    }))
                    .show(ui, |ui| self.header(ui));
                match self.page {
                    Page::Chat => self.chat(ui),
                    Page::Goals => self.goals(ui),
                    Page::Permissions => self.permissions(ui),
                }
            });
        self.folder_dialog(&ctx);
        self.audio_dialog(&ctx);
        if let Some(capture) = &self.diagnostic {
            capture.on_frame(
                &ctx,
                !self.hidden && !self.minimized && !self.quit_requested,
            );
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.channels.commands.try_send(UiCommand::Quit);
    }
}

fn configure_theme(ctx: &egui::Context) {
    // eframe receives the OS theme after app creation. Pin the preference before
    // selecting a style so Windows Light cannot replace the customized palette.
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = SURFACE;
    style.visuals.extreme_bg_color = BACKGROUND;
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.disabled_alpha = 0.72;
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, FOREGROUND);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(35, 46, 60);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(31, 42, 55);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, FOREGROUND);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(46, 63, 77);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 57, 70);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(36, 66, 60);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(36, 66, 60);
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.5, FOREGROUND);
    style.visuals.widgets.open = style.visuals.widgets.inactive;
    style.visuals.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT);
    style.visuals.selection.bg_fill = Color32::from_rgb(40, 77, 72);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(16.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(24.0, FontFamily::Proportional),
    );
    ctx.set_style_of(egui::Theme::Dark, style);
}

fn configure_style(ctx: &egui::Context) {
    configure_theme(ctx);
    // Use the OS-installed font. No Windows font is bundled/redistributed.
    if let Some(windows) = std::env::var_os("WINDIR") {
        let path = PathBuf::from(windows).join("Fonts").join("segoeui.ttf");
        if let Ok(metadata) = std::fs::metadata(&path)
            && metadata.is_file()
            && metadata.len() <= 16_000_000
            && let Ok(bytes) = std::fs::read(path)
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "nora-system".into(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "nora-system".into());
            ctx.set_fonts(fonts);
        }
    }
}

fn create_tray(ctx: &egui::Context) -> (Option<TrayIcon>, Receiver<TraySignal>) {
    let (sender, receiver) = mpsc::sync_channel(16);
    let menu = Menu::new();
    let show = MenuItem::new("Mở NORA", true, None);
    let quit = MenuItem::new("Thoát NORA", true, None);
    if menu.append(&show).is_err() || menu.append(&quit).is_err() {
        return (None, receiver);
    }
    let Ok(icon) = Icon::from_rgba(icon_pixels(), 32, 32) else {
        return (None, receiver);
    };
    let Ok(tray) = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("NORA · Cùng bạn hoàn thành")
        .with_icon(icon)
        .build()
    else {
        return (None, receiver);
    };
    let show_id = show.id().clone();
    let quit_id = quit.id().clone();
    let menu_sender = sender.clone();
    let menu_context = ctx.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let signal = if event.id == show_id {
            Some(TraySignal::Show)
        } else if event.id == quit_id {
            Some(TraySignal::Quit)
        } else {
            None
        };
        if let Some(signal) = signal {
            let _ = menu_sender.try_send(signal);
            menu_context.request_repaint();
        }
    }));
    let tray_context = ctx.clone();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if matches!(
            event,
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } | TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = sender.try_send(TraySignal::Show);
            tray_context.request_repaint();
        }
    }));
    (Some(tray), receiver)
}

fn register_hotkey(ctx: &egui::Context) -> Option<HotkeyRegistration> {
    let manager = GlobalHotKeyManager::new().ok()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
    manager.register(hotkey).ok()?;
    let id = hotkey.id();
    let pressed = Arc::new(AtomicBool::new(false));
    let producer = pressed.clone();
    let context = ctx.clone();
    // Store the latest key state rather than enqueueing releases into a queue
    // that could fill. A lost release must not leave a microphone recording.
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id == id {
            producer.store(event.state == HotKeyState::Pressed, Ordering::Release);
            context.request_repaint();
        }
    }));
    Some(HotkeyRegistration {
        manager,
        hotkey,
        pressed,
    })
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).strong().color(MUTED));
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn icon_pixels() -> Vec<u8> {
    let mut rgba = vec![0; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let p = (y * 32 + x) * 4;
            let in_mark = (7..11).contains(&x) && (7..25).contains(&y)
                || (21..25).contains(&x) && (7..25).contains(&y)
                || (10..23).contains(&x) && (x..x + 4).contains(&y);
            let color = if in_mark {
                [119, 226, 193, 255]
            } else {
                [21, 28, 37, 255]
            };
            rgba[p..p + 4].copy_from_slice(&color);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(foreground: Color32, background: Color32) -> f64 {
        fn luminance(color: Color32) -> f64 {
            let linear = |channel: u8| {
                let value = f64::from(channel) / 255.0;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
        }
        let a = luminance(foreground);
        let b = luminance(background);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn light_system_theme_keeps_dark_readable_labels_and_controls() {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::ThemePreference::System);
        configure_theme(&ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_| {},
        );
        // A headless style test intentionally does not upload font textures.
        output.textures_delta.clear();
        assert_eq!(ctx.system_theme(), Some(egui::Theme::Light));
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        let style = ctx.global_style();
        assert!(style.visuals.dark_mode);
        assert!(contrast(style.visuals.text_color(), SURFACE) >= 4.5);
        assert!(contrast(style.visuals.weak_text_color(), BACKGROUND) >= 4.5);
        for widget in [
            style.visuals.widgets.inactive,
            style.visuals.widgets.hovered,
            style.visuals.widgets.active,
            style.visuals.widgets.open,
        ] {
            assert!(contrast(widget.fg_stroke.color, widget.bg_fill) >= 4.5);
            assert!(contrast(widget.fg_stroke.color, widget.weak_bg_fill) >= 4.5);
        }
    }
}
