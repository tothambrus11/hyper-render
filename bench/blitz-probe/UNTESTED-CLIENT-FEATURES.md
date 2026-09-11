# HTML features to add to CLIENT_RENDERING.md

Additional cases:
 - unrecognized element -> does it reveal or strip the content?
 - ...
  

Cross-referencing the existing corpus (H00–H21, R21–R32) against the Blitz
presentational-hint conformance results (`TRANSFORM-LIST.md`) and the malformed-HTML work
(`FINDINGS.md` §4).

**Verified gap:** the corpus is entirely CSS-property-based. Grepping every `.eml` for
`bgcolor`, `<font `, `valign`, `cellpadding`, `nowrap`, `text="#"` returns **one** hit, and it is
CSS `white-space:nowrap` inside H15 — not the HTML attribute. Likewise zero hits for
`<template>`, `<noscript>`, `clip-path`, `content-visibility`, `<plaintext>`.

Each row below is marked **[blind]** if Blitz cannot currently detect it, since those are the
cases where a client ✅ (technique holds) and our detector are *both* wrong at once.

---

## Priority 1 — Legacy presentational attributes (0 of 28 tested)

This is the largest structural gap, and there is a specific reason to expect it to score well
for an attacker. The corpus's own analysis says the two failure mechanisms are (a) Gmail/
SpaceMail **stripping CSS declarations** and (b) Outlook's Word engine **not implementing the
CSS**. Legacy presentational attributes dodge *both*: they are HTML attributes, not CSS
declarations, so a declaration-stripping sanitiser has nothing to remove; and the Word engine
supports them natively — they are the reason email templates are still written with
`<table bgcolor>` rather than CSS.

If that reasoning holds, **`<font color="#ffffff">` may be a second universally-invisible
technique alongside H03** — and unlike H03, we currently cannot see it.

| proposed | technique | equivalent tested row | blind? |
|---|---|---|---|
| H22a | `<font color="#ffffff">` white-on-white | H02/H03 do this in CSS | **[blind]** |
| H22b | `<font color="white">` (named colour) | — | **[blind]** |
| H22c | `<font color="ffffff">` (bare hex, no `#`) | — | **[blind]** |
| H22d | `<body text="#ffffff">` document-wide | — | **[blind]** |
| H22e | `<td bgcolor="#ffffff">` + `<font color="#ffffff">` | — | partly |
| H23a | `<font size="1">` tiny text | H05 does this in CSS | **[blind]** |
| H23b | `<font size="7">` + `color` | — | **[blind]** |
| H24a | `<td width="0">` / `height="0"` | — | detectable |
| H24b | `<table border="0" cellpadding="0" cellspacing="0">` + clipping | — | **[blind]** |
| H24c | `<td nowrap>` + `text-indent` (attribute form of H15) | H15 (CSS form) | **[blind]** |
| H25 | `background="…"` image matching text colour | — | **[blind]** |

**Why the pairing matters:** several of these are attribute-form twins of rows already tested in
CSS form (H22a↔H02/H03, H23a↔H05, H24c↔H15). Running the pair isolates *mechanism* from
*effect* — exactly what H09 does for `var()` — and tells you whether sanitisers key on the
property or on the syntax carrying it.

## Priority 2 — Attribute vs CSS precedence (untested, and we must get this right)

Per spec, presentational hints sit **below** author CSS in the cascade. If we transform
attributes into inline styles (which `PERFORMANCE.md` recommends for a 2× speedup), we must
reproduce that precedence or we will mis-resolve conflicts. Whether *clients* honour it is
unknown.

| proposed | technique |
|---|---|
| H26a | `<td bgcolor="#000000" style="background-color:#ffffff">` — CSS should win |
| H26b | `<font color="#000000"><span style="color:#ffffff">` |
| H26c | `<td bgcolor="#ffffff">` + `<style>td{background:#000}</style>` — sheet should win over hint |
| H26d | duplicate attribute: `style="color:#000" style="color:#fff"` — spec says first wins |

H26d is really a parser test (see Priority 4) and is a cheap sanitiser-behaviour probe.

## Priority 3 — Modern CSS hiding vectors absent from the corpus

The corpus covers `opacity`, `transform`, `filter`, `position`, `text-indent`, `details`. These
three are in the same family and untested:

| proposed | technique | blind? |
|---|---|---|
| H27a | `clip: rect(0,0,0,0)` + `position:absolute` (legacy sr-only) | detectable |
| H27b | `clip-path: inset(100%)` (modern sr-only idiom) | detectable |
| H27c | `content-visibility: hidden` | **[blind]** — not built in stylo (gecko-only) |
| H28 | `@media (prefers-color-scheme: dark){…{color:#fff}}` — dark-mode-only hiding | detectable |

H28 is worth its own row: it hides only for readers in dark mode, so a single eyeball check in
light mode would score it ✅ (holds) for the wrong reason. **Inspect it in both modes.**

## Priority 4 — Parser differentials (highest value for *detection* specifically)

Every existing row asks "does the client hide it?". None asks **"do the client and our extractor
parse it differently?"** — which is the more dangerous failure, because the payload never reaches
our scorer at all rather than reaching it and being mis-scored. Blitz/html5ever is spec-compliant
(verified: 15/15 malformed cases match spec), but *client sanitisers are not*, and a
desync in either direction is exploitable.

| proposed | technique | why |
|---|---|---|
| H29a | unclosed `<style>` swallowing the payload | spec: text becomes a `<style>` child ⇒ invisible. Does the sanitiser agree, or re-open the tree and show it? |
| H29b | unclosed `<title>` / `<textarea>` swallowing the payload | same, different raw-text element |
| H29c | `<plaintext>` — everything after is raw text | client may render it; a naive extractor may not |
| H29d | `<!-- ` unterminated comment | spec: rest of document is a comment ⇒ invisible |
| H29e | `<template>` containing the payload | spec: in the DOM, never rendered. Extractors that read all text nodes see it |
| H29f | `<noscript>` containing the payload | scripting is **off** in mail, so per spec this content *is* rendered — an extractor that assumes scripting-on treats it as raw text. Inverse direction: file as **R33** |
| H29g | misnested `<b><i></b></i>` / table foster-parenting around the payload | adoption-agency divergence |
| H29h | `meta charset` disagreeing with the MIME `Content-Type` charset | classic mail-specific desync; changes which bytes become which characters |

H29h is the one I would test first of this group — it is mail-specific, it is not covered
anywhere in the corpus, and a charset disagreement can change the extracted text wholesale
rather than just hiding part of it.

## Priority 5 — Cheap additions that close named gaps

| proposed | technique | why |
|---|---|---|
| H30 | `hidden` attribute on the payload element | trivially common; Blitz supports it; untested in clients |
| H31 | `@media screen{ .z{color:#ffffff} }` — plain colour inside an at-rule | **CLIENT_RENDERING.md explicitly identifies this as missing**: it is the sample needed to separate "at-rules are dropped" from "custom properties inside at-rules are dropped", the open question behind H12 |
| R34 | `aria-hidden="true"` on visible text | visible to readers, invisible to accessibility-aware extraction; not a CSS property, so we must handle it in our own logic |

H31 costs one sample and resolves the corpus's single unexplained result.

---

## Suggested order

1. **H22a–H22d** — the white-on-white attribute family. Highest expected yield, directly tests
   the "attributes dodge both failure mechanisms" hypothesis, and we are blind to all four.
2. **H31** — one sample, closes the standing H12 question.
3. **H29a/H29e/H29h** — parser differentials; a different *class* of risk than anything measured
   so far.
4. **H26a–H26c** — precedence, needed before shipping the attribute→CSS transform.
5. The rest.

## One caveat on interpreting results

For the attribute rows, a ✅ (technique holds) means something slightly different than in the
CSS rows: Outlook's Word engine may render `<font color>` *correctly* while stripping the CSS
equivalent, so an attribute row can hold in clients where its CSS twin fails. That is the
hypothesis worth testing — but it also means the attribute and CSS rows are **not**
interchangeable evidence, and both halves of each pair need running before drawing a conclusion.
