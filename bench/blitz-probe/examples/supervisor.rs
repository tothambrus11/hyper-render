//! Supervised worker pool: guard prevents the known crashes, supervisor survives the rest.
//!   cargo run --release --example supervisor
//!
//! Architecture this measures:
//!   parent  -- cheap O(n) guard --> reject obvious bombs (no parse at all)
//!           -- length-prefixed pipe --> persistent worker process (does parse+style)
//!           -- worker died? --> restart, mark that doc suspicious, keep going
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

use blitz_dom::{DocumentConfig, NodeData, NodeId};
use blitz_html::HtmlDocument;

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

/// Cheap pre-parse guard. Returns Err(reason) if the document should not be parsed.
/// Deliberately simple: byte scan only, no tokenizer, so it costs ~nothing.
/// (examples/depth_guard.rs has the accurate tokenizer-based version.)
fn guard(html: &str, html_depth_limit: usize, css_depth_limit: usize) -> Result<(), String> {
    let (mut tag_depth, mut max_tag) = (0i64, 0i64);
    let (mut brace, mut paren, mut max_css) = (0i64, 0i64, 0i64);
    let b = html.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' if i + 1 < b.len() => {
                if b[i + 1] == b'/' { tag_depth -= 1; } else if b[i + 1] != b'!' {
                    tag_depth += 1;
                    max_tag = max_tag.max(tag_depth);
                    if max_tag > html_depth_limit as i64 {
                        return Err(format!("html nesting depth > {html_depth_limit}"));
                    }
                }
            }
            b'{' => { brace += 1; max_css = max_css.max(brace); }
            b'}' => brace -= 1,
            b'(' => { paren += 1; max_css = max_css.max(paren); }
            b')' => paren -= 1,
            _ => {}
        }
        if max_css > css_depth_limit as i64 {
            return Err(format!("css bracket depth > {css_depth_limit}"));
        }
        i += 1;
    }
    Ok(())
}

struct Worker { child: Child, stdin: ChildStdin, stdout: BufReader<ChildStdout> }

impl Worker {
    fn spawn(exe: &std::path::Path) -> Worker {
        let mut c = Command::new(exe).arg("--worker")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let stdin = c.stdin.take().unwrap();
        let stdout = BufReader::new(c.stdout.take().unwrap());
        Worker { child: c, stdin, stdout }
    }
    /// Err(()) means the worker died mid-document.
    fn scan(&mut self, html: &str) -> Result<usize, ()> {
        if write!(self.stdin, "{}\n", html.len()).is_err() { return Err(()); }
        if self.stdin.write_all(html.as_bytes()).is_err() { return Err(()); }
        if self.stdin.flush().is_err() { return Err(()); }
        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) | Err(_) => Err(()),                        // EOF = crashed
            Ok(_) => line.trim().parse().map_err(|_| ()),
        }
    }
    fn kill(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--worker") {
        let stdin = std::io::stdin();
        let mut r = BufReader::new(stdin.lock());
        let mut out = std::io::stdout();
        loop {
            let mut hdr = String::new();
            if r.read_line(&mut hdr).unwrap_or(0) == 0 { return; }
            let n: usize = match hdr.trim().parse() { Ok(n) => n, Err(_) => return };
            let mut buf = vec![0u8; n];
            if r.read_exact(&mut buf).is_err() { return; }
            let hidden = scan(&String::from_utf8_lossy(&buf));  // may abort: that is the point
            writeln!(out, "{hidden}").unwrap();
            out.flush().unwrap();
        }
    }

    const HTML_LIMIT: usize = 512;   // matches Blink's kMaximumHTMLParserDOMTreeDepth
    const CSS_LIMIT: usize = 64;

    let normal = "<html><body><table><tr><td style=\"color:#333\">hi \
                  <span style=\"display:none\">hidden</span></td></tr></table></body></html>"
                  .repeat(50);
    let html_bomb = "<div>".repeat(80_000);
    let css_bomb = format!("<style>{}</style>", "div{".repeat(10_000));
    // Stands in for an UNKNOWN vector -- a crash the guard does not model yet.
    // We can't honestly construct one (the guard covers every vector we know), so this
    // real bomb is marked `bypass` to skip the guard and exercise the recovery path.
    let unknown = format!("<style>{}</style>", "div{".repeat(10_000));

    let exe = std::env::current_exe().unwrap();
    let mut worker = Worker::spawn(&exe);
    let (mut done, mut rejected, mut crashed) = (0, 0, 0);
    let mut restart_ms: Vec<f64> = Vec::new();
    let mut guard_us: Vec<f64> = Vec::new();

    // 200 normal docs with bombs sprinkled in
    // (label, document, bypass_guard)
    let mut queue: Vec<(&str, &String, bool)> = Vec::new();
    for i in 0..200 {
        queue.push(("normal", &normal, false));
        if i == 50 { queue.push(("html-bomb", &html_bomb, false)); }
        if i == 100 { queue.push(("css-bomb", &css_bomb, false)); }
        if i == 150 { queue.push(("unknown-vector (guard bypassed)", &unknown, true)); }
    }

    let t_all = Instant::now();
    for (label, doc, bypass) in queue {
        let tg = Instant::now();
        let verdict = if bypass { Ok(()) } else { guard(doc, HTML_LIMIT, CSS_LIMIT) };
        guard_us.push(tg.elapsed().as_secs_f64() * 1e6);
        if let Err(reason) = verdict {
            rejected += 1;
            println!("  [reject] {label:<22} {reason}  (never parsed)");
            continue;
        }
        match worker.scan(doc) {
            Ok(_) => done += 1,
            Err(()) => {
                crashed += 1;
                let t = Instant::now();
                worker.kill();
                worker = Worker::spawn(&exe);
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                restart_ms.push(ms);
                println!("  [CRASH]  {label:<22} worker died -> restarted in {ms:.1} ms, \
                          doc flagged suspicious");
            }
        }
    }
    let total = t_all.elapsed().as_secs_f64() * 1000.0;
    worker.kill();

    guard_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("\n  scanned OK           : {done}");
    println!("  rejected by guard    : {rejected}  (0 parses, 0 crashes)");
    println!("  crashes survived     : {crashed}");
    if !restart_ms.is_empty() {
        println!("  restart cost         : {:.1} ms (once, only on crash)", restart_ms[0]);
    }
    println!("  guard cost / doc     : p50 {:.1} us, p99 {:.1} us",
             guard_us[guard_us.len() / 2], guard_us[guard_us.len() * 99 / 100]);
    println!("  wall clock           : {total:.0} ms for {} docs", done + rejected + crashed);
}
