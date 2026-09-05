#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    Show,
    Quit,
}

#[derive(Clone, Copy)]
pub struct TraySample {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub gpu_percent: Option<f32>,
    pub process_count: usize,
}

// Twenty-six real observations, one column each. History advances on new samples,
// even when rounded CPU utilization stays unchanged.
#[derive(Default)]
struct CpuMeter {
    history: std::collections::VecDeque<f32>,
}

impl CpuMeter {
    fn sample(&mut self, cpu: f32) -> Vec<u8> {
        let cpu = if cpu.is_finite() {
            cpu.clamp(0.0, 100.0)
        } else {
            0.0
        };
        if self.history.len() == 26 {
            self.history.pop_front();
        }
        self.history.push_back(cpu);
        let mut rgba = vec![0; 32 * 32 * 4];
        let fill_top = 29 - (cpu / 100.0 * 26.0).round() as u32;
        for y in 1..31 {
            for x in 1..31 {
                let color = if x == 1 || x == 30 || y == 1 || y == 30 {
                    [139, 153, 182]
                } else if (3..29).contains(&x) && y >= fill_top && y < 29 {
                    mix([125, 58, 207], [34, 173, 168], (29 - y) as f32 / 26.0)
                } else {
                    [14, 18, 28]
                };
                put(&mut rgba, x, y, [color[0], color[1], color[2], 255]);
            }
        }
        // Bright moving trace against a subdued full-width level meter. Both survive
        // Windows reducing the 32px source to a 16px notification icon.
        let mut previous = None;
        let start_x = 29 - self.history.len() as u32;
        for (index, value) in self.history.iter().enumerate() {
            let x = start_x + index as u32;
            let y = 28 - (value / 100.0 * 24.0).round() as u32;
            let previous_y = previous.unwrap_or(y);
            for line_y in y.min(previous_y)..=y.max(previous_y) {
                for dy in 0..2 {
                    put(&mut rgba, x, (line_y + dy).min(29), [163, 255, 230, 255]);
                }
            }
            previous = Some(y);
        }
        rgba
    }
}

pub fn window_icon() -> std::sync::Arc<eframe::egui::IconData> {
    std::sync::Arc::new(eframe::egui::IconData {
        rgba: icon_pixels(36.0),
        width: 32,
        height: 32,
    })
}

fn icon_pixels(cpu_percent: f32) -> Vec<u8> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    let cpu = cpu_percent.clamp(0.0, 100.0) / 100.0;
    let meter = if cpu < 0.70 {
        mix([168, 85, 247], [46, 230, 215], cpu / 0.70)
    } else {
        mix([46, 230, 215], [242, 79, 92], (cpu - 0.70) / 0.30)
    };

    for y in 2..30 {
        for x in 2..30 {
            let corner_x = 7_u32.saturating_sub(x).max(x.saturating_sub(24));
            let corner_y = 7_u32.saturating_sub(y).max(y.saturating_sub(24));
            if corner_x * corner_x + corner_y * corner_y <= 25 {
                let signal = mix(
                    [79, 35, 132],
                    [14, 102, 99],
                    (x + y).saturating_sub(4) as f32 / 56.0,
                );
                put(&mut rgba, x, y, [signal[0], signal[1], signal[2], 255]);
            }
        }
    }
    for x in 7..25 {
        for y in 6..10 {
            put(&mut rgba, x, y, [224, 220, 238, 255]);
        }
    }
    let fill_top = 25_i32 - (15.0 * cpu).round() as i32;
    for x in 14..18 {
        for y in 10..25 {
            let color = if y as i32 >= fill_top {
                meter
            } else {
                [71, 63, 91]
            };
            put(&mut rgba, x, y, [color[0], color[1], color[2], 255]);
        }
    }
    for x in 5..27 {
        put(&mut rgba, x, 27, [55, 49, 69, 255]);
    }
    let meter_width = (22.0 * cpu).round() as u32;
    for x in 5..(5 + meter_width) {
        for y in 27..29 {
            put(&mut rgba, x, y, [meter[0], meter[1], meter[2], 255]);
        }
    }
    rgba
}

fn put(buffer: &mut [u8], x: u32, y: u32, color: [u8; 4]) {
    let index = ((y * 32 + x) * 4) as usize;
    buffer[index..index + 4].copy_from_slice(&color);
}

fn mix(a: [u8; 3], b: [u8; 3], amount: f32) -> [u8; 3] {
    let amount = amount.clamp(0.0, 1.0);
    let channel =
        |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * amount).round() as u8;
    [
        channel(a[0], b[0]),
        channel(a[1], b[1]),
        channel(a[2], b[2]),
    ]
}

#[cfg(windows)]
mod native {
    use super::{CpuMeter, TrayAction, TraySample};
    use eframe::egui;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread::{self, JoinHandle};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
        TranslateMessage, WM_APP, WM_QUIT,
    };

    const SAMPLE_READY: u32 = WM_APP + 27;

    #[derive(Clone)]
    pub struct TraySink {
        latest: Arc<Mutex<Option<TraySample>>>,
        thread_id: u32,
    }

    impl TraySink {
        pub fn publish(&self, sample: TraySample) {
            if let Ok(mut latest) = self.latest.lock() {
                *latest = Some(sample);
            }
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, SAMPLE_READY, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub struct TrayController {
        sink: TraySink,
        pending: Arc<AtomicU8>,
        worker: Option<JoinHandle<()>>,
        #[cfg(test)]
        applied: Arc<std::sync::atomic::AtomicU64>,
    }

    impl TrayController {
        pub fn new(ctx: egui::Context) -> Option<Self> {
            let latest = Arc::new(Mutex::new(None));
            let worker_latest = Arc::clone(&latest);
            let pending = Arc::new(AtomicU8::new(0));
            let worker_pending = Arc::clone(&pending);
            #[cfg(test)]
            let applied = Arc::new(std::sync::atomic::AtomicU64::new(0));
            #[cfg(test)]
            let worker_applied = Arc::clone(&applied);
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let worker = thread::Builder::new()
                .name("trontop-tray".into())
                .spawn(move || {
                    let mut message = MSG::default();
                    unsafe {
                        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                    }
                    let Some(tray) = create_native_tray(ctx, worker_pending) else {
                        let _ = ready_tx.send(None);
                        return;
                    };
                    let thread_id = unsafe { GetCurrentThreadId() };
                    if ready_tx.send(Some(thread_id)).is_err() {
                        return;
                    }
                    let mut meter = CpuMeter::default();
                    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
                        if message.hwnd.is_invalid() && message.message == SAMPLE_READY {
                            let sample = worker_latest
                                .lock()
                                .ok()
                                .and_then(|mut latest| latest.take());
                            if let Some(sample) = sample {
                                let result = update_native_tray(&tray, &mut meter, sample);
                                #[cfg(test)]
                                if result.is_ok() {
                                    worker_applied.fetch_add(1, Ordering::Release);
                                }
                                if let Err(error) = result {
                                    eprintln!("Trontop tray update failed: {error}");
                                }
                            }
                        } else {
                            unsafe {
                                let _ = TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                        }
                    }
                    // Tray/menu handles must also be destroyed on their owner thread.
                    drop(tray);
                })
                .ok()?;
            match ready_rx.recv().ok().flatten() {
                Some(thread_id) => Some(Self {
                    sink: TraySink { latest, thread_id },
                    pending,
                    worker: Some(worker),
                    #[cfg(test)]
                    applied,
                }),
                None => {
                    let _ = worker.join();
                    None
                }
            }
        }

        pub fn sink(&self) -> TraySink {
            self.sink.clone()
        }

        pub fn poll(&self) -> Option<TrayAction> {
            match self.pending.swap(0, Ordering::AcqRel) {
                1 => Some(TrayAction::Show),
                2 => Some(TrayAction::Quit),
                _ => None,
            }
        }
    }

    impl Drop for TrayController {
        fn drop(&mut self) {
            unsafe {
                let _ = PostThreadMessageW(self.sink.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(worker) = self.worker.take() {
                crate::shutdown::finish(worker, std::time::Duration::from_millis(100));
            }
        }
    }

    fn create_native_tray(ctx: egui::Context, pending: Arc<AtomicU8>) -> Option<TrayIcon> {
        let menu = Menu::new();
        let show = MenuItem::new("Show Trontop", true, None);
        let quit = MenuItem::new("Quit Trontop", true, None);
        let show_id = show.id().clone();
        let quit_id = quit.id().clone();
        let _ = menu.append(&show);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Trontop | Starting native telemetry")
            .with_icon(Icon::from_rgba(CpuMeter::default().sample(0.0), 32, 32).ok()?)
            .build()
            .ok()?;

        let click_pending = Arc::clone(&pending);
        let click_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            ) {
                click_pending.store(1, Ordering::Release);
                click_ctx.request_repaint();
            }
        }));

        let menu_pending = Arc::clone(&pending);
        let menu_ctx = ctx;
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                menu_pending.store(1, Ordering::Release);
                menu_ctx.request_repaint();
            } else if event.id == quit_id {
                menu_pending.store(2, Ordering::Release);
                menu_ctx.request_repaint();
            }
        }));

        Some(tray)
    }

    fn update_native_tray(
        tray: &TrayIcon,
        meter: &mut CpuMeter,
        sample: TraySample,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let icon = Icon::from_rgba(meter.sample(sample.cpu_percent), 32, 32)?;
        tray.set_icon(Some(icon))?;
        let gpu = sample
            .gpu_percent
            .map_or_else(|| "warming".into(), |value| format!("{value:.1}%"));
        let tooltip = format!(
            "Trontop | CPU {:.1}% | Memory {:.1}% | GPU {gpu} | {} processes",
            sample.cpu_percent, sample.memory_percent, sample.process_count
        );
        tray.set_tooltip(Some(tooltip))?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn live_icons_build_at_extremes() {
            for cpu in [0.0, 100.0] {
                assert!(Icon::from_rgba(CpuMeter::default().sample(cpu), 32, 32).is_ok());
            }
        }

        #[test]
        #[ignore = "requires an interactive Windows desktop; creates and removes a real tray icon"]
        fn native_tray_updates_without_any_ui_frames() {
            let tray = TrayController::new(egui::Context::default()).expect("native tray");
            for (index, cpu) in [5.0, 80.0, 80.0, 20.0].into_iter().enumerate() {
                tray.sink().publish(TraySample {
                    cpu_percent: cpu,
                    memory_percent: 40.0,
                    gpu_percent: Some(10.0),
                    process_count: 123,
                });
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while tray.applied.load(Ordering::Acquire) <= index as u64 {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "tray update stalled without UI frames"
                    );
                    thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            assert_eq!(tray.applied.load(Ordering::Acquire), 4);
        }
    }
}

#[cfg(windows)]
pub use native::{TrayController, TraySink};

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
#[derive(Clone)]
pub struct TraySink;

#[cfg(not(windows))]
impl TraySink {
    pub fn publish(&self, _sample: TraySample) {}
}

#[cfg(not(windows))]
impl TrayController {
    pub fn new(_ctx: eframe::egui::Context) -> Option<Self> {
        None
    }

    pub fn sink(&self) -> TraySink {
        TraySink
    }

    pub fn poll(&self) -> Option<TrayAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_level_uses_a_visible_area_and_history_advances_at_constant_load() {
        let low = CpuMeter::default().sample(0.0);
        let high = CpuMeter::default().sample(100.0);
        let changed = low
            .chunks_exact(4)
            .zip(high.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 600,
            "CPU change should affect most of the icon, got {changed} pixels"
        );
        let mut meter = CpuMeter::default();
        meter.sample(90.0);
        let first = meter.sample(20.0);
        assert_ne!(
            first,
            meter.sample(20.0),
            "history must move even within the same CPU bucket"
        );
        for _ in 0..100 {
            meter.sample(20.0);
        }
        assert_eq!(meter.history.len(), 26);
    }
}
