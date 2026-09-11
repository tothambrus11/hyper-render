//! Does malformed CSS/HTML *compute* to what a spec-compliant browser would?
//! Crash-resistance is one thing; agreeing with the recipient's browser is what
//! makes hidden-element detection correct.

use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

fn doc_of(html: &str) -> BaseDocument {
    let mut d = HtmlDocument::from_html(html, DocumentConfig {
        viewport: Some(Viewport::new(800,600,1.0,ColorScheme::Light)), ..Default::default() }).into_inner();
    d.resolve_stylist(0.0);
    d
}

/// Find element with id=`want`, reporting its computed display and whether any
/// ancestor is display:none (the flag our real walker must carry).
fn probe(doc: &BaseDocument, want: &str) -> String {
    let mut stack: Vec<(NodeId, bool)> = vec![(doc.root_node().id, false)];
    while let Some((id, hidden_anc)) = stack.pop() {
        let Some(n) = doc.get_node(id) else { continue };
        let mut now_hidden = hidden_anc;
        if let NodeData::Element(e) = &n.data {
            if let Some(s) = n.primary_styles() {
                if format!("{:?}", s.get_box().clone_display()) == "Display(0)" { now_hidden = true; }
            }
            if e.attr(blitz_dom::local_name!("id")) == Some(want) {
                return match n.primary_styles() {
                    Some(s) => format!("display={:?} hidden_ancestor={hidden_anc}",
                                       s.get_box().clone_display(), ),
                    None => format!("NO STYLES hidden_ancestor={hidden_anc}"),
                };
            }
        }
        for c in n.children.iter().rev() { stack.push((*c, now_hidden)); }
    }
    "NOT FOUND".into()
}

/// Is `text` present anywhere in the DOM, and is it under a display:none ancestor?
fn find_text(doc: &BaseDocument, needle: &str) -> String {
    let mut stack: Vec<(NodeId, bool)> = vec![(doc.root_node().id, false)];
    while let Some((id, hidden)) = stack.pop() {
        let Some(n) = doc.get_node(id) else { continue };
        let mut nh = hidden;
        if matches!(n.data, NodeData::Element(_)) {
            if let Some(s) = n.primary_styles() {
                if format!("{:?}", s.get_box().clone_display()) == "Display(0)" { nh = true; }
            }
        }
        if let NodeData::Text(t) = &n.data {
            if t.content.contains(needle) {
                return format!("present, under_display_none={hidden}");
            }
        }
        for c in n.children.iter().rev() { stack.push((*c, nh)); }
    }
    "ABSENT".into()
}

fn main() {
    // (label, html, expected-per-spec)
    let cases: Vec<(&str, String, &str)> = vec![
        ("css-junk-inline",
         r#"<html><body><p id=t style="display:none;;;garbage:;color">X</p>"#.into(),
         "none (junk decls dropped, valid one survives)"),
        ("css-unbalanced-brace",
         r#"<html><head><style>.a{display:none</style></head><body><p id=t class=a>X"#.into(),
         "none (block implicitly closed at EOF)"),
        ("css-bad-important",
         r#"<html><body><p id=t style="display:none !important junk">X</p>"#.into(),
         "block (tokens after !important invalidate the declaration)"),
        ("css-nested-junk",
         r#"<html><head><style>@media{{{ .a{display:none} </style></head><body><p id=t class=a>X"#.into(),
         "block (.a nested in invalid qualified rule -> dropped)"),
        ("css-valid-media-empty",
         r#"<html><head><style>@media{ .a{display:none} }</style></head><body><p id=t class=a>X"#.into(),
         "none (empty media query list matches all)"),
        ("css-unknown-atrule",
         r#"<html><head><style>@totallybogus x; .a{display:none}</style></head><body><p id=t class=a>X"#.into(),
         "none (unknown at-rule consumed, following rule survives)"),
        ("css-unclosed-comment",
         r#"<html><head><style>.a{display:none} /* unterminated</style></head><body><p id=t class=a>X"#.into(),
         "none (comment before rule irrelevant; rule already parsed)"),
        ("css-bad-selector-then-good",
         r#"<html><head><style>.!!bad{color:red} .a{display:none}</style></head><body><p id=t class=a>X"#.into(),
         "none (invalid selector drops only its own rule)"),
        ("css-semicolon-in-block",
         r#"<html><head><style>.a{;;display:none;;}</style></head><body><p id=t class=a>X"#.into(),
         "none (stray semicolons ignored)"),
    ];

    println!("--- malformed CSS: computed display vs spec ---");
    for (name, html, expect) in &cases {
        println!("{:<26} {:<34} expect: {}", name, probe(&doc_of(html), "t"), expect);
    }

    println!("\n--- malformed HTML: is swallowed content reachable, and correctly hidden? ---");
    let html_cases: Vec<(&str, String, &str)> = vec![
        ("unclosed-style", "<html><body><style>.a{display:none}<p>SWALLOWED".into(),
         "text inside <style> -> under display:none"),
        ("unclosed-script", "<html><body><script>var x=1;<p>SWALLOWED".into(),
         "text inside <script> -> under display:none"),
        ("unclosed-title", "<html><head><title>T<body><p>SWALLOWED".into(),
         "text inside <title> -> under display:none"),
        ("unclosed-textarea", "<html><body><textarea><p>SWALLOWED".into(),
         "text inside <textarea> -> VISIBLE (textarea renders)"),
        ("unclosed-comment", "<html><body><!-- <p>SWALLOWED".into(),
         "inside comment -> ABSENT from text"),
        ("plaintext", "<html><body><plaintext><p>SWALLOWED".into(),
         "text inside <plaintext> -> visible raw text"),
    ];
    for (name, html, expect) in &html_cases {
        println!("{:<20} {:<42} expect: {}", name, find_text(&doc_of(html), "SWALLOWED"), expect);
    }
}
