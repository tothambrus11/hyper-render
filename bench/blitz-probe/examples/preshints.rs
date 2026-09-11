//! Which HTML presentational hints (HTML Standard section 15 "Rendering") does Blitz/stylo
//! actually map into computed style? Each case: minimal doc, read the computed property.
//! PASS = Blitz applies it. MISSING = we must transform it ourselves.
use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

fn doc(html: &str) -> BaseDocument {
    let mut d = HtmlDocument::from_html(html, DocumentConfig {
        viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
        base_url: Some("https://e.invalid/".into()),
        ..Default::default()
    }).into_inner();
    d.resolve_stylist(0.0);
    d
}

/// Read one computed property of the element carrying id="t".
fn prop(d: &BaseDocument, which: &str) -> String {
    let mut st: Vec<NodeId> = vec![d.root_node().id];
    while let Some(id) = st.pop() {
        let Some(n) = d.get_node(id) else { continue };
        if let NodeData::Element(e) = &n.data {
            if e.attr(blitz_dom::local_name!("id")) == Some("t") {
                let Some(s) = n.primary_styles() else { return "NO STYLES".into() };
                return match which {
                    "background-color" => format!("{:?}", s.get_background().clone_background_color()),
                    "color" => format!("{:?}", s.get_inherited_text().clone_color()),
                    "text-align" => format!("{:?}", s.get_inherited_text().clone_text_align()),
                    "width" => format!("{:?}", s.get_position().clone_width()),
                    "height" => format!("{:?}", s.get_position().clone_height()),
                    "font-size" => format!("{:?}", s.get_font().clone_font_size().computed_size()),
                    "font-family" => format!("{:?}", s.get_font().clone_font_family()),
                    "vertical-align" => format!("{:?}/{:?}", s.get_box().clone_alignment_baseline(), s.get_box().clone_baseline_source()),
                    "white-space" => format!("{:?}", s.get_inherited_text().clone_white_space_collapse()),
                    "border-top-width" => format!("{:?}", s.get_border().clone_border_top_width()),
                    "margin-left" => format!("{:?}", s.get_margin().clone_margin_left()),
                    "padding-left" => format!("{:?}", s.get_padding().clone_padding_left()),
                    "border-spacing" => format!("{:?}", s.get_inherited_table().clone_border_spacing()),
                    "list-style-type" => format!("{:?}", s.get_list().clone_list_style_type()),
                    "display" => format!("{:?}", s.get_box().clone_display()),
                    "background-image" => format!("{:?}", s.get_background().clone_background_image()),
                    _ => "?".into(),
                };
            }
        }
        st.extend(n.children.iter().copied());
    }
    "NOT FOUND".into()
}

fn main() {
    // (attribute/element, html, property to inspect, baseline html without the attr)
    let cases: Vec<(&str, String, &str, String)> = vec![
        ("bgcolor=\"#ffffff\"", "<table id=t bgcolor=\"#ffffff\"><tr><td>x".into(), "background-color", "<table id=t><tr><td>x".into()),
        ("bgcolor=\"white\" (named)", "<table id=t bgcolor=\"white\"><tr><td>x".into(), "background-color", "<table id=t><tr><td>x".into()),
        ("bgcolor=\"ffffff\" (bare hex)", "<table id=t bgcolor=\"ffffff\"><tr><td>x".into(), "background-color", "<table id=t><tr><td>x".into()),
        ("body bgcolor", "<body id=t bgcolor=\"#ff0000\">x".into(), "background-color", "<body id=t>x".into()),
        ("<font color=\"#ff0000\">", "<font id=t color=\"#ff0000\">x</font>".into(), "color", "<font id=t>x</font>".into()),
        ("<font color=\"red\">", "<font id=t color=\"red\">x</font>".into(), "color", "<font id=t>x</font>".into()),
        ("<font size=\"7\">", "<font id=t size=\"7\">x</font>".into(), "font-size", "<font id=t>x</font>".into()),
        ("<font face=\"Courier\">", "<font id=t face=\"Courier\">x</font>".into(), "font-family", "<font id=t>x</font>".into()),
        ("body text=\"#ff0000\"", "<body id=t text=\"#ff0000\">x".into(), "color", "<body id=t>x".into()),
        ("align=\"center\" (div)", "<div id=t align=\"center\">x</div>".into(), "text-align", "<div id=t>x</div>".into()),
        ("align=\"right\" (td)", "<table><tr><td id=t align=\"right\">x".into(), "text-align", "<table><tr><td id=t>x".into()),
        ("valign=\"top\" (td)", "<table><tr><td id=t valign=\"top\">x".into(), "vertical-align", "<table><tr><td id=t>x".into()),
        ("width=\"300\" (td)", "<table><tr><td id=t width=\"300\">x".into(), "width", "<table><tr><td id=t>x".into()),
        ("width=\"50%\" (table)", "<table id=t width=\"50%\"><tr><td>x".into(), "width", "<table id=t><tr><td>x".into()),
        ("height=\"200\" (td)", "<table><tr><td id=t height=\"200\">x".into(), "height", "<table><tr><td id=t>x".into()),
        ("img width/height", "<img id=t width=\"100\" height=\"50\" src=\"https://e.invalid/a.png\">".into(), "width", "<img id=t src=\"https://e.invalid/a.png\">".into()),
        ("hspace=\"20\" (img)", "<img id=t hspace=\"20\" src=\"https://e.invalid/a.png\">".into(), "margin-left", "<img id=t src=\"https://e.invalid/a.png\">".into()),
        ("border=\"5\" (table)", "<table id=t border=\"5\"><tr><td>x".into(), "border-top-width", "<table id=t><tr><td>x".into()),
        ("border=\"3\" (img)", "<img id=t border=\"3\" src=\"https://e.invalid/a.png\">".into(), "border-top-width", "<img id=t src=\"https://e.invalid/a.png\">".into()),
        ("cellpadding=\"10\"", "<table cellpadding=\"10\"><tr><td id=t>x".into(), "padding-left", "<table><tr><td id=t>x".into()),
        ("cellspacing=\"10\"", "<table id=t cellspacing=\"10\"><tr><td>x".into(), "border-spacing", "<table id=t><tr><td>x".into()),
        ("nowrap (td)", "<table><tr><td id=t nowrap>x".into(), "white-space", "<table><tr><td id=t>x".into()),
        ("background=\"https://e.invalid/u.png\" (table)", "<table id=t background=\"https://e.invalid/u.png\"><tr><td>x".into(), "background-image", "<table id=t><tr><td>x".into()),
        ("<center>", "<center id=t>x</center>".into(), "text-align", "<div id=t>x</div>".into()),
        ("<nobr>", "<nobr id=t>x</nobr>".into(), "white-space", "<span id=t>x</span>".into()),
        ("type=\"a\" (ol)", "<ol id=t type=\"a\"><li>x".into(), "list-style-type", "<ol id=t><li>x".into()),
        ("marginwidth (body)", "<body id=t marginwidth=\"0\">x".into(), "margin-left", "<body id=t>x".into()),
        ("hidden attribute", "<div id=t hidden>x</div>".into(), "display", "<div id=t>x</div>".into()),
    ];

    println!("{:<32} {:<18} {:<10} {}", "HINT", "PROPERTY", "STATUS", "COMPUTED");
    println!("{}", "-".repeat(110));
    let (mut ok, mut missing) = (0, 0);
    for (name, html, which, base) in &cases {
        let got = prop(&doc(&format!("<html><body>{html}</body></html>")), which);
        let want_not = prop(&doc(&format!("<html><body>{base}</body></html>")), which);
        let applied = got != want_not;
        if applied { ok += 1 } else { missing += 1 }
        let short: String = got.chars().take(42).collect();
        println!("{:<32} {:<18} {:<10} {}", name, which,
                 if applied { "PASS" } else { "MISSING" }, short);
    }
    println!("\napplied: {ok}   MISSING (we must transform): {missing}");
}
