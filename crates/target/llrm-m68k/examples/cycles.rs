//! `cycles [SET]`: the kernels of `bench/<SET>` (`kernels`, or `library`)
//! compiled by this compiler (default and size), run under `qemu-m68k`, and
//! the clocks of the executed instructions inside `k` added up from
//! `llrm_m68k::timing` for the processors of `CYCLES_CPUS` (default the 68000
//! and the 68020, cache case). Needs
//! `$GCC_M68K` (the unpacked m68k binutils, for `objdump`: the pin's
//! `llvm-objdump` shows `addq` and `tst` as `<unknown>`) and `qemu-m68k`.
//! `cycles --exe PATH...`: prices each prebuilt ELF: every executed instruction
//! outside the functions `h_*` and `_start` (`main` included), per processor of
//! `CYCLES_CPUS`, and per call (`CYCLES_CALLS`, default 3000). qemu runs
//! `CYCLES_QEMU_CPU` (default `m68020` for a path containing `68020`, else
//! `m68000`). The clocks are the tables' instruction clocks, not bus cycles.
//! A `*` marks a total that holds a data-dependent term at its lower bound;
//! `<=` one whose only such terms are published maxima (multiply, divide): the
//! true total is at most it; `?` counts executed instructions the tables do not
//! cover.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::isa::Cpu as Model;
use llrm_m68k::kernels::{data, library_data, library_table, program_with_data, source_in, table};
use llrm_m68k::object::{assemble, executable, write_executable};
use llrm_m68k::timing::{DataRegisters, cost_on, cost_on_counted, from_gnu};

// An explicit path: matrix.rs includes this file as a module, where `mod
// tempdir;` would look beside it.
#[path = "tempdir/mod.rs"]
mod tempdir;

/// Executed instructions read before a run is called runaway.
const CAP: usize = 2_000_000;

/// `CYCLES_CPUS=68000,68030,68060`: the processors to price for (default 68000
/// and 68020).
pub(crate) fn models() -> Vec<Model> {
    let list = std::env::var("CYCLES_CPUS").unwrap_or_else(|_| "68000,68020".to_owned());
    list.split(',')
        .map(|one| match one.trim() {
            "68000" => Model::M68000,
            "68010" => Model::M68010,
            "68020" => Model::M68020,
            "68030" => Model::M68030,
            "68040" => Model::M68040,
            "68060" => Model::M68060,
            other => panic!("CYCLES_CPUS: no processor {other}"),
        })
        .collect()
}

/// The program counters qemu executed, in order; `complete` is false when the
/// run was cut short (the time limit or `cap`), so the counts are a prefix.
struct Trace {
    pcs: Vec<u32>,
    complete: bool,
}

/// `CYCLES_TIMEOUT`: seconds a traced run may take. The trace runs at about
/// 2.5 M instructions per second, so a long program needs this raised.
fn time_limit() -> std::time::Duration {
    std::time::Duration::from_secs(std::env::var("CYCLES_TIMEOUT").ok().and_then(|s| s.parse().ok()).unwrap_or(120))
}

fn trace(
    exe: &Path,
    cap: usize,
    cpu: &str,
) -> Trace {
    let mut command = Command::new("qemu-m68k");
    command.args(["-cpu", cpu, "-one-insn-per-tb", "-d", "exec", "-D", "/dev/stderr"]).arg(exe);
    read_trace(command, cap, time_limit())
}

/// Run `command`, hand each stderr line to `on_line` until it returns false,
/// and say whether the run ended by itself: false when `on_line` stopped it or
/// `limit` killed it.
fn run_logged(
    mut command: Command,
    limit: std::time::Duration,
    mut on_line: impl FnMut(&str) -> bool,
) -> bool {
    use std::io::{BufRead, BufReader};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    let mut child = command.stderr(Stdio::piped()).spawn().expect("the traced command starts");
    let stderr = child.stderr.take().unwrap();
    let child = Arc::new(Mutex::new(child));
    let timed_out = Arc::new(AtomicBool::new(false));
    let (finished, watch) = mpsc::channel::<()>();
    let watchdog = {
        let (child, timed_out) = (child.clone(), timed_out.clone());
        std::thread::spawn(move || {
            if watch.recv_timeout(limit) == Err(mpsc::RecvTimeoutError::Timeout) {
                timed_out.store(true, Ordering::SeqCst);
                child.lock().unwrap().kill().ok();
            }
        })
    };
    let mut stopped = false;
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if !on_line(&line) {
            child.lock().unwrap().kill().ok();
            stopped = true;
            break;
        }
    }
    child.lock().unwrap().wait().ok();
    finished.send(()).ok();
    watchdog.join().ok();
    !stopped && !timed_out.load(Ordering::SeqCst)
}

/// The pc of a `Trace` line of qemu's `exec` log.
fn trace_pc(line: &str) -> Option<u32> {
    let fields = line.split_once('[').and_then(|(_, rest)| rest.split_once(']'))?;
    fields.0.split('/').nth(1).and_then(|pc| u32::from_str_radix(pc, 16).ok())
}

/// Run `command` and collect the pcs of its stderr `exec` lines. A run killed
/// at `limit` used to be priced as if it had finished: the jpeg_single cells
/// of 11-09 were prefixes of a 516 M-instruction run (224 M to 311 M, by load).
fn read_trace(
    command: Command,
    cap: usize,
    limit: std::time::Duration,
) -> Trace {
    let mut pcs = Vec::new();
    let complete = run_logged(command, limit, |line| {
        pcs.extend(trace_pc(line));
        pcs.len() <= cap
    });
    Trace { pcs, complete }
}

/// The places each execution of a shift by a register moved, in execution
/// order: `(pc, count register modulo 64)`. The pc trace cannot say what a
/// count register held when it was loaded from a stack slot, so a second run
/// logs the cpu state at these pcs only (`-dfilter`). `shifts` maps each such
/// pc to its count register.
fn shift_counts(
    exe: &Path,
    cpu: &str,
    shifts: &HashMap<u32, usize>,
) -> Vec<(u32, u32)> {
    if shifts.is_empty() {
        return Vec::new();
    }
    let ranges: Vec<String> = shifts.keys().map(|pc| format!("0x{pc:x}+2")).collect();
    let mut command = Command::new("qemu-m68k");
    command
        .args(["-cpu", cpu, "-one-insn-per-tb", "-d", "exec,cpu", "-dfilter", &ranges.join(","), "-D", "/dev/stderr"])
        .arg(exe);
    read_shift_counts(command, shifts, time_limit())
}

fn read_shift_counts(
    command: Command,
    shifts: &HashMap<u32, usize>,
    limit: std::time::Duration,
) -> Vec<(u32, u32)> {
    let mut counts = Vec::new();
    let mut pending: Option<u32> = None;
    run_logged(command, limit, |line| {
        if let Some(pc) = trace_pc(line) {
            pending = Some(pc);
        } else if let (Some(pc), Some((name, value))) = (pending, line.split_once(" = ")) {
            let register = shifts.get(&pc).copied().unwrap_or(usize::MAX);
            if name == format!("D{register}") {
                let value = value.split_whitespace().next().and_then(|hex| u32::from_str_radix(hex, 16).ok());
                counts.extend(value.map(|value| (pc, value % 64)));
                pending = None;
            }
        }
        true
    });
    counts
}

/// A 16-bit multiply by a register or memory source: `(destination data
/// register, instruction bytes, signed)`.
type Multiplies = HashMap<u32, (usize, usize, bool)>;

/// `n` of each execution of a 16-bit multiply (`mulu.w`/`muls.w`), in execution
/// order: `(pc, n)`, `None` where the state could not show it. The 68000 prices
/// the multiply by its source (38 + 2n clocks), which may sit in memory, so a
/// second run logs the cpu state at each multiply and at the instruction after
/// it: the destination's low word before, the product after, give the source.
fn multiply_counts(
    exe: &Path,
    cpu: &str,
    multiplies: &Multiplies,
) -> Vec<(u32, Option<u32>)> {
    if multiplies.is_empty() {
        return Vec::new();
    }
    let ranges: Vec<String> = multiplies
        .iter()
        .flat_map(|(pc, (_, len, _))| [format!("0x{pc:x}+2"), format!("0x{:x}+2", pc + *len as u32)])
        .collect();
    let mut command = Command::new("qemu-m68k");
    command
        .args(["-cpu", cpu, "-one-insn-per-tb", "-d", "exec,cpu", "-dfilter", &ranges.join(","), "-D", "/dev/stderr"])
        .arg(exe);
    read_multiply_counts(command, multiplies, time_limit())
}

fn read_multiply_counts(
    command: Command,
    multiplies: &Multiplies,
    limit: std::time::Duration,
) -> Vec<(u32, Option<u32>)> {
    let mut entries: Vec<(u32, [Option<u32>; 8])> = Vec::new();
    run_logged(command, limit, |line| {
        if let Some(pc) = trace_pc(line) {
            entries.push((pc, [None; 8]));
        } else if let (Some(last), Some((name, value))) = (entries.last_mut(), line.split_once(" = ")) {
            let register = name.strip_prefix('D').and_then(|n| n.parse::<usize>().ok()).filter(|n| *n < 8);
            if let Some(register) = register {
                last.1[register] = value.split_whitespace().next().and_then(|hex| u32::from_str_radix(hex, 16).ok());
            }
        }
        true
    });
    let mut counts = Vec::new();
    for (index, (pc, state)) in entries.iter().enumerate() {
        let Some(&(register, len, signed)) = multiplies.get(pc) else { continue };
        let ones = entries
            .get(index + 1)
            .filter(|next| next.0 == pc + len as u32)
            .and_then(
                |next| {
                    let before = state[register]? & 0xffff;
                    let product = next.1[register]?;
                    if before == 0 {
                        return None; // the product is 0 whatever the source was
                    }
                    let source = if signed {
                        ((product as i32) / i32::from(before as u16 as i16)) as u32 & 0xffff
                    } else {
                        product / before
                    };
                    Some(llrm_m68k::timing::word_ones(signed, source))
                },
            );
        counts.push((*pc, ones));
    }
    counts
}

/// `CYCLES_EXE=program CYCLES_FUNCS=a,b,c`: price a whole program (a
/// difftest seed's `ours`) over those functions instead of the kernel `k`.
/// The matrix names `k` itself: its tables do not move with the environment.
fn functions() -> Vec<String> {
    std::env::var("CYCLES_FUNCS")
        .map_or_else(|_| vec!["k".to_owned()], |list| list.split(',').map(str::to_owned).collect())
}

/// `(address, text)` of every instruction of `objdump -d`, and the address
/// ranges of the priced functions.
fn listing(
    gcc: &Path,
    exe: &Path,
    harness: bool,
    wanted: &[String],
) -> (Vec<(u32, String)>, Vec<std::ops::Range<u32>>) {
    let out = Command::new(gcc.join("usr/bin/m68k-linux-gnu-objdump"))
        .env("LD_LIBRARY_PATH", gcc.join("usr/lib/x86_64-linux-gnu"))
        // An executable linked by gcc says `m68k:68000` whatever the code: the
        // instructions from the 68020 on (`extb.l`, `muls.l`) are `.short` and
        // priced as nothing. The widest decoder reads them all.
        .args(["-m", "m68k:68040", "-d", "--no-show-raw-insn"])
        .arg(exe)
        .output()
        .expect("m68k-linux-gnu-objdump runs");
    parse_listing(&String::from_utf8_lossy(&out.stdout), harness, wanted)
}

/// `harness`: every function but the test harness's own helpers (`h_*`) and
/// the entry stub. `main` is priced: the optimizer inlines the hot functions
/// of a small program into it, and its clocks are the program's.
fn is_harness_function(name: &str) -> bool {
    name.starts_with("h_") || name == "_start"
}

fn parse_listing(
    text: &str,
    harness: bool,
    wanted: &[String],
) -> (Vec<(u32, String)>, Vec<std::ops::Range<u32>>) {
    let mut lines = Vec::new();
    let mut ranges = Vec::new();
    let mut open: Option<u32> = None;
    for line in text.lines() {
        if let Some((address, name)) = line.split_once(" <").and_then(|(a, rest)| Some((a, rest.strip_suffix(">:")?))) {
            let address = u32::from_str_radix(address, 16).unwrap();
            if let Some(start) = open.take() {
                ranges.push(start..address);
            }
            let outside = harness && !is_harness_function(name);
            if outside || wanted.iter().any(|one| one == name) {
                open = Some(address);
            }
            continue;
        }
        let Some((address, text)) = line.split_once(':') else { continue };
        let Ok(address) = u32::from_str_radix(address.trim(), 16) else { continue };
        lines.push((address, from_gnu(&text.trim().replace('\t', " ")).unwrap_or_else(|why| why)));
    }
    if let Some(start) = open {
        ranges.push(start..lines.last().map_or(0, |last| last.0 + 2));
    }
    (lines, ranges)
}

pub(crate) struct Total {
    pub(crate) insns: usize,
    pub(crate) clocks: Vec<u64>,
    pub(crate) partial: bool,
    /// A partial term that is not a published maximum (a shift count at its
    /// lower bound...): the total is no upper bound.
    pub(crate) open_above: bool,
    /// The instructions that made `open_above` (an unnamed lower bound is an instrument gap), by text, with the times
    /// each ran.
    pub(crate) open_terms: std::collections::BTreeMap<String, u64>,
    /// Executed instructions with no disassembly: unpriced on every model.
    pub(crate) unknown: usize,
    /// Executed instructions one model has no row for (a 68020 `extb.l` on the
    /// 68000), per model: only that model's column is partial.
    pub(crate) unpriced: Vec<usize>,
    /// The trace was cut short: every count is a prefix, not a result.
    pub(crate) truncated: bool,
}

impl Total {
    pub(crate) fn new(models: &[Model]) -> Total {
        Total {
            insns: 0,
            clocks: vec![0; models.len()],
            partial: false,
            open_above: false,
            open_terms: std::collections::BTreeMap::new(),
            unknown: 0,
            unpriced: vec![0; models.len()],
            truncated: false,
        }
    }

    /// Charge one executed instruction to every model; a model with no row
    /// for it counts it unpriced and keeps its own mark.
    pub(crate) fn charge(
        &mut self,
        models: &[Model],
        text: &str,
        bytes: u32,
        taken: bool,
        places: Option<u32>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        for (slot, cpu) in models.iter().copied().enumerate() {
            match cost_on_counted(cpu, text, bytes, taken, places) {
                Ok(one) => {
                    self.clocks[slot] += u64::from(one.clocks);
                    self.partial |= one.partial;
                    self.open_above |= one.partial && !one.maximum;
                    if one.partial && !one.maximum {
                        *self.open_terms.entry(text.to_owned()).or_default() += 1;
                    }
                }
                Err(why) => {
                    assert!(!why.starts_with(llrm_m68k::timing::UNPARSEABLE), "{why}");
                    self.unpriced[slot] += 1;
                    if seen.insert(why.clone()) {
                        eprintln!("unpriced: {why}");
                    }
                }
            }
        }
    }

    /// [`Self::charge`] for the next instruction of a trace: a shift by a
    /// register is priced at `counted` places (what the cpu state held when it
    /// ran), else at the places its count register holds when the trace fixed
    /// them.
    pub(crate) fn charge_executed(
        &mut self,
        models: &[Model],
        registers: &mut DataRegisters,
        text: &str,
        bytes: u32,
        taken: bool,
        counted: Option<u32>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        self.charge(models, text, bytes, taken, counted.or_else(|| registers.places_of(text)), seen);
        registers.execute(text);
    }

    /// The table cell of the model in `slot`: its clocks, `*` for a bound,
    /// ` ?n` for unpriced instructions; `cut` when the trace was cut short.
    pub(crate) fn cell(
        &self,
        slot: usize,
    ) -> String {
        if self.truncated {
            return format!("cut@{}", self.insns);
        }
        let (before, after) = match (self.partial, self.open_above) {
            (false, _) => ("", ""),
            (true, true) => ("", "*"),
            (true, false) => ("<=", ""),
        };
        let unknown = if self.unknown_of(slot) > 0 { format!(" ?{}", self.unknown_of(slot)) } else { String::new() };
        format!("{before}{}{after}{unknown}", self.clocks[slot])
    }

    /// The instructions behind a `*`, most often run first: `muls.l %d1, %d0 x4; bfextu (%a0){0:8}, %d1 x2`. Empty for a
    /// total with no open lower bound.
    pub(crate) fn open_terms_line(&self) -> String {
        let mut terms: Vec<(&String, &u64)> = self.open_terms.iter().collect();
        terms.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        terms.iter().take(4).map(|(text, times)| format!("{text} x{times}")).collect::<Vec<_>>().join("; ")
    }

    /// The unpriced instructions of the model in `slot`.
    pub(crate) fn unknown_of(
        &self,
        slot: usize,
    ) -> usize {
        self.unknown + self.unpriced[slot]
    }
}

/// The qemu CPU a run of `exe` is traced on: the one its path names, the 68000
/// when none. A trace on the wrong CPU counts the instructions of another
/// program (the 040 and 060 matrix columns were traced on the 68000).
fn qemu_cpu_of(exe: &Path) -> &'static str {
    let name = exe.to_string_lossy();
    ["68060", "68040", "68030", "68020", "68010"]
        .into_iter()
        .find(|cpu| name.contains(cpu))
        .map_or(
            "m68000",
            |cpu| match cpu {
                "68060" => "m68060",
                "68040" => "m68040",
                "68030" => "m68030",
                "68020" => "m68020",
                _ => "m68010",
            },
        )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use llrm_m68k::isa::Cpu as Model;
    use llrm_m68k::timing::DataRegisters;

    use super::{Total, parse_listing, qemu_cpu_of, read_shift_counts, read_trace};

    /// A small program's hot loop is inlined into `main`; the default priced
    /// everything but `main`, so a 100-iteration loop read as 0 clocks and an
    /// A/B of two compilers as "same" (rep_tails 68020 -O2, 2026-11-09).
    #[test]
    fn main_is_priced_with_the_functions_inlined_into_it() {
        let text = "00000100 <_start>:\n 100:\tjsr 0x120\n00000120 <main>:\n 120:\taddql #1,%d0\n 122:\tbnes 0x120\n 124:\trts\n00000130 <h_report>:\n 130:\trts\n00000140 <sum>:\n 140:\trts\n";
        let (_, ranges) = parse_listing(text, true, &[]);
        assert_eq!(ranges, vec![0x120..0x130, 0x140..0x142]);
    }

    /// The 040 and 060 matrix columns priced the instruction counts of a trace
    /// on `m68000` (partition 040: 202 instructions for gcc's 121).
    #[test]
    fn a_run_is_traced_on_the_cpu_its_path_names() {
        for (path, cpu) in [
            ("sum_68000_O2_ours.elf", "m68000"),
            ("sum_68020_O2_ours.elf", "m68020"),
            ("sum_68040_O2_ours.elf", "m68040"),
            ("sum_68060_Os_gcc.elf", "m68060"),
        ] {
            assert_eq!(qemu_cpu_of(Path::new(path)), cpu, "{path}");
        }
    }

    /// 68020-only `extb.l` was marked unpriced on the 68020 column too
    /// (`?16` there, though the 68020 has a row for it).
    #[test]
    fn an_instruction_one_model_lacks_marks_only_that_models_column() {
        let models = [Model::M68000, Model::M68020];
        let mut total = Total::new(&models);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..16 {
            total.charge(&models, "extb.l %d2", 2, false, None, &mut seen);
        }
        total.charge(&models, "addq.l #1, %d0", 2, false, None, &mut seen);
        assert_eq!(total.unknown_of(0), 16, "68000 lacks extb.l");
        assert_eq!(total.unknown_of(1), 0, "68020 prices extb.l");
        assert!(total.clocks[1] > total.clocks[0]);
    }

    /// memmove_overlap 68020 O2: two `muls.l` at their maximum read `4668*`,
    /// a lower-bound mark on an upper bound, and the cell went blank.
    #[test]
    fn a_total_of_only_maxima_is_marked_as_an_upper_bound() {
        let models = [Model::M68020];
        let run = |trace: &[&str]| {
            let mut total = Total::new(&models);
            let mut registers = DataRegisters::new();
            let mut seen = std::collections::HashSet::new();
            for text in trace {
                total.charge_executed(&models, &mut registers, text, 4, false, None, &mut seen);
            }
            total
        };
        assert!(run(&["moveq #27, %d1", "muls.l %d1, %d0"]).cell(0).starts_with("<="));
        assert!(
            run(&["muls.l %d1, %d0", "bfextu (%a0){0:8}, %d1"]).cell(0).ends_with('*'),
            "a bit-field bound is no upper bound"
        );
    }

    /// A `*` named no instruction, so a `>=` cell could not be told from a cut run (register_upper_half 68020 -Os: ours
    /// `>=4524`, gcc `>=4137`, undecidable). The open terms are listed by text with their counts.
    #[test]
    fn an_open_lower_bound_names_its_instructions() {
        let models = [Model::M68020];
        let mut total = Total::new(&models);
        let mut registers = DataRegisters::new();
        let mut seen = std::collections::HashSet::new();
        for text in ["muls.l %d1, %d0", "bfextu (%a0){0:8}, %d1", "bfextu (%a0){0:8}, %d1"] {
            total.charge_executed(&models, &mut registers, text, 4, false, None, &mut seen);
        }
        assert!(total.cell(0).ends_with('*'));
        assert_eq!(total.open_terms_line(), "bfextu (%a0){0:8}, %d1 x2");
        let mut closed = Total::new(&models);
        closed.charge_executed(&models, &mut registers, "moveq #27, %d1", 2, false, None, &mut seen);
        assert_eq!(closed.open_terms_line(), "");
    }

    /// bitcount_idioms 68000 O1 priced `lsr.l %d2,%d0` (`d2` = 16, run 8 times)
    /// at zero places: 8 clocks where the table says 8 + 2 x 16 = 40, so the
    /// parked parity change read +1504 clocks instead of +1760.
    #[test]
    fn a_shift_by_a_register_the_trace_set_is_priced_at_its_places() {
        let models = [Model::M68000];
        let run = |trace: &[&str]| {
            let mut total = Total::new(&models);
            let mut registers = DataRegisters::new();
            let mut seen = std::collections::HashSet::new();
            for text in trace {
                total.charge_executed(&models, &mut registers, text, 2, false, None, &mut seen);
            }
            total
        };
        let known = run(&["moveq #16,%d2", "move.l %d1, %d0", "lsr.l %d2, %d0"]);
        assert_eq!(known.clocks[0], 4 + 4 + 8 + 2 * 16);
        assert!(!known.partial, "the count is known");
        let copied = run(&["move.l %d1, %d2", "lsr.l %d2, %d0"]);
        assert_eq!(copied.clocks[0], 4 + 8);
        assert!(copied.partial, "an unknown count stays a marked bound");
        let after_call = run(&["moveq #16,%d2", "jsr 0x1000", "lsr.l %d2, %d0"]);
        assert!(after_call.partial, "a call may write any register");
    }

    /// crc 68000 -O2 was priced +20% (10428 -> 12516 clocks) for a change that
    /// makes the count of a shift a `moveq` instead of a reload from the
    /// stack: the reloaded count was priced at 0 places, the `moveq` one at
    /// its 31. Both run the same shifts.
    #[test]
    fn a_shift_count_the_cpu_state_gave_is_priced_whatever_set_the_register() {
        let models = [Model::M68000];
        let run = |trace: &[&str], counted: Option<u32>| {
            let mut total = Total::new(&models);
            let mut registers = DataRegisters::new();
            let mut seen = std::collections::HashSet::new();
            for text in trace {
                total.charge_executed(&models, &mut registers, text, 2, false, counted, &mut seen);
            }
            total
        };
        let shift = run(&["asr.l %d5, %d0"], Some(31));
        assert_eq!(shift.clocks[0], 8 + 2 * 31);
        let reload = run(&["move.l (76,%sp), %d5", "asr.l %d5, %d0"], Some(31));
        assert!(!reload.partial, "the count is known");
        assert_eq!(reload.clocks[0] - shift.clocks[0], run(&["move.l (76,%sp), %d5"], None).clocks[0]);
        assert!(run(&["move.l (76,%sp), %d5", "asr.l %d5, %d0"], None).partial, "without the state it is a bound");
    }

    /// The second run logs the cpu state at the shifts only; each record is a
    /// `Trace` line, then `D<n> = value` among the other registers.
    #[test]
    fn the_cpu_state_at_a_shift_gives_the_count_register_modulo_64() {
        let log = "echo 'Trace 0: 0x7f00 [00000000/00001000/00000000/0]' >&2; \
            echo 'D0 = 00000001   A0 = 00000000' >&2; echo 'D5 = 0000005f   A5 = 00000000' >&2; \
            echo 'Trace 0: 0x7f00 [00000000/00001000/00000000/0]' >&2; echo 'D5 = 0000001a   A5 = 0' >&2";
        let mut command = std::process::Command::new("sh");
        command.args(["-c", log]);
        let shifts = std::collections::HashMap::from([(0x1000, 5)]);
        let counts = read_shift_counts(command, &shifts, std::time::Duration::from_secs(30));
        assert_eq!(counts, [(0x1000, 0x5f % 64), (0x1000, 0x1a)]);
    }

    /// gcc's `muls.w (%a0)+, %d3` priced at the 74-clock bound, so
    /// `loop_versioning` -O3 read `126952*` and `m68k_clk.py` left the cell
    /// empty. The destination's low word before and the product after give
    /// the source: 6 x -5 = -30.
    #[test]
    fn the_state_around_a_multiply_gives_the_ones_of_its_source() {
        let log = "echo 'Trace 0: 0x7f00 [00000000/00001000/00000000/0]' >&2; echo 'D3 = 00000006   A3 = 0' >&2; \
            echo 'Trace 0: 0x7f00 [00000000/00001002/00000000/0]' >&2; echo 'D3 = ffffffe2   A3 = 0' >&2; \
            echo 'Trace 0: 0x7f00 [00000000/00001000/00000000/0]' >&2; echo 'D3 = 00000000   A3 = 0' >&2; \
            echo 'Trace 0: 0x7f00 [00000000/00001002/00000000/0]' >&2; echo 'D3 = 00000000   A3 = 0' >&2";
        let mut command = std::process::Command::new("sh");
        command.args(["-c", log]);
        let multiplies = std::collections::HashMap::from([(0x1000, (3, 2, true))]);
        let counts = super::read_multiply_counts(command, &multiplies, std::time::Duration::from_secs(30));
        // -5 = 0xfffb, a 0 appended: 01/10 transitions at the bottom and at
        // both ends of the 0 bit: 3
        assert_eq!(counts, [(0x1000, Some(3)), (0x1000, None)], "a zero destination fixes no source");
    }

    /// jpeg_single's trace (516 M instructions, 200 s) was killed at the 120 s
    /// limit and priced as a result: 224 M to 311 M instructions, by load.
    #[test]
    fn a_run_cut_by_the_time_limit_is_not_complete() {
        let line = "Trace 0: 0x7f00 [00000000/00001000/00000000/0]";
        let run = |script: String, limit: u64| {
            let mut command = std::process::Command::new("sh");
            command.args(["-c", &script]);
            read_trace(command, usize::MAX, std::time::Duration::from_millis(limit))
        };
        let cut = run(format!("echo '{line}' >&2; exec sleep 30"), 300);
        assert!(!cut.complete);
        assert_eq!(cut.pcs, [0x1000]);
        let whole = run(format!("echo '{line}' >&2"), 30_000);
        assert!(whole.complete);
        assert_eq!(whole.pcs, [0x1000]);
    }
}

pub(crate) fn price(
    gcc: &Path,
    exe: &Path,
    models: &[Model],
    harness: bool,
    wanted: &[String],
) -> Total {
    let (lines, ranges) = listing(gcc, exe, harness, wanted);
    let at: HashMap<u32, (usize, &str)> = lines
        .iter()
        .enumerate()
        .map(|(index, (address, text))| {
            (*address, (lines.get(index + 1).map_or(2, |next| next.0 - address) as usize, text.as_str()))
        })
        .collect();
    let cpu = std::env::var("CYCLES_QEMU_CPU").unwrap_or_else(|_| qemu_cpu_of(exe).to_owned());
    // `CYCLES_CAP`: the executed instructions read before a run is called
    // runaway
    let cap = std::env::var("CYCLES_CAP")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(
            if harness {
                200_000_000
            } else {
                CAP
            },
        );
    let Trace { pcs, complete } = trace(exe, cap, &cpu);
    // the 68020 and later price a register shift by its size alone
    let shifts: HashMap<u32, usize> = if models.iter().any(|m| matches!(m, Model::M68000 | Model::M68010)) {
        at.iter().filter_map(|(pc, (_, text))| Some((*pc, DataRegisters::count_register(text)?))).collect()
    } else {
        HashMap::new()
    };
    let mut counted = shift_counts(exe, &cpu, &shifts).into_iter();
    let multiplies: Multiplies = if models.iter().any(|m| matches!(m, Model::M68000 | Model::M68010)) {
        at.iter()
            .filter_map(|(pc, (len, text))| {
                let (register, signed) = DataRegisters::multiply_destination(text)?;
                Some((*pc, (register, *len, signed)))
            })
            .collect()
    } else {
        HashMap::new()
    };
    let mut multiplied = multiply_counts(exe, &cpu, &multiplies).into_iter();
    let mut total = Total::new(models);
    total.truncated = !complete;
    let mut seen = std::collections::HashSet::new();
    // CYCLES_LISTING=1: each distinct executed instruction, its count and
    // clocks
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    let mut registers = DataRegisters::new();
    let mut after_gap = false;
    for (index, pc) in pcs.iter().enumerate() {
        // the next logged execution of a counted shift is this one; a pc that
        // differs means the two runs diverged, and the rest is priced as before
        let places = if shifts.contains_key(pc) {
            match counted.next() {
                Some((at, places)) if at == *pc => Some(places),
                Some(_) => {
                    counted = Vec::new().into_iter();
                    None
                }
                None => None,
            }
        } else if multiplies.contains_key(pc) {
            match multiplied.next() {
                Some((at, ones)) if at == *pc => ones,
                Some(_) => {
                    multiplied = Vec::new().into_iter();
                    None
                }
                None => None,
            }
        } else {
            None
        };
        if !ranges.iter().any(|range| range.contains(pc)) {
            // code the listing does not cover may write any register
            after_gap = true;
            continue;
        }
        if std::mem::take(&mut after_gap) {
            registers.forget_all();
        }
        total.insns += 1;
        *counts.entry(*pc).or_default() += 1;
        let Some((bytes, text)) = at.get(pc) else {
            total.unknown += 1;
            registers.forget_all();
            continue;
        };
        let taken = pcs.get(index + 1).is_some_and(|next| *next != pc + *bytes as u32);
        total.charge_executed(models, &mut registers, text, *bytes as u32, taken, places, &mut seen);
    }
    if std::env::var_os("CYCLES_LISTING").is_some() {
        for (pc, count) in &counts {
            if let Some((bytes, text)) = at.get(pc) {
                let each: Vec<String> = models
                    .iter()
                    .map(|cpu| format!("{cpu:?} {}", cost_on(*cpu, text, *bytes as u32, false).map_or(0, |c| c.clocks)))
                    .collect();
                eprintln!("{pc:x} x{count} {text}  [{}]", each.join(" / "));
            }
        }
    }
    total
}

/// `CYCLES_LLC=llrm-m68k-llc`: the code of `k` is that driver's assembly for
/// the kernel's IR, assembled by the gas of `$GCC_M68K`, instead of this
/// crate's own output. Leaf code only: a relocation is not resolved.
fn assembled(
    llc: &Path,
    gcc: &Path,
    ir: &str,
    size: bool,
    dir: &Path,
) -> Option<Vec<u8>> {
    let input = dir.join("llc.ll");
    std::fs::write(&input, ir).unwrap();
    let asm = dir.join("llc.s");
    let status = Command::new(llc)
        .args(["-S", "-mcpu=68000", if size { "-Os" } else { "-O2" }, "-o"])
        .arg(&asm)
        .arg(&input)
        .status()
        .unwrap();
    assert!(status.success(), "the llc failed");
    let text = format!(".text\n{}", std::fs::read_to_string(&asm).unwrap());
    let source = dir.join("llc_in.s");
    std::fs::write(&source, text).unwrap();
    let object = dir.join("llc.o");
    let bin = |tool: &str| gcc.join("usr/bin").join(format!("m68k-linux-gnu-{tool}"));
    let run = |tool: &str, args: Vec<&std::ffi::OsStr>| {
        let status = Command::new(bin(tool))
            .env("LD_LIBRARY_PATH", gcc.join("usr/lib/x86_64-linux-gnu"))
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "{tool} failed");
    };
    run("as", vec!["-m68000".as_ref(), source.as_os_str(), "-o".as_ref(), object.as_os_str()]);
    let relocations = Command::new(bin("objdump"))
        .env("LD_LIBRARY_PATH", gcc.join("usr/lib/x86_64-linux-gnu"))
        .arg("-r")
        .arg(&object)
        .output()
        .unwrap();
    if String::from_utf8_lossy(&relocations.stdout).contains("R_68K") {
        return None;
    }
    let raw = dir.join("llc.bin");
    run(
        "objcopy",
        vec!["-O".as_ref(), "binary".as_ref(), "-j".as_ref(), ".text".as_ref(), object.as_os_str(), raw.as_os_str()],
    );
    Some(std::fs::read(raw).unwrap())
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--stamp") {
        println!("{}", env!("LLRM_M68K_STAMP"));
        return;
    }
    let gcc = PathBuf::from(std::env::var("GCC_M68K").expect("GCC_M68K: the unpacked m68k binutils root"));
    let dir = tempdir::Scratch::new("cycles");
    let models = models();
    if std::env::args().nth(1).as_deref() == Some("--exe") {
        let calls: f64 = std::env::var("CYCLES_CALLS").ok().and_then(|n| n.parse().ok()).unwrap_or(3000.0);
        let heads: Vec<String> = models
            .iter()
            .map(|cpu| {
                let name = format!("{cpu:?}");
                format!("{0} clk | {0} clk/call", name.trim_start_matches('M'))
            })
            .collect();
        println!("| exe | insns | {} |", heads.join(" | "));
        for path in std::env::args().skip(2) {
            let total = price(&gcc, Path::new(&path), &models, true, &functions());
            let cells: Vec<String> = (0..models.len())
                .map(|slot| {
                    let per_call = if total.truncated {
                        "-".to_owned()
                    } else {
                        format!("{:.1}", total.clocks[slot] as f64 / calls)
                    };
                    format!("{} | {per_call}", total.cell(slot))
                })
                .collect();
            let name = Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
            println!("| {name} | {} | {} |", total.insns, cells.join(" | "));
            if total.open_above {
                println!("open: {name}: {}", total.open_terms_line());
            }
        }
        return;
    }
    if let Some(exe) = std::env::var_os("CYCLES_EXE") {
        let total = price(&gcc, Path::new(&exe), &models, false, &functions());
        let clocks: Vec<String> = models.iter().zip(&total.clocks).map(|(cpu, clk)| format!("{cpu:?} {clk}")).collect();
        println!(
            "insns {} clocks {} unpriced {} partial {} truncated {}",
            total.insns,
            clocks.join(" "),
            (0..models.len()).map(|slot| total.unknown_of(slot)).max().unwrap_or(0),
            total.partial,
            total.truncated
        );
        return;
    }
    let set = std::env::args().nth(1).unwrap_or_else(|| "kernels".to_owned());
    let (rows, block) = if set == "library" {
        (library_table(), library_data())
    } else {
        (table().into_iter().map(|(name, args, expected)| (name, args, expected, true)).collect(), data())
    };
    let helper = (set == "library").then(|| {
        let module = llrm_mir::parse::module(&source_in(&set, "h")).unwrap();
        items(&module, "h", Cpu::M68000).unwrap()
    });
    let names: Vec<String> = models.iter().map(|cpu| format!("{cpu:?}").trim_start_matches('M').to_owned()).collect();
    let clock_heads =
        |prefix: &str| names.iter().map(|name| format!("{prefix}{name} clk")).collect::<Vec<_>>().join(" | ");
    println!("| kernel | B | insns | {} | -Os B | -Os insns | {} |", clock_heads(""), clock_heads("-Os "));
    println!("|{}", "---|".repeat(5 + 2 * models.len()));
    let llc_mode = std::env::var_os("CYCLES_LLC").is_some();
    for (name, args, expected, leaf) in rows {
        if llc_mode && !leaf {
            println!("| {name} | not a leaf: size and clocks need the runtime linked |");
            continue;
        }
        let mut text = source_in(&set, name);
        let mut runtime_names: Vec<&str> = Vec::new();
        if !leaf {
            text.push_str(source_in(&set, "rt").split_once('\n').map_or("", |(_, rest)| rest));
            runtime_names = vec!["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"];
        }
        let module = llrm_mir::parse::module(&text).unwrap();
        let runtime: Vec<Item> =
            runtime_names.iter().flat_map(|one| items(&module, one, Cpu::M68000).unwrap()).collect();
        let mut cells = Vec::new();
        for options in [Options::default(), Options { size: true, ..Options::default() }] {
            let mut code = items_with(&module, "k", Cpu::M68000, options).unwrap();
            if let (Some(llc), true) = (std::env::var_os("CYCLES_LLC"), leaf) {
                let Some(bytes) = assembled(Path::new(&llc), &gcc, &source_in(&set, name), options.size, &dir) else {
                    cells.push("calls the runtime".to_owned());
                    continue;
                };
                code = vec![Item::Label("k".to_owned()), Item::Bytes(bytes)];
            }
            let size = {
                let mut all = code.clone();
                all.extend(runtime.iter().cloned());
                assemble(&all, 0).unwrap().code.len() - assemble(&runtime, 0).map_or(0, |one| one.code.len())
            };
            code.extend(helper.clone().unwrap_or_default());
            code.extend(runtime.clone());
            let exe = dir.join(format!("{name}.elf"));
            write_executable(&exe, &executable(&program_with_data(&args, code, &block)).unwrap()).unwrap();
            if llc_mode {
                let status =
                    Command::new("timeout").args(["10", "qemu-m68k", "-cpu", "m68000"]).arg(&exe).status().unwrap();
                assert_eq!(status.code(), Some(expected), "{name}: the llc's code gives another result");
            }
            let total = price(&gcc, &exe, &models, false, &functions());
            let clocks: Vec<String> = (0..models.len()).map(|slot| total.cell(slot)).collect();
            cells.push(format!("{size} | {} | {}", total.insns, clocks.join(" | ")));
        }
        println!("| {name} | {} |", cells.join(" | "));
    }
}
