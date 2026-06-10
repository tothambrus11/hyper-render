/// Standalone worker used by parallel_scaling to benchmark renders in a separate process.
///
/// Usage: bench_worker <tasks>
///
/// Runs `tasks` PNG renders of the medium HTML fixture and prints the wall time in
/// nanoseconds to stdout on a single line.
use std::time::Instant;
use hyper_render::{render, Config};

const MEDIUM_HTML: &str = include_str!("../benches/fixtures/medium.html");

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tasks: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(16);

    let start = Instant::now();
    for _ in 0..tasks {
        let _ = std::hint::black_box(render(
            std::hint::black_box(MEDIUM_HTML),
            Config::new().width(800).height(600),
        ).unwrap());
    }
    let elapsed = start.elapsed();

    println!("{}", elapsed.as_nanos());
}
