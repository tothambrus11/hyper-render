> ⚠️ **Timing caveat:** absolute millisecond figures in this file were measured on a CPU
> pinned at 400 MHz and are inflated ~12.5×. Ratios and percentages are unaffected.
> **`PERFORMANCE.md` has the corrected full-clock numbers** — use those.

# Cost of isolation, and the guard + supervisor architecture

Measured on this machine, release builds, `blitz-dom 0.3.0-beta.2`. Reproduce with
`cargo run --release --example bench_isolation [doc_kb]` and
`cargo run --release --example supervisor`.

## Answer: yes, you can stay under 1 ms — but not with a subprocess per document

| Approach | Added cost per document (40 KB) | Within 1 ms budget? |
|---|---|---|
| Inline (no isolation) | — (baseline 41.0 ms) | — |
| **Subprocess per document** | **+17.6 ms** | ❌ 17× over |
| **Persistent worker + pipe** | **+0.11 ms** (p50), +0.23 ms (p95) | ✅ ~9× under |

Spawning a process per document costs ~17 ms — fork + exec + dynamic linking of a large binary,
paid every time. A **persistent worker** pays that once (2 ms at startup) and then costs only
pipe transport.

### Transport cost scales with document size (~2.4 µs/KB)

| Document | Inline p50 | Worker p50 | **Transport p50** | Transport p95 |
|---|---|---|---|---|
| 5 KB | 10.0 ms | 10.2 ms | **0.043 ms** | 0.056 ms |
| 20 KB | 23.3 ms | 24.0 ms | **0.098 ms** | 0.291 ms |
| 40 KB | 41.0 ms | 41.9 ms | **0.105 ms** | 0.233 ms |
| 100 KB | 93.7 ms | 95.9 ms | **0.279 ms** | 0.521 ms |
| 500 KB | 449.1 ms | 457.7 ms | **1.212 ms** | 2.758 ms |

Under 1 ms for anything up to ~400 KB, which covers essentially all mail. Note the only size
that breaches the budget in absolute terms (500 KB) has a 449 ms parse anyway — **as a fraction,
isolation is 0.3–1 % at every size.** If you ever need to beat that, pass large documents by
`memfd`/shared memory instead of the pipe, but that is premature today.

**Caveat:** these are same-machine pipe numbers with a warm page cache and an idle box. Under
real load the p95s will widen — budget against p95/p99, not p50.

## Recommended architecture

```
                 ┌── cheap O(n) guard (~50 µs) ── reject bombs, never parse ──► flag suspicious
request ─► parent┤
                 └── length-prefixed pipe ──► persistent worker ──► parse + resolve_stylist
                          ▲                        │
                          └── died? restart (2 ms) ┘  ──► flag that one doc, keep going
```

Three layers, each catching what the previous one cannot:

1. **Guard (prevention).** Byte scan for HTML nesting depth and CSS bracket depth. ~50 µs on a
   normal document; rejects bombs without parsing them at all.
2. **Worker (containment).** The parse runs in another process, so an abort cannot take down the
   scanner. Costs ~0.1 ms per document.
3. **Supervisor (recovery).** Detects worker death as EOF on the pipe, restarts in **1.9 ms**,
   flags that document, and continues.

### Measured end-to-end (`examples/supervisor.rs`)

203 documents: 200 normal, plus an HTML bomb, a CSS bomb, and one bomb deliberately routed past
the guard to stand in for an unknown vector:

```
[reject] html-bomb        html nesting depth > 512  (never parsed)
[reject] css-bomb         css bracket depth > 64    (never parsed)
[CRASH]  unknown-vector   worker died -> restarted in 1.9 ms, doc flagged suspicious

scanned OK        : 200
rejected by guard : 2   (0 parses, 0 crashes)
crashes survived  : 1
guard cost / doc  : p50 51 µs, p99 154 µs
```

The pipeline never dies, loses exactly one document per crash, and pays ~0.1 ms per document for
the privilege.

**On the "unknown vector" case — an honest note.** I could not construct a document that
genuinely evades the guard: it covers every crash vector found so far. So that entry is a real
bomb explicitly marked to skip the guard, purely to exercise the recovery path. The supervisor
exists for vectors we have *not* found, and this demonstrates the mechanism works, not that a
bypass is known.

## Design notes worth knowing

- **A stack overflow is `SIGABRT`, not a panic.** `catch_unwind` cannot see it, and neither can
  a `Result`. Process death is the only containment; that is why the worker is not optional if
  you want the scanner to survive a miss.
- **One worker loses one document per crash.** With a pool of N workers, a crash costs one
  document and 1/N of throughput for 2 ms. Restarting is cheap enough that you do not need to be
  clever.
- **Guard limits used:** HTML depth **512** (matches Blink's `kMaximumHTMLParserDOMTreeDepth`),
  CSS bracket depth **64**. See `PRIOR-ART.md` for why these match browser practice.
- **The supervisor's guard is a fast byte scan**, not the tokenizer-based one in
  `examples/depth_guard.rs`. The byte scan is ~50 µs and slightly over-approximates depth (it
  does not model implied end tags); the tokenizer version is accurate but does more work. Start
  with the byte scan; move up only if false rejects show up in practice.
- **Recycle workers periodically** (e.g. every N documents) so any slow leak in a long-lived
  process is bounded. Restart is 2 ms, so this is nearly free.
- **Add a wall-clock timeout per document** in the parent. The guard bounds the two known
  superlinear vectors, but O(n²) parsing means a document that slips through can still burn
  seconds. Kill and restart the worker on timeout — same recovery path as a crash.

## Bottom line

Your instinct was right: **guard first, isolate cheaply, supervise for the rest.** The only
change to your plan is that isolation must be a *persistent* worker rather than a subprocess per
document — that is the difference between 0.11 ms and 17.6 ms.
