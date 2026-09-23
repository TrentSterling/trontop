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
    // Processes: NAME is most of the table. Details spreads its nine
    // counters across a wide window instead, so NAME is the widest single
    // column but not a dead zone (see details_counters_spread_at_1600).
    for page in [Page::Processes] {
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

#[test]
fn details_fits_1000x580_and_spreads_counters_at_1600() {
    for (size, spread) in [
        (Vec2::new(1000.0, 580.0), false),
        (Vec2::new(1600.0, 1000.0), true),
    ] {
        let settings = ThemeSettings::default();
        let ctx = egui::Context::default();
        theme::install(&ctx, settings);
        let mut app = app(settings, true);
        app.page = Page::Details;
        let mut output = egui::FullOutput::default();
        for _ in 0..4 {
            output = frame(&ctx, &mut app, size, vec![]);
        }
        let labels = [
            "NAME", "PID", "USER", "STATE", "CPU", "GPU", "MEMORY", "READ", "WRITE", "CPU TIME",
        ];
        // Table headers only: the sidebar meters also paint "CPU" and "GPU".
        let rects: Vec<_> = labels
            .iter()
            .map(|label| {
                text_shapes(&output)
                    .into_iter()
                    .find(|(text, clip)| text.galley.job.text == *label && clip.left() > 196.0)
                    .unwrap_or_else(|| panic!("missing header: {label}"))
                    .1
            })
            .collect();
        // Every key column is on screen: no horizontal scroll needed.
        for (label, rect) in labels.iter().zip(&rects) {
            assert!(
                rect.right() <= size.x - 8.0 && rect.width() > 20.0,
                "{label} off screen at {size:?}: {rect:?}"
            );
        }
        // No counter cell clips its value: PID, rates and CPU time read in
        // full. (A long account name may clip in USER; its hover has it all.)
        let user = rects[2];
        for (text, clip) in text_shapes(&output) {
            let in_user = (clip.center().x - user.center().x).abs() < user.width() / 2.0;
            if clip.left() >= rects[1].left() - 1.0 && clip.top() > rects[0].bottom() && !in_user {
                assert!(
                    !text.galley.elided,
                    "{:?} clipped at {size:?}",
                    text.galley.job.text
                );
            }
        }
        let name = rects[0].width();
        let widest_counter = rects[1..].iter().map(|r| r.width()).fold(0.0, f32::max);
        assert!(
            name >= widest_counter,
            "NAME should stay the widest column at {size:?}: {rects:?}"
        );
        if spread {
            let span = rects[9].right() - rects[0].left();
            assert!(
                name / span <= 0.45,
                "NAME is {:.0}% of the table at {size:?}: a dead zone",
                name / span * 100.0
            );
        }
    }
}
