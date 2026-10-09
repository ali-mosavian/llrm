//! Debug logging by channel, as LLVM's `DEBUG_TYPE` and `-debug-only` do it.
//!
//! `LLRM_DEBUG=unroll,regalloc` writes those channels to stderr, each line led
//! by its channel; `LLRM_DEBUG=all` writes every one. A channel that is off
//! costs one cached lookup, and its message is never formatted.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Whether the compiler checks its own work between passes and phases
/// (`LLRM_VERIFY`, any value but `0`): the module the frontend made is checked
/// either way, as LLVM's release pipeline checks its input once; what each pass
/// and each machine phase returned is checked when this is on. Tests, the gate,
/// torture and the QCport run set it, so a phase that breaks an invariant is
/// caught in every gate run and not in a user's compile.
pub fn verifying() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("LLRM_VERIFY").is_ok_and(|value| value != "0"))
}

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

/// `debug!("channel", "format", args...)`: one line on `channel`, when it is
/// on.
#[macro_export]
macro_rules! debug {
    ($channel:literal, $($arg:tt)*) => {
        if $crate::debug::enabled($channel) {
            eprintln!("[{}] {}", $channel, format_args!($($arg)*));
        }
    };
}

thread_local! {
    static CLOCK: std::cell::RefCell<Clock> = std::cell::RefCell::new(Clock::default());
}

/// What one span name, or one path of names, cost.
#[derive(Clone, Copy, Default)]
struct Stat {
    calls: usize,
    own: Duration,
    total: Duration,
    /// The same in the thread's user-space instructions (or its CPU nanoseconds
    /// where the counter is absent): work done, the same on a loaded host.
    own_work: u64,
    total_work: u64,
}

struct Open {
    name: &'static str,
    start: Instant,
    children: Duration,
    /// The thread's CPU time when it opened, for an outermost step only.
    cpu: Duration,
    /// The thread's work counter when it opened, and what the steps inside it
    /// have taken.
    work: u64,
    children_work: u64,
}

/// The thread's user-space instruction counter, opened on first use:
/// `perf_event_open`, this thread, kernel and hypervisor excluded. -1 where the
/// host will not give one.
fn work_fd() -> i32 {
    thread_local! {
        static FD: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
    }
    FD.with(|fd| {
        if let Some(found) = fd.get() {
            return found;
        }
        // perf_event_attr: type HARDWARE (0), size 128; config INSTRUCTIONS
        // (1); exclude_kernel (bit 5), exclude_hv (bit 6).
        let mut attr = [0u64; 16];
        attr[0] = 128 << 32;
        attr[1] = 1;
        attr[5] = (1 << 5) | (1 << 6);
        // SAFETY: `attr` is a valid, zero-padded perf_event_attr of the size it
        // states; the others are the syscall's own numbers.
        let opened = unsafe { libc::syscall(libc::SYS_perf_event_open, attr.as_ptr(), 0, -1, -1, 8u64) };
        let found = i32::try_from(opened).unwrap_or(-1);
        fd.set(Some(found));
        found
    })
}

/// What the thread has done so far, in `work_unit`s.
fn work_now() -> u64 {
    let fd = work_fd();
    if fd >= 0 {
        let mut count: u64 = 0;
        // SAFETY: `count` is eight writable bytes, what a counter without a
        // read format returns.
        let got = unsafe { libc::read(fd, (&mut count as *mut u64).cast(), 8) };
        if got == 8 {
            return count;
        }
    }
    u64::try_from(cpu_now().as_nanos()).unwrap_or(u64::MAX)
}

/// The thread's work so far (instructions, or CPU nanoseconds where there is no
/// counter), for a caller that bills its own steps.
pub fn work() -> u64 {
    work_now()
}

fn work_unit() -> &'static str {
    if work_fd() >= 0 { "Minstr" } else { "Mcpu-ns" }
}

/// The calling thread's CPU time: what a preempted or waiting thread does not
/// spend.
fn cpu_now() -> Duration {
    let mut found = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `found` is a valid timespec for the call to fill.
    let done = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut found) };
    if done == 0 { Duration::new(found.tv_sec as u64, found.tv_nsec as u32) } else { Duration::ZERO }
}

#[derive(Default)]
struct Clock {
    start: Option<Instant>,
    open: Vec<Open>,
    /// Per span name.
    flat: Vec<(&'static str, Stat)>,
    /// Per path of span names, outermost first.
    tree: Vec<(Vec<&'static str>, Stat)>,
    /// Time in spans with nothing open around them.
    top: Duration,
    /// The same on the thread's CPU clock, which a loaded machine does not
    /// stretch: the thread's CPU time when the clock started, and in the
    /// outermost steps.
    cpu_start: Option<Duration>,
    cpu_top: Duration,
    /// The work counter when the clock started, and in the outermost steps.
    work_start: Option<u64>,
    work_top: u64,
    /// Per counted event: (hits, misses).
    counts: Vec<(&'static str, usize, usize)>,
    /// The function being worked on, and its steps' own time, by function.
    function: Option<String>,
    functions: Vec<(String, Vec<(&'static str, Duration)>)>,
}

fn add<K: PartialEq>(
    rows: &mut Vec<(K, Stat)>,
    key: K,
    calls: usize,
    own: Duration,
    total: Duration,
    own_work: u64,
    total_work: u64,
) {
    let stat = match rows.iter_mut().find(|(one, _)| *one == key) {
        Some((_, stat)) => stat,
        None => {
            rows.push((key, Stat::default()));
            &mut rows.last_mut().expect("pushed").1
        }
    };
    stat.calls += calls;
    stat.own += own;
    stat.total += total;
    stat.own_work += own_work;
    stat.total_work += total_work;
}

/// Starts the wall clock the report's `untimed` line is measured against.
pub fn start() {
    CLOCK.with(|clock| {
        let mut clock = clock.borrow_mut();
        clock.start.get_or_insert_with(Instant::now);
        clock.cpu_start.get_or_insert_with(cpu_now);
        clock.work_start.get_or_insert_with(work_now);
    });
}

/// A step `span` opened, closed when it is dropped.
pub struct Span(bool);

/// Opens the step `name`, nested in the one that is open, while the `time`
/// channel is on: for a stretch of a function that no closure bounds.
pub fn span(name: &'static str) -> Span {
    if !enabled("time") {
        return Span(false);
    }
    CLOCK.with(|clock| {
        let mut clock = clock.borrow_mut();
        let now = Instant::now();
        clock.start.get_or_insert(now);
        let cpu = cpu_now();
        clock.cpu_start.get_or_insert(cpu);
        clock.work_start.get_or_insert_with(work_now);
        clock.open.push(Open { name, start: now, children: Duration::ZERO, cpu, work: work_now(), children_work: 0 });
    });
    Span(true)
}

impl Drop for Span {
    fn drop(&mut self) {
        if !self.0 {
            return;
        }
        CLOCK.with(|clock| {
            let mut clock = clock.borrow_mut();
            let Open { name, start, children, cpu, work, children_work } = clock.open.pop().expect("a span is open");
            let spent = start.elapsed();
            let done = work_now().saturating_sub(work);
            let own_work = done.saturating_sub(children_work);
            let own = spent.saturating_sub(children);
            // A name inside itself counts its outer span's time once.
            let nested = clock.open.iter().any(|one| one.name == name);
            let path: Vec<&'static str> = clock.open.iter().map(|one| one.name).chain([name]).collect();
            add(
                &mut clock.flat,
                name,
                1,
                own,
                if nested { Duration::ZERO } else { spent },
                own_work,
                if nested { 0 } else { done },
            );
            add(&mut clock.tree, path, 1, own, spent, own_work, done);
            match clock.open.last_mut() {
                Some(parent) => {
                    parent.children += spent;
                    parent.children_work += done;
                }
                None => {
                    clock.top += spent;
                    clock.cpu_top += cpu_now().saturating_sub(cpu);
                    clock.work_top += done;
                }
            }
            if let Some(function) = clock.function.clone() {
                let steps = match clock.functions.iter().position(|(one, _)| *one == function) {
                    Some(at) => &mut clock.functions[at].1,
                    None => {
                        clock.functions.push((function, Vec::new()));
                        &mut clock.functions.last_mut().expect("pushed").1
                    }
                };
                match steps.iter_mut().find(|(one, _)| *one == name) {
                    Some((_, total)) => *total += own,
                    None => steps.push((name, own)),
                }
            }
        });
    }
}

/// LLVM's `-time-passes`: `run` timed under `name` while the `time` channel is
/// on, nested in the span that is open. `name` is a literal, so a span on the
/// hot path costs a check.
pub fn timed<T>(
    name: &'static str,
    run: impl FnOnce() -> T,
) -> T {
    let _span = span(name);
    run()
}

/// `timed` for a name made at run time, formatted only when the channel is on.
pub fn timed_by<T>(
    name: impl FnOnce() -> String,
    run: impl FnOnce() -> T,
) -> T {
    if !enabled("time") {
        return run();
    }
    thread_local! {
        static NAMES: std::cell::RefCell<crate::hash::HashSet<&'static str>> = std::cell::RefCell::new(Default::default());
    }
    let name = name();
    let name = NAMES.with(|names| {
        let mut names = names.borrow_mut();
        match names.get(name.as_str()) {
            Some(one) => *one,
            None => {
                let leaked: &'static str = Box::leak(name.into_boxed_str());
                names.insert(leaked);
                leaked
            }
        }
    });
    timed(name, run)
}

/// `run` with the steps inside it also charged to `function`, for the
/// `timefunc` channel.
pub fn in_function<T>(
    function: &str,
    run: impl FnOnce() -> T,
) -> T {
    if !enabled("time") || !enabled("timefunc") {
        return run();
    }
    let before = CLOCK.with(|clock| clock.borrow_mut().function.replace(function.to_owned()));
    let out = run();
    CLOCK.with(|clock| clock.borrow_mut().function = before);
    out
}

/// One lookup of a cached `what`: a hit when it was there, a miss when it was
/// computed.
pub fn counted(
    what: &'static str,
    hit: bool,
) {
    if !enabled("time") {
        return;
    }
    CLOCK.with(|clock| {
        let mut clock = clock.borrow_mut();
        let at = match clock.counts.iter().position(|(one, _, _)| *one == what) {
            Some(at) => at,
            None => {
                clock.counts.push((what, 0, 0));
                clock.counts.len() - 1
            }
        };
        if hit {
            clock.counts[at].1 += 1;
        } else {
            clock.counts[at].2 += 1;
        }
    });
}

fn ms(spent: Duration) -> f64 {
    spent.as_secs_f64() * 1e3
}

/// The `time` channel's report once the work it timed is done: the steps by own
/// time (the top `LLRM_TIME_TOP`, 30 by default), the nesting, the lookups, and
/// the wall clock less the time the outermost steps took, as `untimed`.
/// `timefunc` adds the costliest functions.
pub fn report_times() {
    if !enabled("time") {
        return;
    }
    CLOCK.with(|clock| {
        let clock = std::mem::take(&mut *clock.borrow_mut());
        let wall = clock.start.map(|start| start.elapsed()).unwrap_or_default();
        // Read with the wall clock, before the report's own printing is done.
        let cpu = clock.cpu_start.map(|start| cpu_now().saturating_sub(start)).unwrap_or_default();
        let top = std::env::var("LLRM_TIME_TOP").ok().and_then(|one| one.parse().ok()).unwrap_or(30);
        let mut flat = clock.flat;
        flat.sort_by(|one, other| other.1.own.cmp(&one.1.own));
        eprintln!("[time] by own time:");
        for (name, stat) in flat.iter().take(top) {
            eprintln!(
                "[time] {:>10.3} ms own {:>10.3} ms total {:>7}x {name}",
                ms(stat.own),
                ms(stat.total),
                stat.calls
            );
        }
        // Work done, not time passed: the same on a loaded host, for a gate to
        // read. The unit is the thread's user-space instructions, or
        // its CPU nanoseconds where the host has no counter.
        let unit = work_unit();
        let work = clock.work_start.map(|start| work_now().saturating_sub(start)).unwrap_or_default();
        let mut by_work: Vec<&(&'static str, Stat)> = flat.iter().collect();
        by_work.sort_by(|one, other| other.1.own_work.cmp(&one.1.own_work));
        eprintln!(
            "[instr] by own work, in M{} (the thread's user-space instructions; M{} where the host has no counter):",
            &unit[1..],
            "cpu-ns"
        );
        for (name, stat) in by_work.iter().take(top) {
            eprintln!(
                "[instr] {:>12.3} {unit} own {:>12.3} {unit} total {:>7}x {name}",
                stat.own_work as f64 / 1e6,
                stat.total_work as f64 / 1e6,
                stat.calls
            );
        }
        eprintln!(
            "[instr] total {:.3} {unit}, outermost steps {:.3} {unit}, outside every step {:.3} {unit}",
            work as f64 / 1e6,
            clock.work_top as f64 / 1e6,
            work.saturating_sub(clock.work_top) as f64 / 1e6
        );
        let floor = wall / 200;
        eprintln!("[time] nesting (own / total ms, steps under 0.5% left out):");
        let mut tree = clock.tree;
        tree.sort_by(|one, other| one.0.cmp(&other.0));
        for (path, stat) in &tree {
            if stat.total >= floor {
                let indent = "  ".repeat(path.len() - 1);
                eprintln!(
                    "[time] {:>10.3} {:>10.3} {:>7}x {indent}{}",
                    ms(stat.own),
                    ms(stat.total),
                    stat.calls,
                    path.last().expect("a name")
                );
            }
        }
        if !clock.counts.is_empty() {
            eprintln!("[time] lookups:");
            for (what, hits, misses) in &clock.counts {
                eprintln!("[time] {hits:>9} hits {misses:>9} misses {what}");
            }
        }
        let mut functions = clock.functions;
        functions.sort_by_key(|(_, steps)| std::cmp::Reverse(steps.iter().map(|(_, own)| *own).sum::<Duration>()));
        let shown = std::env::var("LLRM_TIME_FUNCS").ok().and_then(|one| one.parse().ok()).unwrap_or(10);
        for (function, mut steps) in functions.into_iter().take(shown) {
            steps.sort_by_key(|(_, own)| std::cmp::Reverse(*own));
            let all: Duration = steps.iter().map(|(_, own)| *own).sum();
            let worst: Vec<String> =
                steps.iter().take(4).map(|(name, own)| format!("{name} {:.1}", ms(*own))).collect();
            eprintln!("[time] fn {function}: {:.1} ms ({})", ms(all), worst.join(", "));
        }
        eprintln!(
            "[time] wall {:.3} ms, outermost steps {:.3} ms, untimed {:.3} ms ({:.1}%)",
            ms(wall),
            ms(clock.top),
            ms(wall.saturating_sub(clock.top)),
            100.0 * wall.saturating_sub(clock.top).as_secs_f64() / wall.as_secs_f64().max(1e-9)
        );
        // The same by the thread's CPU time: a wait between two steps, a thread
        // another process preempted, is not time in no step.
        eprintln!(
            "[time] cpu {:.3} ms, outermost steps cpu {:.3} ms, untimed cpu {:.3} ms ({:.1}%)",
            ms(cpu),
            ms(clock.cpu_top),
            ms(cpu.saturating_sub(clock.cpu_top)),
            100.0 * cpu.saturating_sub(clock.cpu_top).as_secs_f64() / cpu.as_secs_f64().max(1e-9)
        );
    });
}

/// Runs `main` with the wall clock started, then writes the `time` report: what
/// a binary's `main` is, so that nothing before or after the timed steps goes
/// unreported.
pub fn run_main(main: impl FnOnce() -> i32) -> i32 {
    start();
    let code = main();
    report_times();
    code
}

#[cfg(test)]
mod tests {
    use super::{work_fd, work_now};

    fn spin(rounds: u64) -> u64 {
        let mut sum = 0u64;
        for at in 0..rounds {
            sum = std::hint::black_box(sum.wrapping_add(at));
        }
        sum
    }

    /// Wall time and thread CPU time both read a loaded host's weather (the
    /// same linear pass read 33 to 89 ms between runs): the per-pass column
    /// a scaling gate reads is the thread's user-space instructions, which the
    /// same work repeats to a hair.
    #[test]
    fn test_the_work_counter_counts_the_same_loop_the_same_and_at_least_its_instructions() {
        if work_fd() < 0 {
            // no counter on this host: the column falls back to CPU
            // nanoseconds, which is not this claim
            return;
        }
        let measured = || {
            let before = work_now();
            spin(1_000_000);
            work_now() - before
        };
        let (one, other) = (measured(), measured());
        assert!(one >= 1_000_000, "a million iterations counted {one} instructions");
        assert!(one.abs_diff(other) * 100 <= one, "the same loop read {one} and then {other}");
    }
}
