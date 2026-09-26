//! Debug logging by channel, as LLVM's `DEBUG_TYPE` and `-debug-only` do it.
//!
//! `LLRM_DEBUG=unroll,regalloc` writes those channels to stderr, each line led by its
//! channel; `LLRM_DEBUG=all` writes every one. A channel that is off costs one cached
//! lookup, and its message is never formatted.

use std::sync::OnceLock;

/// Whether `channel` is on.
pub fn enabled(channel: &str) -> bool {
    static CHANNELS: OnceLock<Vec<String>> = OnceLock::new();
    let channels = CHANNELS.get_or_init(|| {
        std::env::var("LLRM_DEBUG")
            .map(|value| value.split(',').map(|one| one.trim().to_owned()).filter(|one| !one.is_empty()).collect())
            .unwrap_or_default()
    });
    !channels.is_empty() && channels.iter().any(|one| one == channel || one == "all")
}

/// `debug!("channel", "format", args...)`: one line on `channel`, when it is on.
#[macro_export]
macro_rules! debug {
    ($channel:literal, $($arg:tt)*) => {
        if $crate::debug::enabled($channel) {
            eprintln!("[{}] {}", $channel, format_args!($($arg)*));
        }
    };
}

thread_local! {
    static TIMES: std::cell::RefCell<Vec<(String, std::time::Duration, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// LLVM's `-time-passes`: `run` timed under `name` while the `time` channel is on.
pub fn timed<T>(name: &str, run: impl FnOnce() -> T) -> T {
    if !enabled("time") {
        return run();
    }
    let start = std::time::Instant::now();
    let out = run();
    let spent = start.elapsed();
    TIMES.with(|times| {
        let mut times = times.borrow_mut();
        match times.iter_mut().find(|(one, _, _)| one == name) {
            Some((_, total, calls)) => {
                *total += spent;
                *calls += 1;
            }
            None => times.push((name.to_owned(), spent, 1)),
        }
    });
    out
}

/// The `time` channel's report, largest first, once the work it timed is done.
pub fn report_times() {
    if !enabled("time") {
        return;
    }
    TIMES.with(|times| {
        let mut times = std::mem::take(&mut *times.borrow_mut());
        times.sort_by(|one, other| other.1.cmp(&one.1));
        for (name, total, calls) in times {
            eprintln!("[time] {:>9.3} ms {calls:>6}x {name}", total.as_secs_f64() * 1e3);
        }
    });
}
