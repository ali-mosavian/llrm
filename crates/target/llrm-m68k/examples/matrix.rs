//! `matrix [SET]`: the kernels of `bench/<SET>` (`kernels`, or `library`) built
//! by this compiler twice (`ours`: `llrm-cc` over the C source, as a user runs
//! it; `ours-ir`: the backend alone on the kernel's IR), m68k gcc (`$GCC_M68K`)
//! and the pin's clang (`$LLRM_PIN_CLANG` into `$LLVM_M68K/llc`) at O1, O2, O3
//! and Os for the 68000, 68010, 68020, 68030, 68040 and 68060, run under
//! `qemu-m68k` and priced as `cycles` does. Prints the competitor-matrix rows:
//! `target prog level compiler code_bytes clocks instrs note`, tab separated.
//! An empty field means the instrument has no number; `note` says why.
//!
//! `clang` is the pin's result for the program: its own front end and mid-end
//! to IR, then its `llc`, so its LSR runs once on its own IR. `ours-ir` and
//! the `llc` columns of `examples/bench` take the committed `bench/*.ll`, which
//! is not that IR: those compare backends on one IR only.

use std::path::{Path, PathBuf};
use std::process::Command;

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::isa::Cpu as Model;
use llrm_m68k::kernels::{
    Arg, data, extra_runs, keep_named, library_data, library_table, program_with_data, source_in, table,
};
use llrm_m68k::object::{assemble, executable, write_executable};

#[allow(dead_code)]
#[path = "cycles.rs"]
mod cycles;
mod tempdir;

/// The sections of an `objdump -h` listing that hold data a kernel could load
/// (`.rodata*`, `.data*`, `.bss*`, literal pools) with a size above zero.
fn data_sections(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (_, name, size) = (fields.next()?, fields.next()?, fields.next()?);
            let data = [".rodata", ".data", ".bss", ".lit"].iter().any(|prefix| name.starts_with(prefix));
            (data && u64::from_str_radix(size, 16).is_ok_and(|size| size > 0)).then(|| name.to_owned())
        })
        .collect()
}

/// The code sections of an `objdump -h` listing other than `.text` with a size
/// above zero (`.text.startup` for `main`, `.text.unlikely` and `.text.hot`
/// for gcc's cold and hot parts): the line after a section's carries its
/// flags.
fn other_code_sections(listing: &str) -> Vec<String> {
    let lines: Vec<&str> = listing.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| {
            let mut fields = line.split_whitespace();
            let (_, name, size) = (fields.next()?, fields.next()?, fields.next()?);
            let flags = lines.get(at + 1)?;
            let code = flags.split([',', ' ']).any(|flag| flag == "CODE");
            (code && name != ".text" && u64::from_str_radix(size, 16).is_ok_and(|size| size > 0))
                .then(|| name.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    /// The rewritten listing lost its final newline: gas warned "end of file
    /// not at end of a line" on every `ours` cell (1056 stderr lines in the
    /// 11-09 run), and a warning on our output hides a mis-assembled line.
    #[test]
    fn rewritten_listing_ends_in_a_newline() {
        assert_eq!(super::relaxing_branches("\tbne\tL1\n\trts\n"), "\tjne\tL1\n\trts\n");
    }

    /// The matrix measured O2 and Os only, so O1 (strict: bytes and clocks) and
    /// O3 had no cells and counted as clean.
    #[test]
    fn every_level_is_measured_by_default() {
        assert_eq!(super::levels(), ["O1", "O2", "O3", "Os"]);
    }

    /// gcc puts `main` in `.text.startup` and cold parts in `.text.unlikely`;
    /// a column that read `.text` alone undercounted those bytes (softfloat's
    /// finding for the ELF feeds; this corpus had none, so no cell moved).
    #[test]
    fn code_outside_text_is_not_counted_by_text_alone() {
        let listing = "Idx Name          Size      VMA\n  0 .text         00000010  00000000\n                  CONTENTS, ALLOC, LOAD, READONLY, CODE\n  1 .text.startup 00000008  00000000\n                  CONTENTS, ALLOC, LOAD, READONLY, CODE\n  2 .comment      00000026  00000000\n                  CONTENTS, READONLY\n";
        assert_eq!(super::other_code_sections(listing), vec![".text.startup".to_owned()]);
    }

    /// A kernel that loads a constant from `.rodata` counted only its `.text`
    /// for every column (arm64's instrument hid up to 24 B that way).
    #[test]
    fn a_kernel_with_rodata_is_not_counted_by_its_text_alone() {
        let listing = "Idx Name          Size      VMA\n  0 .text         00000010  00000000\n  1 .data         00000000  00000000\n  2 .rodata       00000008  00000000\n";
        assert_eq!(super::data_sections(listing), vec![".rodata".to_owned()]);
    }
}

/// The `.text` of an object that holds leaf code: no relocation in it.
fn leaf_text(
    objdump: &Path,
    object: &Path,
) -> Result<Vec<u8>, String> {
    text_bytes(objdump, object, false)
}

/// The text of `object`; with `calls` a relocation to a runtime routine is
/// allowed (the bytes alone are compared, nothing runs).
fn text_bytes(
    objdump: &Path,
    object: &Path,
    calls: bool,
) -> Result<Vec<u8>, String> {
    let headers = Command::new(objdump).arg("-h").arg(object).output().map_err(|e| e.to_string())?;
    if let Some(section) = data_sections(&String::from_utf8_lossy(&headers.stdout)).first() {
        return Err(format!("loads data from {section}: bytes of .text alone would undercount"));
    }
    if let Some(section) = other_code_sections(&String::from_utf8_lossy(&headers.stdout)).first() {
        return Err(format!("code in {section}: bytes of .text alone would undercount"));
    }
    let relocations = Command::new(objdump).arg("-r").arg(object).output().map_err(|e| e.to_string())?;
    let listing = String::from_utf8_lossy(&relocations.stdout).to_string();
    let in_text = listing.split("RELOCATION RECORDS FOR").find(|part| part.trim_start().starts_with("[.text]"));
    if !calls && in_text.is_some_and(|part| part.contains("R_68K")) {
        return Err("calls a runtime routine (relocation in .text)".to_owned());
    }
    let out = Command::new(objdump).args(["-s", "-j", ".text"]).arg(object).output().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines().skip_while(|line| !line.starts_with("Contents of")).skip(1)
    {
        let hex = line.trim_start().split("  ").next().unwrap_or("");
        for group in hex.split(' ').skip(1) {
            bytes.extend((0..group.len()).step_by(2).map(|at| u8::from_str_radix(&group[at..at + 2], 16).unwrap()));
        }
    }
    Ok(bytes)
}

/// The kernel's C source built by m68k gcc into an object's text.
/// The shared external-bar flags (`tools/external_flags.py --target none`, hash
/// cd1e12d510e4); `tools/m68k_column_probe.py` fails when this list and that
/// file differ. Relocation is ours: none (static), asserted by the same probe.
const EXTERNAL_FLAGS: [&str; 5] = [
    "-fomit-frame-pointer",
    "-fno-stack-protector",
    "-fno-stack-clash-protection",
    "-U_FORTIFY_SOURCE",
    "-fno-asynchronous-unwind-tables",
];
/// gcc 15's upstream default, pinned (clang has no such option).
const GCC_ZERO_INIT_PADDING: &str = "-fzero-init-padding-bits=standard";

fn gcc_object(
    root: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let status = Command::new(root.join("usr/bin/m68k-linux-gnu-gcc"))
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg(format!("-B{}/usr/lib/gcc-cross/m68k-linux-gnu/15/", root.display()))
        .arg(format!("-B{}/usr/m68k-linux-gnu/bin/", root.display()))
        .args(["-ffreestanding", "-nostdlib"])
        .args(EXTERNAL_FLAGS)
        .arg(GCC_ZERO_INIT_PADDING)
        .arg(format!("-mcpu={cpu}"))
        .arg(format!("-{level}"))
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(object)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "gcc failed".to_owned())
}

/// This compiler as a user runs it: `llrm-cc` over the C source (clang front
/// end, the mid-end, the m68k backend) to assembly, assembled by gas. The pin's
/// `llvm-mc` does not parse our text (`subq.l`), and gas lengthens a plain
/// forward `bcc` to 4 bytes, so branches become gas's relaxing `jcc`.
fn ours_object(
    llrm_cc: &Path,
    root: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let asm = object.with_extension("s");
    let status = Command::new(llrm_cc)
        .env("LLRM_CC_TARGET", format!("m68k-{cpu}"))
        .arg(format!("-{level}"))
        .arg("-S")
        .arg(source)
        .arg("-o")
        .arg(&asm)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("llrm-cc failed".to_owned());
    }
    let text = std::fs::read_to_string(&asm).map_err(|e| e.to_string())?;
    std::fs::write(&asm, format!(".text\n{}", relaxing_branches(&text))).map_err(|e| e.to_string())?;
    let status = Command::new(root.join("usr/bin/m68k-linux-gnu-as"))
        .env("LD_LIBRARY_PATH", root.join("usr/lib/x86_64-linux-gnu"))
        .arg("--fatal-warnings")
        .arg(format!("-m{cpu}"))
        .arg("-o")
        .arg(object)
        .arg(&asm)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "as failed".to_owned())
}

/// Plain `bcc label` lines as gas's relaxing `jcc label`, which picks the short
/// form when it reaches, as the pin's MC and our assembler do.
fn relaxing_branches(text: &str) -> String {
    const CONDITIONS: [&str; 16] =
        ["ra", "hi", "ls", "cc", "cs", "ne", "eq", "vc", "vs", "pl", "mi", "ge", "lt", "gt", "le", "hs"];
    text.lines()
        .map(|line| {
            let mut words = line.split_whitespace();
            let (Some(op), Some(target), None) = (words.next(), words.next(), words.next()) else {
                return line.to_owned();
            };
            match op.strip_prefix('b') {
                Some(cond) if CONDITIONS.contains(&cond) && !target.starts_with('#') => format!("\tj{cond}\t{target}"),
                _ => line.to_owned(),
            }
        })
        .map(|line| line + "\n")
        .collect()
}

/// The pin's clang (front end and `-O` pipeline, to IR) into the pin's `llc`:
/// the pin's clang is built for x86 only, and the distro's clang 20 miscompiles
/// the kernels (a `move` between a `cmp` and its branch).
fn clang_object(
    clang: &Path,
    llc: &Path,
    cpu: &str,
    level: &str,
    source: &Path,
    object: &Path,
) -> Result<(), String> {
    let ir = object.with_extension("ll");
    let status = Command::new(clang)
        .args(["--target=m68k-linux-gnu", "-ffreestanding", "-fno-pic"])
        .args(EXTERNAL_FLAGS)
        .arg(format!("-mcpu={cpu}"))
        .arg(format!("-{level}"))
        .args(["-S", "-emit-llvm"])
        .arg(source)
        .arg("-o")
        .arg(&ir)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("clang failed".to_owned());
    }
    let status = Command::new(llc)
        .args(["-mtriple=m68k-linux-gnu", "-relocation-model=static", "-O2", "-filetype=obj"])
        .arg(format!("-mcpu=M{cpu}"))
        .arg(&ir)
        .arg("-o")
        .arg(object)
        .status()
        .map_err(|e| e.to_string())?;
    status.success().then_some(()).ok_or_else(|| "llc failed".to_owned())
}

fn main() {
    let gcc_root = PathBuf::from(std::env::var("GCC_M68K").expect("GCC_M68K: the unpacked m68k binutils root"));
    let clang = PathBuf::from(std::env::var("LLRM_PIN_CLANG").expect("LLRM_PIN_CLANG: the pin's clang"));
    let llrm_cc =
        PathBuf::from(std::env::var("LLRM_CC").expect("LLRM_CC: the llrm-cc binary, llrm-m68k-llc beside it"));
    let llc = PathBuf::from(std::env::var("LLVM_M68K").expect("LLVM_M68K: the pin's m68k build, its bin directory"))
        .join("llc");
    let objdump = gcc_root.join("usr/bin/m68k-linux-gnu-objdump");
    let dir = tempdir::Scratch::new("matrix");
    let set = std::env::args().nth(1).unwrap_or_else(|| "kernels".to_owned());
    let kernel_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench").join(&set);
    let (rows, block): (Vec<(&str, Vec<Arg>, i32, bool)>, Vec<u8>) = if set == "library" {
        (library_table(), library_data())
    } else {
        (table().into_iter().map(|(name, args, expected)| (name, args, expected, true)).collect(), data())
    };
    // `MATRIX_PROGS=partition,hex` keeps the named programs; an unknown name
    // stops the run
    let rows = match std::env::var("MATRIX_PROGS") {
        Ok(only) => keep_named(rows, |row| row.0, &only).unwrap_or_else(|why| panic!("MATRIX_PROGS: {why}")),
        Err(_) => rows,
    };
    let cpus = [
        ("68000", Cpu::M68000, Model::M68000),
        ("68010", Cpu::M68010, Model::M68010),
        ("68020", Cpu::M68020, Model::M68020),
        ("68030", Cpu::M68030, Model::M68030),
        ("68040", Cpu::M68040, Model::M68040),
        ("68060", Cpu::M68060, Model::M68060),
    ];
    // `MATRIX_CPUS=68040,68060` keeps the named columns; an unknown name
    // stops the run (68010 and 68030 once produced an empty table, rc 0)
    let only = std::env::var("MATRIX_CPUS").ok();
    if let Some(only) = &only {
        if let Some(bad) = only.split(',').find(|one| !cpus.iter().any(|column| column.0 == *one)) {
            let names: Vec<_> = cpus.iter().map(|column| column.0).collect();
            panic!("MATRIX_CPUS: no column {bad:?}; the columns are {}", names.join(", "));
        }
    }
    for (cpu_name, cpu, model) in cpus {
        if only.as_ref().is_some_and(|only| !only.split(',').any(|one| one == cpu_name)) {
            continue;
        }
        // `fnptr` is handed a pointer to `h`, compiled by this compiler for
        // every column
        let helper = (set == "library").then(|| {
            let module = llrm_mir::parse::module(&source_in(&set, "h")).unwrap();
            items(&module, "h", cpu).unwrap()
        });
        for (name, args, expected, leaf) in &rows {
            let mut text = source_in(&set, name);
            let mut runtime_names: Vec<&str> = Vec::new();
            if !leaf {
                text.push_str(source_in(&set, "rt").split_once('\n').map_or("", |(_, rest)| rest));
                runtime_names = vec!["__mulsi3", "__udivsi3", "__umodsi3", "__divsi3", "__modsi3"];
            }
            let module = llrm_mir::parse::module(&text).unwrap();
            let runtime: Vec<Item> = runtime_names.iter().flat_map(|one| items(&module, one, cpu).unwrap()).collect();
            for level in levels() {
                let row =
                    |compiler: &str, size: Option<usize>, clocks: Option<u64>, insns: Option<usize>, note: &str| {
                        let cell = |value: Option<String>| value.unwrap_or_default();
                        println!(
                            "m68k-{cpu_name}\t{name}\t{level}\t{compiler}\t{}\t{}\t{}\t{note}",
                            cell(size.map(|n| n.to_string())),
                            cell(clocks.map(|n| n.to_string())),
                            cell(insns.map(|n| n.to_string())),
                        );
                    };
                // clocks and instructions inside `k` of one variant's code, and
                // the program's result
                let measure = |code: Vec<Item>, id: &str| -> (Option<u64>, Option<usize>, String) {
                    let mut code = code;
                    code.extend(helper.clone().unwrap_or_default());
                    code.extend(runtime.clone());
                    // the first input, then the others that make the branches
                    // go the other way
                    let mut runs: Vec<(&[Arg], i32)> = vec![(args.as_slice(), *expected)];
                    let more = extra_runs(name);
                    runs.extend(more.iter().map(|(args, expected)| (args.as_slice(), *expected)));
                    let (mut clocks, mut insns, mut notes) = (0u64, 0usize, String::new());
                    for (at, (args, expected)) in runs.iter().enumerate() {
                        let exe = dir.join(format!("{name}_{cpu_name}_{level}_{id}_{at}.elf"));
                        write_executable(&exe, &executable(&program_with_data(args, code.clone(), &block)).unwrap())
                            .unwrap();
                        let status = Command::new("timeout")
                            .args(["10", "qemu-m68k", "-cpu", &format!("m{cpu_name}")])
                            .arg(&exe)
                            .status()
                            .ok()
                            .and_then(|s| s.code());
                        if status != Some(*expected) {
                            return (None, None, format!("wrong result: {status:?}, want {expected}"));
                        }
                        let total = cycles::price(&gcc_root, &exe, &[model], false, &["k".to_owned()]);
                        if total.truncated {
                            return (None, None, "trace cut short: raise CYCLES_TIMEOUT".to_owned());
                        }
                        if total.insns == 0 {
                            return (None, None, "priced no instruction".to_owned());
                        }
                        if total.unknown_of(0) > 0 {
                            notes = format!("{} executed instructions unpriced", total.unknown_of(0));
                        } else if total.partial && notes.is_empty() {
                            notes = "data-dependent term at its bound".to_owned();
                        }
                        clocks += total.clocks[0];
                        insns += total.insns;
                    }
                    // the mean of the runs, rounded
                    let n = runs.len() as u64;
                    let mean = |sum: u64| (sum + n / 2) / n;
                    let inputs = if n > 1 { format!("inputs {n}") } else { String::new() };
                    let note =
                        [notes, inputs].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join("; ");
                    (Some(mean(clocks)), Some(mean(insns as u64) as usize), note)
                };

                let options = Options { size: level == "Os", strict: level == "O1", ..Options::default() };
                match items_with(&module, "k", cpu, options) {
                    Ok(code) => {
                        let size = {
                            let mut all = code.clone();
                            all.extend(runtime.iter().cloned());
                            assemble(&all, 0).unwrap().code.len()
                                - assemble(&runtime, 0).map_or(0, |one| one.code.len())
                        };
                        if *leaf {
                            let (clocks, insns, note) = measure(code, "ours-ir");
                            row("ours-ir", Some(size), clocks, insns, &note);
                        } else {
                            row("ours-ir", Some(size), None, None, "calls the runtime: bytes only");
                        }
                    }
                    Err(error) => row("ours-ir", None, None, None, &format!("compile failed: {error}")),
                }

                for compiler in ["ours", "gcc", "clang"] {
                    let source = kernel_dir.join(format!("{name}.c"));
                    let object = dir.join(format!("{name}_{cpu_name}_{level}_{compiler}.o"));
                    let built = if compiler == "ours" {
                        ours_object(&llrm_cc, &gcc_root, cpu_name, level_flag(level), &source, &object)
                    } else if compiler == "gcc" {
                        gcc_object(&gcc_root, cpu_name, level_flag(level), &source, &object)
                    } else {
                        clang_object(&clang, &llc, cpu_name, level_flag(level), &source, &object)
                    };
                    if !leaf {
                        // the runtime is not linked into these objects: bytes
                        // only, from every column
                        match built.and_then(|()| text_bytes(&objdump, &object, true)) {
                            Ok(bytes) => row(compiler, Some(bytes.len()), None, None, "calls the runtime: bytes only"),
                            Err(error) => row(compiler, None, None, None, &format!("n/a: {error}")),
                        }
                        continue;
                    }
                    match built.and_then(|()| leaf_text(&objdump, &object)) {
                        Ok(bytes) => {
                            let size = bytes.len();
                            let (clocks, insns, note) =
                                measure(vec![Item::Label("k".to_owned()), Item::Bytes(bytes)], compiler);
                            row(compiler, Some(size), clocks, insns, &note);
                        }
                        Err(error) => row(compiler, None, None, None, &format!("n/a: {error}")),
                    }
                }
            }
        }
    }
}

/// The levels a matrix measures: `O1` is strict (bytes and clocks), `O2` and
/// `O3` are priced by the CPU's byte rate, `Os` is bytes. `MATRIX_LEVELS=O1,Os`
/// keeps the named ones; an unknown name stops the run.
const LEVELS: [&str; 4] = ["O1", "O2", "O3", "Os"];

fn levels() -> Vec<&'static str> {
    match std::env::var("MATRIX_LEVELS") {
        Ok(only) => only
            .split(',')
            .map(|one| {
                LEVELS
                    .iter()
                    .copied()
                    .find(|level| *level == one)
                    .unwrap_or_else(|| panic!("MATRIX_LEVELS: unknown level {one}"))
            })
            .collect(),
        Err(_) => LEVELS.to_vec(),
    }
}

/// The `-O` flag of a level: the level itself, for every column.
fn level_flag(level: &str) -> &'static str {
    LEVELS.iter().copied().find(|one| *one == level).unwrap_or_else(|| panic!("unknown level {level}"))
}
