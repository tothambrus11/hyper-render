//! Malformed-HTML resilience harness.
//! `fuzz` runs every case, each in its own subprocess (so a panic/abort/stack
//! overflow in one case can't take the harness down).
//! `fuzz <name>` runs a single case in-process.

use blitz_dom::{BaseDocument, DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use std::time::{Duration, Instant};

fn cases() -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = vec![
        ("empty", "".into()),
        ("text-only", "just text, no tags".into()),
        ("unclosed-style", "<html><body><style>.a{display:none}<p>SWALLOWED".into()),
        ("unclosed-script", "<html><body><script>var x=1;<p>SWALLOWED".into()),
        ("unclosed-comment", "<html><body><!-- <p>SWALLOWED".into()),
        ("unclosed-title", "<html><head><title>T<body><p>SWALLOWED".into()),
        ("unclosed-textarea", "<html><body><textarea><p>SWALLOWED".into()),
        ("plaintext", "<html><body><plaintext><p>NEVER-ELEMENT".into()),
        ("misnested-fmt", "<html><body><b><i>x</b>y</i>z".into()),
        ("table-foster", "<html><body><table><p>FOSTERED</p><tr><td>c</td></tr></table>".into()),
        ("stray-close", "<html><body><p>a</p></body></html><p>AFTER-CLOSE".into()),
        ("multi-body-head", "<html><head></head><body>1<body>2<head>3</body></html>".into()),
        ("foreign-svg", "<html><body><svg><foreignObject><p>IN-SVG</p></foreignObject></svg><p>after".into()),
        ("foreign-math", "<html><body><math><mtext><p>IN-MATH</p></mtext></math>".into()),
        ("null-bytes", "<html><body><p>a\0b</p><div\0>c</div>".into()),
        ("bad-entities", "<html><body><p>&amp &#xZZ; &notarealentity; &#999999999;</p>".into()),
        ("unquoted-attrs", "<html><body><p class=a b=<c d=\"e style=display:none>X".into()),
        ("cdata-outside-foreign", "<html><body><![CDATA[<p>X</p>]]>".into()),
        ("bogus-doctype", "<!DOCTYPE html SYSTEM \"about:legacy-compat\" garbage><html><body><p>quirks".into()),
        ("processing-instr", "<?xml version=\"1.0\"?><html><body><p>X".into()),
        // malformed CSS
        ("css-junk-inline", "<html><body><p style=\"display:none;;;garbage:;color\">X</p>".into()),
        ("css-unbalanced-brace", "<html><head><style>.a{display:none</style></head><body><p class=a>X".into()),
        ("css-bad-important", "<html><body><p style=\"display:none !important junk\">X</p>".into()),
        ("css-nested-junk", "<html><head><style>@media{{{ .a{display:none} </style></head><body><p class=a>X".into()),
        ("css-var-selfref", "<html><body><div style=\"--a:var(--a);display:var(--a)\">X</div>".into()),
    ];
    // generated / size stress
    v.push(("attr-50k", format!("<html><body><p title=\"{}\">X</p>", "a".repeat(50_000))));
    v.push(("deep-div-10k", format!("<html><body>{}X{}", "<div>".repeat(10_000), "</div>".repeat(10_000))));
    v.push(("deep-div-100k", format!("<html><body>{}X", "<div>".repeat(100_000))));
    v.push(("deep-b-10k", format!("<html><body>{}X", "<b>".repeat(10_000))));
    v.push(("deep-unclosed-p-50k", format!("<html><body>{}X", "<p>".repeat(50_000))));
    v.push(("many-siblings-200k", format!("<html><body>{}", "<span>x</span>".repeat(200_000))));
    v.push(("deep-css-nesting", format!("<html><head><style>{}", "div{".repeat(20_000))));
    v.push(("invalid-utf8-ish", "<html><body><p>\u{FFFD}\u{202E}\u{200B}\u{FEFF}</p>".into()));
    v
}

fn walk_count(doc: &BaseDocument) -> (usize, usize, usize) {
    let (mut els, mut styled, mut maxdepth) = (0, 0, 0);
    let mut stack: Vec<(NodeId, usize)> = vec![(doc.root_node().id, 0)];
    while let Some((id, d)) = stack.pop() {
        maxdepth = maxdepth.max(d);
        if let Some(n) = doc.get_node(id) {
            if matches!(n.data, NodeData::Element(_)) {
                els += 1;
                if n.primary_styles().is_some() {
                    styled += 1;
                }
            }
            for c in n.children.iter().rev() {
                stack.push((*c, d + 1));
            }
        }
    }
    (els, styled, maxdepth)
}

fn run_one(html: &str) -> (usize, usize, usize) {
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    )
    .into_inner();
    doc.resolve_stylist(0.0);
    walk_count(&doc)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(name) = args.get(1) {
        let all = cases();
        let (_, html) = all.iter().find(|(n, _)| n == name).expect("unknown case");
        let (els, styled, depth) = run_one(html);
        println!("elements={els} styled={styled} max_depth={depth}");
        return;
    }

    let exe = std::env::current_exe().unwrap();
    println!("{:<24} {:>8}  {:<10} {}", "CASE", "MS", "RESULT", "DETAIL");
    for (name, html) in cases() {
        let start = Instant::now();
        let mut child = std::process::Command::new(&exe)
            .arg(name)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        // poll with a 60s wall-clock cap so a hang is reported, not inherited
        let status = loop {
            match child.try_wait().unwrap() {
                Some(s) => break Some(s),
                None if start.elapsed() > Duration::from_secs(60) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        };
        let ms = start.elapsed().as_millis();
        let out = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&out.stderr);

        let (result, detail) = match status {
            None => ("TIMEOUT".to_string(), format!("input_bytes={}", html.len())),
            Some(s) if s.success() => ("ok".to_string(), stdout),
            Some(s) => {
                let first = stderr
                    .lines()
                    .find(|l| l.contains("panicked") || l.contains("overflow") || l.contains("error"))
                    .unwrap_or("<no message>")
                    .trim()
                    .chars()
                    .take(90)
                    .collect::<String>();
                (format!("FAIL({s})"), first)
            }
        };
        println!("{name:<24} {ms:>8}  {result:<10} {detail}");
    }
}
