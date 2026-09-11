//! Reject deeply-nested HTML *before* parsing it, in O(n).
//!   cargo run --release --example depth_guard
//!
//! Runs html5ever's TOKENIZER only -- no tree builder -- so it never touches the
//! quadratic code path (see quadratic_parse.rs) and never recurses (see
//! stack_overflow.rs). Tokenizing is linear, so the guard is cheap on any input.

use html5ever::tokenizer::{
    BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use html5ever::{local_name, LocalName};
use std::cell::RefCell;

/// Elements that never have children, so they must not increase depth.
fn is_void(n: &LocalName) -> bool {
    matches!(*n, local_name!("area") | local_name!("base") | local_name!("br")
        | local_name!("col") | local_name!("embed") | local_name!("hr") | local_name!("img")
        | local_name!("input") | local_name!("link") | local_name!("meta")
        | local_name!("param") | local_name!("source") | local_name!("track")
        | local_name!("wbr"))
}

#[derive(Default)]
struct DepthSink {
    /// Once the limit is blown we stop doing work entirely. This also bounds the
    /// open-element stack, which keeps the `rposition` scan below O(n^2) on
    /// adversarial input like `<div>`*50k followed by `</span>`*50k.
    limit: usize,
    exceeded: RefCell<bool>,
    stack: RefCell<Vec<LocalName>>,
    max_depth: RefCell<usize>,
    tags: RefCell<usize>,
    in_style: RefCell<bool>,
    /// Max `{` / `(` nesting seen in CSS only (<style> text and style="" attrs).
    css_depth: RefCell<usize>,
}

impl TokenSink for DepthSink {
    type Handle = ();
    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        if *self.exceeded.borrow() {
            return TokenSinkResult::Continue; // already decided; do no further work
        }
        // CSS lives in <style> text and in style="" attributes -- scan only those,
        // so minified JS braces in <script> cannot trigger a false positive.
        if let Token::CharacterTokens(ref t) = token {
            if *self.in_style.borrow() {
                let d = max_bracket_nesting(t);
                let mut c = self.css_depth.borrow_mut();
                *c = (*c).max(d);
            }
        }
        if let Token::TagToken(Tag { kind, name, self_closing, ref attrs, .. }) = token {
            *self.tags.borrow_mut() += 1;
            if kind == TagKind::StartTag {
                *self.in_style.borrow_mut() = name == local_name!("style");
                for a in attrs {
                    if a.name.local == local_name!("style") {
                        let d = max_bracket_nesting(&a.value);
                        let mut c = self.css_depth.borrow_mut();
                        *c = (*c).max(d);
                    }
                }
            } else {
                *self.in_style.borrow_mut() = false;
            }
            let mut stack = self.stack.borrow_mut();
            match kind {
                TagKind::StartTag if !self_closing && !is_void(&name) => {
                    stack.push(name);
                    let mut m = self.max_depth.borrow_mut();
                    *m = (*m).max(stack.len());
                    if *m > self.limit {
                        *self.exceeded.borrow_mut() = true;
                    }
                }
                TagKind::EndTag => {
                    // Pop to the nearest matching open tag (mirrors the spec's
                    // "generate implied end tags" closely enough for a guard).
                    if let Some(i) = stack.iter().rposition(|o| *o == name) {
                        stack.truncate(i);
                    }
                }
                _ => {}
            }
        }
        TokenSinkResult::Continue
    }
}

/// Max nesting of `{` / `(` in a CSS string. The HTML depth guard does NOT catch
/// the CSS stack overflow (see stack_overflow.rs): `<style>` content is raw text to
/// the tokenizer, so HTML depth stays 0 while the CSS parser still recurses per
/// `{` / `(`. This is a conservative O(n) proxy for that recursion depth.
fn max_bracket_nesting(html: &str) -> usize {
    let (mut brace, mut paren, mut max) = (0usize, 0usize, 0usize);
    for b in html.bytes() {
        match b {
            b'{' => brace += 1,
            b'}' => brace = brace.saturating_sub(1),
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            _ => continue,
        }
        max = max.max(brace.max(paren));
    }
    max
}

/// Returns `Err(depth)` if nesting exceeds `limit`.
fn check_depth(html: &str, limit: usize) -> Result<usize, usize> {
    let sink = DepthSink { limit, ..Default::default() };
    let tok = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(html.into());
    let _ = tok.feed(&input);
    tok.end();
    let d = *tok.sink.max_depth.borrow();
    if d > limit { Err(d) } else { Ok(d) }
}

/// Tokenize once and report the CSS bracket depth (style elements + style attrs).
fn css_nesting_of(html: &str) -> usize {
    let sink = DepthSink { limit: usize::MAX, ..Default::default() };
    let tok = Tokenizer::new(sink, TokenizerOpts::default());
    let input = BufferQueue::default();
    input.push_back(html.into());
    let _ = tok.feed(&input);
    tok.end();
    let d = *tok.sink.css_depth.borrow();
    d
}

fn main() {
    const LIMIT: usize = 256; // real pages rarely exceed ~50

    let bomb = "<div>".repeat(80_000);
    let normal = "<html><body><div><p>hello <b>world</b></p></div></body></html>".to_string();
    let void_heavy = "<br>".repeat(50_000); // must NOT be flagged: depth stays 0
    let balanced = "<div></div>".repeat(50_000); // must NOT be flagged: depth stays 1

    for (name, html) in [
        ("normal page", &normal),
        ("80k nested <div> (bomb)", &bomb),
        ("50k <br> (flat)", &void_heavy),
        ("50k balanced <div>", &balanced),
        // adversarial for the guard itself: every </span> scans the whole stack
        ("50k <div> + 50k </span>", &format!("{}{}", "<div>".repeat(50_000), "</span>".repeat(50_000))),
    ] {
        let t = std::time::Instant::now();
        let verdict = check_depth(html, LIMIT);
        let ms = t.elapsed().as_millis();
        match verdict {
            Ok(d) => println!("{name:<26} depth={d:<6} OK        guard took {ms} ms"),
            Err(d) => println!("{name:<26} depth={d:<6} REJECTED  guard took {ms} ms"),
        }
    }

    // The HTML guard alone does NOT stop the stack overflow -- CSS needs its own check.
    const CSS_LIMIT: usize = 64;
    let css_bomb = format!("<style>{}</style>", "div{".repeat(10_000));
    let calc_bomb = format!("<style>a{{width:calc{}1{}}}</style>", "(".repeat(20_000), ")".repeat(20_000));
    println!();
    // Must NOT false-positive: deeply-braced JS lives in <script>, which is not CSS.
    let js = format!("<script>{}x=1{}</script><style>a{{color:red}}</style>",
                     "function f(){".repeat(5_000), "}".repeat(5_000));
    for (name, html) in [("40 KB nested-CSS bomb", &css_bomb), ("nested calc() bomb", &calc_bomb),
                         ("5k-deep JS braces (legit)", &js)] {
        let t = std::time::Instant::now();
        let html_depth = check_depth(html, LIMIT).unwrap_or_else(|d| d);
        let css_depth = css_nesting_of(html);
        println!("{name:<26} html_depth={html_depth} (guard says OK!) css_depth={css_depth} -> {} in {} ms",
            if css_depth > CSS_LIMIT { "REJECTED by CSS guard" } else { "allowed" }, t.elapsed().as_millis());
    }

    println!("\nFor comparison, actually parsing the 80k bomb takes ~30 s (quadratic_parse.rs),");
    println!("and parsing either CSS bomb aborts the process outright (stack_overflow.rs).");
}
