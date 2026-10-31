//! Opt-in, one-shot export of the framebuffer supplied by egui itself.
//! No OS capture API, other window, input automation or network is involved.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

const MAX_DIMENSION: usize = 4096;
const MAX_PIXELS: usize = 8_388_608;

fn dimensions(width: usize, height: usize) -> Option<(u32, u32)> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || width.checked_mul(height)? > MAX_PIXELS
    {
        return None;
    }
    Some((width as u32, height as u32))
}

fn status_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".status.json");
    PathBuf::from(name)
}

fn valid_destination(path: &Path) -> bool {
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        return false;
    }
    for component in path.components() {
        match component {
            Component::CurDir | Component::ParentDir => return false,
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name.contains(':') || name.ends_with('.') || name.ends_with(' ') {
                    return false;
                }
                let stem = name.split('.').next().unwrap_or_default();
                let reserved = stem.eq_ignore_ascii_case("CON")
                    || stem.eq_ignore_ascii_case("PRN")
                    || stem.eq_ignore_ascii_case("AUX")
                    || stem.eq_ignore_ascii_case("NUL")
                    || ["COM", "LPT"].iter().any(|prefix| {
                        stem.get(..3)
                            .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
                            && stem.get(3..).is_some_and(|n| {
                                matches!(
                                    n,
                                    "1" | "2"
                                        | "3"
                                        | "4"
                                        | "5"
                                        | "6"
                                        | "7"
                                        | "8"
                                        | "9"
                                        | "¹"
                                        | "²"
                                        | "³"
                                )
                            })
                    });
                if reserved {
                    return false;
                }
            }
            #[cfg(windows)]
            Component::Prefix(prefix) if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) => {
                // Keep this diagnostic local; disallow UNC/device namespaces.
                return false;
            }
            _ => {}
        }
    }
    true
}

struct CaptureFiles {
    image: File,
    status: File,
}

impl CaptureFiles {
    fn reserve(path: &Path) -> io::Result<Self> {
        if !valid_destination(path) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid screenshot destination",
            ));
        }
        // Reserve the status first. Existing image/status files are never opened
        // for writing, truncated, replaced or deleted, including on failure.
        let mut status = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(status_path(path))?;
        let image = match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(image) => image,
            Err(error) => {
                let _ = write_status(&mut status, "failed", None, Some("destination_unavailable"));
                return Err(error);
            }
        };
        Ok(Self { image, status })
    }

    fn fail(mut self, code: &'static str) {
        let _ = write_status(&mut self.status, "failed", None, Some(code));
    }
}

fn write_status(
    file: &mut File,
    status: &'static str,
    size: Option<(u32, u32)>,
    code: Option<&'static str>,
) -> io::Result<()> {
    // All string values are fixed internal literals. No paths, UI text,
    // credentials, raw pixels or underlying error bodies enter this artifact.
    write!(
        file,
        "{{\"scope\":\"nora_own_framebuffer\",\"status\":\"{status}\""
    )?;
    if let Some((width, height)) = size {
        write!(file, ",\"width\":{width},\"height\":{height}")?;
    }
    if let Some(code) = code {
        write!(file, ",\"code\":\"{code}\"")?;
    }
    writeln!(file, "}}")?;
    file.sync_all()
}

#[cfg(windows)]
pub(crate) use native::DiagnosticCapture;

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use eframe::egui;

    use crate::DesktopError;

    const INITIAL_PAINT_DELAY: Duration = Duration::from_secs(1);
    const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Phase {
        Waiting,
        Requested,
        Success,
        Failed,
    }

    struct CaptureState {
        phase: Phase,
        files: Option<CaptureFiles>,
        since: Option<Instant>,
    }

    #[derive(Clone)]
    pub(crate) struct DiagnosticCapture(Arc<Mutex<CaptureState>>);

    impl DiagnosticCapture {
        pub(crate) fn prepare(path: &Path) -> Result<Self, DesktopError> {
            let files =
                CaptureFiles::reserve(path).map_err(|_| DesktopError::DiagnosticScreenshot)?;
            Ok(Self(Arc::new(Mutex::new(CaptureState {
                phase: Phase::Waiting,
                files: Some(files),
                since: None,
            }))))
        }

        pub(crate) fn fail(&self, code: &'static str) {
            if let Ok(mut state) = self.0.lock()
                && !matches!(state.phase, Phase::Success | Phase::Failed)
            {
                state.phase = Phase::Failed;
                if let Some(files) = state.files.take() {
                    files.fail(code);
                }
            }
        }

        pub(crate) fn finish(&self) -> Result<(), DesktopError> {
            self.fail("closed_before_capture");
            match self.0.lock() {
                Ok(state) if state.phase == Phase::Success => Ok(()),
                _ => Err(DesktopError::DiagnosticScreenshot),
            }
        }

        pub(crate) fn on_frame(&self, ctx: &egui::Context, visible: bool) {
            let Ok(mut state) = self.0.lock() else {
                return;
            };
            if matches!(state.phase, Phase::Success | Phase::Failed) {
                return;
            }
            let elapsed = state.since.get_or_insert_with(Instant::now).elapsed();
            if elapsed >= CAPTURE_TIMEOUT {
                drop(state);
                self.fail("capture_timeout");
                return;
            }
            if state.phase == Phase::Requested || !visible {
                ctx.request_repaint_after(CAPTURE_TIMEOUT - elapsed);
                return;
            }
            if elapsed < INITIAL_PAINT_DELAY {
                ctx.request_repaint_after(INITIAL_PAINT_DELAY - elapsed);
                return;
            }
            let size = ctx.viewport_rect().size() * ctx.pixels_per_point();
            if !size.x.is_finite()
                || !size.y.is_finite()
                || dimensions(size.x.ceil() as usize, size.y.ceil() as usize).is_none()
            {
                drop(state);
                self.fail("invalid_dimensions");
                return;
            }
            state.phase = Phase::Requested;
            state.since = Some(Instant::now());
            drop(state);
            let capture = self.clone();
            ctx.request_screenshot(move |image| capture.complete(&image));
            ctx.request_repaint_after(CAPTURE_TIMEOUT);
        }

        fn complete(&self, image: &egui::ColorImage) {
            let Ok(mut state) = self.0.lock() else {
                return;
            };
            // A callback arriving after failure/timeout cannot write the files.
            if state.phase != Phase::Requested {
                return;
            }
            let Some(mut files) = state.files.take() else {
                state.phase = Phase::Failed;
                return;
            };
            let Some(size) = dimensions(image.size[0], image.size[1]) else {
                state.phase = Phase::Failed;
                files.fail("invalid_dimensions");
                return;
            };
            if image.pixels.len() != image.size[0] * image.size[1] {
                state.phase = Phase::Failed;
                files.fail("invalid_pixel_count");
                return;
            }
            if encode(&mut files.image, image, size).is_err() {
                state.phase = Phase::Failed;
                files.fail("png_write_failed");
                return;
            }
            state.phase = if write_status(&mut files.status, "ok", Some(size), None).is_ok() {
                Phase::Success
            } else {
                Phase::Failed
            };
        }
    }

    fn encode(
        file: &mut File,
        image: &egui::ColorImage,
        size: (u32, u32),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut encoder = png::Encoder::new(&mut *file, size.0, size.1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header()?;
        {
            let mut stream = writer.stream_writer()?;
            // Keep one <=16 KiB row rather than copy the whole framebuffer.
            let mut row = Vec::with_capacity(image.size[0] * 4);
            for pixels in image.pixels.chunks_exact(image.size[0]) {
                row.clear();
                for pixel in pixels {
                    row.extend_from_slice(&pixel.to_srgba_unmultiplied());
                }
                stream.write_all(&row)?;
            }
            stream.finish()?;
        }
        writer.finish()?;
        file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "nora-framebuffer-{}-{stamp}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn image(&self) -> PathBuf {
            self.0.join("nora.png")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.image());
            let _ = std::fs::remove_file(status_path(&self.image()));
            let _ = std::fs::remove_dir(&self.0);
        }
    }

    #[test]
    fn framebuffer_bounds_reject_empty_overflow_and_large_images() {
        assert_eq!(dimensions(4096, 2048), Some((4096, 2048)));
        assert!(dimensions(0, 1).is_none());
        assert!(dimensions(4097, 1).is_none());
        assert!(dimensions(4096, 2049).is_none());
        assert!(dimensions(usize::MAX, usize::MAX).is_none());
    }

    #[test]
    fn existing_image_is_not_overwritten_and_failure_is_reported() {
        let dir = TestDirectory::new();
        let path = dir.image();
        std::fs::write(&path, b"existing image").unwrap();
        assert!(CaptureFiles::reserve(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing image");
        let status = std::fs::read_to_string(status_path(&path)).unwrap();
        assert!(status.contains("\"status\":\"failed\""));
        assert!(status.contains("destination_unavailable"));
    }

    #[test]
    fn existing_status_is_not_overwritten_or_image_created() {
        let dir = TestDirectory::new();
        let path = dir.image();
        let status = status_path(&path);
        std::fs::write(&status, b"existing status").unwrap();
        assert!(CaptureFiles::reserve(&path).is_err());
        assert_eq!(std::fs::read(status).unwrap(), b"existing status");
        assert!(!path.exists());
    }

    #[test]
    fn abandoned_capture_reports_only_fixed_failure_metadata() {
        let dir = TestDirectory::new();
        let path = dir.image();
        let files = CaptureFiles::reserve(&path).unwrap();
        assert_eq!(files.image.metadata().unwrap().len(), 0);
        files.fail("closed_before_capture");
        let status: serde_json::Value =
            serde_json::from_slice(&std::fs::read(status_path(&path)).unwrap()).unwrap();
        assert_eq!(status["scope"], "nora_own_framebuffer");
        assert_eq!(status["status"], "failed");
        assert_eq!(status["code"], "closed_before_capture");
        assert_eq!(status.as_object().unwrap().len(), 3);
    }
}
