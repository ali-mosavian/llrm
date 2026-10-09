//! `rfmt-post [--matches] [--stats] FILE...`: rewrite rustfmt output in place. `--matches` only breaks long
//! `matches!`, the step before rustfmt's second run; `--stats` prints what changed.
//! `rfmt-post [--matches] --stdin`: the same on stdin, the result on stdout; nothing is written when it fails.

use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Totals {
    changed: usize,
    chains: usize,
    too_wide: usize,
    matches: usize,
    failed: bool,
}

fn transform(
    text: &str,
    matches_only: bool,
) -> Result<(String, rfmt_post::Stats), String> {
    match matches_only {
        true => rfmt_post::matches_only(text)
            .map(|(out, matches)| (out, rfmt_post::Stats { chains: 0, too_wide: 0, matches })),
        false => rfmt_post::format(text),
    }
}

fn run(
    path: &str,
    matches_only: bool,
    totals: &Mutex<Totals>,
) {
    let text = std::fs::read_to_string(path).expect("readable file");
    let result = transform(&text, matches_only);
    let mut totals = totals.lock().unwrap();
    match result {
        Ok((out, stats)) => {
            totals.chains += stats.chains;
            totals.too_wide += stats.too_wide;
            totals.matches += stats.matches;
            if out != text {
                std::fs::write(path, out).expect("writable file");
                totals.changed += 1;
            }
        }
        Err(error) => {
            eprintln!("{path}: {error}; left as rustfmt wrote it");
            totals.failed = true;
        }
    }
}

fn filter(matches_only: bool) -> ExitCode {
    let mut text = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut text) {
        eprintln!("<stdin>: {error}");
        return ExitCode::FAILURE;
    }
    match transform(&text, matches_only) {
        Ok((out, _)) => {
            std::io::stdout().write_all(out.as_bytes()).expect("writable stdout");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("<stdin>: {error}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut flag = |name: &str| args.iter().position(|a| a == name).map(|i| args.remove(i)).is_some();
    let (stats, matches_only, stdin) = (flag("--stats"), flag("--matches"), flag("--stdin"));
    if stdin {
        return filter(matches_only);
    }
    let (next, totals) = (AtomicUsize::new(0), Mutex::new(Totals::default()));
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(1, |n| n.get()) {
            scope.spawn(|| {
                while let Some(path) = args.get(next.fetch_add(1, Ordering::Relaxed)) {
                    run(path, matches_only, &totals);
                }
            });
        }
    });
    let totals = totals.into_inner().unwrap();
    if stats {
        eprintln!(
            "{} files changed, {} chains split, {} left (line too wide), {} matches! broken",
            totals.changed, totals.chains, totals.too_wide, totals.matches
        );
    }
    if totals.failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
