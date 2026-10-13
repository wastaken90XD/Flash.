//! CLI entrypoint for `swfr-harness` (TSD 2.6, 2.12 M0–M6, 3.6 M6, 3.14.6).

use std::env;
use std::fs;
use std::path::Path;
use swfr_harness::{
    build_benchmark_swf, format_movie_info, run_benchmark_suite, write_synthetic_test_swfs,
};

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("bench") => {
            print!("{}", run_benchmark_suite());
        }
        Some("dump-test-swfs") => {
            let dir = args.next().unwrap_or_else(|| "dist/test-swfs".to_owned());
            if let Err(err) = write_synthetic_test_swfs(Path::new(&dir)) {
                eprintln!("error writing test swfs: {err}");
                std::process::exit(1);
            }
        }
        Some("info") => {
            let bytes = args
                .next()
                .and_then(|p| fs::read(p).ok())
                .unwrap_or_else(build_benchmark_swf);
            print!("{}", format_movie_info(&bytes));
        }
        Some(path) => {
            let bytes = fs::read(path).unwrap_or_else(|_| build_benchmark_swf());
            print!("{}", format_movie_info(&bytes));
        }
        None => {
            print!("{}", format_movie_info(&build_benchmark_swf()));
        }
    }
}
