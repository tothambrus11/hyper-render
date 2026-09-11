use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;
fn bench(label: &str, html: &str, n: usize) {
    let (mut p, mut s) = (f64::MAX, f64::MAX);
    for _ in 0..15 {
        let t = Instant::now();
        let mut d = HtmlDocument::from_html(html, DocumentConfig::default()).into_inner();
        p = p.min(t.elapsed().as_secs_f64()*1000.0);
        let t = Instant::now();
        d.resolve_stylist(0.0);
        s = s.min(t.elapsed().as_secs_f64()*1000.0);
    }
    println!("{label:<40} parse {p:>7.2}  style {s:>7.2}  ({} el)", n);
}
fn main() {
    let n = 900;
    bench("baseline <div>", &"<div>x</div>".repeat(n), n);
    bench("bgcolor=\"#ffffff\" (valid hex)", &"<div bgcolor=\"#ffffff\">x</div>".repeat(n), n);
    bench("bgcolor=\"white\" (named, unsupported)", &"<div bgcolor=\"white\">x</div>".repeat(n), n);
    bench("width=\"600\"", &"<div width=\"600\">x</div>".repeat(n), n);
    bench("align=\"center\"", &"<div align=\"center\">x</div>".repeat(n), n);
    bench("style=\"background:#fff\" (equivalent)", &"<div style=\"background-color:#ffffff\">x</div>".repeat(n), n);
}
