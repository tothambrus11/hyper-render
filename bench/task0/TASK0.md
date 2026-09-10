# Task 0 — Is Servo's attribute table portable to Blitz, and how big is the port?

Status: **complete**. Answer: the framing in the plan is wrong in a way that makes the
work *smaller*, not larger. Recommendation: **do not switch engines to fix hints, and do
not write a pre-pass.** Complete Blitz's existing trait impl.

Evidence: `probe-output.txt` (runnable via `cargo run --release --example task0_probe`),
`versions.txt`. Servo read at commit `92d90f0e` (2026-09-10).

---

## 1. There is no "Servo attribute table" to port

The plan assumes Servo holds a portable attribute→CSS table. It does not. Servo's
`Element::synthesize_presentational_hints_for_legacy_attributes`
(`components/script/dom/element/element.rs:1246-1576`, ~330 lines) is a chain of
**downcasts to ~15 concrete DOM types**:

```rust
let bgcolor = if let Some(this) = self.downcast::<HTMLBodyElement>() {
    this.get_background_color()
} else if let Some(this) = self.downcast::<HTMLTableElement>() {
    this.get_background_color()
} // ... HTMLTableCellElement, HTMLTableRowElement, HTMLTableSectionElement
```

Those getters read a **pre-parsed, typed** `AttrValue` that was produced eagerly at
attribute-set time by a per-element-type `parse_plain_attribute`. 28 files implement it
(`htmlbodyelement.rs`, `htmlfontelement.rs`, `htmlhrelement.rs`, …):

```rust
// htmlbodyelement.rs:136
fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
    match *name {
        local_name!("bgcolor") | local_name!("text") => AttrValue::from_legacy_color(value.into()),
        ...
```

Blitz has none of this infrastructure: no typed `AttrValue` storage, no per-element-type
class hierarchy to downcast through. Its `ElementData` is one struct holding a tag name
and raw string attributes. **Porting Servo's structure would mean importing Servo's whole
DOM class hierarchy — that is not a viable port, and it is not necessary.**

## 2. The part that actually matters is already in Blitz's build

The hard, spec-fiddly, correctness-critical piece is *parsing legacy attribute values*.
That code is not Servo-side at all — it lives in **stylo**, which Blitz already depends on,
with `default = ["servo"]`, so `style::attr` is **already compiled into every Blitz build**:

| Parser (public in stylo 0.8) | Handles |
|---|---|
| `style::attr::parse_legacy_color` | named colors, bare hex, the full HTML legacy mess |
| `style::attr::parse_length` / `parse_nonzero_length` | HTML dimensions (`px`, `%`, bare) |
| `style::values::specified::FontSize::from_html_size` | `<font size=1..7>` |
| `parse_integer` / `parse_unsigned_integer` | numeric attrs |

Verified by running it inside this crate (Part 1 of the probe):

```
"white"        -> Ok(rgb 255 255 255)      "FFF"          -> Ok(rgb  15  15  15)
"ffffff"       -> Ok(rgb 255 255 255)      "bogus"        -> Ok(rgb 176   0   0)
"#ffffff"      -> Ok(rgb 255 255 255)      "chucknorris"  -> Ok(rgb 192   0   0)
```

That is the real spec parser, quirks included (`chucknorris` → `#C00000`). **We do not need
to write, port, or maintain a colour parser. We need to call the one we already ship.**

## 3. What Blitz actually implements (read from source, not inferred)

`blitz-dom-0.2.4/src/stylo.rs:798-932` — the entire impl is ~135 lines and recognises
**five attribute names**: `align`, `width`, `height`, `bgcolor`, `hidden`.

Its colour parser is a private 20-line function that **bails unless the value starts with `#`**:

```rust
fn parse_color_attr(value: &str) -> Option<(u8, u8, u8, f32)> {
    if !value.starts_with('#') { return None; }   // <-- the entire bug
```

Confirmed end-to-end (Part 2 of the probe — computed style after a real parse+cascade):

| case | background-color | color |
|---|---|---|
| `<table bgcolor="#ffffff">` | `rgb(255,255,255)` | — |
| `<table bgcolor="ffffff">` | **transparent** | — |
| `<table bgcolor="white">` | **transparent** | — |
| `<body bgcolor="white">` | **transparent** | — |
| `<font color="white">` | — | **rgb(0,0,0)** (not applied) |
| `<body text="white">` | — | **rgb(0,0,0)** (not applied) |

This is exactly the spam-relevant surface: white-on-white via named or bare-hex colours
is invisible to us today.

### A caveat on the "12 of 28" figure
I could not verify it — `/home/ambrus/blitz-test/examples/preshints.rs` is not in this
container. From source, Blitz recognises 5 attribute *names*; a count of 12 is only
reachable by counting (attribute × element) pairs. Note that two of those pairs pass for
the **wrong reason**: Blitz applies `align` and `bgcolor` to *any* tag with no gating, so
`<img align=right>` gets `text-align:right` where the spec says `float:right`. A
conformance suite that only asks "did any hint apply?" will score those as passes. **The
28-case suite must assert the specific property and value, not just that something changed.**

## 4. The performance tension — the plan's premise needs correcting

The plan says completing the trait impl "keeps hints on stylo's slow path (they defeat the
style-sharing cache)". Reading stylo, that is not quite the mechanism, and the difference
is actionable.

`sharing/checks.rs:72` — sharing is rejected only when hints **differ**:
```rust
target.pres_hints() == candidate.pres_hints()
```
But that equality bottoms out in `rule_tree/source.rs:22`:
```rust
impl PartialEq for StyleSource {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }   // pointer equality
}
```

**Pointer equality.** And Blitz allocates a *fresh* `Arc` per element inside the hint
synthesis. So two byte-identical `<td bgcolor="#ffffff">` elements produce two distinct
Arcs, compare unequal, and **can never share style** — not because their hints differ, but
because of how they are allocated. That is the mechanism behind the measured 4.2× style
regression, and it is a Blitz implementation detail, not an inherent cost of pres hints.

Second, cheaper finding: Blitz calls `push_style(...)` **once per property**, each with its
own `Arc` and its own `ApplicableDeclarationBlock` (and so its own rule-tree node). Servo
accumulates all of an element's declarations into **one** block and pushes it once
(`element.rs:1566`). An element with 4 legacy attributes costs Blitz 4 allocations and 4
rule-tree nodes where Servo pays 1.

Both are fixable inside Blitz — the standard fix is Gecko's: cache the mapped-attribute
declaration block keyed on the attribute set, so identical attributes yield the *same* Arc
and sharing succeeds.

### This changes pre-registered prediction #3
The plan predicts Gecko shows the same ~4× hint regression because it shares stylo. Gecko
does **not** allocate per element — it interns mapped attribute declarations
(`AttributeStyles`, formerly `nsHTMLStyleSheet`) precisely so the Arcs compare equal. So I
expect **Gecko will not reproduce the regression**, and that a deviation here is not
evidence about stylo but about the embedder. Worth keeping the prediction on record and
testing it — it is now a sharper experiment than when it was written.

## 5. Size of the actual port

Not "port Servo's hierarchy". The work is: extend Blitz's existing single
`for attr in elem.attrs()` loop with tag-gated match arms that call stylo's existing
parsers. No new infrastructure, one function, one file.

| Slice | Scope | Est. |
|---|---|---|
| Fix `bgcolor` to use `parse_legacy_color`; gate to body/table/td/th/tr/thead/tbody/tfoot | **highest spam value** | ~20 lines |
| `<body text>`, `<font color>`, `<hr color>` → `color` | **highest spam value** | ~40 lines |
| `<font size>` / `<font face>` | tiny-text hiding | ~30 lines |
| Gate `align` correctly (float vs text-align per tag); add `valign`, `nowrap` | fixes false passes | ~60 lines |
| `width`/`height` on img/iframe/video/pre/td/th/tr; `hspace`/`vspace`; `border`, `cellspacing`, `cellpadding` | zero-size hiding | ~120 lines |
| `background` (image), `marginwidth`/`topmargin`/…, `dir`, `lang`, list `type` | completeness | ~100 lines |
| Arc-caching of mapped attribute blocks (the perf fix in §4) | restores style sharing | ~80 lines |

**~250 lines for everything spam detection actually needs** (first three rows plus the
sharing fix), ~450 for full spec coverage. Compare: a hand-written pre-pass into inline
styles needs its own legacy-colour parser, its own dimension parser, its own tag gating,
and its own serialisation into a `style=""` string — strictly more code, all of it ours to
maintain, and it would *also* defeat style sharing (every unique inline style does).

## 6. Recommendation

1. **Do not write the attribute→CSS pre-pass.** It is more code than the fix, duplicates
   parsers we already ship, and does not avoid the sharing cost it was meant to avoid.
2. **Do not switch engines to fix hints.** The gap is ~135 lines of incomplete Blitz code
   sitting on top of a complete, already-linked stylo parser.
3. **Complete Blitz's trait impl** (upstreamable — this is a genuine Blitz bug), and fix
   the per-element `Arc` allocation so pres hints stop defeating style sharing.
4. **Re-scope the benchmark.** With the hint question resolved as an in-Blitz fix, the
   surviving reasons to benchmark browsers are the ones the hint work does *not* address:
   the O(n²) nesting cliff, the CSS-nesting stack overflow, the quirks-mode question, and
   the capability audit (detectors that need layout, which Blitz cannot do at all). That is
   still a real study — but it is a robustness-and-capability study, not a speed shoot-out.
