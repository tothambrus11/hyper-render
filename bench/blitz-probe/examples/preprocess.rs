//! Rewriting presentational attributes into inline styles before parsing:
//! fixes the <font color>/named-color detection gaps AND avoids stylo's slow
//! presentational-hint path. Measures the combined win on a realistic email.
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;

fn marketing_email(kb: usize) -> String {
    let row = r##"<table width="600" bgcolor="#ffffff" align="center"><tr>
      <td width="300" bgcolor="#f5f5f5" align="left" style="padding:8px;font-size:14px;color:#333">
      <font color="#ffffff">promo</font>
      <a href="https://example.com" style="color:#1a73e8">Shop</a>
      <span style="display:none">hidden preheader</span></td></tr></table>"##;
    let mut b = String::new();
    while b.len() < kb * 1024 { b.push_str(row); }
    format!("<html><head><style>.a{{color:#000}}</style></head><body>{b}</body></html>")
}

/// Cheap textual rewrite of legacy presentational attributes into `style="..."`.
/// Single pass, no parsing. Also closes the detection gaps (`<font color>`,
/// named-colour `bgcolor`) that stylo's hint path does not implement.
fn preprocess(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + html.len() / 8);
    let b = html.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // find an attribute we care about
        let rest = &html[i..];
        let hit = ["bgcolor=\"", "align=\"", "color=\""].iter()
            .filter_map(|pat| rest.find(pat).map(|p| (p, *pat)))
            .min_by_key(|(p, _)| *p);
        let Some((pos, pat)) = hit else { out.push_str(rest); break };
        let vstart = i + pos + pat.len();
        let Some(vend_rel) = html[vstart..].find('"') else { out.push_str(rest); break };
        let value = &html[vstart..vstart + vend_rel];
        out.push_str(&html[i..i + pos]);
        let css = match pat {
            "bgcolor=\"" => format!("style=\"background-color:{value}\" "),
            "color=\"" => format!("style=\"color:{value}\" "),
            _ => format!("style=\"text-align:{value}\" "),
        };
        out.push_str(&css);
        i = vstart + vend_rel + 1;
    }
    out
}

fn bench(label: &str, html: &str) -> f64 {
    let (mut p, mut s) = (f64::MAX, f64::MAX);
    for _ in 0..15 {
        let t = Instant::now();
        let mut d = HtmlDocument::from_html(html, DocumentConfig::default()).into_inner();
        p = p.min(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        d.resolve_stylist(0.0);
        s = s.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("{label:<38} parse {p:>7.2} ms  style {s:>7.2} ms  total {:>7.2} ms", p + s);
    p + s
}

fn main() {
    for kb in [20usize, 100] {
        let raw = marketing_email(kb);
        println!("--- {} KB marketing email ---", raw.len() / 1024);
        let a = bench("as-is (presentational attrs)", &raw);
        let t = Instant::now();
        let pre = preprocess(&raw);
        let pre_ms = t.elapsed().as_secs_f64() * 1000.0;
        let b = bench("pre-processed to inline styles", &pre);
        println!("{:<38} preprocess {pre_ms:.2} ms -> {:.2}x faster overall\n",
                 "", a / (b + pre_ms));
    }
}
