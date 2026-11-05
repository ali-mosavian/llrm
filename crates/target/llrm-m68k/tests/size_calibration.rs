//! `function_bytes`, the estimate every byte-priced decision reads, counted about half of what the assembled functions
//! are (median 1.9 times short over tests/run/c, `~/scratch/handoff/m68k-operation-costs-card-2026-11-05.md`).
//! `function_scale` (150) is the median over those programs; the matrix kernels are leaf loops with no frame, call or
//! spill, which the estimate already covers at 100 (0.84), so at 150 they sit above the objects (about 1.3). The test
//! bounds both sides on them: the estimate is not below three quarters (the old table's miss) and not above 1.4 (a
//! scale that over-counts leaf code by more than the median program under-counts it).

use llrm_m68k::compile::CodeModel;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{self, Level};
use llrm_m68k::kernels::{DATALAYOUT, source_in, table};
use llrm_m68k::machine::M68kMachine;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::target::Machine;
use llrm_transforms::walk::function_bytes;

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
fn the_estimate_brackets_the_objects_at_both_cpus_and_levels() {
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
            let ratio = estimate as f64 / actual as f64;
            assert!(actual > 0, "{cpu_name} -O{level}: no kernel compiled");
            if !(0.75..=1.4).contains(&ratio) {
                failures.push(format!("{cpu_name} -O{level}: estimate {estimate} B against objects {actual} B ({ratio:.2})"));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
