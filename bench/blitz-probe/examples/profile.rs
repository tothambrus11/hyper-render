//! Where does the time actually go?
use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use std::time::Instant;

fn sample_email(kb: usize) -> String {
    let row = r#"<tr><td style="padding:8px;font-family:Arial;font-size:14px;color:#333">
      <a href="https://example.com/x" style="color:#1a73e8;text-decoration:none">Shop now</a>
      <span style="display:none">hidden preheader text</span></td></tr>"#;
    let mut body = String::new();
    while body.len() < kb * 1024 {
        body.push_str(&format!("<table width=\"600\" bgcolor=\"#ffffff\">{row}</table>"));
    }
    format!("<html><head><style>.a{{color:#000}} .b{{display:none}} \
        @media (max-width:600px){{.c{{display:none}}}}</style></head><body>{body}</body></html>")
}

fn count_elems(doc: &BaseDocument) -> usize {
    let mut n = 0;
    let mut st: Vec<NodeId> = vec![doc.root_node().id];
    while let Some(id) = st.pop() {
        if let Some(x) = doc.get_node(id) {
            if matches!(x.data, NodeData::Element(_)) { n += 1; }
            st.extend(x.children.iter().copied());
        }
    }
    n
}

fn main() {
    let kb: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(40);
    let html = sample_email(kb);
    println!("document {} KB\n", html.len() / 1024);

    // Empty document: isolates fixed per-document construction cost (UA stylesheet, etc.)
    let mut fixed = f64::MAX;
    for _ in 0..20 {
        let t = Instant::now();
        let d = HtmlDocument::from_html("<html><body></body></html>", DocumentConfig::default());
        std::hint::black_box(&d);
        fixed = fixed.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("empty-document construction (UA sheet etc.): {fixed:.2} ms  <-- fixed cost");

    let (mut p, mut s, mut w_bad, mut w_good) = (f64::MAX, f64::MAX, f64::MAX, f64::MAX);
    let mut elems = 0;
    for _ in 0..20 {
        let t = Instant::now();
        let mut doc = HtmlDocument::from_html(&html, DocumentConfig::default()).into_inner();
        p = p.min(t.elapsed().as_secs_f64() * 1000.0);

        let t = Instant::now();
        doc.resolve_stylist(0.0);
        s = s.min(t.elapsed().as_secs_f64() * 1000.0);

        elems = count_elems(&doc);

        // BAD walk: formats a Debug string per element (what my earlier bench did)
        let t = Instant::now();
        let mut hidden = 0;
        let mut st: Vec<NodeId> = vec![doc.root_node().id];
        while let Some(id) = st.pop() {
            if let Some(n) = doc.get_node(id) {
                if matches!(n.data, NodeData::Element(_)) {
                    if let Some(sty) = n.primary_styles() {
                        if format!("{:?}", sty.get_box().clone_display()) == "Display(0)" { hidden += 1; }
                    }
                }
                st.extend(n.children.iter().copied());
            }
        }
        std::hint::black_box(hidden);
        w_bad = w_bad.min(t.elapsed().as_secs_f64() * 1000.0);

        // GOOD walk: typed comparison, no allocation
        let t = Instant::now();
        let mut hidden = 0;
        let mut st: Vec<NodeId> = vec![doc.root_node().id];
        while let Some(id) = st.pop() {
            if let Some(n) = doc.get_node(id) {
                if matches!(n.data, NodeData::Element(_)) {
                    if let Some(sty) = n.primary_styles() {
                        if sty.get_box().clone_display().is_none() { hidden += 1; }
                    }
                }
                st.extend(n.children.iter().copied());
            }
        }
        std::hint::black_box(hidden);
        w_good = w_good.min(t.elapsed().as_secs_f64() * 1000.0);
    }

    println!("elements: {elems}\n");
    println!("  parse (from_html)          {p:8.2} ms");
    println!("  resolve_stylist            {s:8.2} ms");
    println!("  walk w/ format!(\"{{:?}}\")   {w_bad:8.2} ms   <-- my earlier benchmark did this");
    println!("  walk typed (is_none)       {w_good:8.2} ms");
    println!("  ------------------------------------");
    println!("  realistic total            {:8.2} ms", p + s + w_good);
    println!("  what I reported earlier    {:8.2} ms", p + s + w_bad);
}
