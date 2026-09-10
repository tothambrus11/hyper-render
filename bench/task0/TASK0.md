# Task 0 — Is Servo's attribute table portable to Blitz, and how big is the port?

**Answer: the plan's framing is wrong in a way that makes the work smaller.** There is no
portable Servo table; the parsers you need are already linked into Blitz and unused; and the
performance objection to "just complete the trait impl" rests on a **five-line asymmetry in
stylo** that can be fixed directly. Completing the impl is both the correct *and* the fast option.

> **Correction.** My first pass read `blitz-dom 0.2.4` / `stylo 0.8` (what `hyper-render` pins).
> The pipeline under evaluation is `blitz-dom 0.3.0-beta.2` / `stylo 0.20`. Everything below is
> re-derived against the correct versions. The two load-bearing findings survived the version
> change unaltered; the attribute-coverage detail did not, and is corrected in §3.

Evidence: `probe-output.txt`, `sharing-before-after.txt`, `versions.txt`. Servo read at
`92d90f0e` (2026-09-10).

---

## 1. There is no "Servo attribute table" to port

Servo's `Element::synthesize_presentational_hints_for_legacy_attributes`
(`components/script/dom/element/element.rs:1246-1576`) is not a table. It is ~330 lines of
**downcasts to ~15 concrete DOM types**:

```rust
let bgcolor = if let Some(this) = self.downcast::<HTMLBodyElement>() {
    this.get_background_color()
} else if let Some(this) = self.downcast::<HTMLTableElement>() {
    this.get_background_color()
} // ... HTMLTableCellElement, HTMLTableRowElement, HTMLTableSectionElement
```

Those getters read a **pre-parsed, typed** `AttrValue` produced eagerly at attribute-set time by
a per-element `parse_plain_attribute`, implemented across **28 separate files**. Blitz has none
of that: no typed attribute storage, no element class hierarchy. Porting the *structure* means
importing Servo's DOM — not viable, and not necessary.

## 2. The parsers are already in Blitz's build — and Blitz already calls that module

The hard part is parsing legacy attribute values to spec. That code is not Servo-side; it is in
**stylo**, and `stylo` ships `default = ["servo"]`, so `style::servo::attr` is compiled into
every Blitz build:

| Available today | Handles |
|---|---|
| `style::servo::attr::parse_legacy_color` | named colours, bare hex, the full legacy mess |
| `style::servo::attr::parse_length` / `parse_nonzero_length` | HTML dimensions |
| `style::values::specified::FontSize::from_html_size` | `<font size=1..7>` |
| `style::servo::attr::parse_unsigned_integer` | numeric attrs |

**Blitz already imports this module.** `blitz-dom-0.3.0-beta.2/src/stylo.rs:869,1058,1092` call
`style::servo::attr::parse_unsigned_integer`. `parse_legacy_color` sits in the same module,
never referenced:

```
$ grep -rn "parse_legacy_color" blitz-dom-0.3.0-beta.2/src/
(no matches)  -- linked, but unused
```

Verified by calling it from inside this crate (`probe-output.txt`):

```
"white"  -> rgb 255 255 255      "FFF"          -> rgb  15  15  15
"ffffff" -> rgb 255 255 255      "bogus"        -> rgb 176   0   0
"#fff"   -> rgb 255 255 255      "chucknorris"  -> rgb 192   0   0
```

That is the real spec parser, quirks included. **Tier 1 of `TRANSFORM-LIST.md` is not a
transform to write — it is a parser to call.**

## 3. What Blitz 0.3.0-beta.2 actually implements

316 lines (`stylo.rs:811-1127`) covering `align`, `bgcolor`, `border`, `width`, `height`,
`hspace`, `vspace`, `hidden`, `type`, gated across body/table/td/th/tr/thead/tbody/tfoot/col/
colgroup/img/iframe/embed/object/video/marquee/input/svg/hr. Considerably more than 0.2.4.

But the colour path is unchanged from 0.2.4 and is the whole Tier 1 bug — line 836:

```rust
fn parse_color_attr(value: &str) -> Option<(u8, u8, u8, f32)> {
    if !value.starts_with('#') { return None; }   // <-- named + bare-hex dropped here
```

A private 20-line reimplementation, shadowing the correct parser sitting one module away.

### One correction to `preshints-results.txt`
`border="3" (img)` is recorded MISSING. Blitz **does** apply it (`stylo.rs:1055` gates `border`
to `img`/`object`/image-`input`). The oracle compares against a control document, and the
control's default `border-top-width` is `medium` — which computes to **3px**, exactly the value
under test. It is a false negative from a value collision. Measured (`verify-claims-output.txt`):
`border=(none)` → 3px, `border="3"` → 3px, **`border="5"` → 5px, `border="10"` → 10px**.
The real score is **13 applied / 15 missing**, and `border` on `<img>` leaves the transform list.

(`border="5" (table)` is correctly MISSING — Blitz deliberately excludes `table`, with a comment.
Per the HTML rendering spec `<table border>` *should* map, so that one stays on the list.)

## 4. The performance objection dissolves — it is a stylo asymmetry, not a cost of hints

`PERFORMANCE.md` reports that hints defeat style sharing *even when every element is identical*
(900 `<div bgcolor="#ffffff">`: 37.2 ms vs 3.6 ms) and concludes a pre-pass into inline styles is
needed. The measurement is right. The cause is narrower than "hints are slow", and it is fixable.

stylo checks the two cascade sources **differently**:

```rust
// sharing/checks.rs:61 — style attribute
/// First checks pointer identity (fast path), then falls back to value comparison.
(Some(a), Some(b)) => {
    if std::ptr::eq(&*a, &*b) { return true; }
    *a.read_with(guard) == *b.read_with(guard)      // <-- value fallback
}

// sharing/checks.rs:83 — presentational hints
target.pres_hints() == candidate.pres_hints()        // <-- derived PartialEq ...
```

...and that derived `PartialEq` bottoms out in `rule_tree/source.rs:22`:

```rust
impl PartialEq for StyleSource {
    fn eq(&self, other: &Self) -> bool { Arc::ptr_eq(&self.0, &other.0) }   // pointer only
}
```

Blitz allocates a **fresh `Arc` per declaration** (`stylo.rs:824`), so pointer equality never
holds. Style attributes survive this because they get a value fallback; hints have none.

That asymmetry explains the whole measurement. Reproduced on this machine (their `sharing.rs`,
900 elements, style-resolution ms):

| case | stock stylo | shares? |
|---|---|---|
| identical, no attrs | 0.50 | ✅ |
| **identical inline `style`** | **0.54** | ✅ value fallback saves it |
| unique inline `style` | 4.21 | ❌ genuinely different |
| **identical `bgcolor`** | **4.89** | ❌ **pointer-only check** |
| unique `bgcolor` | 4.84 | ❌ |
| identical class rule | 0.58 | ✅ |

Identical `bgcolor` (4.89) costs the same as *unique* `bgcolor` (4.84): the hint path behaves as
if every element were distinct. That is the bug, in one comparison.

`PropertyDeclarationBlock` already implements `PartialEq` by value
(`properties/declaration_block.rs:281`), and Blitz wraps hints with the same `SharedRwLock` that
`guards.author` reads (`blitz-dom/src/stylo.rs:65`), so the fallback is safe to add.

### Measured with the fix applied
Patched `have_same_presentational_hints` to mirror `have_same_style_attribute` — pointer fast
path, then value comparison — and rebuilt against the patched stylo:

| case | stock | patched | |
|---|---|---|---|
| identical, no attrs | 0.50 | 0.52 | |
| identical inline `style` | 0.54 | 0.61 | |
| unique inline `style` | 4.21 | 4.41 | |
| **identical `bgcolor`** | **4.89** | **0.70** | **7.0× faster** |
| unique `bgcolor` | 4.84 | 5.53 | correctly still slow |
| identical class rule | 0.58 | 0.63 | |

`identical bgcolor` lands beside `identical inline style` (0.61) and `identical, no attrs`
(0.52) — sharing restored. `unique bgcolor` stays slow, as it must: those elements genuinely
differ. The patch costs ~0.2–0.7 ms on hint-unique documents (a failed value comparison before
giving up) and pays for itself whenever hints repeat, which is the common case in marketing mail.

**Behaviour is unchanged** under the patch — `examples/preshints` still reports 12 applied / 16
missing, and `bin/malformed` is identical on all six cases. Sharing decisions only affect *how*
a computed style is reached, never *what* it is. Patch and raw numbers:
`stylo-pres-hint-sharing.patch`, `sharing-before-after.txt`.

### This changes pre-registered prediction #3
The plan predicts Gecko shows the same ~4× regression because it shares stylo. Gecko interns
mapped attribute declarations (`AttributeStyles`) so identical attributes yield the *same* Arc
and the pointer fast path hits. I expect **Gecko will not reproduce it** — and if so the finding
is about Blitz's allocation, not about stylo's cascade.

## 5. Bonus: quirks mode is silently dropped (the plan's open question)

**Blitz never propagates the parser's quirks mode into stylo.** html5ever detects it and
blitz-html stores it correctly (`html_sink.rs:295 set_quirks_mode`), but blitz-dom hardcodes:

```rust
// blitz-dom/src/stylo.rs:204
fn quirks_mode(&self) -> QuirksMode { QuirksMode::NoQuirks }   // stored value ignored
// blitz-dom/src/document.rs:426
let stylist = Stylist::new(device, QuirksMode::NoQuirks);
```

Every document is styled as no-quirks regardless of doctype. Measured — a unitless length is
valid *only* in quirks mode, and both forms compute the same (`verify-claims-output.txt`):

```
no doctype (should be quirks) -> height Auto
<!DOCTYPE html> (no-quirks)   -> height Auto     # a browser gives 100px for the first
``` Since real mail frequently has no
doctype, **every number and conformance result we have was taken under the wrong cascade mode**.
Affects unitless lengths and the table-colour-inheritance quirk — the latter bears directly on
white-on-white detection inside tables.

## 6. Size of the port

Not Servo's hierarchy. Extend Blitz's existing `for attr in elem.attrs()` loop with tag-gated
arms calling stylo's parsers. One function, one file, no new infrastructure.

| Slice | Est. |
|---|---|
| Replace `parse_color_attr` with `parse_legacy_color` (fixes named + bare hex everywhere) | **~5 lines** |
| `<font color>`, `<body text>`, `<hr color>` → `color` | ~40 |
| `<font size>` / `<font face>` | ~30 |
| Tier 2: `valign`, `nowrap`, `cellpadding`, `cellspacing`, `background`, `table border` | ~120 |
| stylo `have_same_presentational_hints` value fallback (upstream) | **~12 lines** |
| Propagate quirks mode (§5) | ~15 |

**Tier 1 — the entire white-on-white gap — is ~75 lines, most of it deleting a broken parser.**

## 7. Recommendation

1. **Do not write the attribute→CSS pre-pass.** It duplicates parsers you already ship, is more
   code than the fix, and only works around the stylo asymmetry in §4 rather than removing it.
2. **Do not switch engines to fix hints.** The gap is a private 20-line colour parser shadowing
   the correct one in a module Blitz already imports.
3. **Complete Blitz's trait impl** (upstreamable — a genuine Blitz bug), **and fix the stylo
   sharing asymmetry** (also upstreamable). Correctness and speed, no maintenance burden.
4. **Fix quirks-mode propagation before re-measuring anything**, then re-run the conformance
   suite — current results were taken in the wrong cascade mode.
5. **Re-scope the benchmark.** With hints resolved in-Blitz, the surviving reasons to look at
   browsers are what the fix does *not* address: the O(n²) nesting cliff, the CSS-nesting stack
   overflow, and the capability audit for detectors that need layout. A robustness-and-capability
   study, not a speed shoot-out.
