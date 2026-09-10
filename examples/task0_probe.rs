//! Task 0 probe: does stylo already ship the legacy-attribute parsers that
//! Blitz's presentational-hint implementation is missing?
//!
//! Run: cargo run --release --example task0_probe

use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::Viewport;

/// Part 1: exercise stylo's own legacy-color parser directly.
fn probe_stylo_parser() {
    use style::attr::parse_legacy_color;

    println!("== Part 1: style::attr::parse_legacy_color (already compiled into Blitz) ==");
    let cases = [
        "white",
        "ffffff",
        "#ffffff",
        "#fff",
        "red",
        "FFF",
        "bogus",
        "#gg0000",
        "chucknorris",
    ];
    for c in cases {
        match parse_legacy_color(c) {
            Ok(color) => println!(
                "  {:<14} -> Ok(rgb {:3.0} {:3.0} {:3.0} / a {:.2})",
                format!("{:?}", c),
                color.components.0 * 255.0,
                color.components.1 * 255.0,
                color.components.2 * 255.0,
                color.alpha
            ),
            Err(()) => println!("  {:<14} -> Err", format!("{:?}", c)),
        }
    }
}

/// Part 2: what does Blitz's hint implementation actually apply?
fn probe_blitz_hints() {
    println!("\n== Part 2: Blitz computed style for legacy presentational attributes ==");
    println!(
        "  {:<34} {:<22} {}",
        "case", "background-color", "color"
    );

    let cases: &[(&str, &str, &str)] = &[
        (
            "table bgcolor=\"#ffffff\" (hash)",
            "table",
            r##"<table bgcolor="#ffffff"><tr><td>x</td></tr></table>"##,
        ),
        (
            "table bgcolor=\"ffffff\" (bare)",
            "table",
            r##"<table bgcolor="ffffff"><tr><td>x</td></tr></table>"##,
        ),
        (
            "table bgcolor=\"white\" (named)",
            "table",
            r##"<table bgcolor="white"><tr><td>x</td></tr></table>"##,
        ),
        (
            "body bgcolor=\"white\"",
            "body",
            r##"<body bgcolor="white">x</body>"##,
        ),
        (
            "body bgcolor=\"#ffffff\"",
            "body",
            r##"<body bgcolor="#ffffff">x</body>"##,
        ),
        (
            "font color=\"white\"",
            "font",
            r##"<body><font color="white">x</font></body>"##,
        ),
        (
            "body text=\"white\"",
            "body",
            r##"<body text="white">x</body>"##,
        ),
        (
            "hr color=\"white\"",
            "hr",
            r##"<body><hr color="white"></body>"##,
        ),
    ];

    for (label, tag, html) in cases {
        let full = format!("<!DOCTYPE html><html><head></head>{}</html>", html);
        let mut doc = HtmlDocument::from_html(
            &full,
            DocumentConfig {
                viewport: Some(Viewport::new(800, 600, 1.0, Default::default())),
                ..Default::default()
            },
        );
        doc.resolve(0.0);

        let mut bg_s = "<element not found>".to_string();
        let mut color_s = String::new();

        for (_id, node) in doc.tree().iter() {
            let Some(elem) = node.data.downcast_element() else {
                continue;
            };
            if elem.name.local.as_ref() != *tag {
                continue;
            }
            let Some(styles) = node.primary_styles() else {
                bg_s = "<no styles>".into();
                break;
            };
            let current = styles.clone_color();
            let bg = styles
                .get_background()
                .background_color
                .clone()
                .resolve_to_absolute(&current);
            let fg = current;
            bg_s = if bg.alpha == 0.0 {
                "transparent".into()
            } else {
                format!(
                    "rgb({:.0},{:.0},{:.0})",
                    bg.components.0 * 255.0,
                    bg.components.1 * 255.0,
                    bg.components.2 * 255.0
                )
            };
            color_s = format!(
                "rgb({:.0},{:.0},{:.0})",
                fg.components.0 * 255.0,
                fg.components.1 * 255.0,
                fg.components.2 * 255.0
            );
            break;
        }

        println!("  {:<34} {:<22} {}", label, bg_s, color_s);
    }
}

fn main() {
    probe_stylo_parser();
    probe_blitz_hints();
}
