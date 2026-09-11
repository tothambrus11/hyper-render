use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
fn main() {
    let n: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let mb: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let css = "div{".repeat(n);
    let html = format!("<html><head><style>{css}</style></head><body><a>x</a>");
    let h = std::thread::Builder::new()
        .stack_size(mb * 1024 * 1024)
        .spawn(move || {
            let _ = HtmlDocument::from_html(&html, DocumentConfig {
                viewport: Some(Viewport::new(800,600,1.0,ColorScheme::Light)), ..Default::default() });
            "parsed"
        }).unwrap();
    match h.join() { Ok(m) => println!("n={n} stack={mb}MB -> {m}"), Err(_) => println!("n={n} stack={mb}MB -> THREAD PANIC (caught)") }
}
