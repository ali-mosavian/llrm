//! The smallest external code at every level: what `ours` is graded against.
//!
//! Flags per level, the nearest each compiler has (`flags`):
//!
//! | level | cc65 | sdcc | llvm-mos |
//! |---|---|---|---|
//! | O1 | none (optimizer off, its only lower setting) | default | `-O1` |
//! | O2 | `-O` | default | `-O2` |
//! | O3 | `-Oirs` (inline, register variables, inline std functions: its speed end) | `--opt-code-speed` | `-O3` |
//! | Os | `-O` (cc65's optimizer is size-first; `-Oi`/`-Os` inline and grow) | `--opt-code-size` | `-Os` |
//!
//! cc65's hand-written library routine (`hand`) is one more bar, the same at
//! every level. A bar is the linked bytes of the kernel, as in the other
//! tables. External results do not change, so they are kept in `bars.tsv`
//! beside this file, keyed by kernel, compiler and level with the flags that
//! built them; a row whose flags differ from `flags` is stale and built again.
//! `BENCH_BARS_REFRESH=1` builds every bar again.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    KERNELS, KERNELS2, KERNELS3, Kernel, WORK_LEVEL, cc65_row, hand_kernels, hand_row, llvmmos, ours_row, sdcc,
};

pub const LEVELS: [&str; 4] = ["O1", "O2", "O3", "Os"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Compiler {
    Cc65,
    Sdcc,
    LlvmMos,
    Hand,
}

impl Compiler {
    pub const COMPILERS: [Compiler; 3] = [Compiler::Cc65, Compiler::Sdcc, Compiler::LlvmMos];

    fn name(self) -> &'static str {
        match self {
            Compiler::Cc65 => "cc65",
            Compiler::Sdcc => "sdcc",
            Compiler::LlvmMos => "llvm-mos",
            Compiler::Hand => "hand",
        }
    }
}

/// The optimizer flags `compiler` is given at `level` (`O1`, `O2`, `O3`, `Os`).
pub fn flags(
    compiler: Compiler,
    level: &str,
) -> &'static [&'static str] {
    match (compiler, level) {
        (Compiler::Cc65, "O1") => &[],
        (Compiler::Cc65, "O2" | "Os") => &["-O"],
        (Compiler::Cc65, "O3") => &["-Oirs"],
        (Compiler::Sdcc, "O1" | "O2") => &[],
        (Compiler::Sdcc, "O3") => &["--opt-code-speed"],
        (Compiler::Sdcc, "Os") => &["--opt-code-size"],
        (Compiler::LlvmMos, "O1") => &["-O1"],
        (Compiler::LlvmMos, "O2") => &["-O2"],
        (Compiler::LlvmMos, "O3") => &["-O3"],
        (Compiler::LlvmMos, "Os") => &["-Os"],
        (Compiler::Hand, _) => &[],
        (_, other) => panic!("no level {other}"),
    }
}

/// An external's result: the linked bytes and clocks, or why there is none. A
/// missing bar is never a number: it is not the smallest of anything.
#[derive(Clone, Debug, PartialEq)]
pub enum Bar {
    Have { bytes: i64, cycles: i64 },
    Missing(String),
}

impl Bar {
    /// A measured row; a size that cannot be (0 or less: the stub subtraction
    /// went wrong) is a failed measurement, not a bar.
    pub fn of(row: Result<super::Row, String>) -> Bar {
        match row {
            Ok(r) if r.bytes <= 0 => Bar::Missing(format!("measured {} B", r.bytes)),
            Ok(r) => Bar::Have { bytes: r.bytes, cycles: r.cycles },
            Err(why) => Bar::Missing(why),
        }
    }

    fn bytes(&self) -> Option<i64> {
        match self {
            Bar::Have { bytes, .. } => Some(*bytes),
            Bar::Missing(_) => None,
        }
    }
}

/// The smallest bar and who made it; `None` when no bar exists.
pub fn smallest(bars: &[(Compiler, Bar)]) -> Option<(Compiler, i64)> {
    bars.iter().filter_map(|(c, bar)| Some((*c, bar.bytes()?))).min_by_key(|&(_, bytes)| bytes)
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Within,
    Over(i64),
    NoBar,
    NoOurs,
}

pub fn verdict(
    ours: Option<i64>,
    bar: Option<i64>,
) -> Verdict {
    match (ours, bar) {
        (None, _) => Verdict::NoOurs,
        (_, None) => Verdict::NoBar,
        (Some(o), Some(b)) if o <= b => Verdict::Within,
        (Some(o), Some(b)) => Verdict::Over(o - b),
    }
}

/// `bars.tsv`: `kernel compiler level flags bytes cycles`, or `n/a reason` for
/// the last two.
#[derive(Default)]
pub struct Cache(BTreeMap<(String, String, String), (String, Bar)>);

fn flag_text(flags: &[&str]) -> String {
    if flags.is_empty() { "-".to_owned() } else { flags.join(" ") }
}

impl Cache {
    pub fn parse(text: &str) -> Cache {
        let mut cache = Cache::default();
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 6 {
                continue;
            }
            let bar = match (f[4], f[4].parse::<i64>(), f[5].parse::<i64>()) {
                (_, Ok(bytes), Ok(cycles)) => Bar::Have { bytes, cycles },
                ("n/a", ..) => Bar::Missing(f[5].to_owned()),
                _ => continue,
            };
            cache.0.insert((f[0].to_owned(), f[1].to_owned(), f[2].to_owned()), (f[3].to_owned(), bar));
        }
        cache
    }

    pub fn render(&self) -> String {
        let mut out = String::from(
            "# kernel\tcompiler\tlevel\tflags\tbytes\tcycles (external results; see bars.rs; BENCH_BARS_REFRESH=1 rebuilds)\n",
        );
        for ((kernel, compiler, level), (flags, bar)) in &self.0 {
            let (a, b) = match bar {
                Bar::Have { bytes, cycles } => (bytes.to_string(), cycles.to_string()),
                Bar::Missing(why) => ("n/a".to_owned(), why.replace(['\t', '\n'], " ")),
            };
            out.push_str(&format!("{kernel}\t{compiler}\t{level}\t{flags}\t{a}\t{b}\n"));
        }
        out
    }

    /// The kept bar, if it was built with the flags `compiler` has at `level`
    /// now.
    pub fn get(
        &self,
        kernel: &str,
        compiler: Compiler,
        level: &str,
    ) -> Option<Bar> {
        let (kept, bar) = self.0.get(&(kernel.to_owned(), compiler.name().to_owned(), key_level(compiler, level)))?;
        (*kept == flag_text(flags(compiler, level))).then(|| bar.clone())
    }

    pub fn put(
        &mut self,
        kernel: &str,
        compiler: Compiler,
        level: &str,
        bar: Bar,
    ) {
        let key = (kernel.to_owned(), compiler.name().to_owned(), key_level(compiler, level));
        self.0.insert(key, (flag_text(flags(compiler, level)), bar));
    }
}

/// The hand routine is the same at every level.
fn key_level(
    compiler: Compiler,
    level: &str,
) -> String {
    if compiler == Compiler::Hand { "-".to_owned() } else { level.to_owned() }
}

fn cache_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/bench/bars.tsv")
}

/// Builds `kernel` with `compiler` at `level`. A panic of the harness is a
/// missing bar.
fn build(
    kernel: &Kernel,
    compiler: Compiler,
    level: &str,
) -> Bar {
    WORK_LEVEL.with(|l| *l.borrow_mut() = Some(format!("bars-{level}")));
    let f = flags(compiler, level);
    let row = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match compiler {
        Compiler::Cc65 => cc65_row(kernel, f, "c1"),
        Compiler::Sdcc => sdcc::row(kernel, f, false),
        Compiler::LlvmMos => llvmmos::row(kernel, f[0], false),
        Compiler::Hand => hand_row(kernel),
    }));
    Bar::of(row.unwrap_or_else(|_| Err("the bench panicked".to_owned())))
}

enum Job {
    Ours(usize, &'static str),
    Bar(usize, Compiler, &'static str),
}

/// Every cell at every level: ours against the smallest external. The counted
/// line and the cells over the bar, worst first, per level.
/// `BENCH_BARS_LEVELS=O1,Os` and `BENCH_ONLY=<kernel>` narrow the run.
#[test]
#[ignore = "needs cc65, sdcc, llvm-mos, clang and sim65"]
fn the_bars_at_every_level() {
    let hand = hand_kernels();
    let cells: Vec<&Kernel> = hand.iter().chain(KERNELS).chain(KERNELS2).chain(KERNELS3).collect();
    let only = std::env::var("BENCH_ONLY").ok();
    let levels: Vec<&'static str> = match std::env::var("BENCH_BARS_LEVELS") {
        Ok(list) => LEVELS.into_iter().filter(|l| list.split(',').any(|x| x == *l)).collect(),
        Err(_) => LEVELS.to_vec(),
    };
    let refresh = std::env::var_os("BENCH_BARS_REFRESH").is_some();
    let kept = if refresh {
        Cache::default()
    } else {
        Cache::parse(&std::fs::read_to_string(cache_path()).unwrap_or_default())
    };
    let wanted = |k: &Kernel| only.as_deref().is_none_or(|o| o == k.name);
    let has_hand = |k: &Kernel| k.hand.is_some() || k.name == "lz4";

    let mut jobs = Vec::new();
    for (i, k) in cells.iter().enumerate().filter(|(_, k)| wanted(k)) {
        for &level in &levels {
            jobs.push(Job::Ours(i, level));
            for c in Compiler::COMPILERS {
                if kept.get(k.name, c, level).is_none() {
                    jobs.push(Job::Bar(i, c, level));
                }
            }
        }
        if has_hand(k) && kept.get(k.name, Compiler::Hand, "O2").is_none() {
            jobs.push(Job::Bar(i, Compiler::Hand, "O2"));
        }
    }
    let next = AtomicUsize::new(0);
    let ours: Mutex<BTreeMap<(usize, &str), Bar>> = Mutex::new(BTreeMap::new());
    let cache = Mutex::new(kept);
    let threads = std::env::var("BENCH_BARS_THREADS").ok().and_then(|n| n.parse().ok()).unwrap_or(4);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let n = next.fetch_add(1, Ordering::SeqCst);
                    let Some(job) = jobs.get(n) else { break };
                    match job {
                        Job::Ours(i, level) => {
                            WORK_LEVEL.with(|l| *l.borrow_mut() = Some(format!("bars-{level}")));
                            let flag = format!("-{level}");
                            let row = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                ours_row(cells[*i], &flag, "o2")
                            }));
                            let bar = Bar::of(row.unwrap_or_else(|_| Err("the bench panicked".to_owned())));
                            ours.lock().unwrap().insert((*i, level), bar);
                        }
                        Job::Bar(i, c, level) => {
                            let bar = build(cells[*i], *c, level);
                            // kept as it comes: a long run that dies keeps what
                            // it built
                            let mut cache = cache.lock().unwrap();
                            cache.put(cells[*i].name, *c, level, bar);
                            if n % 20 == 0 {
                                std::fs::write(cache_path(), cache.render()).unwrap();
                            }
                        }
                    }
                }
            });
        }
    });
    let cache = cache.into_inner().unwrap();
    if !jobs.iter().all(|j| matches!(j, Job::Ours(..))) {
        std::fs::write(cache_path(), cache.render()).unwrap();
    }
    let ours = ours.into_inner().unwrap();

    let mut report = String::new();
    let mut counted = Vec::new();
    for &level in &levels {
        let mut over: Vec<(i64, String)> = Vec::new();
        let (mut within, mut within_hand, mut total, mut no_bar, mut no_ours) = (0, 0, 0, 0, 0);
        report.push_str(&format!(
            "\n## {level}\n\n| kernel | ours | cc65 | sdcc | llvm-mos | hand | smallest | verdict |\n|---|---|---|---|---|---|---|---|\n"
        ));
        for (i, k) in cells.iter().enumerate().filter(|(_, k)| wanted(k)) {
            let get = |c| cache.get(k.name, c, level).unwrap_or(Bar::Missing("not built".to_owned()));
            let bars: Vec<(Compiler, Bar)> = Compiler::COMPILERS.into_iter().map(|c| (c, get(c))).collect();
            let hand_bar = if has_hand(k) { Some(get(Compiler::Hand)) } else { None };
            let ours_bar = ours.get(&(i, level)).cloned().unwrap_or(Bar::Missing("not built".to_owned()));
            let best = smallest(&bars);
            let mut all = bars.clone();
            all.extend(hand_bar.clone().map(|b| (Compiler::Hand, b)));
            let best_hand = smallest(&all);
            let v = verdict(ours_bar.bytes(), best.map(|b| b.1));
            let cell = |b: &Bar| match b {
                Bar::Have { bytes, cycles } => format!("{bytes} B, {cycles} cy"),
                Bar::Missing(why) => format!("n/a ({})", why.chars().take(60).collect::<String>()),
            };
            report.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {:?} |\n",
                k.name,
                cell(&ours_bar),
                cell(&bars[0].1),
                cell(&bars[1].1),
                cell(&bars[2].1),
                hand_bar.as_ref().map_or("-".to_owned(), cell),
                best.map_or("none".to_owned(), |(c, b)| format!("{b} B {}", c.name())),
                v
            ));
            total += 1;
            match v {
                Verdict::Within => within += 1,
                Verdict::Over(n) => over.push((
                    n,
                    format!(
                        "{} +{n} ({} B vs {} {})",
                        k.name,
                        ours_bar.bytes().unwrap(),
                        best.unwrap().1,
                        best.unwrap().0.name()
                    ),
                )),
                Verdict::NoBar => no_bar += 1,
                Verdict::NoOurs => no_ours += 1,
            }
            if verdict(ours_bar.bytes(), best_hand.map(|b| b.1)) == Verdict::Within {
                within_hand += 1;
            }
        }
        over.sort_by(|a, b| b.0.cmp(&a.0));
        counted.push(format!(
            "{level} {within}/{total} (with cc65 hand asm {within_hand}; no bar {no_bar}, ours n/a {no_ours})"
        ));
        report.push_str(&format!(
            "\nOver the bar at {level}, worst first: {}\n",
            over.iter().map(|o| o.1.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    report.push_str(&format!("\nCounted (ours bytes <= smallest of cc65, sdcc, llvm-mos): {}\n", counted.join("; ")));
    println!("{report}");
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bars.md"), &report).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_differ_by_level_where_the_compiler_has_the_setting() {
        // llvm-mos has four settings; cc65 three (O2 and Os are the same -O by
        // design: its optimizer is size-first); sdcc three (O1 and O2
        // are the default).
        let distinct = |c| {
            let mut all: Vec<_> = LEVELS.iter().map(|l| flags(c, l)).collect();
            all.sort();
            all.dedup();
            all.len()
        };
        assert_eq!(distinct(Compiler::LlvmMos), 4);
        assert_eq!(distinct(Compiler::Cc65), 3);
        assert_eq!(distinct(Compiler::Sdcc), 3);
        assert_ne!(flags(Compiler::Cc65, "O3"), flags(Compiler::Cc65, "O2"));
        assert_ne!(flags(Compiler::Sdcc, "Os"), flags(Compiler::Sdcc, "O3"));
        assert_eq!(flags(Compiler::LlvmMos, "O1"), ["-O1"]);
    }

    /// A bar that failed to build once counted as 0 bytes and "beat"
    /// everything.
    #[test]
    fn a_missing_external_is_missing_not_a_zero_byte_bar() {
        let failed = Bar::of(Err("sdcc: error".to_owned()));
        assert_eq!(failed, Bar::Missing("sdcc: error".to_owned()));
        let zero = Bar::of(Ok(super::super::Row { bytes: 0, body: 0, cycles: 5, instructions: None }));
        assert!(matches!(zero, Bar::Missing(_)));
        let bars = [(Compiler::Cc65, failed), (Compiler::Sdcc, Bar::Have { bytes: 9, cycles: 1 })];
        assert_eq!(smallest(&bars), Some((Compiler::Sdcc, 9)));
        assert_eq!(smallest(&[(Compiler::Cc65, Bar::Missing("x".to_owned()))]), None);
        assert_eq!(verdict(Some(3), None), Verdict::NoBar);
        assert_eq!(verdict(None, Some(3)), Verdict::NoOurs);
        assert_eq!(verdict(Some(10), Some(9)), Verdict::Over(1));
        assert_eq!(verdict(Some(9), Some(9)), Verdict::Within);
    }

    #[test]
    fn the_cache_round_trips_and_drops_a_row_built_with_other_flags() {
        let mut cache = Cache::default();
        cache.put("add8", Compiler::Sdcc, "Os", Bar::Have { bytes: 12, cycles: 30 });
        cache.put("add8", Compiler::Cc65, "O1", Bar::Missing("cc65: boom".to_owned()));
        let again = Cache::parse(&cache.render());
        assert_eq!(again.get("add8", Compiler::Sdcc, "Os"), Some(Bar::Have { bytes: 12, cycles: 30 }));
        assert_eq!(again.get("add8", Compiler::Cc65, "O1"), Some(Bar::Missing("cc65: boom".to_owned())));
        assert_eq!(again.get("add8", Compiler::Sdcc, "O3"), None);
        let stale = cache.render().replace("--opt-code-size", "--opt-code-speed");
        assert_eq!(Cache::parse(&stale).get("add8", Compiler::Sdcc, "Os"), None);
    }
}
