use blitz_dom::{DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use std::time::Instant;

fn parse(html: &str) -> HtmlDocument {
    HtmlDocument::from_html(html, DocumentConfig {
        viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
        ..Default::default()
    })
}
fn count(doc: &blitz_dom::BaseDocument) -> usize {
    let mut n = 0;
    let mut st: Vec<NodeId> = vec![doc.root_node().id];
    while let Some(id) = st.pop() {
        if let Some(node) = doc.get_node(id) {
            if matches!(node.data, NodeData::Element(_)) { n += 1; }
            st.extend(node.children.iter().copied());
        }
    }
    n
}

fn main() {
    let mode = std::env::args().nth(1).unwrap();
    let n: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    match mode.as_str() {
        // CSS nesting depth -> does the style parser overflow?
        "css" => {
            let html = format!("<html><head><style>{}", "div{".repeat(n));
            let t = Instant::now();
            let d = parse(&html);
            println!("css n={n} parse_ok elements={} {}ms", count(&d), t.elapsed().as_millis());
        }
        // DOM nesting depth -> split parse vs style timing
        "dom" => {
            let html = format!("<html><body>{}X", "<div>".repeat(n));
            let t0 = Instant::now();
            let mut d = parse(&html).into_inner();
            let parse_ms = t0.elapsed().as_millis();
            let t1 = Instant::now();
            d.resolve_stylist(0.0);
            println!("dom n={n} parse={parse_ms}ms style={}ms elements={}",
                     t1.elapsed().as_millis(), count(&d));
        }
        _ => unreachable!(),
    }
}
