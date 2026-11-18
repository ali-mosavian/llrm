//! The scheduling model of an AArch64 CPU for the pre-RA machine scheduler:
//! `TargetSchedModel` over the pin's `MCSchedModel` (the S1 tables of
//! `llrm_dagisel::sched`) and `AArch64Subtarget::overrideSchedPolicy`.
//!
//! # How it works
//! An instruction's form name is mapped to the pin's opcode
//! (`opcodes.instr`'s last column, then the opcode's number), the opcode to its
//! scheduling class, a variant class resolved by the instruction's own
//! operands ([`SchedPred`]). The class gives the micro-ops, the resources it
//! writes and the latency of its first def. An opcode the model does not state
//! takes `TargetInstrInfo::defaultDefLatency`: 0 for a copy, the model's
//! `LoadLatency` for a load, else 1.
//!
//! # Example
//! ```text
//! generic (Cortex-A510): ADDWri -> 1 micro-op, 1 cycle on CortexA510UnitALU
//! bidirectional, latency heuristic on (only Apple A7 and Neoverse V2 turn
//! it off)
//! ```
//!
//! # Not translated
//! - SB6: `ReadAdvance` (the use's forwarding adjustment of a latency) is not
//!   applied; the edge latency is the def's write latency. Owner arm64-phi,
//!   exit 2026-11-20.
//! - SB7: a form name that is several LLVM opcodes (`[A|B]`, a load's width)
//!   takes the first; `IsScaledAddr` is false. Owner arm64-phi, exit
//!   2026-11-20.
//!
//! # Used by
//! `lir_view`'s `AllocTarget::scheduling_model`.

use std::collections::HashMap;
use std::sync::OnceLock;

use llrm_codegen::backend::genericdriver::RegionPolicy;
use llrm_codegen::backend::machinesched::default_def_latency;
use llrm_codegen::backend::schedboundary::{Machine, ProcResource, Unit, WriteRes};
use llrm_codegen::backend::schedmodel::{GenericModel, MemBase, MemOp, SchedulingModel};
use llrm_codegen::model::ir::Loc;
use llrm_codegen::model::lir::Insn;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;
use llrm_dagisel::generated::aarch64::sched::CORTEX_A510_MODEL;
use llrm_dagisel::sched::{Modeled, SchedModel, SchedPred};

use crate::opcode_rows::llvm_names;

/// The pin's opcode number of each opcode name the DAG tables emit.
fn opcode_numbers() -> &'static HashMap<&'static str, u16> {
    static NUMBERS: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    NUMBERS.get_or_init(|| MACHINE_OPCODES.iter().map(|&(number, name)| (name, number)).collect())
}

/// An AArch64 CPU's model.
pub struct Arm64Model {
    model: &'static SchedModel,
    /// `FeatureDisableLatencySchedHeuristic` of the CPU's tuning.
    disable_latency: bool,
}

impl Arm64Model {
    /// `-mcpu=generic`: the Cortex-A510 model, whose tuning keeps the latency
    /// heuristic.
    pub fn generic() -> Self {
        Self { model: &CORTEX_A510_MODEL, disable_latency: false }
    }

    /// The model's class data for `insn`, or `None` where the model says
    /// nothing.
    fn class(
        &self,
        insn: &Insn,
    ) -> Option<llrm_dagisel::sched::ClassData> {
        let opcode = *opcode_numbers().get(pin_opcode(insn)?)?;
        // An opcode only the vector patterns emit has no class while the vectors are off: it was unmodeled before them.
        if !crate::machine::vectors_enabled() && llrm_dagisel::generated::aarch64::sched::dynamic_only(opcode) {
            return None;
        }
        let shifted = shift_amount(insn).is_some_and(|amount| amount != 0);
        let mut holds = |pred: SchedPred| match pred {
            SchedPred::HasShiftedReg => shifted,
            SchedPred::IsScaledAddr => false,
        };
        match self.model.resolve(opcode, &mut holds)? {
            Modeled::Class(data) => Some(data),
            Modeled::Unmodeled | Modeled::Unsupported => None,
        }
    }
}

/// The opcode the pin has for `insn` where the scheduler runs: the LLVM name
/// of its form, or of the operation `lir_view` names without a form (a frame
/// address is `ADDXri` of a frame index, a zero is `COPY $xzr`), with a
/// register offset's opcode chosen by its extend (`roX` for a 64-bit index,
/// `roW` for a 32-bit one; the shift is an operand, not part of the opcode).
fn pin_opcode(insn: &Insn) -> Option<&'static str> {
    let name = insn.what.as_ref()?.name.as_deref()?;
    // A floating-point zero is a pseudo (`FMOVS0`) until the pin expands it after
    // allocation to `fmov s, wzr`; the scheduler sees the pseudo.
    let zero_source = matches!(
        insn.what.as_ref()?.sources.first(),
        Some(Loc::Reg(reg)) if reg.register == crate::reg::XZR
    );
    match name {
        "FMOVHWr" | "FMOVHXr" if zero_source => return Some("FMOVH0"),
        "FMOVSWr" if zero_source => return Some("FMOVS0"),
        "FMOVDXr" if zero_source => return Some("FMOVD0"),
        "FrameAddress" => return Some("ADDXri"),
        "Zero" => return Some("COPY"),
        _ => {}
    }
    let register_offset = name.split_once('.').and_then(|(head, suffix)| {
        let digits = suffix.strip_prefix("ro")?.as_bytes();
        let wide = matches!(digits.first()?, b'3' | b'7');
        Some((format!("{head}.ro21"), if wide { "roX" } else { "roW" }))
    });
    match register_offset {
        Some((row, tail)) => llvm_names(&row).iter().copied().find(|opcode| opcode.ends_with(tail)),
        None => llvm_names(name).first().copied(),
    }
}

/// `MachineInstr::isCopy()` and which of its operands are physical, for
/// `biasPhysReg`: a zero is `COPY $xzr` (the register is no operand of ours);
/// a copy names a physical register as a register location where a virtual
/// one is a held value.
fn copy_physical(insn: &Insn) -> Option<(bool, bool)> {
    let what = insn.what.as_ref()?;
    match what.name.as_deref()? {
        "Zero" => Some((false, true)),
        "COPY" | "COPY_FPR" => Some((
            matches!(what.dests.first(), Some(Loc::Reg(_))),
            matches!(what.sources.first(), Some(Loc::Reg(_))),
        )),
        _ => None,
    }
}

/// The shift amount of a shifted-register form: its last source, an
/// immediate.
fn shift_amount(insn: &Insn) -> Option<i64> {
    match insn.what.as_ref()?.sources.last()? {
        Loc::Imm(imm) => Some(imm.value),
        _ => None,
    }
}

/// The model Darwin keeps (SB8): X86.td's `GenericModel` numbers, on a target
/// with no x87 stack.
pub struct DarwinModel;

impl SchedulingModel for DarwinModel {
    fn machine(&self) -> Machine {
        GenericModel.machine()
    }

    fn policy(&self) -> RegionPolicy {
        GenericModel.policy()
    }

    fn unit(
        &self,
        insn: &Insn,
    ) -> Unit {
        GenericModel.unit(insn)
    }

    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32 {
        GenericModel.def_latency(insn)
    }

    fn x87_stack(&self) -> bool {
        false
    }
}

impl SchedulingModel for Arm64Model {
    fn machine(&self) -> Machine {
        let resources = self
            .model
            .resources
            .iter()
            .map(|one| ProcResource {
                num_units: u32::from(one.num_units),
                buffer_size: one.buffer_size,
                sub_units: one.sub_units.iter().map(|&unit| usize::from(unit)).collect(),
            })
            .collect();
        Machine::new(self.model.issue_width, self.model.micro_op_buffer_size.max(0) as u32, resources)
    }

    fn x87_stack(&self) -> bool {
        false
    }

    fn exit_edges(&self) -> bool {
        true
    }

    /// `AArch64Subtarget::overrideSchedPolicy`.
    fn policy(&self) -> RegionPolicy {
        RegionPolicy::bidirectional(self.disable_latency)
    }

    /// `initSUnits`: the micro-ops and resource writes of the class, and
    /// whether it uses a buffer-1 (unbuffered) or buffer-0 (reserved)
    /// resource.
    fn unit(
        &self,
        insn: &Insn,
    ) -> Unit {
        let class = self.class(insn);
        if llrm_support::env_flag!("LLRM_MS_TRACE") {
            eprintln!(
                "machinesched class {} {}",
                insn.what.as_ref().and_then(|what| what.name.as_deref()).unwrap_or("-"),
                if class.is_some() { "hit" } else { "miss" }
            );
        }
        let copy_physical = copy_physical(insn);
        let Some(class) = class else { return Unit { micro_ops: 1, copy_physical, ..Unit::default() } };
        let mut unit = Unit {
            copy_physical,
            micro_ops: u32::from(class.num_micro_ops),
            begin_group: class.begin_group,
            end_group: class.end_group,
            ..Unit::default()
        };
        for write in class.write_proc_res {
            unit.writes.push(WriteRes {
                idx: usize::from(write.resource),
                release_at_cycle: u32::from(write.release_at_cycle),
                acquire_at_cycle: u32::from(write.acquire_at_cycle),
            });
            match self.model.resources[usize::from(write.resource)].buffer_size {
                0 => unit.has_reserved_resource = true,
                1 => unit.is_unbuffered = true,
                _ => {}
            }
        }
        unit
    }

    /// `computeOperandLatency` without the use's `ReadAdvance` (SB6).
    fn def_latency(
        &self,
        insn: &Insn,
    ) -> u32 {
        match self.class(insn).and_then(|class| class.write_latency.first().copied()) {
            // `capLatency`: a variable latency (-1) is 1000.
            Some(entry) => u32::try_from(entry.cycles).unwrap_or(1000),
            None => default_def_latency(insn, self.model.load_latency),
        }
    }

    /// `AArch64InstrInfo::getMemOperandsWithOffsetWidth` for the base plus
    /// immediate forms (`ui` and a frame slot). SB9: a pre/post-indexed form
    /// reports none (it only exists after `ldst_update`).
    fn mem_op(
        &self,
        insn: &Insn,
    ) -> Option<MemOp> {
        let what = insn.what.as_ref()?;
        let (access, form) = what.name.as_deref()?.split_once('.')?;
        let load = match access.get(..3)? {
            "LDR" => true,
            "STR" => false,
            _ => return None,
        };
        let width = access_bytes(access.get(3..)?)?;
        let Some(Loc::Mem(cell)) = what.sources.last() else { return None };
        let (base, offset) = match form {
            "ui" => (
                match cell.base {
                    Some(held) => MemBase::Virtual(held.value),
                    None => MemBase::Phys(cell.through),
                },
                cell.offset,
            ),
            "fi" => {
                let at = cell.addr?;
                (MemBase::Frame(at.slot_home()?), at.disp)
            }
            _ => return None,
        };
        Some(MemOp { base, offset, width, load })
    }

    /// `AArch64InstrInfo::shouldClusterMemOps`: two accesses that the load
    /// and store optimizer can make a pair.
    fn should_cluster(
        &self,
        (first, a): (&Insn, &MemOp),
        (second, b): (&Insn, &MemOp),
        cluster_size: u32,
        _bytes: u32,
    ) -> bool {
        if std::mem::discriminant(&a.base) != std::mem::discriminant(&b.base) {
            return false;
        }
        if !matches!(a.base, MemBase::Frame(_)) && a.base != b.base {
            return false;
        }
        // Only up to a single pair.
        if cluster_size > 2 {
            return false;
        }
        let (Some(kind_a), Some(kind_b)) = (pair_kind(first), pair_kind(second)) else { return false };
        if !can_pair(kind_a, kind_b) || !candidate_to_pair(first, a) || !candidate_to_pair(second, b) {
            return false;
        }
        // The offsets in units of the access; an unscaled form needs a
        // multiple.
        if a.offset % i64::from(a.width) != 0 || b.offset % i64::from(b.width) != 0 {
            return false;
        }
        let (scaled_a, scaled_b) = (a.offset / i64::from(a.width), b.offset / i64::from(b.width));
        // The pair instructions have a 7-bit signed offset.
        if !(-64..=63).contains(&scaled_a) {
            return false;
        }
        match a.base {
            MemBase::Frame(slot) => matches!(b.base, MemBase::Frame(other) if other == slot),
            _ => scaled_a + 1 == scaled_b,
        }
    }
}

/// The bytes a load or store form moves: its name after `LDR`/`STR`.
fn access_bytes(kind: &str) -> Option<u32> {
    Some(match kind {
        "B" | "SB" | "SBW" | "SBX" => 1,
        "H" | "SH" | "SHW" | "SHX" => 2,
        "W" | "SW" | "F4" => 4,
        "X" | "F8" => 8,
        "F16" => 16,
        _ => return None,
    })
}

/// `isPairableLdStInst`: the form's access kind, if a pair can hold it.
fn pair_kind(insn: &Insn) -> Option<&str> {
    let name = insn.what.as_ref()?.name.as_deref()?;
    let (access, form) = name.split_once('.')?;
    let kind = access.get(3..)?;
    (matches!(form, "ui" | "fi") && matches!(kind, "W" | "X" | "F4" | "F8" | "F16" | "SW")
        && (kind != "SW" || access.starts_with("LDR")))
    .then_some(kind)
}

/// `canPairLdStOpc`: the same kind, or a word with a sign-extended word.
fn can_pair(
    a: &str,
    b: &str,
) -> bool {
    a == b || matches!((a, b), ("W", "SW") | ("SW", "W"))
}

/// `isCandidateToMergeOrPair`: not volatile, and a load does not write its
/// own base register.
fn candidate_to_pair(
    insn: &Insn,
    op: &MemOp,
) -> bool {
    if insn.volatile() {
        return false;
    }
    match op.base {
        MemBase::Virtual(value) => !insn.defines.contains(&value),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_codegen::model::ir::{Held, Imm, Operation, Semantics};

    use super::*;

    fn form(
        name: &str,
        sources: Vec<Loc>,
    ) -> Insn {
        let held = |value| Loc::Held(Held { value, width: 4, offset: 0 });
        let what =
            Semantics { name: Some(name.into()), dests: vec![held(1)], sources, ..Semantics::new(Operation::Binary) };
        Insn::new(0, Some((0, 0)), Some(what), vec![1], vec![2])
    }

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4, offset: 0 })
    }

    fn imm(value: i64) -> Loc {
        Loc::Imm(Imm { value, width: 4, address: None })
    }

    /// A frame address is `ADDXri` of a frame index to the pin; the model
    /// missed it and priced it as an unmodeled opcode (no resource write).
    #[test]
    fn a_frame_address_is_priced_as_an_add_immediate() {
        let model = Arm64Model::generic();
        let address = form("FrameAddress", vec![]);
        let add = form("ADDXri", vec![held(2), imm(1), imm(0)]);
        let resources = |one: &Insn| model.unit(one).writes.iter().map(|write| write.idx).collect::<Vec<_>>();
        assert_eq!(resources(&address), resources(&add));
        assert_eq!(model.def_latency(&address), model.def_latency(&add));
        assert!(!model.unit(&address).writes.is_empty(), "it writes a resource of the model");
    }

    /// A copy and a zero (`COPY $xzr`) are the model's `COPY` class (one
    /// micro-op of one cycle on a resource): `COPY` had no number in the
    /// table, so both were priced as unmodeled (matmul O2: 2 copies, 2 zeros).
    #[test]
    fn a_copy_and_a_zero_are_priced_by_the_models_copy_class() {
        let model = Arm64Model::generic();
        for name in ["COPY", "Zero"] {
            let one = form(name, vec![held(2)]);
            assert_eq!((model.unit(&one).micro_ops, model.def_latency(&one)), (1, 1), "{name}");
            assert!(!model.unit(&one).writes.is_empty(), "{name} writes a resource of the model");
        }
    }

    /// `fmov s0, wzr` is the pin's `FMOVS0` where the scheduler runs (the
    /// pseudo expands after allocation): its own class (four cycles on the
    /// A510), not the three of a transfer from a value.
    #[test]
    fn a_floating_point_zero_is_the_pin_pseudo_not_a_transfer() {
        let zero = |name: &str, source: Loc| pin_opcode(&form(name, vec![source]));
        let wzr = Loc::Reg(llrm_lir::Reg { register: crate::reg::XZR, width: 4 });
        assert_eq!(zero("FMOVSWr", wzr.clone()), Some("FMOVS0"));
        assert_eq!(zero("FMOVDXr", wzr.clone()), Some("FMOVD0"));
        assert_eq!(zero("FMOVSWr", held(2)), Some("FMOVSWr"), "a move from a value stays a transfer");
        let model = Arm64Model::generic();
        let latency = |name: &str, source: Loc| model.def_latency(&form(name, vec![source]));
        assert_eq!((latency("FMOVSWr", wzr), latency("FMOVSWr", held(2))), (4, 3));
    }

    /// `COPY $xzr` is a copy of a physical register in; a virtual copy is
    /// none: `biasPhysReg` starts the first from the top (nbody O2).
    #[test]
    fn a_zero_is_a_copy_of_a_physical_register() {
        let model = Arm64Model::generic();
        assert_eq!(model.unit(&form("Zero", vec![])).copy_physical, Some((false, true)));
        let phys = Loc::Reg(llrm_lir::Reg { register: crate::reg::X0, width: 8 });
        assert_eq!(model.unit(&form("COPY", vec![phys])).copy_physical, Some((false, true)));
        assert_eq!(model.unit(&form("COPY", vec![held(2)])).copy_physical, Some((false, false)));
        assert_eq!(model.unit(&form("ADDXri", vec![held(2), imm(1), imm(0)])).copy_physical, None);
    }

    /// A register offset is `roX` for a 64-bit index (`uxtx`, `sxtx`) and
    /// `roW` for a 32-bit one, whatever the shift: `STRX.ro30` is `STRXroX`.
    /// Only `ro21` was in the file, so every other register offset missed
    /// the model.
    #[test]
    fn a_register_offset_is_the_pin_opcode_of_its_index_width() {
        let opcode = |name: &str| pin_opcode(&form(name, vec![]));
        assert_eq!(opcode("STRX.ro30"), Some("STRXroX"));
        assert_eq!(opcode("STRX.ro31"), Some("STRXroX"));
        assert_eq!(opcode("STRX.ro21"), Some("STRXroW"));
        assert_eq!(opcode("LDRX.ro60"), Some("LDRXroW"));
        assert_eq!(opcode("LDRX.ro70"), Some("LDRXroX"));
        let model = Arm64Model::generic();
        assert!(!model.unit(&form("STRX.ro30", vec![])).writes.is_empty(), "it writes a resource of the model");
    }

    /// `SUBXri`, `SUBWri` and others a form stands for had no number in the
    /// table (only the opcodes the DAG patterns emit were listed), so the
    /// model said nothing about them and the scheduler priced them as
    /// unmodeled (matmul O2: `SUBXri`).
    #[test]
    fn every_opcode_a_form_stands_for_has_a_number_in_the_table() {
        let numbered: std::collections::HashSet<&str> =
            opcode_numbers().keys().flat_map(|names| names.split('|')).collect();
        let missing: Vec<_> =
            crate::opcode_rows::LLVM_OPCODES.iter().filter(|name| !numbered.contains(**name)).collect();
        assert!(missing.is_empty(), "{} opcodes without a number: {missing:?}", missing.len());
    }

    /// The module header's example, read off the pin's Cortex-A510 tables:
    /// an add of an immediate is one micro-op of one cycle.
    #[test]
    fn an_add_immediate_is_one_micro_op_of_one_cycle_on_the_generic_cpu() {
        let model = Arm64Model::generic();
        let add = form("ADDWri", vec![held(2), imm(1), imm(0)]);
        assert_eq!((model.unit(&add).micro_ops, model.def_latency(&add)), (1, 1));
        assert!(!model.unit(&add).writes.is_empty(), "it writes a resource of the model");
    }

    /// An opcode the model does not state is priced as the pin prices it by
    /// default: the unit is one micro-op and the latency is
    /// `defaultDefLatency`.
    #[test]
    fn a_form_the_model_does_not_know_takes_the_default_latency() {
        let model = Arm64Model::generic();
        let unknown = Arc::new(form("NoSuchForm", vec![held(2)]));
        assert_eq!(
            (model.unit(&unknown).micro_ops, model.def_latency(&unknown)),
            (1, default_def_latency(&unknown, 3))
        );
    }

    /// With the vector types live the generated tables dropped `FMOVWSr` (a
    /// scalar float bitcast, `fmov w, s`) and gave `MVNIv2i32` a class: with
    /// the vectors off the first was priced by default and the second by its
    /// row, and `sfm16_float` scheduled its `fmov` after the `fcmp` (8 of 188
    /// objects differed from release/next). The bitcast keeps its class on
    /// both sides; an opcode only the vectors reach has a class exactly while
    /// they are on (`MOVIv2d_ns` is hand-selected too, so the model always
    /// priced it).
    #[test]
    fn a_scalar_bitcast_has_a_class_and_a_vector_movi_has_one_only_with_the_vectors_on() {
        let model = Arm64Model::generic();
        let bitcast = Arc::new(form("FMOVWSr", vec![held(2)]));
        assert_ne!(model.def_latency(&bitcast), default_def_latency(&bitcast, 3), "FMOVWSr is priced by its row");
        // `MOVIv2d_ns` is a hand-selected opcode with a class since #9048;
        // `MOVIv2i32` only the vector patterns emit.
        let movi = Arc::new(form("MOVIv2i32", vec![imm(0)]));
        let priced_by_default = model.def_latency(&movi) == default_def_latency(&movi, 3);
        assert_eq!(priced_by_default, !crate::machine::vectors_enabled(), "MOVIv2i32 has a class only with the vectors on");
    }

    /// AArch64 schedules from both ends and keeps the latency heuristic on
    /// the generic CPU.
    #[test]
    fn the_policy_is_bidirectional_with_the_latency_heuristic() {
        let policy = Arm64Model::generic().policy();
        assert_eq!((policy.only_top_down, policy.only_bottom_up, policy.disable_latency), (false, false, false));
    }

    fn access(
        name: &str,
        base: u32,
        offset: i64,
    ) -> Insn {
        let cell = Loc::Mem(llrm_codegen::model::ir::Mem { base: Some(Held { value: base, width: 8, offset: 0 }), offset, ..llrm_codegen::model::ir::Mem::new(None, 8) });
        let what = Semantics { name: Some(name.into()), dests: vec![held(1)], sources: vec![cell], ..Semantics::new(Operation::Move) };
        Insn::new(0, Some((0, 0)), Some(what), vec![1], vec![base])
    }

    fn clusters(
        model: &Arm64Model,
        (first, a): (&str, i64),
        (second, b): (&str, i64),
    ) -> bool {
        let (first, second) = (access(first, 7, a), access(second, 7, b));
        let (x, y) = (model.mem_op(&first).unwrap(), model.mem_op(&second).unwrap());
        model.should_cluster((&first, &x), (&second, &y), 2, x.width + y.width)
    }

    /// `shouldClusterMemOps`: two word loads from the same base at
    /// neighbouring slots can be an `ldp`; a gap, another size or a byte
    /// load cannot.
    #[test]
    fn neighbouring_accesses_of_one_base_cluster_and_the_rest_do_not() {
        let model = Arm64Model::generic();
        assert!(clusters(&model, ("LDRW.ui", 8), ("LDRW.ui", 12)));
        assert!(clusters(&model, ("LDRX.ui", 0), ("LDRX.ui", 8)));
        assert!(!clusters(&model, ("LDRW.ui", 8), ("LDRW.ui", 16)), "a slot between");
        assert!(!clusters(&model, ("LDRW.ui", 8), ("LDRX.ui", 12)), "two sizes");
        assert!(!clusters(&model, ("LDRB.ui", 8), ("LDRB.ui", 9)), "no byte pair");
        assert!(!clusters(&model, ("LDRX.ui", 512), ("LDRX.ui", 520)), "the pair's 7-bit offset ends at slot 63");
        assert!(!clusters(&model, ("LDRW.ui", 6), ("LDRW.ui", 10)), "an unscaled offset is no multiple of the size");
    }
}
