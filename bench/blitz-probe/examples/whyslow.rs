use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;

fn bench(label: &str, html: &str, elems: usize) {
    let (mut p, mut s) = (f64::MAX, f64::MAX);
    for _ in 0..15 {
        let t = Instant::now();
        let mut d = HtmlDocument::from_html(html, DocumentConfig::default()).into_inner();
        p = p.min(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        d.resolve_stylist(0.0);
        s = s.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("{label:<34} {:>6} el  {:>8.2} KB  parse {p:>7.2} ms  style {s:>7.2} ms  ({:.1} us/el)",
             elems, html.len() as f64 / 1024.0, (p + s) * 1000.0 / elems as f64);
}

fn main() {
    let n = 900;
    bench("plain <div>x</div>", &"<div>x</div>".repeat(n).to_string(), n);
    bench("with class attr",
          &format!("{}", "<div class=\"a b c\">x</div>".repeat(n)), n);
    bench("with inline style (1 decl)",
          &format!("{}", "<div style=\"color:#333\">x</div>".repeat(n)), n);
    bench("with inline style (4 decls)",
          &format!("{}", "<div style=\"padding:8px;font-family:Arial;font-size:14px;color:#333\">x</div>".repeat(n)), n);
    bench("with bgcolor attr",
          &format!("{}", "<div bgcolor=\"#ffffff\">x</div>".repeat(n)), n);
    bench("tables, no styles",
          &format!("{}", "<table><tr><td>x</td></tr></table>".repeat(n / 3)), n);
    // a big <style> sheet instead of inline styles
    let sheet: String = (0..200).map(|i| format!(".c{i}{{color:#333;padding:8px}}")).collect();
    bench("sheet rules + class attrs",
          &format!("<style>{sheet}</style>{}", "<div class=\"c7\">x</div>".repeat(n)), n);
}
