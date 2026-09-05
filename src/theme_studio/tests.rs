use super::*;

fn frame(
    ctx: &egui::Context,
    studio: &mut Studio,
    settings: &mut ThemeSettings,
    open: &mut bool,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| studio.show(ui.ctx(), settings, open, true),
    )
}

#[test]
fn hex_import_and_saved_library_are_atomic_and_bounded() {
    assert_eq!(parse_hex("#12aB90"), Some([18, 171, 144]));
    for text in ["#fff", "GG0088", "ééé", "#1234567"] {
        assert!(parse_hex(text).is_none());
    }
    let mut studio = Studio::default();
    let mut settings = ThemeSettings::default();
    studio.transfer = "{broken".into();
    let original = settings;
    studio.import(&mut settings);
    assert_eq!(settings, original);
    assert!(studio.notice.as_ref().unwrap().1);
    studio.transfer = ThemeSettings::demigod().encode();
    studio.import(&mut settings);
    assert_eq!(settings, ThemeSettings::demigod());
    for i in 0..20 {
        studio.name = format!("Palette {i}");
        studio.save_named(settings);
    }
    assert_eq!(studio.saved.len(), MAX_SAVED);
    studio.name = "Palette 0".into();
    studio.save_named(original);
    assert_eq!(studio.saved[0].theme, original);
    let encoded = studio.encode_library();
    let mut restored = Studio::default();
    restored.load_library(&encoded);
    assert_eq!(restored.encode_library(), encoded);
    for invalid in ["{}", "{broken", &" ".repeat(128 * 1024 + 1)] {
        restored.load_library(invalid);
        assert_eq!(restored.encode_library(), encoded);
    }
    let mut duplicate: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    duplicate["themes"][1] = duplicate["themes"][0].clone();
    restored.load_library(&duplicate.to_string());
    assert_eq!(restored.encode_library(), encoded);
}

#[test]
fn studio_all_tabs_fit_and_emit_no_native_window_commands() {
    for dark in [true, false] {
        for size in [Vec2::new(1040.0, 640.0), Vec2::new(1280.0, 760.0)] {
            for tab in 0..4 {
                let ctx = egui::Context::default();
                let mut settings = ThemeSettings {
                    dark,
                    ..Default::default()
                };
                theme::install(&ctx, settings);
                let mut studio = Studio {
                    tab,
                    ..Default::default()
                };
                let mut open = true;
                let mut output = egui::FullOutput::default();
                for _ in 0..20 {
                    output = frame(&ctx, &mut studio, &mut settings, &mut open, size, vec![]);
                }
                let mut texts = Vec::new();
                fn visit(
                    shape: &egui::Shape,
                    clip: egui::Rect,
                    texts: &mut Vec<(String, egui::Rect, egui::Rect)>,
                ) {
                    match shape {
                        egui::Shape::Text(t) => texts.push((
                            t.galley.job.text.clone(),
                            t.galley.rect.translate(t.pos.to_vec2()),
                            clip,
                        )),
                        egui::Shape::Vec(v) => {
                            for s in v {
                                visit(s, clip, texts);
                            }
                        }
                        _ => {}
                    }
                }
                for s in &output.shapes {
                    visit(&s.shape, s.clip_rect, &mut texts);
                }
                for label in ["Palette", "Appearance", "Presets", "My themes", "Done"] {
                    let (_, rect, clip) = texts
                        .iter()
                        .find(|(t, _, _)| t == label)
                        .unwrap_or_else(|| panic!("missing {label}"));
                    assert!(
                        clip.expand(1.0).contains_rect(*rect),
                        "clipped {label} {size:?} tab {tab}: {rect:?} / {clip:?}"
                    );
                    assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(*rect));
                }
                for (_, viewport) in output.viewport_output {
                    assert!(viewport.commands.is_empty());
                }
                assert!(open);
            }
        }
    }
}

#[test]
fn ramp_local_drag_keyboard_and_hover_keep_geometry() {
    let ctx = egui::Context::default();
    let mut settings = ThemeSettings::default();
    theme::install(&ctx, settings);
    let mut selected = 0;
    let mut render = |events: Vec<egui::Event>| {
        let mut response = None;
        let out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(600.0, 200.0),
                )),
                events,
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    response = Some(ramp(
                        ui,
                        &mut settings,
                        &mut selected,
                        theme::tokens(ThemeSettings::default()),
                    ));
                });
            },
        );
        (response.unwrap(), out, settings, selected)
    };
    let (r, _, initial, _) = render(vec![]);
    let left = r.rect.left() + 12.0;
    let width = r.rect.width() - 24.0;
    let from = egui::pos2(left + width / 3.0, r.rect.top() + 20.0);
    let to = egui::pos2(left + width * 0.47, from.y);
    render(vec![
        egui::Event::PointerMoved(from),
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    ]);
    let (hover, _, dragged, sel) = render(vec![egui::Event::PointerMoved(to)]);
    assert_eq!(hover.rect, r.rect);
    assert_eq!(sel, 1);
    assert!((dragged.stops[1].position - 0.47).abs() < 0.002);
    assert_eq!(dragged.stops[0], initial.stops[0]);
    render(vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    let (_, _, keyed, _) = render(vec![egui::Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    assert!(keyed.stops[1].position > dragged.stops[1].position);
}

#[test]
fn session_revert_does_not_leak_across_editor_reopening() {
    let ctx = egui::Context::default();
    let mut studio = Studio::default();
    let mut settings = ThemeSettings::default();
    let mut open = true;
    theme::install(&ctx, settings);
    let size = Vec2::new(1040.0, 640.0);
    frame(&ctx, &mut studio, &mut settings, &mut open, size, vec![]);
    assert_eq!(studio.baseline, Some(settings));
    settings = ThemeSettings::demigod();
    open = false;
    frame(&ctx, &mut studio, &mut settings, &mut open, size, vec![]);
    assert!(studio.baseline.is_none());
    open = true;
    frame(&ctx, &mut studio, &mut settings, &mut open, size, vec![]);
    assert_eq!(studio.baseline, Some(settings));
}
