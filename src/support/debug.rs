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
        if $crate::support::debug::enabled($channel) {
            eprintln!("[{}] {}", $channel, format_args!($($arg)*));
        }
    };
}
