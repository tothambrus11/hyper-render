//! Cost of isolation: inline vs subprocess-per-document vs persistent worker.
//!   cargo run --release --example bench_isolation
//! Child mode: `bench_isolation --child` (one doc on stdin) or `--worker` (many).
use blitz_dom::{DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

/// A realistic marketing email: nested tables, inline styles, a <style> block.
fn sample_email(kb: usize) -> String {
    let row = r#"<tr><td style="padding:8px;font-family:Arial;font-size:14px;color:#333">
      <a href="https://example.com/x" style="color:#1a73e8;text-decoration:none">Shop now</a>
      <span style="display:none">hidden preheader text</span></td></tr>"#;
    let mut body = String::new();
    while body.len() < kb * 1024 {
        body.push_str(&format!("<table width=\"600\" bgcolor=\"#ffffff\">{row}</table>"));
    }
    format!("<html><head><style>.a{{color:#000}} .b{{display:none}} \
        @media (max-width:600px){{.c{{display:none}}}}</style></head><body>{body}</body></html>")
}

/// The actual work: parse + resolve styles + count hidden elements.
fn scan(html: &str) -> usize {
    let mut doc = HtmlDocument::from_html(html, DocumentConfig::default()).into_inner();
    doc.resolve_stylist(0.0);
    let mut hidden = 0;
    let mut stack: Vec<NodeId> = vec![doc.root_node().id];
    while let Some(id) = stack.pop() {
        if let Some(n) = doc.get_node(id) {
            if matches!(n.data, NodeData::Element(_)) {
                if let Some(s) = n.primary_styles() {
                    if s.get_box().clone_display().is_none() { hidden += 1; }
                }
            }
            stack.extend(n.children.iter().copied());
        }
    }
    hidden
}

fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p) as usize]
}

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();

    // ---- child: one document per process ----
    if arg == "--child" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).unwrap();
        println!("{}", scan(&s));
        return;
    }
    // ---- worker: many documents, one process (length-prefixed lines) ----
    // --echo does the identical IPC but skips scanning, isolating transport cost.
    if arg == "--echo" || arg == "--worker" {
        let scanning = arg == "--worker";
        let stdin = std::io::stdin();
        let mut r = BufReader::new(stdin.lock());
        let mut out = std::io::stdout();
        loop {
            let mut hdr = String::new();
            if r.read_line(&mut hdr).unwrap_or(0) == 0 { return; }
            let n: usize = match hdr.trim().parse() { Ok(n) => n, Err(_) => return };
            let mut buf = vec![0u8; n];
            if r.read_exact(&mut buf).is_err() { return; }
            let hidden = if scanning { scan(&String::from_utf8_lossy(&buf)) } else { buf.len() };
            writeln!(out, "{hidden}").unwrap();
            out.flush().unwrap();
        }
    }

    let kb: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(40);
    let html = sample_email(kb);
    let iters = 200;
    let exe = std::env::current_exe().unwrap();
    println!("document: {} KB, {iters} iterations\n", html.len() / 1024);

    // ---- 1. inline ----
    let mut inline = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        std::hint::black_box(scan(&html));
        inline.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    // ---- 2. subprocess per document ----
    let mut spawn = Vec::new();
    for _ in 0..iters.min(60) {
        let t = Instant::now();
        let mut c = Command::new(&exe).arg("--child")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        c.stdin.take().unwrap().write_all(html.as_bytes()).unwrap();
        let _ = c.wait_with_output().unwrap();
        spawn.push(t.elapsed().as_secs_f64() * 1000.0);
    }

    // ---- 3. persistent worker (spawn once, reuse) ----
    let t_spawn = Instant::now();
    let mut w = Command::new(&exe).arg("--worker")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut wi = w.stdin.take().unwrap();
    let mut wo = BufReader::new(w.stdout.take().unwrap());
    let worker_startup = t_spawn.elapsed().as_secs_f64() * 1000.0;

    let mut ipc = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        write!(wi, "{}\n", html.len()).unwrap();
        wi.write_all(html.as_bytes()).unwrap();
        wi.flush().unwrap();
        let mut line = String::new();
        wo.read_line(&mut line).unwrap();
        ipc.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    drop(wi);
    let _ = w.wait();

    // ---- 4. pure IPC transport (echo worker: same pipes, no scanning) ----
    let mut e = Command::new(&exe).arg("--echo")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut ei = e.stdin.take().unwrap();
    let mut eo = BufReader::new(e.stdout.take().unwrap());
    let mut transport = Vec::new();
    for _ in 0..iters {
        let t = Instant::now();
        write!(ei, "{}\n", html.len()).unwrap();
        ei.write_all(html.as_bytes()).unwrap();
        ei.flush().unwrap();
        let mut line = String::new();
        eo.read_line(&mut line).unwrap();
        transport.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    drop(ei);
    let _ = e.wait();

    println!("{:<34} {:>9} {:>9} {:>9}", "", "p50 ms", "p95 ms", "max ms");
    for (name, v) in [("1. inline (no isolation)", &mut inline),
                      ("2. subprocess per document", &mut spawn),
                      ("3. persistent worker (total)", &mut ipc),
                      ("4. pure IPC transport (echo)", &mut transport)] {
        println!("{name:<34} {:>9.3} {:>9.3} {:>9.3}", pct(v, 0.50), pct(v, 0.95), pct(v, 1.0));
    }
    println!("\nworker startup (one time): {worker_startup:.1} ms");
}
