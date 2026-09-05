//! Trontop local patch (MIT): non-blocking native GPU recovery and texture replay.
//! No native windows, event loops, process restarts or input are created here.
use crate::{RenderState, RendererEvent, RendererOptions, WgpuConfiguration};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

/// CPU mirror of live managed textures, not a history of uploads. Partial atlas
/// updates are merged; retired textures disappear at the next frame boundary.
#[derive(Default)]
pub struct TextureReplay {
    images: HashMap<epaint::TextureId, epaint::ImageDelta>,
    retired: Vec<epaint::TextureId>,
    pending: bool,
}

impl TextureReplay {
    /// Keep replay pending across frames that cannot recreate a native surface.
    /// `apply` continues to merge new deltas until a guarded upload is possible.
    pub fn invalidate(&mut self) {
        self.pending = true;
    }

    /// Call only after surface recreation and under the painter's queue guard.
    /// A full replay already includes this frame's delta, so skip its usual upload.
    pub fn upload_if_pending(&mut self, state: &RenderState) -> bool {
        if !self.pending {
            return false;
        }
        self.upload(state);
        self.pending = false;
        true
    }

    pub fn image(&self, id: epaint::TextureId) -> Option<&epaint::ImageDelta> {
        self.images.get(&id)
    }

    pub fn apply(&mut self, delta: &epaint::textures::TexturesDelta) {
        for id in self.retired.drain(..) {
            self.images.remove(&id);
        }
        for (id, change) in &delta.set {
            if let Some([x, y]) = change.pos {
                if let Some(current) = self.images.get_mut(id) {
                    let epaint::ImageData::Color(source) = &change.image;
                    let epaint::ImageData::Color(target) = &mut current.image;
                    if x.saturating_add(source.width()) <= target.width()
                        && y.saturating_add(source.height()) <= target.height()
                    {
                        let target = Arc::make_mut(target);
                        for row in 0..source.height() {
                            let start = (y + row) * target.width() + x;
                            let source_start = row * source.width();
                            target.pixels[start..start + source.width()].copy_from_slice(
                                &source.pixels[source_start..source_start + source.width()],
                            );
                        }
                        current.options = change.options;
                    }
                }
            } else {
                self.images.insert(*id, change.clone());
            }
        }
        self.retired.clone_from(&delta.free);
    }

    pub fn upload(&self, state: &RenderState) {
        let mut renderer = state.renderer.write();
        for (id, image) in &self.images {
            renderer.update_texture(&state.device, &state.queue, *id, image);
        }
    }

    pub fn bytes(&self) -> usize {
        self.images
            .values()
            .map(|i| i.image.width() * i.image.height() * 4)
            .sum()
    }
}

/// At most one replacement-device worker. Polling/drop never joins a driver call.
/// A failed attempt backs off rather than spinning or growing a thread backlog.
#[derive(Default)]
pub struct DeviceRecovery {
    lost: Arc<AtomicBool>,
    pending: Option<mpsc::Receiver<Option<RenderState>>>,
    retry_at: Option<Instant>,
    failures: u32,
}

impl DeviceRecovery {
    pub fn arm(
        &mut self,
        state: &RenderState,
        wake: Arc<dyn Fn() + Send + Sync>,
        config: &WgpuConfiguration,
    ) {
        self.lost = Arc::new(AtomicBool::new(false));
        let lost = Arc::clone(&self.lost);
        let report = Arc::clone(&config.on_renderer_event);
        state
            .device
            .set_device_lost_callback(move |reason, _message| {
                // Normal destruction must not look like a hardware fault. Upload
                // failure still detects explicit device.destroy() in fault tests.
                if reason != wgpu::DeviceLostReason::Destroyed && !lost.swap(true, Ordering::AcqRel)
                {
                    report(RendererEvent::DeviceLost);
                    wake();
                }
            });
    }

    pub fn needed(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    pub fn fail_upload(&self, config: &WgpuConfiguration) {
        if !self.lost.swap(true, Ordering::AcqRel) {
            (config.on_renderer_event)(RendererEvent::UploadFailed);
        }
    }

    pub fn poll_or_start(
        &mut self,
        config: &WgpuConfiguration,
        instance: &wgpu::Instance,
        surface: Option<Arc<wgpu::Surface<'static>>>,
        options: RendererOptions,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Option<RenderState> {
        if !self.needed() {
            return None;
        }
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(Some(state)) => {
                    self.pending = None;
                    // Keep the minimum interval between attempts even if the
                    // replacement immediately fails its first frame.
                    self.failures = 0;
                    self.arm(&state, wake, config);
                    return Some(state);
                }
                Err(mpsc::TryRecvError::Empty) => return None,
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.failures = self.failures.saturating_add(1);
                    self.retry_at =
                        Some(Instant::now() + Duration::from_secs(1u64 << self.failures.min(5)));
                    (config.on_renderer_event)(RendererEvent::RecoveryFailed);
                    return None;
                }
            }
        }
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return None;
        }
        let (send, receive) = mpsc::sync_channel(1);
        let config_copy = config.clone();
        let instance = instance.clone();
        let spawn = std::thread::Builder::new()
            .name("trontop-gpu-recovery".into())
            .spawn(move || {
                let state = pollster::block_on(RenderState::create(
                    &config_copy,
                    &instance,
                    surface.as_deref(),
                    options,
                ))
                .ok();
                // Release the selection-only surface reference before publishing.
                // The UI may immediately replace/drop its old swapchain; the
                // worker must not keep that swapchain alive on the same HWND.
                drop(surface);
                let _ = send.send(state);
                wake();
            });
        match spawn {
            Ok(_handle) => {
                self.pending = Some(receive);
                self.retry_at = Some(Instant::now() + Duration::from_secs(1));
                (config.on_renderer_event)(RendererEvent::RecoveryStarted);
            }
            Err(_) => {
                self.retry_at = Some(Instant::now() + Duration::from_secs(5));
                (config.on_renderer_event)(RendererEvent::RecoveryFailed);
            }
        }
        None
    }
}
