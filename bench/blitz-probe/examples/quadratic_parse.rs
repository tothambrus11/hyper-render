//! Minimal repro: HTML parsing is O(n^2) in nesting depth.
//!   cargo run --release --example quadratic_parse
//! Time roughly 4x per doubling of depth (quadratic), while the input only doubles.
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use std::time::Instant;

fn main() {
    for n in [10_000, 20_000, 40_000, 80_000] {
        let html = "<div>".repeat(n); // unclosed is fine; depth is what matters
        let t = Instant::now();
        HtmlDocument::from_html(&html, DocumentConfig::default());
        println!("{n:>6} nested <div> ({:>6} KB): {:>6} ms", html.len() / 1024, t.elapsed().as_millis());
    }
}
