//! What the shared `PhiElimination` and `TwoAddress` must keep of the m68k
//! code the private pre-RA passes make today, stated on the same 20 functions
//! as `shared_prefix.rs`. A private pass is retired only after its premise
//! holds here.

use std::rc::Rc;

use llrm_codegen::backend::{classes::RegisterClasses, cpu::tuned_for, peep::Rules, target::Segments};
use llrm_codegen::model::lir::LirBody;
use llrm_lir::registers::Regs;
use llrm_m68k::dagisel::function::select_function;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::mfunc::{MFunction, Shape, emit_with, expand_cmov};
use llrm_m68k::regalloc::M68kRegisters;
use llrm_m68k::target::M68k;
use llrm_spike_mf::mfunc::{Inst, Kind, Operand, Reg};
use llrm_spike_mf::regalloc::Target as _;
use llrm_target::Target;

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

/// Every defined function of the fixtures, as the private path starts from it.
fn functions() -> Vec<(String, MFunction)> {
    let mut out = Vec::new();
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
            let mut function = emit_with(&name, &blocks, Shape::PIN).expect("emits");
            expand_cmov(&mut function).expect("expands");
            out.push((format!("{file} @{name}"), function));
        }
    }
    out
}

/// The body after each shared phase up to and including `last`, as
/// `(phase name, body)`, starting from the converted SSA body.
fn through(
    function: &MFunction,
    last: &str,
) -> (llrm_spike_mf::tolir::Converted<M68kRegisters>, Vec<(&'static str, LirBody)>) {
    let profile = tuned_for(&M68k, "m68000", false).unwrap_or_else(|why| panic!("{why}"));
    let regs = Regs(M68k.registers());
    let segments = Segments::of(&M68k.machine(), regs, M68k.operand_bytes());
    let classes = Rc::new(RegisterClasses::of(&M68k));
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
    let converted = llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(function, Regs(&llrm_m68k::REGISTER_INFO)).expect("converts");
    let _lanes = llrm_codegen::backend::lanes::enter(Regs(&llrm_m68k::REGISTER_INFO));
    let mut body = converted.body.clone();
    let mut stages = Vec::new();
    for phase in phases.iter_mut() {
        use llrm_codegen::model::passes::LIRTransform;
        body = phase.transform(body).unwrap_or_else(|why| panic!("{} refuses: {why}", phase.class_name()));
        stages.push((phase.class_name(), body.clone()));
        if phase.class_name() == last {
            break;
        }
    }
    (converted, stages)
}

/// The position, among the register operands the converter keeps, of operand
/// `at` of `inst` (a reserved register is dropped, as in `convert`).
fn position(
    inst: &Inst,
    at: usize,
    def: bool,
) -> usize {
    inst.operands[..at]
        .iter()
        .filter(|operand| match operand {
            Operand::Def(Reg::Virt(_)) | Operand::Use(Reg::Virt(_)) => matches!(operand, Operand::Def(_)) == def,
            Operand::Def(Reg::Phys(register)) | Operand::Use(Reg::Phys(register)) => {
                matches!(operand, Operand::Def(_)) == def && !M68kRegisters::reserved(M68kRegisters::root(*register))
            }
            _ => false,
        })
        .count()
}

/// `(tied pairs checked, pairs whose def and use are not one value)` over
/// every instruction of `body` that is one of `function`'s own.
fn ties(
    function: &MFunction,
    converted: &llrm_spike_mf::tolir::Converted<M68kRegisters>,
    body: &LirBody,
) -> (usize, usize) {
    let (mut checked, mut open) = (0, 0);
    for block in &body.blocks {
        for insn in block.insns.iter() {
            let Some(&(b, i)) = converted.origin.get((insn.at / 2) as usize) else { continue };
            let inst = &function.blocks[b].insts[i];
            let (Kind::Op(name), Some(what)) = (inst.kind, &insn.what) else { continue };
            if what.name.as_deref() != Some(name) {
                continue;
            }
            for &(def, used) in &inst.ties {
                let (d, u) = (position(inst, def, true), position(inst, used, false));
                checked += 1;
                if insn.defines.get(d) != insn.uses.get(u) {
                    open += 1;
                }
            }
        }
    }
    (checked, open)
}

/// The shared `TwoAddress` ties exactly the pairs m68k's instructions declare
/// (`Inst::ties`): before it a tied def and use are two values, after it each
/// is one, on every instruction of the 20 functions. The private
/// `two_address` can go once this and the copy count agree.
#[test]
fn the_shared_two_address_ties_every_pair_m68k_declares() {
    let (mut checked, mut open_before) = (0, 0);
    for (what, function) in functions() {
        let (converted, stages) = through(&function, "TwoAddress");
        let before = stages.iter().rev().find(|(name, _)| *name == "PhiElimination").map(|(_, body)| body).expect("PhiElimination ran");
        let after = &stages.last().expect("TwoAddress ran").1;
        open_before += ties(&function, &converted, before).1;
        let (n, open) = ties(&function, &converted, after);
        assert_eq!(open, 0, "{what}: {open} of {n} tied pairs are still two values after TwoAddress");
        checked += n;
    }
    assert!(checked >= 20, "only {checked} tied pairs reached the check");
    assert!(open_before > 0, "no pair was open before TwoAddress: the check sees nothing");
}

/// Flag setter -> reader pairs of `body` (a setter, then a later reader of
/// its codes in the block) with a copy between them, and all such pairs.
fn split_pairs(
    function: &MFunction,
    converted: &llrm_spike_mf::tolir::Converted<M68kRegisters>,
    body: &LirBody,
) -> (usize, usize) {
    let (mut split, mut all) = (0, 0);
    for block in &body.blocks {
        let mut setter: Option<bool> = None; // Some(copy seen since)
        for insn in block.insns.iter() {
            let Some(&(b, i)) = converted.origin.get((insn.at / 2) as usize) else { continue };
            let inst = &function.blocks[b].insts[i];
            let is_copy = insn.what.as_ref().and_then(|w| w.name.as_deref()) == Some("COPY");
            if is_copy {
                setter = setter.map(|_| true);
                continue;
            }
            if inst.reads_flags && !inst.sets_flags {
                if let Some(copied) = setter {
                    all += 1;
                    split += usize::from(copied);
                }
            }
            if inst.sets_flags {
                setter = Some(false);
            }
        }
    }
    (split, all)
}

/// m68k declares no compare+branch bundle (`reads_flags_set_just_before` is
/// false), so the shared `PhiElimination` is free to put a phi copy between a
/// flag setter and its reader; m68k moves set CCR, so the copy would change the
/// branch. `split_flag_edges` (pre-RA) and `preserve_flags` (post-RA) are what
/// stop that today: they stay until this count is 0 on its own. The test
/// states the count, so a change in either direction is a decision.
#[test]
fn the_shared_phi_elimination_can_put_a_copy_between_a_flag_setter_and_its_reader() {
    let (mut before, mut after, mut pairs) = (0, 0, 0);
    for (_, function) in functions() {
        let (converted, stages) = through(&function, "PhiElimination");
        let (b, all) = split_pairs(&function, &converted, &converted.body);
        let (a, _) = split_pairs(&function, &converted, &stages.last().expect("ran").1);
        before += b;
        after += a;
        pairs += all;
    }
    println!("flag setter/reader pairs {pairs}; with a copy between: before {before}, after PhiElimination {after}");
    assert!(pairs >= 5, "only {pairs} setter/reader pairs in the fixtures: the test sees nothing");
    assert!(after >= before, "PhiElimination removed copies between setters and readers");
}
