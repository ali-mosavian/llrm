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
            let blocks =
                select_function(&module, &name, Cpu::M68000).unwrap_or_else(|why| panic!("{file} @{name}: {why}"));
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
    let converted =
        llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(function, Regs(&llrm_m68k::REGISTER_INFO)).expect("converts");
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
            let Some(&(b, i)) = converted.origin.get(llrm_spike_mf::tolir::ordinal_of(insn)) else { continue };
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
        let before = stages
            .iter()
            .rev()
            .find(|(name, _)| *name == "PhiElimination")
            .map(|(_, body)| body)
            .expect("PhiElimination ran");
        let after = &stages.last().expect("TwoAddress ran").1;
        open_before += ties(&function, &converted, before).1;
        let (n, open) = ties(&function, &converted, after);
        assert_eq!(open, 0, "{what}: {open} of {n} tied pairs are still two values after TwoAddress");
        checked += n;
    }
    assert!(checked >= 20, "only {checked} tied pairs reached the check");
    assert!(open_before > 0, "no pair was open before TwoAddress: the check sees nothing");
}

/// The `mov`s TwoAddress added, each with the name of the instruction after
/// it (the one it feeds). By identity, not by `at`: MachineSink hands two
/// instructions one `at`, so an `at` lookup names the wrong instruction.
fn added_moves(
    before: &LirBody,
    after: &LirBody,
) -> Vec<String> {
    let was: std::collections::HashSet<u64> =
        before.blocks.iter().flat_map(|block| block.insns.iter()).map(|insn| insn.id()).collect();
    let is_mov = |insn: &llrm_codegen::model::lir::Insn| {
        insn.what.as_ref().is_some_and(|what| what.name.as_deref() == Some("mov"))
    };
    let mut out = Vec::new();
    for block in &after.blocks {
        let insns: Vec<_> = block.insns.iter().collect();
        for (i, insn) in insns.iter().enumerate() {
            if is_mov(insn) && !was.contains(&insn.id()) {
                let next = insns[i + 1..].iter().find(|one| !is_mov(one));
                out.push(next.and_then(|one| one.what.as_ref()?.name.as_deref()).unwrap_or_default().to_string());
            }
        }
    }
    out
}

/// The converse of the test above: the shared `TwoAddress` copies a source only
/// before an instruction m68k declares a tie for. Without `@ties rows` in
/// m68k's register description it ties by x86's rule (an `Add` is tied), so
/// `LEA32p d, #48, v` got `mov t, #48` ahead of it, which m68k cannot place
/// (3 of 20 fixture functions refused, 11-10). Counted: each `mov` TwoAddress
/// adds ahead of an instruction with no declared tie. 36 before the rows; 3
/// after, which were the instrument: it found the instruction by `at`, and
/// MachineSink gives two instructions one `at` (pack_ref, add_bits_64,
/// sf64_pack), so a copy before a tied `OR8dd` was charged to a `MOVZXd32d8`.
#[test]
fn the_shared_two_address_ties_only_the_pairs_m68k_declares() {
    let functions = functions();
    let tied_names: std::collections::HashSet<&str> = functions
        .iter()
        .flat_map(|(_, function)| function.blocks.iter().flat_map(|block| block.insts.iter()))
        .filter(|inst| !inst.ties.is_empty())
        .filter_map(|inst| if let Kind::Op(name) = inst.kind { Some(name) } else { None })
        .collect();
    let (mut extra, mut tied) = (Vec::new(), 0);
    for (what, function) in &functions {
        let (_, stages) = through(function, "TwoAddress");
        let before = stages
            .iter()
            .rev()
            .find(|(name, _)| *name == "PhiElimination")
            .map(|(_, body)| body)
            .expect("PhiElimination ran");
        let after = &stages.last().expect("TwoAddress ran").1;
        for name in added_moves(before, after) {
            if tied_names.contains(name.as_str()) {
                tied += 1;
            } else {
                extra.push(format!("{what}: a copy before {name:?}"));
            }
        }
    }
    assert!(tied >= 20, "only {tied} copies stand before a declared tie: the check sees nothing");
    assert!(extra.is_empty(), "TwoAddress copied a source before instructions m68k declares no tie for: {extra:#?}");
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
            let Some(&(b, i)) = converted.origin.get(llrm_spike_mf::tolir::ordinal_of(insn)) else { continue };
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

/// Instructions of `body` whose origin, read as `from_ssa` and `from_lir` read
/// it (`tolir::ordinal_of`), is another MIR instruction than the one they are.
fn misattached(
    function: &MFunction,
    converted: &llrm_spike_mf::tolir::Converted<M68kRegisters>,
    body: &LirBody,
) -> usize {
    let mut open = 0;
    for insn in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        // one the converter did not number (the `jmp` on an edge MachineSink
        // splits) has no origin
        if insn.numbered.is_none() {
            continue;
        }
        let Some(&(b, i)) = converted.origin.get(llrm_spike_mf::tolir::ordinal_of(insn)) else { continue };
        let (Kind::Op(name), Some(what)) = (function.blocks[b].insts[i].kind, &insn.what) else { continue };
        let own = what.name.as_deref();
        // "" is a branch to the next block, which the sink empties in place
        if own != Some(name) && !matches!(own, Some("mov" | "COPY" | "")) {
            open += 1;
        }
    }
    open
}

/// MachineSink sets a sunk instruction's `at` to the first of the block it goes
/// to, so `at / 2` named the MIR instruction it landed beside: the sunk `OR8dd`
/// of pack_ref got the operands and opcode of the `MOVZXd32d8` next to it
/// (30 mis-attached over the fixture functions, 11-11). The origin now rides on
/// the instruction (`Insn::numbered`).
#[test]
fn machine_sink_keeps_every_instruction_attached_to_its_origin() {
    let (mut before, mut sunk, mut after) = (0, 0, 0);
    for (_, function) in functions() {
        let (converted, stages) = through(&function, "MachineSink");
        before += misattached(&function, &converted, &converted.body);
        let last = stages.len() - 1;
        sunk += misattached(&function, &converted, &stages[last - 1].1);
        after += misattached(&function, &converted, &stages[last].1);
    }
    println!("mis-attached: converted {before}, before MachineSink {sunk}, after {after}");
    assert_eq!(before, 0, "the converted body already mis-attaches: the check is wrong");
    assert_eq!(after, sunk, "MachineSink moved instructions onto another instruction's origin");
}

/// The prelude `Options::shared_prelude` runs (`regalloc::shared_prelude`)
/// sinks instructions, and every one stays attached to its own origin: read by
/// `at`, 30 of the fixture functions' instructions named a neighbour's. Counted
/// over the prelude's own output, not a replay of its phases.
#[test]
fn the_prelude_sinks_and_every_instruction_keeps_its_origin() {
    let (mut moved, mut open) = (0, 0);
    for (_, function) in functions() {
        let converted = llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(&function, Regs(&llrm_m68k::REGISTER_INFO))
            .expect("converts");
        let _lanes = llrm_codegen::backend::lanes::enter(Regs(&llrm_m68k::REGISTER_INFO));
        let body = llrm_m68k::regalloc::shared_prelude(converted.body.clone())
            .unwrap_or_else(|why| panic!("prelude refuses: {why}"));
        open += misattached(&function, &converted, &body);
        moved += body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .filter(|insn| insn.numbered.is_some_and(|n| i64::from(n) != insn.at / 2))
            .count();
    }
    println!("sunk {moved}, mis-attached {open}");
    assert!(moved > 0, "the prelude moved no instruction: it runs no MachineSink");
    assert_eq!(open, 0, "the prelude left instructions attached to another instruction's origin");
}

/// The control for the converter's own tie pairs: where no operand is dropped
/// they are the pairs the description's row states, for every instruction of
/// the fixture functions that has one.
#[test]
fn the_converters_tie_pairs_are_the_rows_where_nothing_is_dropped() {
    let regs = Regs(&llrm_m68k::REGISTER_INFO);
    let (mut same, mut dropped) = (0, 0);
    for (what, function) in functions() {
        let converted = llrm_spike_mf::tolir::to_ssa_lir::<M68kRegisters>(&function, regs).expect("converts");
        for insn in converted.body.blocks.iter().flat_map(|block| block.insns.iter()) {
            let Some(semantics) = &insn.what else { continue };
            let Some(pairs) = &semantics.tied else { continue };
            let Some(&(b, i)) = converted.origin.get(llrm_spike_mf::tolir::ordinal_of(insn)) else { continue };
            let inst = &function.blocks[b].insts[i];
            let name = semantics.name.as_deref().unwrap_or("");
            let row = (regs.effects)(name, semantics.dests.len(), semantics.sources.len())
                .first()
                .map(|row| row.ties.to_vec());
            let lost = inst
                .operands
                .iter()
                .any(
                    |operand| match operand {
                        Operand::Use(Reg::Phys(register)) | Operand::Def(Reg::Phys(register)) => {
                            M68kRegisters::reserved(M68kRegisters::root(*register))
                        }
                        Operand::Block(_) => true,
                        _ => false,
                    },
                );
            if lost {
                dropped += 1;
                continue;
            }
            assert_eq!(row.as_ref(), Some(pairs), "{what}: {name}: the converter's pairs are not the row's");
            same += 1;
        }
    }
    assert!(same > 0, "no tied instruction reached the control");
    println!("{same} tied instructions equal their row, {dropped} lose an operand");
}
