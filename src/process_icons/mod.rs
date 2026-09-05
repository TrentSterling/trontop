//! Lazy, memory-only executable artwork. Native I/O never runs on the UI thread.
use eframe::egui::{self, Color32, TextureHandle, TextureId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

#[cfg(windows)]
mod native;

const SIDE: usize = 32;
const PIXEL_BYTES: usize = SIDE * SIDE * 4;
const CAPACITY: usize = 256;
const OUTSTANDING: usize = 32;
const UPLOADS_PER_FRAME: usize = 8;
const RETRY: Duration = Duration::from_secs(60);
const REFRESH: Duration = Duration::from_secs(600);

struct Pixels(Vec<u8>);
struct Request {
    id: u64,
    path: PathBuf,
}
struct Loaded {
    request: Request,
    pixels: Option<Pixels>,
}

struct Worker {
    requests: mpsc::SyncSender<Request>,
    results: mpsc::Receiver<Loaded>,
    stop: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // No join or forced cancellation. The one worker owns all in-flight native
        // resources. Dropping the receiver releases a blocked result sender too.
    }
}

struct Entry {
    texture: Option<TextureHandle>,
    pending: Option<u64>,
    refresh_at: Instant,
    used_frame: u64,
}

#[derive(Default)]
pub struct Cache {
    worker: Option<Worker>,
    entries: HashMap<PathBuf, Entry>,
    frame: u64,
    next_id: u64,
    outstanding: usize,
}

impl Cache {
    pub fn spawn(ctx: egui::Context) -> Self {
        #[cfg(windows)]
        {
            Self::with_loader(ctx, native::extract)
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            Self::default()
        }
    }

    fn with_loader(
        ctx: egui::Context,
        mut load: impl FnMut(&Path) -> Option<Pixels> + Send + 'static,
    ) -> Self {
        let (requests, receiver) = mpsc::sync_channel::<Request>(OUTSTANDING);
        let (sender, results) = mpsc::sync_channel(OUTSTANDING);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let result = std::thread::Builder::new()
            .name("trontop-icons".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let request = match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(request) => request,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    if stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let pixels = load(&request.path).filter(|p| p.0.len() == PIXEL_BYTES);
                    if stopped.load(Ordering::Relaxed)
                        || sender.send(Loaded { request, pixels }).is_err()
                    {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        if result.is_err() {
            return Self::default();
        }
        Self {
            worker: Some(Worker {
                requests,
                results,
                stop,
            }),
            ..Self::default()
        }
    }

    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.poll(ctx, Instant::now());
    }

    fn poll(&mut self, ctx: &egui::Context, now: Instant) {
        self.frame = self.frame.wrapping_add(1);
        for index in 0..UPLOADS_PER_FRAME {
            let Some(worker) = &self.worker else {
                break;
            };
            let loaded = match worker.results.try_recv() {
                Ok(loaded) => loaded,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    self.outstanding = 0;
                    for entry in self.entries.values_mut() {
                        entry.pending = None;
                    }
                    break;
                }
            };
            self.outstanding = self.outstanding.saturating_sub(1);
            if let Some(entry) = self.entries.get_mut(&loaded.request.path)
                && entry.pending == Some(loaded.request.id)
            {
                entry.pending = None;
                entry.refresh_at = now
                    + if loaded.pixels.is_some() {
                        REFRESH
                    } else {
                        RETRY
                    };
                if let Some(pixels) = loaded.pixels {
                    let image = egui::ColorImage::from_rgba_unmultiplied([SIDE, SIDE], &pixels.0);
                    // No executable paths in texture/debug names or diagnostics.
                    entry.texture = Some(ctx.load_texture(
                        format!("process-icon-{}", loaded.request.id),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                // A failed refresh leaves the prior artwork visible. It is identity
                // decoration, never a process-identity or telemetry authority.
            }
            if index + 1 == UPLOADS_PER_FRAME {
                ctx.request_repaint();
            }
        }
    }

    fn texture(&mut self, path: &Path, now: Instant) -> Option<TextureId> {
        let key = local_executable(path)?;
        if !self.entries.contains_key(&key) {
            self.worker.as_ref()?;
            if self.entries.len() >= CAPACITY {
                let victim = self
                    .entries
                    .iter()
                    .filter(|(_, e)| e.pending.is_none() && e.used_frame != self.frame)
                    .min_by_key(|(_, e)| e.used_frame)
                    .map(|(p, _)| p.clone())?;
                self.entries.remove(&victim);
            }
            self.entries.insert(
                key.clone(),
                Entry {
                    texture: None,
                    pending: None,
                    refresh_at: now,
                    used_frame: self.frame,
                },
            );
        }
        let entry = self.entries.get_mut(&key).unwrap();
        entry.used_frame = self.frame;
        if entry.pending.is_none()
            && now >= entry.refresh_at
            && self.outstanding < OUTSTANDING
            && let Some(worker) = &self.worker
        {
            self.next_id = self.next_id.wrapping_add(1);
            let request = Request {
                id: self.next_id,
                path: key,
            };
            if worker.requests.try_send(request).is_ok() {
                entry.pending = Some(self.next_id);
                self.outstanding += 1;
            }
        }
        entry.texture.as_ref().map(TextureHandle::id)
    }

    pub fn paint(
        &mut self,
        ui: &mut egui::Ui,
        path: Option<&Path>,
        size: f32,
        color: Color32,
        sense: egui::Sense,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(size), sense);
        if response.contains_pointer() {
            ui.painter().rect_filled(
                rect.expand(2.0),
                4.0,
                ui.visuals().widgets.hovered.weak_bg_fill,
            );
        }
        if let Some(texture) = path.and_then(|p| self.texture(p, Instant::now())) {
            ui.painter().image(
                texture,
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            crate::icons::Icon::Processes.paint(ui.painter(), rect, color);
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), "Process icon")
        });
        response
    }

    #[cfg(test)]
    pub(crate) fn fixture_icon(&mut self, ctx: &egui::Context, path: &Path, rgba: Vec<u8>) {
        assert_eq!(rgba.len(), PIXEL_BYTES);
        let texture = ctx.load_texture(
            "test-process-icon",
            egui::ColorImage::from_rgba_unmultiplied([SIDE, SIDE], &rgba),
            egui::TextureOptions::LINEAR,
        );
        self.entries.insert(
            local_executable(path).unwrap(),
            Entry {
                texture: Some(texture),
                pending: None,
                refresh_at: Instant::now() + REFRESH,
                used_frame: self.frame,
            },
        );
    }
}

/// Lexical only: never canonicalize/stat a path on the render thread.
fn local_executable(path: &Path) -> Option<PathBuf> {
    let raw = path.to_str()?;
    if raw.len() > 4096 || raw.chars().any(char::is_control) {
        return None;
    }
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(raw);
    let path = raw.replace('/', "\\");
    let bytes = path.as_bytes();
    if bytes.len() < 7
        || !bytes[0].is_ascii_alphabetic()
        || &bytes[1..3] != b":\\"
        || !path.to_ascii_lowercase().ends_with(".exe")
    {
        return None;
    }
    for component in path[3..].split('\\') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with(['.', ' '])
        {
            return None;
        }
    }
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests;
