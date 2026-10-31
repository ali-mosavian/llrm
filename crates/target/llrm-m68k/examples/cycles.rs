//! `cycles [SET]`: the kernels of `bench/<SET>` (`kernels`, or `library`)
//! compiled by this compiler (default and size), run under `qemu-m68k`, and
//! the clocks of the executed instructions inside `k` added up from
//! `llrm_m68k::timing` for the 68000 and the 68020 (cache case). Needs
//! `$LLVM_M68K` (the pin's m68k build) for `llvm-objdump` and `qemu-m68k`.
//! A `*` marks a total that holds a data-dependent term at its bound; `?`
//! counts executed instructions the tables do not cover.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use llrm_m68k::compile::{Item, Options, items, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::kernels::{data, library_data, library_table, program_with_data, source_in, table};
use llrm_m68k::object::{assemble, executable, write_executable};
use llrm_m68k::timing::cost;

/// Executed instructions read before a run is called runaway.
const CAP: usize = 2_000_000;

/// The program counters qemu executed, in order.
fn trace(exe: &Path) -> Vec<u32> {
    use std::io::{BufRead, BufReader};
    let mut child = Command::new("timeout")
        .args(["10", "qemu-m68k", "-cpu", "m68000", "-one-insn-per-tb", "-d", "exec", "-D", "/dev/stderr"])
        .arg(exe)
        .stderr(Stdio::piped())
        .spawn()
        .expect("timeout runs qemu-m68k");
    let mut pcs = Vec::new();
    for line in BufReader::new(child.stderr.take().unwrap()).lines().map_while(Result::ok) {
        let Some(fields) = line.split_once('[').and_then(|(_, rest)| rest.split_once(']')) else { continue };
        if let Some(pc) = fields.0.split('/').nth(1).and_then(|pc| u32::from_str_radix(pc, 16).ok()) {
            pcs.push(pc);
        }
        if pcs.len() > CAP {
            child.kill().ok();
            break;
        }
    }
    child.wait().ok();
    pcs
}

/// `(address, text)` of every instruction of `llvm-objdump -d`, and the
/// address range of `k`.
fn listing(
    llvm: &Path,
    exe: &Path,
) -> (Vec<(u32, String)>, std::ops::Range<u32>) {
    let out = Command::new(llvm.join("llvm-objdump")).args(["-d", "--no-show-raw-insn"]).arg(exe).output().unwrap();
    let mut lines = Vec::new();
    let mut range = 0..0;
    let mut in_k = false;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((address, name)) = line.split_once(" <").and_then(|(a, rest)| Some((a, rest.strip_suffix(">:")?))) {
            if in_k {
                range.end = u32::from_str_radix(address, 16).unwrap();
                in_k = false;
            }
            if name == "k" {
                range.start = u32::from_str_radix(address, 16).unwrap();
                in_k = true;
            }
            continue;
        }
        let Some((address, text)) = line.split_once(':') else { continue };
        let Ok(address) = u32::from_str_radix(address.trim(), 16) else { continue };
        lines.push((address, text.trim().replace('\t', " ")));
    }
    if in_k {
        range.end = lines.last().map_or(0, |last| last.0 + 2);
    }
    (lines, range)
}

struct Total {
    insns: usize,
    clocks: [u64; 2],
    partial: bool,
    unknown: usize,
}

fn price(
    llvm: &Path,
    exe: &Path,
) -> Total {
    let (lines, range) = listing(llvm, exe);
    let at: HashMap<u32, (usize, &str)> = lines
        .iter()
        .enumerate()
        .map(|(index, (address, text))| (*address, (lines.get(index + 1).map_or(2, |next| next.0 - address) as usize, text.as_str())))
        .collect();
    let pcs = trace(exe);
    let mut total = Total { insns: 0, clocks: [0; 2], partial: false, unknown: 0 };
    for (index, pc) in pcs.iter().enumerate() {
        if !range.contains(pc) {
            continue;
        }
        total.insns += 1;
        let Some((bytes, text)) = at.get(pc) else {
            total.unknown += 1;
            continue;
        };
        let taken = pcs.get(index + 1).is_some_and(|next| *next != pc + *bytes as u32);
        for (slot, cpu) in [Cpu::M68000, Cpu::M68020].into_iter().enumerate() {
            match cost(cpu, text, *bytes as u32, taken) {
                Ok(one) => {
                    total.clocks[slot] += u64::from(one.clocks);
                    total.partial |= one.partial;
                }
                Err(why) => {
                    if slot == 0 {
                        total.unknown += 1;
                        eprintln!("unpriced: {why}");
                    }
                }
            }
        }
    }
    total
}

fn main() {
    let llvm = PathBuf::from(std::env::var("LLVM_M68K").expect("LLVM_M68K: the pin's m68k build, its bin directory"));
    let dir = std::env::temp_dir().join(format!("llrm-m68k-cycles-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
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
    println!("| kernel | B | insns | 68000 clk | 68020 clk | -Os B | -Os insns | -Os 68000 | -Os 68020 |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for (name, args, _, leaf) in rows {
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
            let size = {
                let mut all = code.clone();
                all.extend(runtime.iter().cloned());
                assemble(&all, 0).unwrap().code.len() - assemble(&runtime, 0).map_or(0, |one| one.code.len())
            };
            code.extend(helper.clone().unwrap_or_default());
            code.extend(runtime.clone());
            let exe = dir.join(format!("{name}.elf"));
            write_executable(&exe, &executable(&program_with_data(&args, code, &block)).unwrap()).unwrap();
            let total = price(&llvm, &exe);
            let mark = if total.partial { "*" } else { "" };
            let unknown = if total.unknown > 0 { format!(" ?{}", total.unknown) } else { String::new() };
            cells.push(format!(
                "{size} | {} | {}{mark}{unknown} | {}{mark}{unknown}",
                total.insns, total.clocks[0], total.clocks[1]
            ));
        }
        println!("| {name} | {} |", cells.join(" | "));
    }
    std::fs::remove_dir_all(&dir).ok();
}
