//! Two spam-shaped inputs the other probes never exercised:
//! remote sub-resources with no net provider, and prefers-color-scheme hiding.
use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use std::time::Instant;

fn doc_of(html: &str, cs: ColorScheme) -> BaseDocument {
    let mut d = HtmlDocument::from_html(html, DocumentConfig {
        viewport: Some(Viewport::new(800, 600, 1.0, cs)),
        ..Default::default()   // net_provider: None
    }).into_inner();
    d.resolve_stylist(0.0);
    d
}
fn disp(doc: &BaseDocument, want: &str) -> String {
    let mut st: Vec<NodeId> = vec![doc.root_node().id];
    while let Some(id) = st.pop() {
        if let Some(n) = doc.get_node(id) {
            if let NodeData::Element(e) = &n.data {
                if e.attr(blitz_dom::local_name!("id")) == Some(want) {
                    return match n.primary_styles() {
                        Some(s) => format!("{:?}", s.get_box().clone_display()),
                        None => "NO STYLES".into() };
                }
            }
            st.extend(n.children.iter().copied());
        }
    }
    "NOT FOUND".into()
}

fn main() {
    println!("-- remote sub-resources, net_provider: None --");
    for (name, html) in [
        ("link-stylesheet", r#"<html><head><link rel=stylesheet href="http://192.0.2.1/a.css"></head><body><p id=t>X</p>"#),
        ("import-in-style", r#"<html><head><style>@import url(http://192.0.2.1/a.css); .a{display:none}</style></head><body><p id=t class=a>X</p>"#),
        ("img-remote",      r#"<html><body><img src="http://192.0.2.1/a.png"><p id=t>X</p>"#),
    ] {
        let t = Instant::now();
        let d = doc_of(html, ColorScheme::Light);
        println!("{name:<18} display={:<14} {}ms", disp(&d, "t"), t.elapsed().as_millis());
    }

    println!("\n-- prefers-color-scheme hiding --");
    let html = r#"<html><head><style>@media (prefers-color-scheme: dark){ .x{display:none} }</style></head>
      <body><p id=t class=x>X</p>"#;
    for cs in [ColorScheme::Light, ColorScheme::Dark] {
        println!("{cs:?}: display={}", disp(&doc_of(html, cs), "t"));
    }
}
