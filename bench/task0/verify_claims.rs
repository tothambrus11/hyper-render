//! Verify two claims made in the Task 0 review:
//!  (a) border="3" on <img> is a FALSE MISSING in preshints-results.txt (3px == `medium` default)
//!  (b) Blitz drops the parser's quirks mode (stylo.rs:204 hardcodes NoQuirks)
use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

fn doc(html: &str) -> BaseDocument {
    let mut d = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
            base_url: Some("https://e.invalid/".into()),
            ..Default::default()
        },
    )
    .into_inner();
    d.resolve_stylist(0.0);
    d
}

fn prop(d: &BaseDocument, which: &str) -> String {
    let mut st: Vec<NodeId> = vec![d.root_node().id];
    while let Some(id) = st.pop() {
        let Some(n) = d.get_node(id) else { continue };
        if let NodeData::Element(e) = &n.data {
            if e.attr(blitz_dom::local_name!("id")) == Some("t") {
                let Some(s) = n.primary_styles() else {
                    return "NO STYLES".into();
                };
                return match which {
                    "border-top-width" => {
                        format!("{:?}", s.get_border().clone_border_top_width())
                    }
                    "height" => format!("{:?}", s.get_position().clone_height()),
                    _ => "?".into(),
                };
            }
        }
        st.extend(n.children.iter().copied());
    }
    "NOT FOUND".into()
}

fn main() {
    println!("(a) border on <img> -- is the reported MISSING a value collision?");
    for v in ["(none)", "3", "5", "10"] {
        let html = if v == "(none)" {
            "<img id=t src=\"https://e.invalid/a.png\">".to_string()
        } else {
            format!("<img id=t border=\"{v}\" src=\"https://e.invalid/a.png\">")
        };
        println!(
            "    border={:<7} -> {}",
            v,
            prop(&doc(&html), "border-top-width")
        );
    }

    println!("\n(b) quirks mode -- unitless length is valid ONLY in quirks mode");
    let quirks = "<html><body><div id=t style=\"height:100\">x</div></body></html>";
    let strict = format!("<!DOCTYPE html>{quirks}");
    println!("    no doctype (should be quirks) -> height {}", prop(&doc(quirks), "height"));
    println!("    <!DOCTYPE html> (no-quirks)   -> height {}", prop(&doc(&strict), "height"));
    println!("    if these are identical, the parser's quirks mode never reached stylo.");
}
