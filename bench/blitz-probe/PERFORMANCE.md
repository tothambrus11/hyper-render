# Performance vs the 30 ms budget — measured at full clock

Re-measured after the machine was restarted with proper CPU scaling. Verification: the same
sanity loop that took **7,630 ms** before now takes **612 ms** — a **12.5×** speedup, matching
the 400 MHz → 5 GHz ratio exactly. My earlier estimate ("divide by 10–12") was correct; the
numbers below are the real ones and supersede everything reported earlier.

Machine: i9-12900H, release build, `blitz-dom 0.3.0-beta.2`, single core.

## Verdict: the budget holds for real email, with ~4× headroom

| Document | parse | style | classify | **total** | vs 30 ms |
|---|---|---|---|---|---|
| 40 KB (874 el) | 1.44 ms | 1.48 ms | 0.01 ms | **2.93 ms** | ✅ 10× under |
| 100 KB (2,176 el) | 3.08 ms | 3.82 ms | 0.04 ms | **6.95 ms** | ✅ 4× under |
| 200 KB | 6.07 ms | 7.83 ms | — | **13.90 ms** | ✅ 2× under |
| 500 KB (10,864 el) | 15.03 ms | 20.45 ms | 0.32 ms | **35.80 ms** | ❌ **over** |

Typical marketing email is 20–100 KB, so the normal case lands around **3–7 ms**. Only very
large documents (~400 KB+) breach the budget.

Fixed per-document construction cost (UA stylesheet parse): **0.26 ms** (40 KB doc) to 0.43 ms.

## The presentational-attribute lever: ~1.9–2.1× (confirmed at full clock)

The ratio predicted from the low-clock runs held exactly:

| Document | as-is | attrs stripped | speedup |
|---|---|---|---|
| 100 KB | 6.86 ms | **3.69 ms** | 1.86× |
| 200 KB | 13.90 ms | **6.99 ms** | 1.99× |
| 500 KB | **36.24 ms** ❌ | **17.62 ms** ✅ | 2.06× |

Style resolution alone drops **4.2×** (100 KB: 3.82 → 0.92 ms; 500 KB: 21.41 → 4.97 ms).

**This is what brings 500 KB documents inside the budget** — 36.2 ms → 17.6 ms.

Cause (measured, 900 identical `<div>`s): stylo's style-sharing cache lets identical elements
reuse a computed style, and presentational hints defeat it *even when every element is
identical* — each hint synthesises a fresh declaration block, so nothing is shareable.

| 900 identical `<div>`s | style |
|---|---|
| no attributes | 3.6 ms |
| identical `style="color:#333333"` | 4.0 ms |
| **identical `bgcolor="#ffffff"`** | **37.2 ms** |

(Those two rows are the low-clock measurements; the *ratio* is the point and it reproduces.)

Since marketing email is saturated with `bgcolor`/`align`/`width` on tables, this is the common
case. And it is the **same pre-processing already required** to fix the `<font color>` and
named-colour `bgcolor` detection gaps (`FINDINGS.md` §3) — one change buys correctness and ~2×.

⚠️ **Implementation warning:** my first attempt measured *zero* speedup because it appended a
second `style="..."` attribute to elements that already had one (html5ever keeps the first and
drops the duplicate) and skipped `width`/`height`. A correct rewrite must **merge into the
existing `style` attribute** and cover **every** hint attribute. Verify by measuring: if style
time does not drop ~4×, it is not working.

## Isolation is free at full clock

| Document | inline | subprocess/doc | persistent worker | **transport p50** | p95 |
|---|---|---|---|---|---|
| 5 KB | 0.824 ms | 2.777 ms | 0.969 ms | **0.006 ms** | 0.008 ms |
| 20 KB | 2.113 ms | 4.297 ms | 2.061 ms | **0.010 ms** | 0.014 ms |
| 40 KB | 3.723 ms | 7.113 ms | 3.653 ms | **0.011 ms** | 0.015 ms |
| 100 KB | 7.999 ms | 14.781 ms | 8.403 ms | **0.024 ms** | 0.047 ms |
| 500 KB | 44.071 ms | 58.354 ms | 47.088 ms | **0.103 ms** | 0.137 ms |

- **Persistent worker: 6–103 µs** — 10–150× under your 1 ms limit. Worker total tracks inline
  within noise.
- **Subprocess per document: +2 to +7 ms** — still 2–7× over the 1 ms limit. The conclusion is
  unchanged: persistent worker, not per-document spawn.

Supervisor at full clock: **guard 4.2 µs p50 / 43 µs p99**, **worker restart 0.4 ms**, 200 docs
scanned, 2 bombs rejected without parsing, 1 injected crash survived.

## Full per-document budget (100 KB email)

| | as-is | optimized |
|---|---|---|
| guard (byte scan) | 0.004 ms | 0.004 ms |
| IPC to worker | 0.024 ms | 0.024 ms |
| parse + style + classify | 6.95 ms | 3.69 ms |
| **total** | **~7.0 ms** | **~3.7 ms** |

Isolation is **0.4 %** of the budget. Everything else is parse + style.

## The real latency threat is not size — it is nesting

Unchanged by the clock fix, because it was always CPU-bound and sustained:

| Nested `<div>` | parse |
|---|---|
| 10 k (48 KB) | 290 ms |
| 20 k (97 KB) | 1,134 ms |
| 40 k (195 KB) | 5,218 ms |
| 80 k (390 KB) | **27,021 ms** |

A 390 KB document that would otherwise cost ~30 ms costs **27 seconds** if it is deeply nested —
900× the budget, from an input no larger than a normal newsletter. **Size caps alone do not
protect the budget; the depth guard does.** The guard rejects this in 7 ms without parsing.

Stack-overflow thresholds are unchanged (stack size does not depend on clock): the 40 KB CSS
bomb still aborts with SIGABRT (exit 134).

## Recommendations

1. **Ship the pre-processing.** ~2×, required for correctness anyway, and it is what puts 500 KB
   documents inside the budget.
2. **Keep the depth guard.** It is the only thing standing between you and a 27-second parse.
3. **Persistent worker, never per-document spawn.** 0.024 ms vs 6.8 ms at 100 KB.
4. **Set the worker deadline to the budget** (30 ms) and treat a breach like a crash — kill,
   restart, flag. Cheap: restart is 0.4 ms.
5. Ignore the 0.26 ms fixed UA-stylesheet cost; it is under 4 % and there is no API to share a
   parsed stylist across documents.

## Reproduce

```
cargo run --release --example profile 100        # stage breakdown
cargo run --release --example hints_cost 500     # the ~2x lever
cargo run --release --example bench_isolation 100 # isolation overhead
cargo run --release --example supervisor          # guard + worker + restart
cargo run --release --example quadratic_parse     # the nesting cliff
```
