//! `cycles [SET]`: the kernels of `bench/<SET>` (`kernels`, or `library`)
//! compiled by this compiler (default and size), run under `qemu-m68k`, and
//! the clocks of the executed instructions inside `k` added up from
//! `llrm_m68k::timing` for the processors of `CYCLES_CPUS` (default the 68000
//! and the 68020, cache case). Needs
//! `$GCC_M68K` (the unpacked m68k binutils, for `objdump`: the pin's
//! `llvm-objdump` shows `addq` and `tst` as `<unknown>`) and `qemu-m68k`.
//! `cycles --exe PATH...`: prices each prebuilt ELF: every executed instruction
//! outside the functions `h_*`, `main` and `_start`, per processor of
//! `CYCLES_CPUS`, and per call (`CYCLES_CALLS`, default 3000). qemu runs
//! `CYCLES_QEMU_CPU` (default `m68020` for a path containing `68020`, else
//! `m68000`). The clocks are the tables' instruction clocks, not bus cycles.
//! A `*` marks a total that holds a data-dependent term at its bound; `?`
//! counts executed instructions the tables do not cover.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::isa::Cpu as Model;
use llrm_m68k::kernels::{data, library_data, library_table, program_with_data, source_in, table};
use llrm_m68k::object::{assemble, executable, write_executable};
use llrm_m68k::timing::{cost_on, from_gnu};
use llrm_support::hash::HashMap;

/// Executed instructions read before a run is called runaway.
const CAP: usize = 2_000_000;

/// `CYCLES_CPUS=68000,68030,68060`: the processors to price for (default 68000
/// and 68020).
fn models() -> Vec<Model> {
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

/// The program counters qemu executed, in order.
fn trace(
    exe: &Path,
    cap: usize,
    cpu: &str,
) -> Vec<u32> {
    use std::io::{BufRead, BufReader};
    let mut child = Command::new("timeout")
        .args(["120", "qemu-m68k", "-cpu", cpu, "-one-insn-per-tb", "-d", "exec", "-D", "/dev/stderr"])
        .arg(exe)
        .stderr(Stdio::piped())
        .spawn()
        .expect("timeout runs qemu-m68k");
    let mut pcs = Vec::new();
    for line in BufReader::new(child.stderr.take().unwrap()).lines().map_while(Result::ok) {
        // a trace that ends in a fault is a short program, not a cheap one
        assert!(
            !line.contains("uncaught target signal"),
            "{} faulted on {cpu} after {} instructions: {line} (CYCLES_QEMU_CPU names the CPU to run on)",
            exe.display(),
            pcs.len()
        );
        let Some(fields) = line.split_once('[').and_then(|(_, rest)| rest.split_once(']')) else { continue };
        if let Some(pc) = fields.0.split('/').nth(1).and_then(|pc| u32::from_str_radix(pc, 16).ok()) {
            pcs.push(pc);
        }
        if pcs.len() > cap {
            child.kill().ok();
            break;
        }
    }
    child.wait().ok();
    pcs
}

/// The qemu CPU an executable runs on: the newest of 68020, 68030, 68040 and
/// 68060 its path names, else the 68000.
fn qemu_cpu_for(path: &str) -> &'static str {
    ["68060", "68040", "68030", "68020"]
        .into_iter()
        .find(|tag| path.contains(tag))
        .map_or(
            "m68000",
            |tag| match tag {
                "68060" => "m68060",
                "68040" => "m68040",
                "68030" => "m68030",
                _ => "m68020",
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// gas assembled `bgt L` with a 16-bit displacement: a table priced the
    /// compiler's text 2 B a branch above its own encoder.
    #[test]
    fn a_branch_to_a_label_is_left_for_gas_to_relax() {
        assert_eq!(
            relaxed("\tbgt\t.LBB0_2\n\tbra\t.L3\n\tbsr\tf\n\tbne\t(%a0)\n"),
            "\tjgt\t.LBB0_2\n\tjra\t.L3\n\tjbsr\tf\n\tbne\t(%a0)\n"
        );
    }

    /// The 68020 table compiled with `-mcpu=68000`: its clocks were 68000 code
    /// on 68020 timing, and crc16 was judged on them.
    #[test]
    fn a_processors_table_is_compiled_and_assembled_for_it() {
        assert_eq!(driver_cpu("68020"), "-mcpu=68020");
        assert_eq!(assembler_arch("68020"), "-m68020");
    }

    use std::process::Command;

    use super::{parse_listing, qemu_cpu_for, trace};

    /// A 68020 program (`divu.l`) assembled and linked with the m68k binutils;
    /// `None` without `$GCC_M68K`.
    fn program_with_divu_l() -> Option<std::path::PathBuf> {
        let root = std::env::var("GCC_M68K").ok().filter(|root| !root.is_empty())?;
        let dir = std::env::temp_dir().join(format!("cycles-fault-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (source, object, exe) = (dir.join("p.s"), dir.join("p.o"), dir.join("p.exe"));
        std::fs::write(
            &source,
            ".globl _start\n_start:\n\tmoveq #5,%d0\n\tmoveq #3,%d1\n\tdivu.l %d1,%d0\n\tmoveq #1,%d0\n\tmoveq #0,%d1\n\ttrap #0\n",
        )
        .unwrap();
        let tool = |name: &str| {
            let mut command = Command::new(format!("{root}/usr/bin/m68k-linux-gnu-{name}"));
            command.env("LD_LIBRARY_PATH", format!("{root}/usr/lib/x86_64-linux-gnu"));
            command
        };
        assert!(tool("as").args(["-m68020", "-o"]).arg(&object).arg(&source).status().unwrap().success());
        assert!(tool("ld").arg("-o").arg(&exe).arg(&object).status().unwrap().success());
        Some(exe)
    }

    /// The 68020 program on `m68000` faulted at `divu.l`; its trace of three
    /// instructions was priced as the whole program, with no error.
    #[test]
    #[should_panic(expected = "faulted on m68000")]
    fn a_trace_that_ends_in_a_fault_is_refused() {
        let Some(exe) = program_with_divu_l() else { panic!("faulted on m68000 (skipped: no GCC_M68K)") };
        trace(&exe, 1000, "m68000");
    }

    #[test]
    fn the_same_program_traces_on_the_cpu_it_needs() {
        let Some(exe) = program_with_divu_l() else { return };
        assert!(trace(&exe, 1000, "m68020").len() >= 6);
    }

    /// A 68040 or 68060 program ran on `m68000`, faulted at its first 68020+
    /// instruction and was priced for the few it had executed (339
    /// instructions, 0.1 clock a call), with no error.
    #[test]
    fn a_path_naming_a_newer_cpu_runs_on_it() {
        assert_eq!(qemu_cpu_for("/t/f.68000"), "m68000");
        assert_eq!(qemu_cpu_for("/t/f.68020"), "m68020");
        assert_eq!(qemu_cpu_for("/t/f.68030"), "m68030");
        assert_eq!(qemu_cpu_for("/t/f.68040.exe"), "m68040");
        assert_eq!(qemu_cpu_for("/t/sf32-68060"), "m68060");
        assert_eq!(qemu_cpu_for("/t/plain"), "m68000");
    }
}

/// `CYCLES_EXE=program CYCLES_FUNCS=a,b,c`: price a whole program (a
/// difftest seed's `ours`) over those functions instead of the kernel `k`.
fn functions() -> Vec<String> {
    std::env::var("CYCLES_FUNCS")
        .map_or_else(|_| vec!["k".to_owned()], |list| list.split(',').map(str::to_owned).collect())
}

/// `(address, text)` of every instruction of `objdump -d`, and the address
/// ranges of the priced functions: `callees` besides the wanted ones.
fn listing(
    gcc: &Path,
    exe: &Path,
    harness: bool,
    callees: &[&str],
) -> (Vec<(u32, String)>, Vec<std::ops::Range<u32>>) {
    let out = Command::new(gcc.join("usr/bin/m68k-linux-gnu-objdump"))
        .env("LD_LIBRARY_PATH", gcc.join("usr/lib/x86_64-linux-gnu"))
        .args(["-d", "--no-show-raw-insn"])
        .arg(exe)
        .output()
        .expect("m68k-linux-gnu-objdump runs");
    let mut wanted = functions();
    wanted.extend(callees.iter().map(|name| (*name).to_owned()));
    parse_listing(&String::from_utf8_lossy(&out.stdout), &wanted, harness)
}

/// [`listing`] of the text `objdump -d` printed.
fn parse_listing(
    objdump: &str,
    wanted: &[String],
    harness: bool,
) -> (Vec<(u32, String)>, Vec<std::ops::Range<u32>>) {
    let mut lines = Vec::new();
    let mut ranges = Vec::new();
    let mut open: Option<u32> = None;
    for line in objdump.lines() {
        if let Some((address, name)) = line.split_once(" <").and_then(|(a, rest)| Some((a, rest.strip_suffix(">:")?))) {
            let address = u32::from_str_radix(address, 16).unwrap();
            if let Some(start) = open.take() {
                ranges.push(start..address);
            }
            // `harness`: every function but the test harness's own
            let outside = harness && !(name.starts_with("h_") || name == "main" || name == "_start");
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

struct Total {
    insns: usize,
    clocks: Vec<u64>,
    partial: bool,
    unknown: usize,
}

fn price(
    gcc: &Path,
    exe: &Path,
    models: &[Model],
    harness: bool,
    callees: &[&str],
    qemu: &str,
) -> Total {
    let (lines, ranges) = listing(gcc, exe, harness, callees);
    let at: HashMap<u32, (usize, &str)> = lines
        .iter()
        .enumerate()
        .map(|(index, (address, text))| {
            (*address, (lines.get(index + 1).map_or(2, |next| next.0 - address) as usize, text.as_str()))
        })
        .collect();
    let (cap, cpu) = if harness {
        let cpu = std::env::var("CYCLES_QEMU_CPU").unwrap_or_else(|_| qemu_cpu_for(&exe.to_string_lossy()).to_owned());
        (200_000_000, cpu)
    } else {
        (CAP, qemu.to_owned())
    };
    let pcs = trace(exe, cap, &cpu);
    let mut total = Total { insns: 0, clocks: vec![0; models.len()], partial: false, unknown: 0 };
    let mut seen = llrm_support::hash::HashSet::default();
    // CYCLES_LISTING=1: each distinct executed instruction, its count and
    // clocks
    let mut counts: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for (index, pc) in pcs.iter().enumerate() {
        if !ranges.iter().any(|range| range.contains(pc)) {
            continue;
        }
        total.insns += 1;
        *counts.entry(*pc).or_default() += 1;
        let Some((bytes, text)) = at.get(pc) else {
            total.unknown += 1;
            continue;
        };
        let taken = pcs.get(index + 1).is_some_and(|next| *next != pc + *bytes as u32);
        let mut failed = false;
        for (slot, cpu) in models.iter().copied().enumerate() {
            match cost_on(cpu, text, *bytes as u32, taken) {
                Ok(one) => {
                    total.clocks[slot] += u64::from(one.clocks);
                    total.partial |= one.partial;
                }
                Err(why) => {
                    failed = true;
                    if seen.insert(why.clone()) {
                        eprintln!("unpriced: {why}");
                    }
                }
            }
        }
        total.unknown += usize::from(failed);
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

/// `asm` with each branch to a label as the gas pseudo-branch that picks its
/// own displacement: gas assembles `bgt L` with a 16-bit displacement however
/// near L is, where this compiler's own encoder (and gcc's `jgt`) use 8 bits
/// when they fit. Priced as printed, every forward branch cost 2 B and some
/// clocks more than the code the compiler writes (max -O2 18 B against 16).
fn relaxed(asm: &str) -> String {
    const CONDITIONS: [&str; 14] = ["hi", "ls", "cc", "cs", "ne", "eq", "vc", "vs", "pl", "mi", "ge", "lt", "gt", "le"];
    asm.lines()
        .map(|line| {
            let Some((indent, rest)) = line.split_once(|c: char| !c.is_whitespace()).map(|(indent, _)| {
                let at = indent.len();
                (&line[..at], &line[at..])
            }) else {
                return line.to_owned();
            };
            let Some((mnemonic, operand)) = rest.split_once(char::is_whitespace) else { return line.to_owned() };
            let label = operand.trim().starts_with(|c: char| c == '.' || c.is_alphabetic() || c == '_');
            let pseudo = match mnemonic {
                "bra" => Some("jra".to_owned()),
                "bsr" => Some("jbsr".to_owned()),
                _ => mnemonic
                    .strip_prefix('b')
                    .filter(|condition| CONDITIONS.contains(condition))
                    .map(|condition| format!("j{condition}")),
            };
            match pseudo {
                Some(pseudo) if label => format!("{indent}{pseudo}\t{}", operand.trim()),
                _ => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The driver's flag for the processor a table is for: the code is compiled
/// for the processor it is then run and priced on (the tool once compiled
/// every table for the 68000, so the 68020 column priced 68000 code).
fn driver_cpu(cpu: &str) -> String {
    format!("-mcpu={cpu}")
}

/// The assembler's flag for the same processor.
fn assembler_arch(cpu: &str) -> String {
    format!("-m{cpu}")
}

/// The IR `CYCLES_LLC` compiles: `$CYCLES_RAW_IR_DIR/<set>/<name>.ll` where
/// there is one (clang's output with no passes, what `llrm-cc` hands the
/// driver), else the bench's own IR. Some bench IR is hand-written (gcd's
/// selects), so a gap measured on it is not the shipped path's.
fn shipped_source(
    set: &str,
    name: &str,
) -> String {
    std::env::var_os("CYCLES_RAW_IR_DIR")
        .map(|dir| Path::new(&dir).join(set).join(format!("{name}.ll")))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_else(|| source_in(set, name))
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
    cpu: &str,
) -> Option<Vec<u8>> {
    let input = dir.join("llc.ll");
    std::fs::write(&input, ir).unwrap();
    let asm = dir.join("llc.s");
    let status = Command::new(llc)
        .args(["-S".to_owned(), driver_cpu(cpu)])
        .arg(std::env::var("CYCLES_LLC_LEVEL").unwrap_or_else(|_| (if size { "-Os" } else { "-O2" }).to_owned()))
        .arg("-o")
        .arg(&asm)
        .arg(&input)
        .status()
        .unwrap();
    assert!(status.success(), "the llc failed");
    let text = format!(".text\n{}", relaxed(&std::fs::read_to_string(&asm).unwrap()));
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
    let arch = assembler_arch(cpu);
    run("as", vec![arch.as_ref(), source.as_os_str(), "-o".as_ref(), object.as_os_str()]);
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

/// The run's directory, deleted when the run ends however it ends. Not the
/// shared `scratch` module: `matrix` includes this file as a module of its own.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("llrm-m68k-cycles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn main() {
    let gcc = PathBuf::from(std::env::var("GCC_M68K").expect("GCC_M68K: the unpacked m68k binutils root"));
    let dir = Scratch::new();
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
            let total = price(&gcc, Path::new(&path), &models, true, &[], "m68000");
            let mark = if total.partial { "*" } else { "" };
            let unknown = if total.unknown > 0 { format!(" ?{}", total.unknown) } else { String::new() };
            let cells: Vec<String> =
                total.clocks.iter().map(|one| format!("{one}{mark}{unknown} | {:.1}", *one as f64 / calls)).collect();
            let name = Path::new(&path).file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
            println!("| {name} | {} | {} |", total.insns, cells.join(" | "));
        }
        return;
    }
    if let Some(exe) = std::env::var_os("CYCLES_EXE") {
        let total = price(&gcc, Path::new(&exe), &models, false, &[], "m68000");
        let clocks: Vec<String> = models.iter().zip(&total.clocks).map(|(cpu, clk)| format!("{cpu:?} {clk}")).collect();
        println!(
            "insns {} clocks {} unpriced {} partial {}",
            total.insns,
            clocks.join(" "),
            total.unknown,
            total.partial
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
    // With `CYCLES_LLC` the code is that driver's for one processor, so each
    // processor gets a table of its own: compiled, assembled, run and priced
    // for it. Without, the crate's own 68000 code is priced on every model.
    let llc_mode = std::env::var_os("CYCLES_LLC").is_some();
    let groups: Vec<Vec<Model>> =
        if llc_mode { models.iter().map(|one| vec![*one]).collect() } else { vec![models.clone()] };
    for models in groups {
        let cpu_name = format!("{:?}", models[0]).trim_start_matches('M').to_owned();
        let qemu = if llc_mode { format!("m{cpu_name}") } else { "m68000".to_owned() };
        let qemu = qemu.as_str();
        if llc_mode {
            println!("## {cpu_name} build");
        }
        let names: Vec<String> =
            models.iter().map(|cpu| format!("{cpu:?}").trim_start_matches('M').to_owned()).collect();
        let clock_heads =
            |prefix: &str| names.iter().map(|name| format!("{prefix}{name} clk")).collect::<Vec<_>>().join(" | ");
        println!("| kernel | B | insns | {} | -Os B | -Os insns | {} |", clock_heads(""), clock_heads("-Os "));
        println!("|{}", "---|".repeat(5 + 2 * models.len()));
        for (name, args, expected, leaf) in &rows {
            let (name, expected, leaf) = (*name, *expected, *leaf);
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
                    let Some(bytes) =
                        assembled(Path::new(&llc), &gcc, &shipped_source(&set, name), options.size, &dir, &cpu_name)
                    else {
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
                        Command::new("timeout").args(["10", "qemu-m68k", "-cpu", qemu]).arg(&exe).status().unwrap();
                    assert_eq!(status.code(), Some(expected), "{name}: the llc's code gives another result");
                }
                let total = price(&gcc, &exe, &models, false, &runtime_names, qemu);
                let mark = if total.partial { "*" } else { "" };
                let unknown = if total.unknown > 0 { format!(" ?{}", total.unknown) } else { String::new() };
                let clocks: Vec<String> = total.clocks.iter().map(|one| format!("{one}{mark}{unknown}")).collect();
                cells.push(format!("{size} | {} | {}", total.insns, clocks.join(" | ")));
            }
            println!("| {name} | {} |", cells.join(" | "));
        }
    }

    /// A kernel that calls a runtime routine laid out after it priced only
    /// `k`: the routine's instructions were in no range, so muldiv's clocks and
    /// instruction count left out every multiply and divide.
    #[test]
    fn a_priced_function_brings_the_routines_it_calls() {
        let objdump =
            "00000100 <k>:\n 100:\tjsr 200\n 106:\trts\n\n00000200 <__mulsi3>:\n 200:\tmulu.w d1,d0\n 202:\trts\n";
        let only_k = parse_listing(objdump, &["k".to_owned()], false).1;
        let with_callee = parse_listing(objdump, &["k".to_owned(), "__mulsi3".to_owned()], false).1;
        assert!(!only_k.iter().any(|range| range.contains(&0x200)), "{only_k:?}");
        assert!(with_callee.iter().any(|range| range.contains(&0x200)), "{with_callee:?}");
    }
}
