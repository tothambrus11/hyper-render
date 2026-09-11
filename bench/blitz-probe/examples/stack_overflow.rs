//! Minimal repro: ~40 KB of nested CSS aborts the process with a stack overflow.
//!   cargo run --release --example stack_overflow
//! => thread 'main' has overflowed its stack / fatal runtime error: stack overflow
//! Exit status 134 (SIGABRT). Not catchable: not a panic, so catch_unwind will not help.
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;

fn main() {
    let css = "div{".repeat(10_000); // 40 KB, no closing braces needed
    println!("parsing {} bytes of nested CSS...", css.len());
    HtmlDocument::from_html(&format!("<style>{css}</style>"), DocumentConfig::default());
    println!("survived (unexpected)");
}
