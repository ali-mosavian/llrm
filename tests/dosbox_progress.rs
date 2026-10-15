//! The DOSBox-X waits give up on no progress, not on a wall clock.

#[path = "common/dosbox.rs"]
mod dosbox;

use std::time::Duration;

use dosbox::{Progress, cpu_ticks};

#[test]
fn a_wait_on_a_process_that_keeps_computing_is_never_stalled_however_long_it_takes() {
    let mut progress = Progress::new(Duration::from_secs(120));
    // 20 minutes at one tick a minute: the 120 s wall bound it replaces gave up
    // at 2.
    for minute in 1..=20 {
        assert!(!progress.stalled(minute, Duration::from_secs(60 * minute)));
    }
}

#[test]
fn a_wait_on_a_process_that_uses_no_cpu_is_stalled_after_the_idle_time() {
    let mut progress = Progress::new(Duration::from_secs(120));
    assert!(!progress.stalled(7, Duration::from_secs(10)));
    assert!(!progress.stalled(7, Duration::from_secs(130)));
    assert!(progress.stalled(7, Duration::from_secs(131)));
}

#[test]
fn this_processes_cpu_ticks_are_read_and_rise_when_it_computes() {
    let before = cpu_ticks(std::process::id());
    let began = std::time::Instant::now();
    while began.elapsed() < Duration::from_millis(300) {
        std::hint::black_box(0u64.wrapping_add(1));
    }
    assert!(cpu_ticks(std::process::id()) > before);
}
