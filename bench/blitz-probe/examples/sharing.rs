//! Hypothesis: stylo's style-sharing cache makes identical elements cheap, and
//! ANYTHING unique per element (inline style or presentational hint) defeats it.
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;
fn bench(label: &str, html: &str) {
    let (mut p, mut s) = (f64::MAX, f64::MAX);
    for _ in 0..15 {
        let t = Instant::now();
        let mut d = HtmlDocument::from_html(html, DocumentConfig::default()).into_inner();
        p = p.min(t.elapsed().as_secs_f64()*1000.0);
        let t = Instant::now();
        d.resolve_stylist(0.0);
        s = s.min(t.elapsed().as_secs_f64()*1000.0);
    }
    println!("{label:<44} parse {p:>7.2}  style {s:>7.2}");
}
fn main() {
    let n = 900;
    bench("identical, no attrs", &"<div>x</div>".repeat(n));
    bench("identical inline style", &"<div style=\"color:#333333\">x</div>".repeat(n));
    bench("UNIQUE inline style per element",
        &(0..n).map(|i| format!("<div style=\"color:#{:06x}\">x</div>", i * 7)).collect::<String>());
    bench("identical bgcolor", &"<div bgcolor=\"#ffffff\">x</div>".repeat(n));
    bench("UNIQUE bgcolor per element",
        &(0..n).map(|i| format!("<div bgcolor=\"#{:06x}\">x</div>", i * 7)).collect::<String>());
    bench("identical class (sheet rule)",
        &format!("<style>.k{{color:#333}}</style>{}", "<div class=\"k\">x</div>".repeat(n)));
}
