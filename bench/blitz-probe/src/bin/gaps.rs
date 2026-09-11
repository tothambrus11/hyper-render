use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

fn doc_of(html: &str) -> BaseDocument {
    let mut d = HtmlDocument::from_html(html, DocumentConfig {
        viewport: Some(Viewport::new(800,600,1.0,ColorScheme::Light)), ..Default::default() }).into_inner();
    d.resolve_stylist(0.0);
    d
}
fn find(doc: &BaseDocument, tag: &str) -> Vec<NodeId> {
    let mut out = vec![]; let mut st = vec![doc.root_node().id];
    while let Some(id) = st.pop() {
        if let Some(n) = doc.get_node(id) {
            if let NodeData::Element(e) = &n.data { if e.name.local.as_ref() == tag { out.push(id); } }
            st.extend(n.children.iter().copied());
        }
    } out
}

fn main() {
    // Which "hiding" declarations survive into computed style?
    let html = r##"<html><body>
      <p id=1 style="clip:rect(0,0,0,0)">clip</p>
      <p id=2 style="clip-path:inset(100%)">clip-path</p>
      <p id=3 style="transform:scale(0)">transform</p>
      <p id=4 style="filter:opacity(0)">filter</p>
      <p id=5 style="width:0;height:0;overflow:hidden">zero-size</p>
      <p id=6 style="max-height:0;overflow:hidden">max-height</p>
      <p id=7 style="content-visibility:hidden">content-visibility</p>
      <p id=8 style="-webkit-text-security:disc">text-security</p>
      <p id=9 style="color:transparent">color-transparent</p>
      <p id=10 style="letter-spacing:-1em">letter-spacing</p>
      <p id=11 style="text-transform:uppercase;font-size:1px">tiny</p>
      <p id=12 aria-hidden="true">aria-hidden</p>
      <p id=13 style="display:none" hidden>both</p>
    </body></html>"##;
    let d = doc_of(html);
    for id in find(&d, "p") {
        let n = d.get_node(id).unwrap();
        let which = match &n.data { NodeData::Element(e) => e.attr(blitz_dom::local_name!("id")).unwrap_or("?").to_string(), _ => "?".into() };
        let Some(s) = n.primary_styles() else { println!("id={which} NO STYLES"); continue };
        println!("id={:<3} display={:?} opacity={} transform={} filter={} width={:?} maxh={:?} color_a={} letter={:?}",
            which, s.get_box().clone_display(), s.get_effects().clone_opacity(),
            !s.get_box().clone_transform().0.is_empty(),
            !s.get_effects().clone_filter().0.is_empty(),
            s.get_position().clone_width(), s.get_position().clone_max_height(),
            s.get_inherited_text().clone_color().alpha,
            s.get_inherited_text().clone_letter_spacing());
        println!("      clip={:?} clip_path={:?}", s.get_effects().clone_clip(), s.get_svg().clone_clip_path());
    }

    // Concurrency: two documents resolving on two threads (default StyleThreading)
    println!("\n-- concurrent resolve on 2 threads (default StyleThreading) --");
    let hs: Vec<_> = (0..2).map(|i| std::thread::spawn(move || {
        for _ in 0..20 { let _ = doc_of("<html><body><div style='display:none'>x</div></body></html>"); }
        format!("thread {i} ok")
    })).collect();
    for h in hs { match h.join() { Ok(m) => println!("{m}"), Err(_) => println!("THREAD PANICKED") } }
}
