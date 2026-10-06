//! Debug logging by channel, as LLVM's `DEBUG_TYPE` and `-debug-only` do it.
//!
//! `LLRM_DEBUG=unroll,regalloc` writes those channels to stderr, each line led by its
//! channel; `LLRM_DEBUG=all` writes every one. A channel that is off costs one cached
//! lookup, and its message is never formatted.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

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
    static CLOCK: std::cell::RefCell<Clock> = std::cell::RefCell::new(Clock::default());
}

/// What one span name, or one path of names, cost.
#[derive(Clone, Copy, Default)]
struct Stat {
    calls: usize,
    own: Duration,
    total: Duration,
}

struct Open {
    name: &'static str,
    start: Instant,
    children: Duration,
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
    /// Per counted event: (hits, misses).
    counts: Vec<(&'static str, usize, usize)>,
    /// The function being worked on, and its steps' own time, by function.
    function: Option<String>,
    functions: Vec<(String, Vec<(&'static str, Duration)>)>,
}

fn add<K: PartialEq>(rows: &mut Vec<(K, Stat)>, key: K, calls: usize, own: Duration, total: Duration) {
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
}

/// Starts the wall clock the report's `untimed` line is measured against.
pub fn start() {
    CLOCK.with(|clock| {
        clock.borrow_mut().start.get_or_insert_with(Instant::now);
    });
}

/// A step `span` opened, closed when it is dropped.
pub struct Span(bool);

/// Opens the step `name`, nested in the one that is open, while the `time` channel is on: for
/// a stretch of a function that no closure bounds.
pub fn span(name: &'static str) -> Span {
    if !enabled("time") {
        return Span(false);
    }
    CLOCK.with(|clock| {
        let mut clock = clock.borrow_mut();
        let now = Instant::now();
        clock.start.get_or_insert(now);
        clock.open.push(Open { name, start: now, children: Duration::ZERO });
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
            let Open { name, start, children } = clock.open.pop().expect("a span is open");
            let spent = start.elapsed();
            let own = spent.saturating_sub(children);
            // A name inside itself counts its outer span's time once.
            let nested = clock.open.iter().any(|one| one.name == name);
            let path: Vec<&'static str> = clock.open.iter().map(|one| one.name).chain([name]).collect();
            add(&mut clock.flat, name, 1, own, if nested { Duration::ZERO } else { spent });
            add(&mut clock.tree, path, 1, own, spent);
            match clock.open.last_mut() {
                Some(parent) => parent.children += spent,
                None => clock.top += spent,
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

/// LLVM's `-time-passes`: `run` timed under `name` while the `time` channel is on, nested
/// in the span that is open. `name` is a literal, so a span on the hot path costs a check.
pub fn timed<T>(name: &'static str, run: impl FnOnce() -> T) -> T {
    let _span = span(name);
    run()
}

/// `timed` for a name made at run time, formatted only when the channel is on.
pub fn timed_by<T>(name: impl FnOnce() -> String, run: impl FnOnce() -> T) -> T {
    if !enabled("time") {
        return run();
    }
    thread_local! {
        static NAMES: std::cell::RefCell<std::collections::HashSet<&'static str>> = std::cell::RefCell::new(Default::default());
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

/// `run` with the steps inside it also charged to `function`, for the `timefunc` channel.
pub fn in_function<T>(function: &str, run: impl FnOnce() -> T) -> T {
    if !enabled("time") || !enabled("timefunc") {
        return run();
    }
    let before = CLOCK.with(|clock| clock.borrow_mut().function.replace(function.to_owned()));
    let out = run();
    CLOCK.with(|clock| clock.borrow_mut().function = before);
    out
}

/// One lookup of a cached `what`: a hit when it was there, a miss when it was computed.
pub fn counted(what: &'static str, hit: bool) {
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

/// The `time` channel's report once the work it timed is done: the steps by own time (the
/// top `LLRM_TIME_TOP`, 30 by default), the nesting, the lookups, and the wall clock less the
/// time the outermost steps took, as `untimed`. `timefunc` adds the costliest functions.
pub fn report_times() {
    if !enabled("time") {
        return;
    }
    CLOCK.with(|clock| {
        let clock = std::mem::take(&mut *clock.borrow_mut());
        let wall = clock.start.map(|start| start.elapsed()).unwrap_or_default();
        let top = std::env::var("LLRM_TIME_TOP").ok().and_then(|one| one.parse().ok()).unwrap_or(30);
        let mut flat = clock.flat;
        flat.sort_by(|one, other| other.1.own.cmp(&one.1.own));
        eprintln!("[time] by own time:");
        for (name, stat) in flat.iter().take(top) {
            eprintln!("[time] {:>10.3} ms own {:>10.3} ms total {:>7}x {name}", ms(stat.own), ms(stat.total), stat.calls);
        }
        let floor = wall / 200;
        eprintln!("[time] nesting (own / total ms, steps under 0.5% left out):");
        let mut tree = clock.tree;
        tree.sort_by(|one, other| one.0.cmp(&other.0));
        for (path, stat) in &tree {
            if stat.total >= floor {
                let indent = "  ".repeat(path.len() - 1);
                eprintln!("[time] {:>10.3} {:>10.3} {:>7}x {indent}{}", ms(stat.own), ms(stat.total), stat.calls, path.last().expect("a name"));
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
        for (function, mut steps) in functions.into_iter().take(10) {
            steps.sort_by_key(|(_, own)| std::cmp::Reverse(*own));
            let all: Duration = steps.iter().map(|(_, own)| *own).sum();
            let worst: Vec<String> = steps.iter().take(4).map(|(name, own)| format!("{name} {:.1}", ms(*own))).collect();
            eprintln!("[time] fn {function}: {:.1} ms ({})", ms(all), worst.join(", "));
        }
        eprintln!("[time] wall {:.3} ms, outermost steps {:.3} ms, untimed {:.3} ms ({:.1}%)", ms(wall), ms(clock.top), ms(wall.saturating_sub(clock.top)), 100.0 * wall.saturating_sub(clock.top).as_secs_f64() / wall.as_secs_f64().max(1e-9));
    });
}

/// Runs `main` with the wall clock started, then writes the `time` report: what a binary's
/// `main` is, so that nothing before or after the timed steps goes unreported.
pub fn run_main(main: impl FnOnce() -> i32) -> i32 {
    start();
    let code = main();
    report_times();
    code
}
