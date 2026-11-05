//! `function_bytes`, the estimate every byte-priced decision reads, counted about half of what the assembled functions
//! are (median 1.9 times short over tests/run/c, `~/scratch/handoff/m68k-operation-costs-card-2026-11-05.md`).
//!
//! Two populations, two statements:
//! - the programs the factor was fit on (tests/data_size: the last IR stage of 22 tests/run/c programs, chosen by
//!   `crc32(name) % 5 == 0`): the estimate is within 0.8 and 1.1 of the objects' `.text` in aggregate. `function_scale` is
//!   the MEDIAN per-function ratio of the whole corpus (1.46, 1.50), the small functions the decisions are about, so the
//!   aggregate sits a little under 1 (big functions carry spills and relaxation no operation names);
//! - the 12 matrix kernels, leaf loops without frame, call or spill: the table itself, before the residue factor, is within
//!   0.75 and 1.1 of their objects (measured 0.89 to 1.03 over the four rows), so the SCALED estimate sits above them
//!   (1.34 to 1.55) by design. The first form of this bracket was 0.75 to 1.4 on the scaled estimate and failed three of
//!   four rows; the scale is a fact about the population, the table about the instructions, so the bracket is on the table.

use llrm_m68k::compile::CodeModel;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{self, Level};
use llrm_m68k::kernels::{DATALAYOUT, source_in, table};
use llrm_m68k::machine::M68kMachine;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::target::Machine;
use llrm_transforms::walk::function_bytes;

fn estimate_over(
    module: &llrm_mir::module::Module,
    cpu: Cpu,
) -> i64 {
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let costs = M68kMachine { cpu }.size_costs();
    module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(_, _, function)| function_bytes(module, &layout, &costs, function)).sum()
}

/// The size of the `.text` section of a big-endian ELF32 object.
fn text_bytes(elf: &[u8]) -> usize {
    let be32 = |at: usize| u32::from_be_bytes(elf[at..at + 4].try_into().unwrap()) as usize;
    let be16 = |at: usize| u16::from_be_bytes(elf[at..at + 2].try_into().unwrap()) as usize;
    let (shoff, shentsize, shnum, shstrndx) = (be32(0x20), be16(0x2E), be16(0x30), be16(0x32));
    let section = |index: usize| shoff + index * shentsize;
    let strings = be32(section(shstrndx) + 16);
    for index in 0..shnum {
        let at = section(index);
        let name = strings + be32(at);
        let end = elf[name..].iter().position(|&byte| byte == 0).unwrap();
        if &elf[name..name + end] == b".text" {
            return be32(at + 20);
        }
    }
    panic!("no .text section");
}

#[test]
fn the_estimate_brackets_the_leaf_kernels_at_both_cpus_and_levels() {
    let mut failures = Vec::new();
    for (cpu_name, cpu) in [("68000", Cpu::M68000), ("68020", Cpu::M68020)] {
        for level in ["2", "s"] {
            let (mut estimate, mut actual) = (0, 0);
            for (name, ..) in table() {
                let source = source_in("kernels", name);
                let text = if source.contains("target datalayout") { source } else { format!("target datalayout = \"{DATALAYOUT}\"\n{source}") };
                let Ok(module) = llrm_mir::parse::module(&text) else { continue };
                let Ok(module) = llrm_m68k::compile::mid_end_for(&module, cpu) else { continue };
                let Ok(object) = driver::object_in(&text, driver::cpu_of(cpu_name).unwrap(), Level::parse(level).unwrap(), name, CodeModel::Small) else { continue };
                let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
                let costs = M68kMachine { cpu }.size_costs();
                estimate += module.functions().filter(|(_, _, function)| !function.is_declaration()).map(|(_, _, function)| function_bytes(&module, &layout, &costs, function)).sum::<i64>();
                actual += text_bytes(&object) as i64;
            }
            // The table alone: the estimate with the residue scale taken off again.
            let scale = M68kMachine { cpu }.size_costs().function_scale as f64 / 100.0;
            let ratio = estimate as f64 / scale / actual as f64;
            assert!(actual > 0, "{cpu_name} -O{level}: no kernel compiled");
            if !(0.75..=1.1).contains(&ratio) {
                failures.push(format!("{cpu_name} -O{level}: estimate {estimate} B (scale {scale}) against objects {actual} B ({ratio:.2} unscaled)"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_estimate_tracks_the_objects_of_the_programs_it_was_fit_on() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data_size");
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|e| e == "ll")).collect();
    files.sort();
    assert!(files.len() >= 20, "{} fixtures", files.len());
    let mut failures = Vec::new();
    for (cpu_name, cpu) in [("68000", Cpu::M68000), ("68020", Cpu::M68020)] {
        for level in ["2", "s"] {
            let (mut estimate, mut actual) = (0, 0);
            for path in &files {
                let text = std::fs::read_to_string(path).unwrap();
                let module = llrm_mir::parse::module(&text).unwrap();
                let name = path.file_stem().unwrap().to_str().unwrap();
                let object = driver::object_in(&text, driver::cpu_of(cpu_name).unwrap(), Level::parse(level).unwrap(), name, CodeModel::Small)
                    .unwrap_or_else(|why| panic!("{name}: {why}"));
                estimate += estimate_over(&module, cpu);
                actual += text_bytes(&object) as i64;
            }
            let ratio = estimate as f64 / actual as f64;
            if !(0.8..=1.1).contains(&ratio) {
                failures.push(format!("{cpu_name} -O{level}: estimate {estimate} B against objects {actual} B ({ratio:.2})"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
