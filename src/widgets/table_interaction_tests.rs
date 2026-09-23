use super::*;

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn focus_stroke(output: &egui::FullOutput, rect: egui::Rect) -> Option<Stroke> {
    output.shapes.iter().find_map(|clipped| {
        if let egui::Shape::Rect(shape) = &clipped.shape
            && shape.rect == rect.expand2(Vec2::new(4.0, 0.0)).shrink(1.0)
            && shape.stroke.width > 0.0
        {
            assert!(clipped.clip_rect.contains_rect(shape.rect));
            Some(shape.stroke)
        } else {
            None
        }
    })
}

#[test]
fn table_cells_show_keyboard_focus_without_moving_text_or_changing_tab_order() {
    for dark in [true, false] {
        for accent in [[0, 0, 0], [255, 255, 255], [255, 255, 0], [168, 85, 247]] {
            let settings = ThemeSettings {
                dark,
                accent,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            theme::install(&ctx, settings);
            let t = theme::tokens(settings);
            let mut responses = Vec::new();
            let mut original_text = None;
            // The first four cells are interactive, including measured zero and
            // a large heat fill. The final disabled cell must be skipped by Tab.
            for phase in 0..6 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(640.0, 480.0),
                        )),
                        events: if phase == 0 {
                            vec![]
                        } else {
                            vec![key(egui::Key::Tab)]
                        },
                        ..Default::default()
                    },
                    |ui| {
                        responses.clear();
                        for index in 0..5 {
                            let rect = egui::Rect::from_min_size(
                                egui::pos2(20.0, 20.0 + index as f32 * 40.0),
                                Vec2::new(180.0, 30.0),
                            );
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(rect)
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    if index == 4 {
                                        ui.disable();
                                    }
                                    let response = if index < 2 {
                                        table_label(
                                            ui,
                                            RichText::new(if index == 0 {
                                                "NAME"
                                            } else {
                                                "Fixture.Render.exe"
                                            })
                                            .color(t.text),
                                        )
                                    } else {
                                        let value = if index == 2 { 0.0 } else { 100.0 };
                                        heat_cell_response(
                                            ui,
                                            value,
                                            format!("{value:.1}%"),
                                            t.accent,
                                            t.text,
                                            t,
                                        )
                                    };
                                    assert_eq!(response.rect, rect);
                                    responses.push(response);
                                },
                            );
                        }
                    },
                );
                let positions: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            Some(text.visual_bounding_rect())
                        } else {
                            None
                        }
                    })
                    .collect();
                if let Some(before) = &original_text {
                    assert_eq!(before, &positions, "focus changed text geometry");
                }
                original_text = Some(positions);
                // egui clears focus at the end before wrapping on a later pass.
                let expected = (1..=4).contains(&phase).then(|| phase - 1);
                let focused: Vec<_> = responses
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.has_focus())
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(
                    focused,
                    expected.into_iter().collect::<Vec<_>>(),
                    "Tab failed in phase {phase}"
                );
                for (index, response) in responses.iter().enumerate() {
                    let stroke = focus_stroke(&output, response.rect);
                    assert_eq!(
                        stroke.is_some(),
                        Some(index) == expected,
                        "missing or unexpected focus on cell {index}, phase {phase}"
                    );
                    if let Some(stroke) = stroke {
                        assert!(stroke.width >= 1.0);
                        for surface in [
                            t.panel,
                            t.panel_raised,
                            t.row_hover,
                            t.accent_dim,
                            t.surface(t.accent),
                        ] {
                            assert!(theme::contrast_ratio(stroke.color, surface) >= 4.5);
                        }
                    }
                }
                assert!(output.platform_output.commands.is_empty());
                assert!(
                    output
                        .viewport_output
                        .values()
                        .all(|viewport| viewport.commands.is_empty())
                );
            }
        }
    }
}

#[test]
fn table_keyboard_activation_keeps_disabled_cells_inert() {
    for heat in [false, true] {
        for enabled in [true, false] {
            for activate in [egui::Key::Enter, egui::Key::Space] {
                let ctx = egui::Context::default();
                let settings = ThemeSettings::default();
                theme::install(&ctx, settings);
                let t = theme::tokens(settings);
                let mut id = None;
                for phase in 0..3 {
                    if phase == 1 {
                        ctx.memory_mut(|memory| memory.request_focus(id.unwrap()));
                    }
                    let mut clicked = false;
                    let mut focused = false;
                    let mut rect = egui::Rect::NOTHING;
                    let output = ctx.run_ui(
                        egui::RawInput {
                            events: if phase == 2 {
                                vec![key(activate)]
                            } else {
                                vec![]
                            },
                            ..Default::default()
                        },
                        |ui| {
                            ui.set_max_size(Vec2::new(180.0, 30.0));
                            if !enabled {
                                ui.disable();
                            }
                            let response = if heat {
                                heat_cell_response(ui, 27.5, "27.5%".into(), t.accent, t.text, t)
                            } else {
                                table_label(ui, RichText::new("MEMORY"))
                            };
                            id = Some(response.id);
                            clicked = response.clicked();
                            focused = response.has_focus();
                            rect = response.rect;
                        },
                    );
                    assert_eq!(clicked, enabled && phase == 2);
                    if !enabled {
                        assert!(!focused);
                        assert!(focus_stroke(&output, rect).is_none());
                    }
                }
            }
        }
    }
}
