//! Production controls under local egui input; no windows or OS clipboard.
use super::*;

struct Harness {
    ctx: egui::Context,
    studio: Studio,
    settings: ThemeSettings,
    open: bool,
    editable: bool,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let settings = ThemeSettings::default();
        theme::install(&ctx, settings);
        Self {
            ctx,
            settings,
            studio: Studio {
                tab: 3,
                ..Default::default()
            },
            open: true,
            editable: true,
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. }
                | egui::Event::PointerButton { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280.0, 900.0),
                )),
                events,
                modifiers,
                ..Default::default()
            },
            |ui| {
                self.studio
                    .show(ui.ctx(), &mut self.settings, &mut self.open, self.editable)
            },
        );
        assert!(
            output
                .viewport_output
                .values()
                .all(|v| v.commands.is_empty())
        );
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .all(|c| matches!(c, egui::OutputCommand::CopyText(_)))
        );
        output
    }
    fn click(&mut self, label: &str) -> egui::FullOutput {
        let mut output = egui::FullOutput::default();
        for _ in 0..15 {
            output.append(self.frame(vec![]));
        }
        let position = texts(&output)
            .into_iter()
            .find(|(text, rect, clip)| text == label && clip.contains_rect(*rect))
            .unwrap_or_else(|| panic!("Missing visible {label}"))
            .1
            .center();
        for pressed in [true, false] {
            output.append(self.frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]));
        }
        output
    }
}
fn texts(output: &egui::FullOutput) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn visit(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(String, egui::Rect, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(text) => out.push((
                text.galley.job.text.clone(),
                text.visual_bounding_rect(),
                clip,
            )),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    visit(shape, clip, out);
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    for shape in &output.shapes {
        visit(&shape.shape, shape.clip_rect, &mut out);
    }
    out
}

#[test]
fn named_theme_save_replace_load_and_delete_follow_real_controls() {
    let mut h = Harness::new();
    let original = h.settings;
    h.click("Name this theme");
    h.frame(vec![egui::Event::Text("  Test palette  ".into())]);
    h.click("Save current");
    assert_eq!(h.studio.revision(), 1);
    assert_eq!(h.studio.saved.len(), 1);
    assert_eq!(h.studio.saved[0].name, "Test palette");
    assert_eq!(h.studio.saved[0].theme, original);
    h.settings = ThemeSettings::demigod();
    h.click("Replace saved");
    assert_eq!(h.studio.revision(), 2);
    assert_eq!(h.studio.saved.len(), 1);
    assert_eq!(h.studio.saved[0].theme, ThemeSettings::demigod());
    h.settings = original;
    h.click("Load");
    assert_eq!(h.settings, ThemeSettings::demigod());
    assert_eq!(
        h.studio.revision(),
        2,
        "Loading does not rewrite the library"
    );
    h.click("Delete");
    assert_eq!(
        h.settings,
        ThemeSettings::demigod(),
        "Deleting a preset preserves active appearance"
    );
    assert_eq!(h.studio.revision(), 3);
    assert!(h.studio.saved.is_empty());
    h.click("Revert session");
    assert_eq!(h.settings, original);
    h.click("Done");
    assert!(!h.open);
    assert!(h.studio.baseline.is_none());
}

#[test]
fn theme_copy_import_failure_success_and_revert_preserve_library() {
    let mut h = Harness::new();
    h.studio.name = "Saved fixture".into();
    h.click("Save current");
    let library = h.studio.encode_library();
    let original = h.settings;
    let output = h.click("Copy current theme");
    let [egui::OutputCommand::CopyText(code)] = output.platform_output.commands.as_slice() else {
        panic!("Expected one captured clipboard command");
    };
    assert_eq!(ThemeSettings::decode(code), Some(original));
    assert_eq!(h.studio.transfer, *code);
    h.studio.transfer = "{broken".into();
    h.click("Import theme");
    assert_eq!(h.settings, original);
    assert!(h.studio.notice.as_ref().unwrap().1);
    h.studio.transfer = ThemeSettings::demigod().encode();
    h.click("Import theme");
    assert_eq!(h.settings, ThemeSettings::demigod());
    assert!(!h.studio.notice.as_ref().unwrap().1);
    assert_eq!(h.studio.encode_library(), library);
    h.click("Revert session");
    assert_eq!(h.settings, original);
    assert!(h.studio.notice.is_none());
    assert_eq!(h.studio.encode_library(), library);
}

#[test]
fn full_theme_library_refuses_new_names_but_allows_replacement() {
    let mut h = Harness::new();
    for index in 0..MAX_SAVED {
        h.studio.name = format!("Fixture {index}");
        h.click("Save current");
    }
    let before = h.studio.encode_library();
    h.studio.name = "Overflow".into();
    h.click("Save current");
    assert_eq!(h.studio.encode_library(), before);
    assert_eq!(h.studio.revision(), MAX_SAVED as u64);
    h.studio.name = "Fixture 0".into();
    h.settings = ThemeSettings::demigod();
    h.click("Replace saved");
    assert_eq!(h.studio.saved[0].theme, ThemeSettings::demigod());
    assert_eq!(h.studio.saved.len(), MAX_SAVED);
}

#[test]
fn pending_preferences_disable_theme_actions_without_recording_temporary_baseline() {
    let mut h = Harness::new();
    h.editable = false;
    h.studio.name = "Disabled".into();
    h.studio.transfer = ThemeSettings::demigod().encode();
    let original = h.settings;
    for label in [
        "Save current",
        "Import theme",
        "Randomize",
        "Reset to TrontStack",
    ] {
        let output = h.click(label);
        assert!(output.platform_output.commands.is_empty());
        assert_eq!(h.settings, original);
        assert!(h.studio.saved.is_empty());
        assert!(h.studio.baseline.is_none());
    }
    h.settings = ThemeSettings::demigod();
    h.editable = true;
    h.frame(vec![]);
    assert_eq!(h.studio.baseline, Some(ThemeSettings::demigod()));
    h.click("Reset to TrontStack");
    assert_eq!(h.settings, ThemeSettings::default());
    h.click("Revert session");
    assert_eq!(h.settings, ThemeSettings::demigod());
}

fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn gradient_peg_keyboard_moves_and_selection_preserve_neighbor_limits() {
    let mut h = Harness::new();
    h.studio.tab = 0;
    h.click("2");
    assert_eq!(h.studio.selected, 1);
    let initial = h.settings.stops[1].position;
    for (key_code, modifiers, expected) in [
        (egui::Key::ArrowLeft, egui::Modifiers::NONE, initial - 0.01),
        (
            egui::Key::ArrowRight,
            egui::Modifiers::SHIFT,
            initial + 0.04,
        ),
    ] {
        h.frame(vec![key(key_code, modifiers, true)]);
        h.frame(vec![key(key_code, modifiers, false)]);
        assert!((h.settings.stops[1].position - expected).abs() < 0.0001);
    }
    h.frame(vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE, true)]);
    h.frame(vec![key(egui::Key::ArrowUp, egui::Modifiers::NONE, false)]);
    assert_eq!(h.studio.selected, 0);
    h.frame(vec![key(
        egui::Key::ArrowLeft,
        egui::Modifiers::SHIFT,
        true,
    )]);
    h.frame(vec![key(
        egui::Key::ArrowLeft,
        egui::Modifiers::SHIFT,
        false,
    )]);
    assert_eq!(h.settings.stops[0].position, 0.0);
    for _ in 0..5 {
        h.frame(vec![key(egui::Key::ArrowDown, egui::Modifiers::NONE, true)]);
        h.frame(vec![key(
            egui::Key::ArrowDown,
            egui::Modifiers::NONE,
            false,
        )]);
    }
    assert_eq!(h.studio.selected, 3);
    h.frame(vec![key(
        egui::Key::ArrowRight,
        egui::Modifiers::SHIFT,
        true,
    )]);
    h.frame(vec![key(
        egui::Key::ArrowRight,
        egui::Modifiers::SHIFT,
        false,
    )]);
    assert_eq!(h.settings.stops[3].position, 1.0);
    assert!(
        h.settings
            .stops
            .windows(2)
            .all(|pair| pair[1].position - pair[0].position >= 0.0099)
    );
    let gradient_focus = h.ctx.memory(|memory| memory.focused()).unwrap();
    h.frame(vec![key(egui::Key::Tab, egui::Modifiers::NONE, true)]);
    h.frame(vec![key(egui::Key::Tab, egui::Modifiers::NONE, false)]);
    assert_ne!(
        h.ctx.memory(|memory| memory.focused()),
        Some(gradient_focus)
    );
    h.click("2");
    h.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE, true)]);
    h.frame(vec![key(egui::Key::Escape, egui::Modifiers::NONE, false)]);
    assert_ne!(
        h.ctx.memory(|memory| memory.focused()),
        Some(gradient_focus)
    );
}

#[test]
fn hex_color_edit_retains_last_valid_color_until_input_is_complete() {
    let mut h = Harness::new();
    h.studio.tab = 0;
    let original = h.settings.stops[0].color;
    h.click(&hex(original));
    let select_all = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    h.frame(vec![key(egui::Key::A, select_all, true)]);
    h.frame(vec![key(egui::Key::A, select_all, false)]);
    h.frame(vec![egui::Event::Text("#GG0088".into())]);
    assert_eq!(h.settings.stops[0].color, original);
    assert_eq!(h.studio.hex[0], "#GG0088");
    h.frame(vec![key(egui::Key::A, select_all, true)]);
    h.frame(vec![key(egui::Key::A, select_all, false)]);
    h.frame(vec![egui::Event::Text("#12AB90".into())]);
    assert_eq!(h.settings.stops[0].color, [0x12, 0xab, 0x90]);
}
