> ⚠️ **Timing caveat:** absolute millisecond figures in this file were measured on a CPU
> pinned at 400 MHz and are inflated ~12.5×. Ratios and percentages are unaffected.
> **`PERFORMANCE.md` has the corrected full-clock numbers** — use those.

# Blitz for hidden-element detection — evaluation

**Evaluated:** `blitz-dom` / `blitz-html` / `blitz-traits` `0.3.0-beta.2` (crates.io), rustc 1.96.0, Linux.
Behavioral claims are verified by the probes in `src/bin/`; source-level claims are checked
against the **registry** source for `0.3.0-beta.2`, not the git clone (see the version note).

> **Version note:** the git `main` branch is *ahead* of the published `0.3.0-beta.2` tag despite
> declaring the same version. `main` has a `resolved_style_value(node_id, "display")`
> (a `getComputedStyle` equivalent) that **does not exist in the published crate**. Read the
> registry source (`~/.cargo/registry/src/*/blitz-dom-0.3.0-beta.2/`), not the clone.

## Verdict

Blitz is a good fit. It gives us a real Stylo cascade — inheritance, `var()`, `@media`,
specificity, `!important`, UA stylesheet — **and it exposes a styles-only entry point that
skips layout entirely**, which is exactly the constraint we wanted.

On resilience it is strong where it counts: across 15 malformed-CSS/HTML cases it computed the
**same `display` a spec-compliant browser would, 15/15** — including the adversarial cases where
the correct answer is that a literal `display:none` in the source must *not* take effect. That
is the specific thing a regex-based detector gets wrong.

Two things need work before production: a **stack-overflow DoS reachable with ~40 KB of CSS**
(the bug is in `stylo`/`cssparser`, not Blitz), and a few **missing hiding vectors**
(`content-visibility`, `<font color>`, named-color legacy attrs).

## 1. The API we need

`BaseDocument::resolve_stylist(now)` is public and self-contained: it flushes the stylist and
runs Stylo's style traversal. It does **not** touch Taffy layout or painting. That is our
entry point.

```rust
let mut doc = HtmlDocument::from_html(html, DocumentConfig {
    viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
    ..Default::default()          // every field is Option — no net/shell/nav providers needed
}).into_inner();
doc.resolve_stylist(0.0);          // styles only, no layout

// per node:
let styles = doc.get_node(id).unwrap().primary_styles();   // Option<impl Deref<Target=ServoArc<ComputedValues>>>
styles.get_box().clone_display();                          // typed, no string parsing
```

Read **typed** values off `ComputedValues` rather than the string API: it is the only option on
the published crate, it avoids parsing, and it sidesteps the fact that `resolved_style_value`'s
`width`/`height` are *used* values that would require layout.

- `default-features = false` on both crates works and drops the dep tree **238 → 195** crates
  (sheds `accessibility`, `svg`, `system_fonts`, `woff`). Do it.
- `DocumentConfig::style_threading` defaults to `Sequential`. **The doc comment claims the
  default is `Parallel` — the code says `Sequential`.** Sequential is the safe one; two
  documents resolving concurrently under `Parallel` can panic (upstream issue #430). Verified:
  two threads × 20 documents under the default is clean.
- ⚠️ `resolve_stylist` calls `.unwrap()` on `first_element_child()`. `resolve()` guards this
  case and returns early; `resolve_stylist` does not. Empty input happens to be safe (html5ever
  always synthesises `<html><head><body>`), but don't feed it a hand-built empty document.

## 2. What resolves correctly (verified)

`Display(0)` = `none`, `514` = `block`, `258` = `inline`.

| Case | Result |
|---|---|
| `--x:none` on parent; `display:var(--x)` on child | `Display(0)` ✅ |
| `display:var(--undefined, none)` fallback | `Display(0)` ✅ |
| `var()` reference cycle `--a:var(--b);--b:var(--a)` | no panic; invalid-at-computed-value-time → `inline` (spec-correct) ✅ |
| `visibility:hidden` parent, `visibility:visible` child | child computes `Visible` — inheritance is real, per-node value is truth ✅ |
| `--size:0px` on grandparent → `font-size:var(--size)` | `0.0px` ✅ |
| inline style vs class rule, `!important`, `#id` specificity | correct cascade ✅ |
| `<style>` in `<body>` | applied ✅ |
| `@media (max-width:600px)` at 800px vs 400px viewport | `block` vs `none` — viewport-sensitive ✅ |
| `@media (prefers-color-scheme: dark)` under `ColorScheme::Light` vs `Dark` | `block` vs `none` — **dark-mode-only hiding is detectable** by scanning both ✅ |
| `left:-9999px`, `opacity:0`, `font-size:0`, `text-indent:-9999px` | all readable ✅ |
| `color:#fff` + `background-color:#fff` | both readable ✅ |
| `transform:scale(0)`, `filter:opacity(0)`, `width:0`, `max-height:0`, `color:transparent`, `letter-spacing:-1em` | all readable ✅ |
| `clip: rect(0,0,0,0)` | `Rect(top/right/bottom/left = 0px)` via `get_effects().clone_clip()` ✅ |
| `clip-path: inset(100%)` | `Shape(Rect(100% inset))` via `get_svg().clone_clip_path()` ✅ |
| `hidden` attribute, `bgcolor="#ffffff"` | applied as presentational hints ✅ |
| `head`/`script`/`style`/`template` | `display:none` from UA sheet ✅ |

### The one that will bite: `display:none` subtrees get no styles

```
<div style="display:none">   -> Display(0)
  <span style="color:red">   -> NO STYLES   (primary_styles() == None)
```

Stylo skips the subtree. `<template>` content and `<title>` are likewise unstyled. **Our walker
must carry an "inside a hidden ancestor" flag itself** and must not treat `None` as "visible" or
as "no such node" — text under a `None` element is still present in the DOM and is still
spam-relevant. This is correct browser behaviour, not a bug, but it is a trap.

## 3. Gaps — hiding vectors Blitz will not see

| Vector | Status | Impact |
|---|---|---|
| `content-visibility: hidden` | **Unavailable** — Stylo defines it `engine = "gecko"`, so it is not built in Blitz's Servo-mode config; `clone_content_visibility()` does not exist | a modern hiding vector, invisible to us |
| `<font color="#ffffff">` | **Not implemented.** Preshints cover `align`/`width`/`height`/`bgcolor`/`hidden` only — there is no `color` attribute handling | white-on-white via `<font>` reads as black text |
| `bgcolor="white"` (named) | **Not applied.** `parse_color_attr` accepts only `#rgb`/`#rrggbb`; browsers accept named colors and bare hex via the legacy color rules | named-color background hiding missed |
| `aria-hidden="true"` | not a CSS property | must handle in our own logic |

The `<font color>` and named-`bgcolor` gaps are the ones that matter for spam — both are
staples of hidden-text email. Either patch `synthesize_presentational_hints_for_legacy_attributes`
upstream, or pre-process these attributes into inline styles before parsing.

**Out of scope by design** (genuinely need layout, per the no-positional-rendering constraint):
element covered by another element; text clipped by a *nonzero-size* `overflow:hidden` ancestor.
Also note `background-color` does **not** inherit — same-color-as-background detection needs our
own ancestor walk to the nearest non-transparent background, and `opacity` composes
multiplicatively down the tree. Both are approximations without layout; flag them as such.

**Untested, and likely not covered by the styles-only path:** `::before`/`::after` `content:`
text. Blitz appears to materialize pseudo-elements during *box construction*
(`collect_layout_children` / `CONSTRUCT_BOX`), which runs under `resolve()`, not
`resolve_stylist()`. This is the inverse problem — text the recipient sees that is absent from
DOM text — so it is outside the stated scope, but do not assume the styles-only path surfaces it.

### `<noscript>` is visible — and that is what we want

`blitz-html` sets `scripting_enabled: false`, so `<noscript>` parses as **real elements**, and
the UA sheet only hides `noscript` under `@media (scripting)`. Verified: `<noscript><p>` content
computes as visible `block`. This matches an email client (scripting off) and is the correct
model for mail. If we ever scan in a *browser* context, this flips and must be revisited.

## 4. Resilience to malformed HTML

Full matrix in `fuzz-results.txt` (`./target/debug/fuzz`, each case in its own subprocess).

**31 of 33 malformed cases parse cleanly; 27 of them in ~20 ms.** html5ever is spec-compliant, so
unclosed `<style>`/`<script>`/`<!--`/`<title>`/`<textarea>`, `<plaintext>`, misnested formatting
tags, table foster-parenting, stray `</body></html>` followed by content, duplicate
`<body>`/`<head>`, SVG/MathML foreign content, NUL bytes, bogus entities, unquoted/malformed
attributes, CDATA, bogus doctypes, processing instructions, and malformed CSS (`display:none;;;garbage`,
unbalanced `{`, `!important junk`, self-referential vars) are all handled without panic or
misparse. **This is the important property: it reproduces the tree the recipient's browser
builds, rather than "fixing" the HTML** — which is what makes the detection correct. The
reference target here is a *spec-compliant browser*. Real mail clients pre-sanitize before their
own parser runs (Gmail strips/rewrites `<style>`, `position`, and more), so for mail we are
modelling the pre-sanitization input, not Gmail's final render.

### Does malformed markup *confuse detection*? No — 15/15 match spec

Crash-resistance is only half the question; the half that matters is whether a spam-authored
malformation makes us compute a **different** `display` than the recipient's browser would.
Verified with `./target/debug/malformed` (full output in `malformed-results.txt`) — every case
agrees with the CSS/HTML spec, including the ones where the correct answer is "the hiding
does *not* apply":

| Malformed input | Blitz computes | Spec | |
|---|---|---|---|
| `display:none;;;garbage:;color` (inline) | `none` | `none` — junk decls dropped, valid one survives | ✅ |
| `.a{display:none` (unclosed block at EOF) | `none` | `none` — block implicitly closed | ✅ |
| `display:none !important junk` | `block` | `block` — tokens after `!important` invalidate the decl | ✅ |
| `@media{{{ .a{display:none}` | `block` | `block` — `.a` nested in an invalid qualified rule, dropped | ✅ |
| `@media{ .a{display:none} }` (empty query) | `none` | `none` — empty media list matches all | ✅ |
| `@totallybogus x; .a{display:none}` | `none` | `none` — unknown at-rule consumed, next rule survives | ✅ |
| `.a{display:none} /* unterminated` | `none` | `none` | ✅ |
| `.!!bad{color:red} .a{display:none}` | `none` | `none` — invalid selector drops only its own rule | ✅ |
| `.a{;;display:none;;}` | `none` | `none` — stray semicolons ignored | ✅ |

The two "should stay visible" cases (`!important junk`, `@media{{{`) are the important ones: a
naive regex/substring detector would see `display:none` and wrongly conclude the text is hidden.
Blitz gets both right, which is precisely the value it adds over string matching.

Content swallowed by unclosed raw-text elements is also classified correctly — it stays reachable
in the DOM *and* lands under the right visibility:

| Input | Text reachable? | Under `display:none`? | Correct |
|---|---|---|---|
| `<style>.a{...}<p>SWALLOWED` | yes | **yes** (inside `<style>`) | ✅ |
| `<script>var x=1;<p>SWALLOWED` | yes | **yes** (inside `<script>`) | ✅ |
| `<title>T<body><p>SWALLOWED` | yes | **yes** (inside `<head>`) | ✅ |
| `<textarea><p>SWALLOWED` | yes | no — `<textarea>` renders | ✅ |
| `<!-- <p>SWALLOWED` | **absent** (comment) | n/a | ✅ |
| `<plaintext><p>SWALLOWED` | yes | no — renders as raw text | ✅ |

This is the concrete payoff of the ancestor-flag requirement in §2: text hidden by being
*swallowed into `<style>`* is only detectable if the walker propagates the hidden-ancestor flag,
since those text nodes have no styles of their own.

**Remote sub-resources with `net_provider: None` are safe.** `<link rel=stylesheet>`, a remote
`<img>`, and `@import url(...)` inside `<style>` all parse in ≤1 ms with no panic and no
blocking. Notably `@import` does not poison its own stylesheet — a sibling `.a{display:none}`
rule in the same `<style>` still applied. Because `resolve_stylist` bypasses the
`has_pending_critical_resources()` gate that `resolve()` checks, styles compute *without* the
remote sheet — which is the behaviour we want, and matches mail clients that block remote CSS.

Large-but-flat inputs are fine: a 50 KB attribute, 50 k unclosed `<p>`, and 200 k siblings all
parse in ≤ 0.5 s.

### 🔴 Finding 1 — stack overflow on nested CSS (~40 KB payload, uncatchable)

Nested CSS constructs recurse in the parser and **abort the process** with
`fatal runtime error: stack overflow` (SIGABRT/134). Release build, default 8 MB main stack:

| Payload | Largest OK | Crashes at | Bytes to crash |
|---|---|---|---|
| `div{` × n (CSS nesting) | 5,000 | 10,000 | **~40 KB** |
| `calc(((…1…)))` × n | 10,000 | 20,000 | **~40 KB** |
| `:is(` × n | 5,000 | 10,000 | ~50 KB |
| `:not(` × n | 5,000 | 10,000 | ~60 KB |
| `@media screen{` × n | 5,000 | 10,000 | ~140 KB |
| `@supports (color:red){` × n | 5,000 | 10,000 | ~220 KB |

**The recursion is in `stylo` / `cssparser`, not in Blitz** (`div{` hits Stylo's nested-rule
parser; `calc(((` hits `cssparser`'s `parse_nested_block`). File upstream accordingly — a Blitz
version bump alone will not fix it.

Non-recursive CSS of the same size is fine (`div ` × 20,000 descendant combinators, 80 KB, parses
OK) — so it is recursion depth, not input size.

`RUST_MIN_STACK` does **not** help (it doesn't apply to the main thread). Parsing on a spawned
thread with a bigger stack raises the ceiling but is **not a bound** — verified 64 MB survives
n=10 k but dies at n=50 k; 512 MB survives n=200 k. And a stack overflow on *any* thread aborts
the whole process; Rust cannot catch it.

**Mitigation must be layered:** (1) cap CSS input size and reject/strip pathological nesting
depth before parsing, **and** (2) parse untrusted documents in a **subprocess** so an abort kills
a worker, not the scanner. A large-stack thread alone is not sufficient.

### 🔴 Finding 1b — `outer_html()` recurses and aborts (~32 k nesting)

`Node::write_outer_html_in_style` recurses once per DOM level. `outer_html()` survives 30 k
nesting and aborts between 30 k and 35 k, while *parsing* the same document succeeds — verified
by gdb (pure self-recursion). This is the same bug class as **RUSTSEC-2019-0001** in `ammonia`
(CVSS 7.5), fixed there by serializing iteratively. Unlike the CSS overflows, this one is in
**Blitz's own code**. **Do not call `outer_html()` on untrusted input.** See `PRIOR-ART.md`.

### 🟠 Finding 2 — HTML parsing is quadratic in nesting depth

Release, nested `<div>`:

| Depth | Parse | Style (`resolve_stylist`) |
|---|---|---|
| 10 k | 307 ms | 11 ms |
| 20 k | 1.3 s | 25 ms |
| 40 k | 5.6 s | 39 ms |
| 80 k | 27.5 s | 74 ms |

Parse is ~4.5× per doubling — **O(n²)**. Style resolution is linear and negligible (74 ms at
80 k), which further confirms the styles-only path is cheap. (The `deep-div-100k` TIMEOUT in
`fuzz-results.txt` is a *debug* build; extrapolating the release curve, 100 k would take ~45 s.)
A ~400 KB nesting bomb costs ~30 s of release CPU per message: a cheap DoS.
Cap nesting depth (a few thousand is far beyond anything legitimate) and impose a wall-clock
timeout per document.

Note the quadratic style cost seen in debug builds disappears in release — **benchmark in
release only**.

## 5. Recommendations

1. Use `resolve_stylist` + `primary_styles()` typed accessors. Do not call `resolve()`; we never
   need Taffy.
2. `default-features = false` on `blitz-dom` and `blitz-html`.
3. Keep `StyleThreading::Sequential` (the actual default) if scanning concurrently.
4. **Sandbox parsing in a subprocess with a wall-clock timeout and an input-size cap.** This is
   required, not optional — Finding 1 is an uncatchable abort.
5. Enforce limits before parsing: total bytes, CSS bytes, and nesting depth (HTML and CSS).
6. Walk the tree **iteratively** and track hidden ancestors yourself (`display:none` subtrees are
   unstyled). Our probes' walkers do this.
7. Close the gaps: pre-process `<font color>` / named-color `bgcolor` into inline styles, and
   detect `content-visibility` by scraping the declaration text, since Stylo does not build it.
8. Pin `=0.3.0-beta.2` — it is a beta, and `main` has already diverged under the same version
   number.

## 6. Minimal repros and the nesting-depth guard

Three self-contained examples (`examples/`), each runnable with `cargo run --release --example <name>`:

| Example | What it shows |
|---|---|
| `stack_overflow` | 8 lines: 40 KB of `div{` aborts the process (exit 134) |
| `quadratic_parse` | 10 lines: nested `<div>` parse time 4× per doubling |
| `depth_guard` | rejects both bombs in O(n), before parsing |

### Can we cap nesting depth and just flag as spam? Yes — and you should.

Neither html5ever 0.39 nor blitz-html exposes any depth or size limit, so the cap has to be
ours. The right place is **before** the tree builder, because the tree builder *is* the
quadratic part. `examples/depth_guard.rs` runs html5ever's **tokenizer only** (`pub mod
tokenizer`, no tree builder): tokenizing is linear and non-recursive, so the guard is cheap on
exactly the inputs that are expensive to parse.

Measured:

| Input | Guard verdict | Guard time | Parse time if allowed |
|---|---|---|---|
| normal page | depth 5, OK | 0 ms | — |
| 80 k nested `<div>` | **REJECTED** | 9 ms | ~30 s |
| 40 KB nested-CSS bomb | **REJECTED** (CSS guard) | 0 ms | process abort |
| nested `calc()` bomb | **REJECTED** (CSS guard) | 0 ms | process abort |
| 50 k `<br>` (flat) | depth 0, OK | 5 ms | fine |
| 50 k balanced `<div></div>` | depth 1, OK | 9 ms | fine |
| 5 k-deep JS braces | OK (css_depth 1) | 0 ms | fine |

Roughly **3,000× cheaper than the parse it prevents**, with no false positives on flat,
balanced, void-heavy, or script-heavy documents.

Three things that are easy to get wrong, all handled in the example:

1. **The HTML depth guard does not catch the CSS bomb.** `<style>` content is raw text to the
   tokenizer, so HTML depth stays 1 while the CSS parser still recurses per `{`/`(`. You need a
   **separate CSS bracket-depth check**, or the crash goes straight through. This is the single
   most important point here.
2. **Scope the CSS check to actual CSS.** A whole-document byte scan for `{` false-positives on
   minified JS. The example only counts brackets inside `<style>` text and `style=""`
   attributes — verified: 5 k-deep JS braces score `css_depth = 1`.
3. **The guard must short-circuit, or it is quadratic itself.** Matching an end tag scans the
   open-element stack, so `<div>`×50k + `</span>`×50k took **817 ms** before adding an early
   bail-out and **9 ms** after. Once the limit is blown, stop all work; that also bounds the
   stack, which is what restores linearity. (Depth then reports `limit + 1`, i.e. "at least",
   not an exact count — fine for a reject decision.)

Suggested limits: **HTML depth 256** and **CSS bracket depth 64**. Real pages rarely exceed ~50
HTML depth; both bombs need thousands. Deep nesting has no legitimate use in mail, so treating a
breach as a spam signal (rather than only a parse guard) is reasonable — but note it is
*evasion-shaped*, not spam-shaped: attackers use it to break scanners, so "reject and flag for
review" is safer than "silently drop".

**The guard is a filter, not a fix.** It is a cheap first line that removes the pathological
inputs; subprocess isolation (§4) is still required, since it bounds only the two vectors we
know about.

### Where the bugs actually live

The quadratic parse is **spec-mandated behaviour in html5ever's tree builder, not a Blitz bug**:
each `<div>` start tag runs the "close any open `<p>` in button scope" check, and scope lookups
walk the open-element stack until they hit a scope boundary. `<div>` is not a boundary, so the
walk is O(depth) per tag. This is also why `<p>`×50k was fast (each `<p>` closes the previous,
so the stack stays shallow). Likewise the stack overflow is in `stylo`/`cssparser`. **Upgrading
Blitz will not fix either** — file upstream, and guard locally meanwhile.

## Reproducing

```
cargo run --bin blitz-probe    # CSS resolution: var(), inheritance, @media, cascade
cargo run --bin fuzz           # 33 malformed-HTML cases, each subprocess-isolated
cargo run --bin gaps           # which hiding declarations survive into computed style
cargo run --bin malformed      # malformed CSS/HTML: computed display vs spec
cargo run --bin remote         # remote sub-resources; prefers-color-scheme hiding
cargo run --release --bin narrow  css|dom <n>     # scaling / overflow thresholds
cargo run --release --bin css2   <kind> <n>       # CSS recursion shapes
cargo run --release --bin mitigate <n> <stack_mb> # large-stack mitigation

cargo run --release --example stack_overflow   # minimal: 40 KB CSS -> abort (exit 134)
cargo run --release --example quadratic_parse  # minimal: O(n^2) parse
cargo run --release --example depth_guard      # O(n) pre-parse guard for both
cargo run --release --example serialize_overflow 35000  # recursive outer_html() abort
```
