# Where to report the two parsing issues

Attribution below is from **gdb backtraces and a Blitz-free reproduction**, not inference.
Nothing has been filed — these are drafts for you to send.

## Summary

| Issue | Root cause lives in | Existing upstream issue | What to do |
|---|---|---|---|
| O(n²) HTML parse | **`servo/html5ever`** tree builder | **[#289](https://github.com/servo/html5ever/issues/289)** — open since 2017, names our exact function | Comment on #289 with a fresh repro. Don't open a duplicate. |
| CSS stack overflow | **`servo/stylo`** (driver) + `servo/rust-cssparser` (helper) | **[cssparser #405](https://github.com/servo/rust-cssparser/issues/405)** — open, maintainer says **"expected"** | Don't file upstream. File at **`DioxusLabs/blitz`** for an embedder-level limit. |
| **Serializer stack overflow** (`outer_html()`) | **`DioxusLabs/blitz`** — its own code | none | **File at Blitz.** Clean bug, known fix (ammonia did it), good PR candidate. |

**Neither will be fixed upstream on a timeline you can depend on. Ship the guard
(`examples/depth_guard.rs`) regardless of what you file.**

---

## 1. O(n²) HTML parse → `servo/html5ever`

**Evidence it is not Blitz:** `upstream-repros/html5ever-quadratic/` depends only on
`html5ever` + `markup5ever_rcdom` — no Blitz anywhere — and reproduces cleanly:

```
-- nested <div>: depth grows --        -- control: 2x the bytes, depth 1 --
  5000 deep   24 KB    64 ms             5000 pairs   53 KB    1 ms
 10000 deep   48 KB   247 ms            10000 pairs  107 KB    3 ms
 20000 deep   97 KB  1130 ms            20000 pairs  214 KB    6 ms
 40000 deep  195 KB  4594 ms            40000 pairs  429 KB   13 ms
```

Depth quadruples the time per doubling; byte count does not matter. **Cause:** every `<div>`
start tag runs "close any open `<p>` in button scope", and scope lookups walk the open-element
stack until a scope boundary. `<div>` is not a boundary, so it is O(depth) per tag.

**Existing issue #289 ("Avoid taking more than O(n) time even for malicious input")** already
covers this — a 2017 comment even pins it to `close_p_element_in_button_scope`, the same
function. It has been open **9 years**. Maintainers noted the only simple fix (capping the
open-element list) violates the spec.

**Recommended action:** add a comment to #289 rather than opening a duplicate — a current-version
repro on an issue that stalled for lack of one is genuinely useful. Draft:

> Still reproducible on html5ever 0.39 (Sept 2026). Minimal case needs no formatting elements at
> all — plain `"<div>".repeat(n)` with `RcDom` is enough:
>
> | depth | input | time |
> |---|---|---|
> | 10k | 48 KB | 247 ms |
> | 20k | 97 KB | 1130 ms |
> | 40k | 195 KB | 4594 ms |
>
> Control with the same shape but depth 1 (`"<div></div>".repeat(n)`, ~2× the bytes) is linear:
> 1/3/6/13 ms. So it is nesting depth, not input size — consistent with the
> `close_p_element_in_button_scope` diagnosis in this thread. 390 KB of input costs ~30 s of CPU,
> which matters for server-side sanitizers and mail scanners. Repro: <attach
> `upstream-repros/html5ever-quadratic/`>.

## 2. CSS stack overflow → file at `DioxusLabs/blitz`, not upstream

**Backtrace (gdb, release).** Nested rules — the recursion cycle is stylo-driven:

```
style::stylesheets::rule_parser::NestedRuleParser::parse_nested
  -> <NestedRuleParser as QualifiedRuleParser>::parse_block      (stylo)
  -> cssparser::parser::parse_nested_block                        (cssparser: generic helper)
  -> cssparser::rules_and_declarations::parse_qualified_rule
  -> ... repeats until SIGABRT
```

Nested `calc()` is a **second, independent** path, and it is entirely stylo:

```
style::values::specified::calc::...::parse_argument / parse_product / parse_one
  -> cssparser::parser::parse_nested_block  ... repeats
```

So cssparser only supplies the generic block helper; **stylo owns the code that decides to
descend** in both paths. Notably stylo *does* have a `StackLimitChecker` — but it is used for
style invalidation traversal, not for parsing.

**Why not to file upstream:** cssparser
[#405](https://github.com/servo/rust-cssparser/issues/405) reports exactly this class
(~17,500 opening brackets → stack overflow). Maintainer `emilio` replied:

> "That seems expected since we don't implement any builtin nesting limit."

That is a *won't-fix by design*: Gecko and Servo impose their limits at the embedder layer, so
upstream expects the consumer to bound input. A new issue would be closed the same way.

**Therefore the correct target is `DioxusLabs/blitz`** — Blitz offers
`HtmlDocument::from_html(&str, _)` as a safe-looking API over untrusted input while providing
none of the bounding that Gecko/Servo add on top of stylo. There is currently **no Blitz issue
about untrusted-input robustness at all** (searched: "stack overflow", "nesting", "fuzz",
"malicious"). Draft:

> **Parsing untrusted HTML can abort the process (stack overflow) on ~40 KB of input**
>
> `HtmlDocument::from_html` aborts with `fatal runtime error: stack overflow` (SIGABRT, exit 134)
> on small adversarial CSS. It is not a panic, so `catch_unwind` does not help.
>
> ```rust
> let css = "div{".repeat(10_000); // 40 KB
> HtmlDocument::from_html(&format!("<style>{css}</style>"), DocumentConfig::default());
> ```
>
> Release build, default 8 MB main stack, blitz 0.3.0-beta.2. Also reachable via
> `calc(((…)))` (~40 KB) and `:is(`/`:not(` nesting (~50 KB) — two distinct recursion paths
> (nested-rule parsing and `calc` parsing), backtraces available.
>
> The recursion is in stylo/cssparser, and upstream considers an unbounded parser expected
> (servo/rust-cssparser#405) because embedders are meant to bound input. Since Blitz *is* the
> embedder here, would you accept a configurable nesting/size limit — e.g. a
> `DocumentConfig::max_css_nesting_depth` that fails parsing instead of aborting? Happy to send
> a PR. Related: HTML tree building is also O(n²) in nesting depth (servo/html5ever#289), so a
> depth cap would address both.

## 3. Recursive serializer → `DioxusLabs/blitz` (clearly their bug)

Found while researching prior art: `Node::outer_html()` recurses per DOM level and aborts on
deeply nested input — **the exact bug ammonia was assigned
[RUSTSEC-2019-0001](https://rustsec.org/advisories/RUSTSEC-2019-0001.html) for** (CVSS 7.5).
Unlike the two CSS overflows, this is Blitz's own code, so it is unambiguously theirs, and the
fix is already demonstrated upstream in another project: serialize iteratively.

gdb, release, blitz-dom 0.3.0-beta.2 — pure self-recursion:

```
#0  blitz_dom::node::node::Node::write_outer_html_in_style ()
#1  blitz_dom::node::node::Node::write_outer_html_in_style ()
#2  ... all the way down
```

Survives 30k nesting, aborts between 30k and 35k — while *parsing* the same document succeeds.
Repro: `cargo run --release --example serialize_overflow 35000`. Draft:

> **`outer_html()` aborts the process on deeply nested DOMs (recursive serializer)**
>
> `Node::write_outer_html_in_style` recurses once per DOM level, so serializing a deep tree
> overflows the stack and aborts (SIGABRT). Not a panic, so `catch_unwind` does not help.
>
> ```rust
> let doc = HtmlDocument::from_html(&"<div>".repeat(35_000), DocumentConfig::default()).into_inner();
> doc.root_element().outer_html(); // fatal runtime error: stack overflow
> ```
>
> Release, default 8 MB stack, blitz-dom 0.3.0-beta.2. Threshold is between 30k and 35k; parsing
> the same input succeeds, so this is serialization specifically. Backtrace is pure self-recursion
> in `write_outer_html_in_style`.
>
> This is the same bug class as RUSTSEC-2019-0001 in `ammonia` (CVSS 7.5), which was fixed by
> serializing iteratively rather than recursively — an explicit work stack would fix it here too.
> Relevant to anyone calling `outer_html()` on untrusted HTML. Happy to send a PR.

**Also worth raising separately at Blitz:** `BaseDocument::resolve_stylist` calls `.unwrap()` on
`first_element_child()` while `resolve()` guards the same case — a small, easy fix.

---

## Do not block on upstream

#289 is 9 years old; #405 is marked expected. Both guards in `examples/depth_guard.rs`
(HTML depth 256, CSS bracket depth 64) reject these inputs in <10 ms, ~3000× cheaper than the
parse they prevent. Keep subprocess isolation too: the guard bounds only the vectors we know
about, and a stack overflow anywhere is uncatchable.
