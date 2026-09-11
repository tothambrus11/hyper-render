use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
fn main() {
    let kind = std::env::args().nth(1).unwrap();
    let n: usize = std::env::args().nth(2).unwrap().parse().unwrap();
    let css = match kind.as_str() {
        "nest"    => "div{".repeat(n),
        "is"      => format!("{}a{}{{color:red}}", ":is(".repeat(n), ")".repeat(n)),
        "not"     => format!("{}a{}{{color:red}}", ":not(".repeat(n), ")".repeat(n)),
        "paren"   => format!("a{{width:calc{}1{}}}", "(".repeat(n), ")".repeat(n)),
        "media"   => format!("{}a{{color:red}}", "@media screen{".repeat(n)),
        "supports"=> format!("{}a{{color:red}}", "@supports (color:red){".repeat(n)),
        "attrsel" => format!("{}a{{color:red}}", "div ".repeat(n)),
        _ => unreachable!(),
    };
    let bytes = css.len();
    let html = format!("<html><head><style>{css}</style></head><body><a>x</a>");
    let _ = HtmlDocument::from_html(&html, DocumentConfig {
        viewport: Some(Viewport::new(800,600,1.0,ColorScheme::Light)), ..Default::default() });
    println!("{kind} n={n} css_bytes={bytes} OK");
}
