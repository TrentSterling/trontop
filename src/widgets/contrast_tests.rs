use super::*;

fn shapes<'a>(shape: &'a egui::Shape, output: &mut Vec<&'a egui::Shape>) {
    if let egui::Shape::Vec(children) = shape {
        for child in children {
            shapes(child, output);
        }
    } else {
        output.push(shape);
    }
}

fn text_and_fill(output: &egui::FullOutput, label: &str) -> (egui::Rect, Color32, Color32) {
    let mut flat = Vec::new();
    for clipped in &output.shapes {
        shapes(&clipped.shape, &mut flat);
    }
    let (index, text) = flat
        .iter()
        .enumerate()
        .find_map(|(i, shape)| {
            if let egui::Shape::Text(text) = shape
                && text.galley.job.text == label
            {
                Some((i, text))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing text {label}"));
    let rect = text.visual_bounding_rect();
    let fill = flat[..index]
        .iter()
        .rev()
        .find_map(|shape| {
            if let egui::Shape::Rect(shape) = shape
                && shape.fill.a() == 255
                && shape.rect.contains(rect.center())
            {
                Some(shape.fill)
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("missing opaque face for {label}"));
    let color = text.override_text_color.unwrap_or_else(|| {
        let color = text.galley.job.sections[0].format.color;
        if color == Color32::PLACEHOLDER {
            text.fallback_color
        } else {
            color
        }
    });
    (rect, color, fill)
}

fn assert_readable(output: &egui::FullOutput, label: &str) -> egui::Rect {
    let (rect, color, fill) = text_and_fill(output, label);
    let ratio = theme::contrast_ratio(color, fill);
    assert!(ratio >= 4.5, "{label}: {color:?} on {fill:?} = {ratio}");
    rect
}

#[test]
fn contrast_action_text_tracks_idle_hover_pressed_and_keyboard_focus() {
    for dark in [true, false] {
        for accent in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 255, 0],
            [25, 101, 226],
            [168, 85, 247],
        ] {
            for icon in [false, true] {
                let settings = ThemeSettings {
                    dark,
                    accent,
                    secondary: [255, 255, 255],
                    ..Default::default()
                };
                let t = theme::tokens(settings);
                let ctx = egui::Context::default();
                theme::install(&ctx, settings);
                ctx.style_mut_of(
                    if dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    },
                    |style| style.animation_time = 0.0,
                );
                let mut button = egui::Rect::NOTHING;
                let mut text_rect = None;
                let mut id = None;
                for phase in 0..6 {
                    let pointer = if matches!(phase, 1..=3) {
                        button.center()
                    } else {
                        egui::pos2(630.0, 470.0)
                    };
                    let mut events = vec![egui::Event::PointerMoved(pointer)];
                    if matches!(phase, 2 | 3) {
                        events.push(egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed: phase == 2,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                    if phase == 4 {
                        ctx.memory_mut(|m| m.request_focus(id.unwrap()));
                    }
                    if phase == 5 {
                        ctx.memory_mut(|m| m.surrender_focus(id.unwrap()));
                    }
                    let mut output = egui::FullOutput::default();
                    for warm in 0..3 {
                        output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::new(640.0, 480.0),
                                )),
                                events: if warm == 0 { events.clone() } else { vec![] },
                                ..Default::default()
                            },
                            |ui| {
                                let response = if icon {
                                    icon_button(
                                        ui,
                                        Icon::Startup,
                                        "Contrast action",
                                        Vec2::new(180.0, 32.0),
                                        t.accent,
                                        t,
                                    )
                                } else {
                                    // Explicit caller color was one source of the regression.
                                    action_button(
                                        ui,
                                        RichText::new("Contrast action").color(Color32::WHITE),
                                        Vec2::new(180.0, 32.0),
                                        t.accent,
                                        t,
                                    )
                                };
                                button = response.rect;
                                id = Some(response.id);
                            },
                        );
                    }
                    let rect = assert_readable(&output, "Contrast action");
                    if let Some(before) = text_rect {
                        assert_eq!(rect, before, "contrast changed geometry");
                    }
                    text_rect = Some(rect);
                }
            }
        }
    }
}

#[test]
fn contrast_badges_and_selected_device_captions_survive_extreme_themes() {
    for dark in [true, false] {
        for accent in [[0, 0, 0], [255, 255, 255], [255, 255, 0], [0, 255, 255]] {
            let settings = ThemeSettings {
                dark,
                accent,
                secondary: accent,
                hover_strength: 0.30,
                zebra_strength: 0.18,
                ..Default::default()
            };
            let t = theme::tokens(settings);
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let mut target = egui::Rect::NOTHING;
            for widget in 0..3 {
                for hovered in [false, true] {
                    let mut output = egui::FullOutput::default();
                    for _ in 0..3 {
                        output = ctx.run_ui(
                            egui::RawInput {
                                events: vec![egui::Event::PointerMoved(if hovered {
                                    target.center()
                                } else {
                                    egui::pos2(900.0, 900.0)
                                })],
                                ..Default::default()
                            },
                            |ui| {
                                ui.set_width(220.0);
                                match widget {
                                    0 => status_pill(ui, "Sensor status", t.accent),
                                    1 => {
                                        device_button(
                                            ui,
                                            true,
                                            "DEVICE CAPTION",
                                            "42.0%",
                                            &VecDeque::new(),
                                            t.accent,
                                            t,
                                        );
                                    }
                                    _ => inventory_status(
                                        ui,
                                        "Provider",
                                        "Cached status",
                                        "Last usable sample",
                                        t.secondary,
                                        t,
                                        true,
                                    ),
                                }
                            },
                        );
                    }
                    let label = ["Sensor status", "DEVICE CAPTION", "Cached status"][widget];
                    target = assert_readable(&output, label);
                }
            }
        }
    }
}

#[test]
fn contrast_installed_selection_and_hover_surfaces_preserve_small_text() {
    for dark in [true, false] {
        for accent in [
            [0, 0, 0],
            [255, 255, 255],
            [255, 255, 0],
            [255, 0, 255],
            [0, 255, 255],
        ] {
            let s = ThemeSettings {
                dark,
                accent,
                secondary: accent,
                hover_strength: 0.30,
                zebra_strength: 0.18,
                column_strength: 0.16,
                ..Default::default()
            };
            let t = theme::tokens(s);
            let ctx = egui::Context::default();
            theme::install(&ctx, s);
            let style = ctx.style_of(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            let v = &style.visuals;
            for bg in [
                t.panel,
                t.panel_raised,
                t.row_hover,
                t.accent_dim,
                v.faint_bg_color,
                v.selection.bg_fill,
                v.widgets.hovered.weak_bg_fill,
                v.widgets.active.weak_bg_fill,
                v.widgets.open.weak_bg_fill,
            ] {
                for fg in [t.text, t.text_muted, t.ink(t.accent)] {
                    assert!(theme::contrast_ratio(fg, bg) >= 4.5, "{fg:?} on {bg:?}");
                }
                assert!(theme::contrast_ratio(v.weak_text_color(), bg) >= 4.5);
            }
            // Saved RGB values and the swatch/gradient model are not remapped.
            assert_eq!(s.accent, accent);
            assert_eq!(t.accent.to_array()[..3], accent);
        }
    }
}

#[test]
fn contrast_heat_values_keep_ink_on_bounded_opaque_tiles() {
    for dark in [true, false] {
        for color in [[0, 0, 0], [255, 255, 255], [255, 255, 0], [0, 255, 255]] {
            let s = ThemeSettings {
                dark,
                accent: color,
                secondary: color,
                ..Default::default()
            };
            let t = theme::tokens(s);
            let ctx = egui::Context::default();
            theme::install(&ctx, s);
            for value in [0.1, 37.0, 100.0] {
                let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            egui::pos2(20.0, 20.0),
                            Vec2::new(110.0, 30.0),
                        )),
                        |ui| {
                            heat_cell(ui, value, "Heat value".into(), t.accent, t);
                        },
                    );
                });
                assert_readable(&output, "Heat value");
            }
        }
    }
}
