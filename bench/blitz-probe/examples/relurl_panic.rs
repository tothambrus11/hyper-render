//! Ordinary HTML with a relative URL panics when DocumentConfig has no base_url.
//!   cargo run --release --example relurl_panic
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
fn main() {
    for html in ["<img src=\"a.png\">", "<img src=\"/a.png\">",
                 "<link rel=stylesheet href=\"s.css\">", "<img src=\"https://e.com/a.png\">"] {
        let a = std::panic::catch_unwind(|| {
            let _ = HtmlDocument::from_html(html, DocumentConfig::default());
        });
        let b = std::panic::catch_unwind(|| {
            let _ = HtmlDocument::from_html(html, DocumentConfig {
                base_url: Some("https://example.invalid/".into()), ..Default::default() });
        });
        println!("{:<45} default={:<7} with base_url={}", html,
                 if a.is_ok() { "ok" } else { "PANIC" }, if b.is_ok() { "ok" } else { "PANIC" });
    }
}
