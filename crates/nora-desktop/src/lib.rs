//! Native UI only: no LLM, filesystem broker, microphone or model executes here.
//! All user actions cross a bounded command channel to the trusted runtime.

pub mod contracts;
pub mod state;

#[cfg(windows)]
mod windows;

use thiserror::Error;

pub use contracts::{AppSnapshot, UiChannels};

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("NORA desktop hiện hỗ trợ Windows")]
    UnsupportedPlatform,
    #[error("Không thể khởi tạo cửa sổ NORA. Hãy kiểm tra driver hiển thị.")]
    Initialization,
    #[error("Chế độ kiểm tra giao diện qua mạng không được phép trong bản chạy này")]
    InspectionDisabled,
}

/// Options are selected by the trusted launcher, never by model output.
#[derive(Debug, Clone)]
pub struct DesktopOptions {
    pub start_hidden: bool,
    pub close_to_tray: bool,
}

impl Default for DesktopOptions {
    fn default() -> Self {
        Self {
            start_hidden: false,
            close_to_tray: true,
        }
    }
}

pub fn run(channels: UiChannels, initial: AppSnapshot) -> Result<(), DesktopError> {
    run_with_options(channels, initial, DesktopOptions::default())
}

#[cfg(windows)]
pub fn run_with_options(
    channels: UiChannels,
    initial: AppSnapshot,
    options: DesktopOptions,
) -> Result<(), DesktopError> {
    windows::run(channels, initial, options)
}

#[cfg(not(windows))]
pub fn run_with_options(
    _channels: UiChannels,
    _initial: AppSnapshot,
    _options: DesktopOptions,
) -> Result<(), DesktopError> {
    Err(DesktopError::UnsupportedPlatform)
}
