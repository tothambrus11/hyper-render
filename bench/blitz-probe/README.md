# blitz-probe — the harness behind the Task 0 findings

Standalone probe crate used to produce every measurement in `../task0/`. It was
rebuilt from scratch in this container because the original `/home/ambrus/blitz-test`
was not available here.

Pinned to **blitz-dom / blitz-html / blitz-traits `=0.3.0-beta.2`** — the versions the
spam pipeline actually runs — not the `0.2.x` that `hyper-render` itself depends on.
It is deliberately *not* a workspace member of `hyper-render`; build it on its own.

## Reproducing

```bash
cargo run --release --example preshints      # 28-case conformance matrix -> preshints-results.txt
cargo run --release --example sharing        # style-sharing cost by hint pattern
cargo run --release --example verify_claims  # quirks-mode + border="3" checks
cargo run --release --example quadratic_parse  # O(n^2) nesting cliff
cargo run --release --example stack_overflow   # nested-CSS SIGABRT
cargo run --release --example depth_guard       # depth-cap experiment
```

### The one external dependency

`Cargo.toml` carries:

```toml
[patch.crates-io]
stylo = { path = "/tmp/stylo-patched" }
```

That path is a checkout of stylo with `../task0/stylo-pres-hint-sharing.patch` applied,
and it **will not exist on a fresh machine**. To rebuild it:

```bash
cargo vendor --versioned-dirs /tmp/vendor          # or clone the matching stylo tag
cp -r /tmp/vendor/stylo-0.20.0 /tmp/stylo-patched
patch -p1 -d /tmp/stylo-patched < ../task0/stylo-pres-hint-sharing.patch
```

To run the STOCK (unpatched) numbers instead, delete the `[patch.crates-io]` section.
`sharing-before-after.txt` in `../task0/` reports both columns.

## Results files

| File | What it is |
|---|---|
| `preshints-results.txt` | 28 legacy attributes vs blitz 0.3.0-beta.2 — **applied: 12, missing: 16** |
| `fuzz-results.txt` | malformed-input corpus outcomes |
| `malformed-results.txt` | crash/clamp/hang behaviour |

## Analysis notes

`FINDINGS.md`, `PERFORMANCE.md`, `ISOLATION.md`, `PRIOR-ART.md`, `REPORTING.md`,
`TRANSFORM-LIST.md`, `UNTESTED-CLIENT-FEATURES.md` are working notes written alongside
the probes. `../task0/TASK0.md` is the summary that supersedes them where they disagree.
