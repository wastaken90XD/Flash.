//! CLI entrypoint for `swfr-harness` (TSD 2.6, 2.12 M0).

use std::env;
use std::fs;
use swfr_harness::{format_av_info, inspect_av_info};

fn main() {
    let mut args = env::args().skip(1);
    let bytes = match args.next() {
        Some(path) if path != "info" => fs::read(&path).unwrap_or_else(|_| b"SWFR_STUB".to_vec()),
        Some(_) => match args.next() {
            Some(path) => fs::read(&path).unwrap_or_else(|_| b"SWFR_STUB".to_vec()),
            None => b"SWFR_STUB".to_vec(),
        },
        None => b"SWFR_STUB".to_vec(),
    };

    if let Ok(av) = inspect_av_info(&bytes) {
        println!("{}", format_av_info(&av));
    }
}
