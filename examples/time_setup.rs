/// Microbenchmark to isolate setup costs in the render pipeline.
use std::time::Instant;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::Viewport;
use parley::FontContext;

const MEDIUM_HTML: &str = include_str!("../benches/fixtures/medium.html");
const RUNS: usize = 20;

fn time<F: FnMut()>(label: &str, mut f: F) {
    // warmup
    for _ in 0..3 { f(); }
    let t = Instant::now();
    for _ in 0..RUNS { f(); }
    let avg = t.elapsed() / RUNS as u32;
    println!("{label:<45} {avg:>10.2?}");
}

fn make_viewport() -> Viewport {
    Viewport::new(800, 600, 1.0, blitz_traits::shell::ColorScheme::Light)
}

fn main() {
    println!("{:<45} {:>10}", "Operation", "Avg/call");
    println!("{}", "-".repeat(57));

    // 1. FontContext::default() — cold after warmup (OS file cache warm)
    time("FontContext::default()", || {
        let _ = std::hint::black_box(FontContext::default());
    });

    // 2. FontContext::clone() — from a pre-built instance
    let base_ctx = FontContext::default();
    time("FontContext::clone()", || {
        let _ = std::hint::black_box(base_ctx.clone());
    });

    // 3. HtmlDocument::from_html() with fresh FontContext each time
    time("from_html() [fresh FontContext]", || {
        let doc_config = DocumentConfig {
            viewport: Some(make_viewport()),
            ..Default::default()
        };
        let mut doc = HtmlDocument::from_html(
            std::hint::black_box(MEDIUM_HTML),
            doc_config,
        );
        doc.resolve(0.0);
        let _ = std::hint::black_box(&doc);
    });

    // 4. HtmlDocument::from_html() with cloned FontContext
    let base_ctx = FontContext::default();
    time("from_html() [cloned FontContext]", || {
        let doc_config = DocumentConfig {
            viewport: Some(make_viewport()),
            font_ctx: Some(base_ctx.clone()),
            ..Default::default()
        };
        let mut doc = HtmlDocument::from_html(
            std::hint::black_box(MEDIUM_HTML),
            doc_config,
        );
        doc.resolve(0.0);
        let _ = std::hint::black_box(&doc);
    });

    // 5. Full render (for comparison)
    time("render() full [fresh FontContext]", || {
        let _ = std::hint::black_box(
            hyper_render::render(
                std::hint::black_box(MEDIUM_HTML),
                hyper_render::Config::new().width(800).height(600),
            ).unwrap()
        );
    });

    // 6. Full render with cloned FontContext, using raw API
    let base_ctx = FontContext::default();
    time("render() full [cloned FontContext]", || {
        let doc_config = DocumentConfig {
            viewport: Some(make_viewport()),
            font_ctx: Some(base_ctx.clone()),
            ..Default::default()
        };
        let mut doc = HtmlDocument::from_html(
            std::hint::black_box(MEDIUM_HTML),
            doc_config,
        );
        doc.resolve(0.0);
        use anyrender::render_to_buffer;
        use anyrender_vello_cpu::VelloCpuImageRenderer;
        use blitz_paint::paint_scene;
        let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| paint_scene(scene, doc.as_ref(), 1.0, 800, 600),
            800, 600,
        );
        let _ = std::hint::black_box(buffer);
    });
}
