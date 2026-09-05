#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    Show,
    Quit,
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
                put(&mut rgba, x, y, [18, 17, 27, 255]);
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
    use super::{TrayAction, icon_pixels};
    use eframe::egui;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    pub struct TrayController {
        tray: TrayIcon,
        pending: Arc<AtomicU8>,
        last_cpu_bucket: u8,
    }

    impl TrayController {
        pub fn new(ctx: egui::Context) -> Option<Self> {
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
                .with_icon(meter_icon(0.0).ok()?)
                .build()
                .ok()?;

            let pending = Arc::new(AtomicU8::new(0));
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

            Some(Self {
                tray,
                pending,
                last_cpu_bucket: u8::MAX,
            })
        }

        pub fn update(
            &mut self,
            cpu_percent: f32,
            memory_percent: f32,
            gpu_percent: Option<f32>,
            process_count: usize,
        ) {
            let bucket = (cpu_percent.clamp(0.0, 100.0) / 2.0).round() as u8;
            if bucket != self.last_cpu_bucket {
                if let Ok(icon) = meter_icon(cpu_percent) {
                    let _ = self.tray.set_icon(Some(icon));
                }
                self.last_cpu_bucket = bucket;
            }
            let gpu = gpu_percent.map_or_else(|| "warming".into(), |value| format!("{value:.1}%"));
            let tooltip = format!(
                "Trontop | CPU {cpu_percent:.1}% | Memory {memory_percent:.1}% | GPU {gpu} | {process_count} processes"
            );
            let _ = self.tray.set_tooltip(Some(tooltip));
        }

        pub fn poll(&self) -> Option<TrayAction> {
            match self.pending.swap(0, Ordering::AcqRel) {
                1 => Some(TrayAction::Show),
                2 => Some(TrayAction::Quit),
                _ => None,
            }
        }
    }

    fn meter_icon(cpu_percent: f32) -> Result<Icon, tray_icon::BadIcon> {
        Icon::from_rgba(icon_pixels(cpu_percent), 32, 32)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn live_icons_build_at_extremes() {
            assert!(meter_icon(0.0).is_ok());
            assert!(meter_icon(100.0).is_ok());
        }
    }
}

#[cfg(windows)]
pub use native::TrayController;

#[cfg(not(windows))]
pub struct TrayController;

#[cfg(not(windows))]
impl TrayController {
    pub fn new(_ctx: eframe::egui::Context) -> Option<Self> {
        None
    }

    pub fn update(
        &mut self,
        _cpu_percent: f32,
        _memory_percent: f32,
        _gpu_percent: Option<f32>,
        _process_count: usize,
    ) {
    }

    pub fn poll(&self) -> Option<TrayAction> {
        None
    }
}
