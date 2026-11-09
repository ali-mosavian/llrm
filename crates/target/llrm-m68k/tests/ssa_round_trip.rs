//! A shared stage hosted on the SSA function (`host_ssa`) hands back what it was
//! given when it changes nothing. A round trip that altered a function would
//! make every stage hosted through it look like a regression of the stage.

use llrm_lir::registers::Regs;
use llrm_m68k::dagisel::function::select_function;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{emit, expand_cmov};
use llrm_m68k::regalloc::M68kRegisters;
use llrm_spike_mf::tolir::host_ssa;

struct Null;

impl llrm_codegen::model::passes::LIRTransform for Null {
    fn class_name(&self) -> &'static str {
        "Null"
    }

    fn transform(
        &mut self,
        body: llrm_codegen::model::lir::LirBody,
    ) -> Result<llrm_codegen::model::lir::LirBody, String> {
        Ok(body)
    }
}

/// Removes the first instruction of the entry block that is not a terminator.
struct DropOne;

impl llrm_codegen::model::passes::LIRTransform for DropOne {
    fn class_name(&self) -> &'static str {
        "DropOne"
    }

    fn transform(
        &mut self,
        body: llrm_codegen::model::lir::LirBody,
    ) -> Result<llrm_codegen::model::lir::LirBody, String> {
        let entry = &body.blocks[0];
        let mut insns: Vec<_> = entry.insns.iter().cloned().collect();
        insns.remove(0);
        let blocks: Vec<_> =
            body.blocks.iter().enumerate().map(|(at, block)| if at == 0 { block.with_insns(insns.clone()) } else { block.clone() }).collect();
        Ok(body.with_blocks(blocks))
    }
}

const FIXTURES: &[(&str, &str)] = &[
    ("sieve_pin_o2", include_str!("fixtures/sieve_pin_o2.ll")),
    ("udivmod_w", include_str!("fixtures/udivmod_w.ll")),
    ("partition_cc", include_str!("fixtures/partition_cc.ll")),
    ("arg_pointers", include_str!("fixtures/arg_pointers.ll")),
    ("pack_ref", include_str!("fixtures/pack_ref.ll")),
    ("hex_low_byte_copy", include_str!("fixtures/hex_low_byte_copy.ll")),
    ("byte_spill_reload", include_str!("fixtures/byte_spill_reload.ll")),
    ("soft_add", include_str!("fixtures/soft_add.ll")),
    ("soft_pack", include_str!("fixtures/soft_pack.ll")),
    ("inline_bar", include_str!("fixtures/inline_bar.ll")),
];

/// Every defined function of every fixture, before and after a stage that does
/// nothing, at each CPU: the same machine function.
#[test]
fn a_stage_that_changes_nothing_hands_the_function_back_unchanged() {
    let mut checked = 0;
    for (file, text) in FIXTURES {
        let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{file}: {why}"));
        llrm_mir::parse::upgrade_lifetimes(&mut module);
        let names: Vec<String> =
            module.functions().filter(|(_, _, one)| !one.is_declaration()).map(|(id, _, _)| module.global(id).name.as_ref().expect("a defined function has a name").clone()).collect();
        for name in names {
            for cpu in [Cpu::M68000, Cpu::M68020] {
                let blocks = select_function(&module, &name, cpu).unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
                let mut function = emit(&name, &blocks).unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
                expand_cmov(&mut function).unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
                let back = host_ssa::<M68kRegisters>(&function, Regs(&llrm_m68k::REGISTER_INFO), |_| Box::new(Null))
                    .unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
                assert_eq!(format!("{function:?}"), format!("{back:?}"), "{file} @{name} at {cpu:?} changed");
                checked += 1;
            }
        }
    }
    assert!(checked >= 18, "only {checked} functions went round");
}

/// The instrument: the comparison above does see a stage that removes an
/// instruction, so "unchanged" is a finding and not the comparison's blind spot.
#[test]
fn a_stage_that_removes_an_instruction_is_seen() {
    let mut module = llrm_mir::parse::module(include_str!("fixtures/sieve_pin_o2.ll")).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let blocks = select_function(&module, "bench_sieve", Cpu::M68000).expect("selects");
    let mut function = emit("bench_sieve", &blocks).expect("emits");
    expand_cmov(&mut function).expect("expands");
    let back = host_ssa::<M68kRegisters>(&function, Regs(&llrm_m68k::REGISTER_INFO), |_| Box::new(DropOne)).expect("round trip");
    assert_ne!(format!("{function:?}"), format!("{back:?}"));
}
