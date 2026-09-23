//! P17: the flexible remainder column in a table is the text column, not
//! whichever fixed-width column happened to sit last.
use super::*;

/// The clip rect egui assigned the header cell painting `label`: with the
/// column's `.clip(true)`, this rect tracks the column's own width (a few
/// pixels of margin aside), not just the label glyphs' bounding box.
fn header_clip(output: &egui::FullOutput, label: &str) -> egui::Rect {
    text_shapes(output)
        .into_iter()
        .find(|(text, _)| text.galley.job.text == label)
        .unwrap_or_else(|| panic!("missing header: {label}"))
        .1
}

#[test]
fn processes_and_details_name_column_is_most_of_the_table_at_1600() {
    for page in [Page::Processes, Page::Details] {
        let settings = ThemeSettings::default();
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = page;
        app.tree_mode = false;
        let size = Vec2::new(1600.0, 1000.0);
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let name = header_clip(&output, "NAME");
        let last_label = if page == Page::Details {
            "CPU TIME"
        } else {
            "WRITE"
        };
        let last = header_clip(&output, last_label);
        let table_left = name.left();
        let table_right = last.right();
        let span = table_right - table_left;
        assert!(span > 0.0, "{page:?}: non-positive table span {span}");
        let name_share = (name.right() - table_left) / span;
        assert!(
            name_share >= 0.60,
            "{page:?}: NAME right edge is only {:.1}% of the table width \
             (name={name:?}, last={last:?})",
            name_share * 100.0
        );
    }
}

#[test]
fn startup_command_column_is_wider_than_name_and_source_at_1600() {
    let settings = ThemeSettings::default();
    let ctx = egui::Context::default();
    theme::install(&ctx, settings);
    let mut app = app(settings, true);
    app.page = Page::Startup;
    let size = Vec2::new(1600.0, 1000.0);
    let mut output = egui::FullOutput::default();
    for _ in 0..4 {
        output = frame(&ctx, &mut app, size, vec![]);
    }
    let name = header_clip(&output, "NAME").width();
    let command = header_clip(&output, "COMMAND / FILE").width();
    let source = header_clip(&output, "SOURCE").width();
    assert!(
        command > name && command > source,
        "COMMAND / FILE ({command}) must be the widest Startup column \
         (NAME={name}, SOURCE={source})"
    );
}
