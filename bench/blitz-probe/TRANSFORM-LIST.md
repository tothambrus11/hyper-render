# Presentational attributes Blitz does NOT apply — the transform list

Measured, not read off the spec: each row was probed by parsing a minimal document and comparing
the computed property against the same document without the attribute
(`cargo run --release --example preshints`, raw output in `preshints-results.txt`).

**Result: of 28 presentational hints tested, Blitz applies 12 and misses 16.**

## What we must transform (16)

Ordered by how much each matters for hidden-text detection.

### Tier 1 — directly enable hidden text (must fix)

| Attribute | Applies to | CSS equivalent | Why it matters |
|---|---|---|---|
| `color="…"` | `<font>` | `color: …` | **White-on-white text.** Not implemented at all. |
| `text="…"` | `<body>` | `color: …` | Document-wide text colour; same attack. |
| `bgcolor="white"` (named) | `<body>`, `<table>`, `<tr>`, `<td>`, `<th>`, sections | `background-color: white` | Only `#rgb`/`#rrggbb` parse today. Named colours silently ignored. |
| `bgcolor="ffffff"` (bare hex, no `#`) | same as above | `background-color: #ffffff` | Browsers accept it via legacy colour parsing; Blitz does not. |
| `size="…"` | `<font>` | `font-size: …` | `size="1"` ⇒ tiny text, a classic hiding trick. |

### Tier 2 — affect layout/visibility judgements

| Attribute | Applies to | CSS equivalent |
|---|---|---|
| `border="…"` | `<table>`, `<img>`, `<object>` | `border-width: …px` |
| `cellpadding="…"` | `<table>` (→ cells) | `padding: …px` on `td`/`th` |
| `cellspacing="…"` | `<table>` | `border-spacing: …px` |
| `valign="…"` | `<td>`, `<th>`, `<tr>`, sections, `<col>` | `vertical-align: …` |
| `nowrap` | `<td>`, `<th>` | `white-space: nowrap` |
| `background="url"` | `<body>`, `<table>`, `<tr>`, `<td>`, `<th>` | `background-image: url(…)` |

### Tier 3 — cosmetic, low detection value

| Attribute / element | CSS equivalent |
|---|---|
| `face="…"` on `<font>` | `font-family: …` |
| `<nobr>` element | `white-space: nowrap` |
| `type="a"` on `<ol>`/`<ul>`/`<li>` | `list-style-type: …` |
| `hspace`/`vspace` on `<object>`, `<marquee>` | `margin: …` (works on `<img>` only) |
| `<basefont>` | `font-*` (obsolete; most clients ignore) |

## What Blitz already handles (12) — do not transform these

`bgcolor="#rrggbb"` (incl. on `<body>`) · `align` on block elements and table cells ·
`width`/`height` on `<td>`, `<table>`, `<img>` (px and %) · `hspace` on `<img>` ·
`marginwidth` on `<body>` · `<center>` · the `hidden` attribute.

## Two things worth knowing before you build the test set

**1. `vertical-align` is not a stylo longhand in this build.** It became a *shorthand* for
`alignment-baseline`/`baseline-shift`/`baseline-source` (CSS Inline Layout 3). So even a
CSS-based `vertical-align: top` may not round-trip the way you expect — read the longhands, and
test this one carefully in clients. Same situation as `content-visibility` (gecko-only,
unavailable).

**2. Transforming is *faster* than the "correct" path — this is not just a workaround.**
Presentational hints defeat stylo's style-sharing cache even when every element is identical
(900 identical `<div bgcolor="#ffffff">`: **37.2 ms** of style time vs **4.0 ms** for the same
elements with an identical inline `style`). Stripping hints from a 500 KB email cuts total time
**2.06×** and style time **4.2×**, and is what brings large emails inside the 30 ms budget
(`PERFORMANCE.md`). So the transform buys correctness *and* performance — it is the right design,
not a compromise.

If you would still rather not own it, the alternative is upstreaming these into Blitz's
`synthesize_presentational_hints_for_legacy_attributes` — but note that would land them on the
*slow* hint path, so you would gain the correctness and lose the 2×.

## Test email for client rendering

`email-client-test.html` contains one labelled row per case above, each pairing the legacy
attribute against its CSS equivalent so you can see divergence directly in a client. Send it
through Gmail / Outlook / Apple Mail and note, per case:

- Does the client honour the legacy attribute at all? (Many strip or rewrite it.)
- Does it match the CSS equivalent?
- Is text that we would call "hidden" actually invisible to the reader?

The third question is the one that matters: our detector should match **what the recipient
sees**, and email clients pre-sanitize before their own renderer runs, so client behaviour — not
the spec — is ground truth for mail.
