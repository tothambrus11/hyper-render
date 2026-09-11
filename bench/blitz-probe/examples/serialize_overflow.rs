//! Does Blitz's DOM serializer recurse? (This is the shape of RUSTSEC-2019-0001,
//! the ammonia CVE: recursive HTML serialization aborts on deeply nested input.)
//!   cargo run --release --example serialize_overflow
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let doc = HtmlDocument::from_html(&"<div>".repeat(n), DocumentConfig::default()).into_inner();
    println!("parsed {n} nested <div>; serializing...");
    let html = doc.root_element().outer_html();
    println!("serialized {} bytes (survived)", html.len());
}
