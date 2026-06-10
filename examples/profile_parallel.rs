/// CPU flamegraph profiler for parallel rendering.
///
/// Runs two profiled sections and writes flamegraph SVGs:
///   flamegraph_sequential.svg  — 1 thread, TASKS renders
///   flamegraph_parallel_N.svg  — N threads, TASKS renders
///
/// Usage:
///   cargo run --release --example profile_parallel
///   cargo run --release --example profile_parallel -- --threads 8
///
/// Open the SVGs in a browser. Wider bars = more CPU time.
/// Look for bars that grow disproportionately in the parallel flamegraph.
use hyper_render::{Config, Renderer};
use pprof::ProfilerGuard;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

const MEDIUM_HTML: &str = include_str!("../benches/fixtures/medium.html");
const TASKS: usize = 256;
const SAMPLE_FREQ_HZ: i32 = 2000;

fn render_tasks_sequential(renderer: &Renderer, tasks: usize) {
    for _ in 0..tasks {
        let _ = black_box(
            renderer
                .render(MEDIUM_HTML, Config::new().width(800).height(600))
                .unwrap(),
        );
    }
}

fn render_tasks_parallel(renderer: &Arc<Renderer>, tasks: usize, threads: usize) {
    let tasks_per_thread = tasks.div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let r = Arc::clone(renderer);
                s.spawn(move || {
                    for _ in 0..tasks_per_thread {
                        let _ = black_box(
                            r.render(MEDIUM_HTML, Config::new().width(800).height(600))
                                .unwrap(),
                        );
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
}

fn capture<F: FnOnce()>(label: &str, freq: i32, f: F) {
    let guard = ProfilerGuard::new(freq).expect("failed to start pprof");
    let t = Instant::now();
    f();
    let elapsed = t.elapsed();

    let report = guard.report().build().expect("failed to build pprof report");
    let path = format!("flamegraph_{label}.svg");
    let file = std::fs::File::create(&path).expect("failed to create flamegraph file");
    report
        .flamegraph(file)
        .expect("failed to write flamegraph");

    println!(
        "{label:<35} {elapsed:.2?}  →  {path}"
    );
}

fn main() {
    let threads: usize = std::env::args()
        .skip_while(|a| a != "--threads")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));

    println!("Building Renderer (font scan)...");
    let renderer = Arc::new(Renderer::new());

    // Warmup — not profiled
    render_tasks_sequential(&renderer, 4);

    println!("\nProfiling {TASKS} tasks at {SAMPLE_FREQ_HZ} Hz sampling...\n");
    println!("{:<35} {:>10}    output", "Section", "Wall time");
    println!("{}", "-".repeat(65));

    capture("sequential", SAMPLE_FREQ_HZ, || {
        render_tasks_sequential(&renderer, TASKS);
    });

    capture(&format!("parallel_{threads}t"), SAMPLE_FREQ_HZ, || {
        render_tasks_parallel(&renderer, TASKS, threads);
    });

    println!("\nOpen the SVGs in a browser to inspect call stacks.");
    println!("Key things to look for in the parallel flamegraph vs sequential:");
    println!("  - Wider mutex/lock bars  → lock contention");
    println!("  - New frames not in sequential (e.g. park, futex_wait) → thread blocking");
    println!("  - Shallower stacks overall → less useful work per CPU cycle");
}
