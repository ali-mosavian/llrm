//! The shared flow's phases up to the allocator, run in order over m68k's SSA
//! function (`bench_sieve`, the pin's -O2 IR): none refuses, and only MachineCSE,
//! PhiElimination, TwoAddress and the Coalescer change it. The x86-only phases
//! (FlagsCopy, CallFrameOpt, FixupSetCC, ...) are no-ops here, so the phase list
//! needs no `omitted` names for this function; a phase that starts changing
//! m68k code is caught by this test, not by a corpus sweep.

use std::rc::Rc;

use llrm_codegen::backend::{classes::RegisterClasses, cpu::tuned_for, peep::Rules, target::Segments};
use llrm_codegen::model::lir::LirBody;
use llrm_codegen::model::passes::LIRTransform;
use llrm_lir::registers::Regs;
use llrm_m68k::dagisel::function::select_function;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{emit_with, expand_cmov};
use llrm_m68k::regalloc::M68kRegisters;
use llrm_m68k::target::M68k;
use llrm_target::Target;

/// Every instruction with its operands and effects, and the phis.
fn shape(body: &LirBody) -> String {
    body.blocks.iter().map(|block| format!("{:?}{:?}", block.phis, block.insns)).collect::<Vec<_>>().join("|")
}

const FIXTURES: &[(&str, &str)] = &[
    ("sieve_pin_o2", include_str!("fixtures/sieve_pin_o2.ll")),
    ("nbody_pin_o2", include_str!("fixtures/nbody_pin_o2.ll")),
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

/// The phases that change a function, over every defined function of the fixtures.
#[test]
fn only_the_phases_m68k_has_a_use_for_change_a_function() {
    let profile = tuned_for(&M68k, "m68000", false).unwrap_or_else(|why| panic!("{why}"));
    let regs = Regs(M68k.registers());
    let segments = Segments::of(&M68k.machine(), regs, M68k.operand_bytes());
    let classes = Rc::new(RegisterClasses::of(&M68k));
    let mut changed: std::collections::BTreeSet<&'static str> = Default::default();
    let mut functions = 0;
    for (file, text) in FIXTURES {
        let mut module = llrm_mir::parse::module(text).unwrap_or_else(|why| panic!("{file}: {why}"));
        llrm_mir::parse::upgrade_lifetimes(&mut module);
        let names: Vec<String> = module
            .functions()
            .filter(|(_, _, one)| !one.is_declaration())
            .map(|(id, _, _)| module.global(id).name.as_ref().expect("a name").clone())
            .collect();
        for name in names {
            let blocks = select_function(&module, &name, Cpu::M68000).unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
            let mut function = emit_with(&name, &blocks, true).expect("emits");
            expand_cmov(&mut function).expect("expands");
            let mut phases = llrm_codegen::flow::machine_with(
                &Default::default(),
                None,
                None,
                None,
                false,
                profile,
                &segments,
                &classes,
                &Rules::NONE,
                &M68k.frame_registers(),
                llrm_codegen::backend::shadow::Selector::Old,
                &[],
            )
            .unwrap_or_else(|why| panic!("{why}"));
            let until = phases.iter().position(|phase| phase.class_name() == "RegAlloc").expect("RegAlloc is built");
            let converted =
                llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(&function, Regs(&llrm_m68k::REGISTER_INFO)).expect("converts");
            let _lanes = llrm_codegen::backend::lanes::enter(Regs(&llrm_m68k::REGISTER_INFO));
            let mut body = converted.body.clone();
            for phase in phases.iter_mut().take(until) {
                let before = shape(&body);
                body = phase.transform(body).unwrap_or_else(|why| panic!("{file} @{name}: {} refuses: {why}", phase.class_name()));
                if shape(&body) != before {
                    changed.insert(phase.class_name());
                }
            }
            functions += 1;
        }
    }
    println!("{functions} functions: {changed:?}");
    assert!(functions >= 12, "only {functions} functions");
    let allowed = ["MachineCSE", "PhiElimination", "TwoAddress", "Coalescer"];
    let extra: Vec<_> = changed.iter().filter(|name| !allowed.contains(name)).collect();
    assert!(extra.is_empty(), "phases that change m68k code and are not planned for it: {extra:?}");
}

/// The converted body through the verifier (`flow::checked`, as `assemble::phased`
/// runs each phase) is refused at the first phase: a physical register is one
/// value for the whole function (`Made::pinned`), defined at every write, so the
/// body is not SSA. Hosted stages skip the verifier today (`host_ssa` calls
/// `transform`). Step 4 turns this test round: the body must pass.
#[test]
fn the_converted_body_is_not_ssa_for_the_verifier() {
    llrm_support::debug::require_verification();
    let profile = tuned_for(&M68k, "m68000", false).unwrap_or_else(|why| panic!("{why}"));
    let regs = Regs(M68k.registers());
    let segments = Segments::of(&M68k.machine(), regs, M68k.operand_bytes());
    let classes = Rc::new(RegisterClasses::of(&M68k));
    let mut module = llrm_mir::parse::module(include_str!("fixtures/sieve_pin_o2.ll")).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let blocks = select_function(&module, "bench_sieve", Cpu::M68000).expect("selects");
    let mut function = emit_with("bench_sieve", &blocks, true).expect("emits");
    expand_cmov(&mut function).expect("expands");
    let mut phases = llrm_codegen::flow::machine_with(
        &Default::default(),
        None,
        None,
        None,
        false,
        profile,
        &segments,
        &classes,
        &Rules::NONE,
        &M68k.frame_registers(),
        llrm_codegen::backend::shadow::Selector::Old,
        &[],
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let converted = llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(&function, Regs(&llrm_m68k::REGISTER_INFO)).expect("converts");
    let _lanes = llrm_codegen::backend::lanes::enter(Regs(&llrm_m68k::REGISTER_INFO));
    let refused = llrm_codegen::flow::checked(converted.body.clone(), phases[0].as_mut(), true, None, &classes, &segments, &M68k);
    let Err(llrm_codegen::flow::Checked::Malformed(malformed)) = refused else { panic!("the verifier took the body") };
    assert!(malformed.0.contains("defined 2 times"), "{}", malformed.0);
    // the value it names is a pinned register, not a virtual one
    let named: u32 = malformed.0.split("value#").nth(1).and_then(|rest| rest.split(' ').next()).and_then(|n| n.parse().ok()).expect("a value is named");
    assert!(converted.body.pins.contains_key(&named), "value {named} is not pinned: {}", malformed.0);
}
