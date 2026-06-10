use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use hyper_render::{render, Config, Renderer};
use std::time::{Duration, Instant};

const MEDIUM_HTML: &str = include_str!("fixtures/medium.html");

/// Number of render tasks per measurement sample.
const TASKS: usize = 16;

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

/// Run `task_count` renders sequentially and return the elapsed wall time.
fn run_sequential(task_count: usize) -> Duration {
    let start = Instant::now();
    for _ in 0..task_count {
        let _ = black_box(render_one());
    }
    start.elapsed()
}

/// Distribute `task_count` renders across `thread_count` threads and return
/// the elapsed wall time (from first spawn to last join).
fn run_parallel(task_count: usize, thread_count: usize) -> Duration {
    let tasks_per_thread = task_count.div_ceil(thread_count);
    let start = Instant::now();

    std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(thread_count);
        for _ in 0..thread_count {
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

    start.elapsed()
}

/// Run `task_count` renders sequentially using a shared Renderer.
fn run_sequential_renderer(task_count: usize, renderer: &Renderer) -> Duration {
    let start = Instant::now();
    for _ in 0..task_count {
        let _ = black_box(render_one_with(renderer));
    }
    start.elapsed()
}

fn bench_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_throughput");
    group.throughput(Throughput::Elements(TASKS as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("threads_1_sequential", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                total += run_sequential(TASKS);
            }
            total
        });
    });

    let renderer = Renderer::new();
    group.bench_function("threads_1_sequential_renderer", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                total += run_sequential_renderer(TASKS, &renderer);
            }
            total
        });
    });

    group.finish();
}

/// Distribute `task_count` renders across `thread_count` threads using a shared
/// Renderer (clones the cached FontContext cheaply per render).
fn run_parallel_renderer(task_count: usize, thread_count: usize, renderer: &Renderer) -> Duration {
    let tasks_per_thread = task_count.div_ceil(thread_count);
    let start = Instant::now();

    std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(thread_count);
        for _ in 0..thread_count {
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

    start.elapsed()
}

fn bench_parallel(c: &mut Criterion) {
    let logical_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let thread_counts: Vec<usize> = [1, 2, 4, 8, 16]
        .iter()
        .copied()
        .filter(|&t| t <= logical_cpus * 2)
        .collect();

    let renderer = std::sync::Arc::new(Renderer::new());

    let mut group = c.benchmark_group("parallel_throughput");
    group.throughput(Throughput::Elements(TASKS as u64));
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    for threads in thread_counts.iter().copied() {
        group.bench_with_input(
            BenchmarkId::new("threads", threads),
            &threads,
            |b, &threads| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        total += run_parallel(TASKS, threads);
                    }
                    total
                });
            },
        );
    }

    for threads in thread_counts.iter().copied() {
        let r = std::sync::Arc::clone(&renderer);
        group.bench_with_input(
            BenchmarkId::new("threads_renderer", threads),
            &threads,
            move |b, &threads| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        total += run_parallel_renderer(TASKS, threads, &r);
                    }
                    total
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_sequential, bench_parallel);
criterion_main!(benches);
