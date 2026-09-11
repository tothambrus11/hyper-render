use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;
use regex_lite::Regex;
fn email(kb: usize) -> String {
    let row = r##"<table width="600" bgcolor="#ffffff" align="center"><tr>
      <td width="300" bgcolor="#f5f5f5" align="left" style="padding:8px;font-size:14px;color:#333">
      <a href="https://example.com" style="color:#1a73e8">Shop</a>
      <span style="display:none">hidden</span></td></tr></table>"##;
    let mut b = String::new();
    while b.len() < kb*1024 { b.push_str(row); }
    format!("<html><body>{b}</body></html>")
}
fn bench(label:&str, html:&str){
    let (mut p,mut s)=(f64::MAX,f64::MAX);
    for _ in 0..15 {
        let t=Instant::now();
        let mut d=HtmlDocument::from_html(html,DocumentConfig::default()).into_inner();
        p=p.min(t.elapsed().as_secs_f64()*1000.0);
        let t=Instant::now(); d.resolve_stylist(0.0);
        s=s.min(t.elapsed().as_secs_f64()*1000.0);
    }
    println!("{label:<40} parse {p:>7.2}  style {s:>7.2}  total {:>7.2}", p+s);
}
fn main(){
    let raw = email(std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100));
    let re = Regex::new(r#"\s(bgcolor|align|width|height)="[^"]*""#).unwrap();
    let stripped = re.replace_all(&raw, "").to_string();
    println!("--- {} KB email ---", raw.len()/1024);
    bench("with presentational attrs", &raw);
    bench("presentational attrs STRIPPED", &stripped);
}
