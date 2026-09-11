# How other Rust HTML-parser users prevent stack overflow and timeouts

Research into resource-constrained / security-sensitive consumers of Rust HTML parsers, and
what each technique implies for our scanner. Claims are labelled by how they were checked.

## The short version

Four distinct techniques are in production use. **Nobody relies on a big stack.** The two that
actually apply to us are a **depth cap at tree construction** (what browsers do) and **iterative
tree algorithms** (what ammonia was forced into by a CVE). Streaming is the strongest technique
but is incompatible with needing a CSS cascade.

| Technique | Who | Applies to us? |
|---|---|---|
| Hard depth cap during tree construction | Blink (512), Gecko (historically 200) | **Yes** — this is our `depth_guard` |
| Replace recursion with iteration | ammonia, after RUSTSEC-2019-0001 | **Yes** — and we found Blitz has this exact bug |
| Don't build a tree at all (streaming) | lol-html / Cloudflare Workers | No — we need computed styles |
| Grow the stack on demand (`stacker`) | rustc | **No** — see the caveat below |
| Process/isolate + CPU budget | Cloudflare Workers | Yes — keep the subprocess plan |

---

## 1. Browsers cap nesting depth, and violate the spec to do it

**Blink — verified from current Chromium source**
(`third_party/blink/renderer/core/html/parser/html_construction_site.h`):

```cpp
static constexpr unsigned kMaximumHTMLParserDOMTreeDepth = 512;
```

The enforcement in `html_construction_site.cc` is the interesting part — it does **not** reject
the document, it *flattens* it:

```cpp
// Add as a sibling of the parent if we have reached the maximum depth allowed.
if (open_elements_.StackDepth() > kMaximumHTMLParserDOMTreeDepth + 1 &&
    location.parent->parentNode()) {
  UseCounter::Count(..., WebFeature::kMaximumHTMLParserDOMTreeDepthHit);
  location.parent = location.parent->parentNode();
}
```

Content is preserved and reparented one level up; depth stays bounded; a UseCounter records the
hit. **Gecko** (per Bugzilla [354161](https://bugzilla.mozilla.org/show_bug.cgi?id=354161),
[256180](https://bugzilla.mozilla.org/show_bug.cgi?id=256180)) historically pruned at depth 200
(`MAX_REFLOW_DEPTH`), explicitly because deep recursion "easily runs out of stack space on
Windows". Note that limit belongs to the older parser/reflow path — I did not find an equivalent
cap in today's `nsHtml5TreeBuilder`, so treat the 200 as historical precedent, not current
behaviour.

**What this means for us.** A depth cap is the mainstream answer, and both engines accepted
spec violation to get it — so our guard is not exotic. Two consequences worth internalising:

- Our 256 limit is in the right range (Blink 512). Consider **512** to match Blink exactly, so
  "what we refuse to parse" lines up with "what the recipient's browser flattens".
- **Deep nesting is not itself proof of spam.** A browser renders it fine, flattened. What it
  *is* is evasion-shaped: it targets scanners, not renderers. So "reject + flag for review"
  remains the right disposition — not "classify as spam".

## 2. ammonia: a CVE forced recursion out of tree algorithms — and Blitz has the same bug

[RUSTSEC-2019-0001](https://rustsec.org/advisories/RUSTSEC-2019-0001.html) (CVSS 7.5, ammonia
< 2.1.0): *"Uncontrolled recursion leads to abort in HTML serialization."* `ammonia::clean` and
`Document::to_string` recursed over the DOM, so deeply nested input aborted the process. **Fix:
serialize iteratively.** Not a depth limit, not a bigger stack — the recursion was removed.

This is the single most transferable result, because **Blitz has the identical bug today.**
Verified with gdb on `blitz-dom 0.3.0-beta.2`:

```
#0  blitz_dom::node::node::Node::write_outer_html_in_style ()
#1  blitz_dom::node::node::Node::write_outer_html_in_style ()
#2  ... (self-recursion all the way down)
```

`outer_html()` survives 30k nesting and **aborts between 30k and 35k**, while *parsing* the same
input succeeds. Repro: `cargo run --release --example serialize_overflow 35000`.

This is a **third, independent** stack-overflow vector, distinct from the two CSS ones — and
unlike those, it is **Blitz's own code**, so it is squarely Blitz's to fix, and the fix is known
(ammonia already did it). Practical takeaway: **do not call `outer_html()` on untrusted
documents** until this is fixed. Note ammonia's other five advisories are all sanitizer-bypass
("format-injection"), not DoS — a reminder that *bypass* and *DoS* are separate risk tracks.

## 3. lol-html: don't build a tree (strongest, but not available to us)

Cloudflare's [lol-html](https://github.com/cloudflare/lol-html) backs Workers' `HTMLRewriter`
and is explicitly built for untrusted input in constrained environments. It is a **streaming
rewriter with no DOM**, so nesting depth cannot drive recursion at all — the failure mode is
designed out rather than bounded. It also exposes explicit budgets:

```rust
MemorySettings { preallocated_parsing_buffer_size, max_allowed_memory_usage }
```

plus `with_graceful_bail_out_on_memory_limit_exceeded()`, which flushes bytes received-but-not-
emitted before returning `MemoryLimitExceededError`, so a limit breach degrades instead of
breaking the response.

**Why we can't use it:** we need inherited/computed CSS, which requires a tree and a cascade.
Worth stating plainly so nobody proposes it as a swap. The transferable idea is the *shape* of
the API — an explicit memory budget and a graceful bail-out — not the parser.

## 4. `stacker`: what rustc does, and why we should not

rustc's `ensure_sufficient_stack` calls `stacker::maybe_grow(RED_ZONE = 100 KiB,
STACK_PER_RECURSION = 1 MiB, f)`, allocating a fresh segment when the stack runs low. It is
absent from our dependency tree, and **stylo does not use it** (checked: no `stacker` in
`Cargo.lock`, no `ensure_sufficient_stack` in stylo).

Rust's own docs warn that stacker *"can mask excessive stack usage but runaway recursion can
still exhaust all memory."* For us that is a downgrade, not a fix: it converts a fast, obvious
abort into a slow OOM that also takes out co-tenant work. It matches my measured result that a
bigger stack only moves the threshold (64 MB dies at 50k, 512 MB survives 200k) — **not a
bound.** Skip it.

## 5. Isolation and CPU budgets

Cloudflare runs untrusted parsing inside per-isolate memory isolation with per-request CPU
limits, and a handler exception halts parsing and errors the body. The generalisable rule:
**a limit you cannot enforce in-process must be enforced by the thing that can kill you.** In
Rust specifically, a stack overflow is a `SIGABRT`, not a panic — `catch_unwind` cannot see it —
so the only in-process defence is *never reaching* the recursion. That is precisely why the
pre-parse guard and the subprocess are complements, not alternatives.

## Where Blitz is structurally better than the alternatives

Two failure modes common to Rust DOM libraries do **not** apply, which is worth knowing before
anyone proposes switching:

- **Recursive `Drop`.** `markup5ever_rcdom` and `rctree`-style DOMs own children via `Rc`, so
  dropping a deep tree recurses and can overflow. Blitz stores nodes in a flat
  `NodeTree(SlotMap<NodeKey, Node>)` with `ThinVec<NodeId>` children — indices, not owning
  pointers — so teardown is flat. Verified: parsing and dropping 80k-deep documents never
  crashed on drop.
- **Recursive tree building.** html5ever's tree builder is an explicit state machine, so 80k
  nesting parses without stack growth. Its problem is algorithmic (O(n²) scope walks), not
  stack depth — a slow document, not a crash.

So Blitz's three overflow vectors are all in *style parsing* (stylo/cssparser) and *serialization*
(Blitz), not in the DOM or the HTML tree builder.

## Recommendations

1. **Keep the pre-parse depth guard.** It is what browsers do. Consider raising HTML depth to
   **512** to match Blink.
2. **Treat a depth breach as "reject and flag", not "spam".** Browsers render these fine; the
   nesting targets us, not the recipient.
3. **Never call `outer_html()` on untrusted input** until the recursive serializer is fixed —
   it is a live abort at ~32k nesting.
4. **Keep the subprocess.** Stack overflow is `SIGABRT`; nothing in-process can catch it.
5. **Do not add `stacker`** and do not rely on a larger thread stack. Both trade a fast crash
   for a slower, worse one.
6. Adopt lol-html's API shape where it fits: an explicit budget plus graceful bail-out beats an
   unbounded call that either returns or dies.

## Sources

- [RUSTSEC-2019-0001 (ammonia recursive serialization)](https://rustsec.org/advisories/RUSTSEC-2019-0001.html)
- [Chromium `html_construction_site.h` / `.cc`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/html/parser/)
- [cloudflare/lol-html](https://github.com/cloudflare/lol-html) · [MemorySettings](https://docs.rs/lol_html/latest/lol_html/struct.MemorySettings.html) · [Cloudflare: Rust streaming HTML parser/rewriter](https://blog.cloudflare.com/html-parsing-2/)
- [stacker](https://docs.rs/stacker/latest/stacker/) · [rustc `ensure_sufficient_stack`](https://doc.rust-lang.org/stable/nightly-rustc/rustc_data_structures/stack/fn.ensure_sufficient_stack.html)
- Bugzilla [354161](https://bugzilla.mozilla.org/show_bug.cgi?id=354161), [256180](https://bugzilla.mozilla.org/show_bug.cgi?id=256180) (Gecko depth pruning)
- [rust-ammonia/ammonia](https://github.com/rust-ammonia/ammonia) · [RustSec advisory-db](https://github.com/rustsec/advisory-db)
