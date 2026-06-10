/// Parallel scaling experiment: compares in-process threads vs separate processes.
///
/// Runs TASKS renders distributed across T workers for T in THREAD_COUNTS,
/// using both modes, then writes results to scaling_results.csv and prints a table.
///
/// Usage:
///   cargo run --release --example parallel_scaling
use hyper_render::{render, Config, Renderer};
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MEDIUM_HTML: &str = include_str!("../benches/fixtures/medium.html");
const TASKS: usize = 16;
const SAMPLES: usize = 5;
const THREAD_COUNTS: &[usize] = &[1, 2, 4, 8, 16];

fn render_one() -> Vec<u8> {
    render(
        black_box(MEDIUM_HTML),
        Config::new().width(800).height(600),
    )
    .unwrap()
}

fn render_one_with(renderer: &Renderer) -> Vec<u8> {
    renderer
        .render(
            black_box(MEDIUM_HTML),
            Config::new().width(800).height(600),
        )
        .unwrap()
}

// ---------------------------------------------------------------------------
// In-process: distribute TASKS renders across `threads` std::threads
// ---------------------------------------------------------------------------
fn measure_inprocess(threads: usize, samples: usize) -> Duration {
    let tasks_per_thread = TASKS.div_ceil(threads);
    let mut total = Duration::ZERO;

    for _ in 0..samples {
        let start = Instant::now();
        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(threads);
            for _ in 0..threads {
                handles.push(s.spawn(|| {
                    for _ in 0..tasks_per_thread {
                        let _ = black_box(render_one());
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
        total += start.elapsed();
    }

    total / samples as u32
}

// ---------------------------------------------------------------------------
// In-process with Renderer: reuses cached FontContext via cheap clone
// ---------------------------------------------------------------------------
fn measure_inprocess_renderer(threads: usize, samples: usize, renderer: &Arc<Renderer>) -> Duration {
    let tasks_per_thread = TASKS.div_ceil(threads);
    let mut total = Duration::ZERO;

    for _ in 0..samples {
        let start = Instant::now();
        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(threads);
            for _ in 0..threads {
                handles.push(s.spawn(|| {
                    for _ in 0..tasks_per_thread {
                        let _ = black_box(render_one_with(renderer));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
        total += start.elapsed();
    }

    total / samples as u32
}

// ---------------------------------------------------------------------------
// Per-thread clone: clone Renderer once per thread at spawn time so each
// thread owns its FontContext with zero cross-thread sharing during renders.
// ---------------------------------------------------------------------------
fn measure_inprocess_renderer_cloned(threads: usize, samples: usize, renderer: &Renderer) -> Duration {
    let tasks_per_thread = TASKS.div_ceil(threads);
    let mut total = Duration::ZERO;

    for _ in 0..samples {
        let start = Instant::now();
        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(threads);
            for _ in 0..threads {
                let local = renderer.clone(); // ~96 ns Arc bumps, done before timing matters
                handles.push(s.spawn(move || {
                    for _ in 0..tasks_per_thread {
                        let _ = black_box(render_one_with(&local));
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
        total += start.elapsed();
    }

    total / samples as u32
}

// ---------------------------------------------------------------------------
// Out-of-process: spawn `threads` independent bench_worker processes,
// each doing tasks_per_thread renders, and measure wall time across all of them.
// ---------------------------------------------------------------------------
fn measure_subprocess(worker_bin: &PathBuf, threads: usize, samples: usize) -> Duration {
    let tasks_per_thread = TASKS.div_ceil(threads);
    let mut total = Duration::ZERO;

    for _ in 0..samples {
        let start = Instant::now();

        let mut children: Vec<_> = (0..threads)
            .map(|_| {
                Command::new(worker_bin)
                    .arg(tasks_per_thread.to_string())
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .expect("failed to spawn bench_worker")
            })
            .collect();

        for child in &mut children {
            child.wait().expect("bench_worker failed");
        }

        total += start.elapsed();
    }

    total / samples as u32
}

fn parse_worker_ns(worker_bin: &PathBuf, tasks: usize) -> u64 {
    let out = Command::new(worker_bin)
        .arg(tasks.to_string())
        .output()
        .expect("failed to run bench_worker");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .expect("bench_worker output was not a number")
}

fn main() {
    // Find the release bench_worker binary next to this binary, or build it.
    let worker_bin = {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        p.push("bench_worker");
        if !p.exists() {
            eprintln!("bench_worker binary not found at {p:?}");
            eprintln!("Build it first with:");
            eprintln!("  cargo build --release --example bench_worker");
            std::process::exit(1);
        }
        p
    };

    // Warm up.
    println!("Warming up...");
    let _ = render_one();
    let _ = render_one();

    let renderer = Arc::new(Renderer::new());

    // Baselines: 1 worker, TASKS renders.
    let baseline_inprocess = measure_inprocess(1, SAMPLES);
    let baseline_renderer = measure_inprocess_renderer(1, SAMPLES, &renderer);
    let baseline_subprocess = Duration::from_nanos(
        (0..SAMPLES)
            .map(|_| parse_worker_ns(&worker_bin, TASKS))
            .sum::<u64>() / SAMPLES as u64,
    );

    println!("\nBaseline (1 worker, {TASKS} tasks, {SAMPLES} samples):");
    println!("  in-process  (fresh FontContext): {:.1} ms", baseline_inprocess.as_secs_f64() * 1000.0);
    println!("  in-process  (Renderer, cached):  {:.1} ms", baseline_renderer.as_secs_f64() * 1000.0);
    println!("  subprocess  (fresh FontContext): {:.1} ms", baseline_subprocess.as_secs_f64() * 1000.0);

    let mut csv_rows: Vec<String> = vec![
        "threads,mode,mean_ms,speedup,efficiency_pct".to_string(),
    ];

    println!(
        "\n{:>8}  {:>20}  {:>10}  {:>8}  {:>6}",
        "Threads", "Mode", "Mean(ms)", "Speedup", "Effic%"
    );
    println!("{}", "-".repeat(60));

    for &threads in THREAD_COUNTS {
        let ip    = measure_inprocess(threads, SAMPLES);
        let ipr   = measure_inprocess_renderer(threads, SAMPLES, &renderer);
        let iprc  = measure_inprocess_renderer_cloned(threads, SAMPLES, &renderer);
        let sp    = measure_subprocess(&worker_bin, threads, SAMPLES);

        for (mode, dur, baseline) in [
            ("in-process",           ip,   baseline_inprocess),
            ("Renderer shared Arc",  ipr,  baseline_renderer),
            ("Renderer per-thread",  iprc, baseline_renderer),
            ("subprocess",           sp,   baseline_subprocess),
        ] {
            let ms = dur.as_secs_f64() * 1000.0;
            let speedup = baseline.as_secs_f64() / dur.as_secs_f64();
            let eff = speedup / threads as f64 * 100.0;
            println!("{threads:>8}  {mode:>20}  {ms:>10.1}  {speedup:>8.2}x  {eff:>5.1}%");
            csv_rows.push(format!("{threads},{mode},{ms:.2},{speedup:.4},{eff:.2}"));
        }
    }

    let csv_path = "scaling_results.csv";
    std::fs::write(csv_path, csv_rows.join("\n") + "\n").unwrap();
    println!("\nResults written to {csv_path}");
    println!("Plot with: python3 benches/plot_scaling.py");
}
