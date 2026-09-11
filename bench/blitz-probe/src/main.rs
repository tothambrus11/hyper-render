use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

/// Parse HTML and resolve styles ONLY -- no layout, no painting.
fn style_only(html: &str, width: u32, height: u32) -> BaseDocument {
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(width, height, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    )
    .into_inner();
    doc.resolve_stylist(0.0);
    doc
}

fn tag_of(doc: &BaseDocument, id: NodeId) -> String {
    match doc.get_node(id).map(|n| &n.data) {
        Some(NodeData::Element(e)) => e.name.local.to_string(),
        Some(NodeData::Text(_)) => "#text".into(),
        Some(NodeData::Comment { .. }) => "#comment".into(),
        Some(NodeData::Document(_)) => "#document".into(),
        Some(NodeData::AnonymousBlock(_)) => "#anon".into(),
        None => "#gone".into(),
    }
}

/// Iterative walk (no recursion) so deep trees can't blow our stack.
fn walk(doc: &BaseDocument, mut f: impl FnMut(NodeId, usize)) {
    let mut stack = vec![(doc.root_node().id, 0usize)];
    while let Some((id, depth)) = stack.pop() {
        f(id, depth);
        if let Some(n) = doc.get_node(id) {
            for c in n.children.iter().rev() {
                stack.push((*c, depth + 1));
            }
        }
    }
}

/// Everything a hidden-element detector needs, read off stylo's ComputedValues.
fn describe(doc: &BaseDocument, id: NodeId) -> String {
    let Some(node) = doc.get_node(id) else { return "no node".into() };
    let Some(s) = node.primary_styles() else { return "NO STYLES".into() };
    let b = s.get_box();
    let pos = s.get_position();
    let ivis = s.get_inherited_box();
    let itext = s.get_inherited_text();
    let font = s.get_font();
    let bg = s.get_background();
    format!(
        "display={:?} visibility={:?} opacity={} position={:?} left={:?} \
         font-size={:?} color={:?} bg={:?} overflow={:?}/{:?} text-indent={:?}",
        b.clone_display(),
        ivis.clone_visibility(),
        s.get_effects().clone_opacity(),
        b.clone_position(),
        pos.clone_left(),
        font.clone_font_size().computed_size(),
        itext.clone_color(),
        bg.clone_background_color(),
        b.clone_overflow_x(),
        b.clone_overflow_y(),
        itext.clone_text_indent(),
    )
}

fn dump(label: &str, html: &str) {
    println!("\n========== {label} ==========");
    let doc = style_only(html, 800, 600);
    walk(&doc, |id, depth| {
        let Some(node) = doc.get_node(id) else { return };
        match &node.data {
            NodeData::Element(_) => println!(
                "{:indent$}<{}> {}",
                "",
                tag_of(&doc, id),
                describe(&doc, id),
                indent = depth * 2
            ),
            NodeData::Text(t) => {
                let txt = t.content.trim();
                if !txt.is_empty() {
                    println!("{:indent$}#text {:?}", "", txt, indent = depth * 2);
                }
            }
            _ => {}
        }
    });
}

fn main() {
    dump("1. var() driving display", r##"<html><body>
      <div style="--x:none"><p style="display:var(--x)">HIDDEN-BY-VAR</p></div>
      <p style="display:var(--undefined,none)">HIDDEN-BY-VAR-FALLBACK</p>
    </body></html>"##);

    dump("2. var reference cycle (must not panic)", r##"<html><body>
      <div style="--a:var(--b); --b:var(--a); display:var(--a)">CYCLE</div>
    </body></html>"##);

    dump("3. inherited visibility re-shown by child", r##"<html><body>
      <div style="visibility:hidden"><span style="visibility:visible">SHOWN-AGAIN</span></div>
    </body></html>"##);

    dump("4. inherited var from grandparent -> font-size:0", r##"<html><body>
      <div style="--size:0px"><section><span style="font-size:var(--size)">ZERO-FONT</span></section></div>
    </body></html>"##);

    dump("5. descendants of display:none -- do they get styles?", r##"<html><body>
      <div style="display:none"><span style="color:red">CHILD-OF-NONE</span></div>
    </body></html>"##);

    dump("6. <style> in body, specificity, !important", r##"<html><body>
      <style>.s { display:none } #k { color:blue }</style>
      <p class="s" style="display:block">INLINE-WINS</p>
      <p class="s">SHEET-HIDES</p>
      <p id="k">BLUE</p>
    </body></html>"##);

    dump("7. classic offscreen / zero / transparent tricks", r##"<html><body>
      <p style="position:absolute;left:-9999px">OFFSCREEN</p>
      <p style="opacity:0">TRANSPARENT</p>
      <p style="font-size:0">ZEROFONT</p>
      <p style="text-indent:-9999px">INDENTED-AWAY</p>
      <p style="clip:rect(0,0,0,0);position:absolute">CLIPPED</p>
      <p style="color:#ffffff;background-color:#ffffff">WHITE-ON-WHITE</p>
    </body></html>"##);

    dump("8. hidden attr, noscript, legacy color attrs", r##"<html><body>
      <p hidden>HIDDEN-ATTR</p>
      <noscript><p>NOSCRIPT-CONTENT</p></noscript>
      <font color="white">FONT-NAMED-COLOR</font>
      <font color="#ffffff">FONT-HEX-COLOR</font>
      <div bgcolor="white">BGCOLOR-NAMED</div>
      <div bgcolor="#ffffff">BGCOLOR-HEX</div>
    </body></html>"##);

    dump("9. head/script/style/template are display:none", r##"<html><head><title>T</title></head><body>
      <template><p>IN-TEMPLATE</p></template>
      <script>var x="SCRIPT-TEXT"</script>
      <p>VISIBLE</p>
    </body></html>"##);

    // media query sensitivity to viewport
    let mq = r##"<html><head><style>@media (max-width:600px){ .m{display:none} }</style></head>
      <body><p class="m">MEDIA</p></body></html>"##;
    println!("\n========== 10. @media viewport sensitivity ==========");
    for w in [800u32, 400u32] {
        let doc = style_only(mq, w, 600);
        walk(&doc, |id, _| {
            if tag_of(&doc, id) == "p" {
                println!("  viewport {w}px -> {}", describe(&doc, id));
            }
        });
    }
}
