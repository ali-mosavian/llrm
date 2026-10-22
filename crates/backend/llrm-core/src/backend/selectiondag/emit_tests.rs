use std::collections::HashMap;

use llrm_dagisel::generated::x86_i486::MACHINE_OPCODES;
use llrm_lir::registers::RegId;
use llrm_target::Target;

use super::emit::{EmitContext, EmitError, RegisterFile, emit};
use super::opcodes::OpcodeMap;
use super::schedule::{SchedInfo, schedule};
use super::*;
use crate::model::ir::{Loc, Reg};

const NO_FLAGS: &[u16] = &[];

struct Info;

impl SchedInfo for Info {
    fn num_defs(
        &self,
        opcode: u32,
    ) -> usize {
        llrm_dagisel::instr::desc(opcode as u16).map_or(0, |d| usize::from(d.num_defs))
    }

    fn implicit_defs(
        &self,
        _opcode: u32,
    ) -> &[u16] {
        NO_FLAGS
    }

    fn is_call(
        &self,
        _opcode: u32,
    ) -> bool {
        false
    }
}

/// Physical register 1 is EAX; everything else is virtual.
struct Regs;

impl RegisterFile for Regs {
    fn physical(
        &self,
        reg: u32,
    ) -> Option<Reg> {
        (reg == 1).then_some(Reg { register: RegId::EAX, width: 32 })
    }
}

fn opcode(name: &str) -> u32 {
    u32::from(MACHINE_OPCODES.iter().find(|(_, n)| *n == name).unwrap_or_else(|| panic!("{name}")).0)
}

fn context(f: impl FnOnce(&EmitContext<'_>)) {
    let target = llrm_x86_m32::M32;
    let map = OpcodeMap::new(llrm_instr::parse(&target.forms_text(), target.dialect()).expect("x86.instr"));
    let names: HashMap<u16, &str> = MACHINE_OPCODES.iter().copied().collect();
    f(&EmitContext { map: &map, names: &names, regs: &Regs });
}

fn text(insns: &[std::sync::Arc<crate::model::lir::Insn>]) -> Vec<String> {
    insns
        .iter()
        .map(|one| {
            let what = one.what.as_ref().expect("semantics");
            format!("{:?} := {} {:?}", what.dests, what.name.as_deref().unwrap_or(""), what.sources)
        })
        .collect()
}

fn machine(
    dag: &mut SelectionDAG,
    name: &str,
    values: &[EVT],
    ops: &[SDValue],
) -> SDValue {
    dag.get_node_multi(ISD::Machine(opcode(name)), values, ops)
}

fn copy_to(
    dag: &mut SelectionDAG,
    chain: SDValue,
    reg: u32,
    value: SDValue,
) -> SDValue {
    let register = dag.get_register(EVT::I32, reg);
    dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER], &[chain, register, value])
}

/// `r3 = r1 + r2` through `ADD32rr` and a copy: the add reads both source
/// registers and the copy reads the add's result. A `CopyToReg` the emitter
/// dropped would leave the register unwritten.
#[test]
fn an_add_and_a_copy_become_two_instructions() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = copy_from(&mut dag, entry, 10);
    let b = copy_from(&mut dag, entry, 11);
    let sum = machine(&mut dag, "ADD32rr", &[EVT::I32], &[a, b]);
    let root = copy_to(&mut dag, entry, 12, sum);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits");
        let insns = &blocks[0].insns;
        let lines = text(insns);
        assert_eq!(insns.len(), 2, "{lines:?}");
        assert!(lines[0].contains("add"), "{lines:?}");
        assert!(lines[1].contains("mov"), "{lines:?}");
        let add = insns[0].what.as_ref().unwrap();
        let mov = insns[1].what.as_ref().unwrap();
        assert_eq!(add.sources.len(), 2);
        assert_eq!(mov.sources[0], add.dests[0]);
    });
}

fn copy_from(
    dag: &mut SelectionDAG,
    chain: SDValue,
    reg: u32,
) -> SDValue {
    let register = dag.get_register(EVT::I32, reg);
    dag.get_node_multi(ISD::CopyFromReg, &[EVT::I32, EVT::OTHER], &[chain, register])
}

/// Copying a value to a physical register is a `mov` into that register.
#[test]
fn a_copy_to_a_physical_register_is_a_mov() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let five = dag.get_constant(5, EVT::I32);
    let root = copy_to(&mut dag, entry, 1, five);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits");
        let insns = &blocks[0].insns;
        let mov = insns[0].what.as_ref().unwrap();
        assert!(matches!(mov.dests[0], Loc::Reg(Reg { register: RegId::EAX, .. })), "{mov:?}");
        assert!(matches!(mov.sources[0], Loc::Imm(ref imm) if imm.value == 5), "{mov:?}");
    });
}

/// A condition-coded opcode has no single form: it is refused, not guessed.
#[test]
fn an_unmapped_opcode_is_an_error() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = copy_from(&mut dag, entry, 10);
    let b = copy_from(&mut dag, entry, 11);
    let bsf = machine(&mut dag, "BSF32rr", &[EVT::I32], &[a, b]);
    let root = copy_to(&mut dag, entry, 12, bsf);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        assert!(matches!(emit(&dag, &units, ctx), Err(EmitError::Unmapped(_))));
    });
}

/// At i486 a `select` reaches the emitter as `CMOV_GR32`, and there is no
/// `cmov`: the emitter must make the diamond, a branch on the condition and
/// a phi of the two values in the join block. Emitted as one instruction it
/// would select the wrong operand on half the inputs.
#[test]
fn a_select_without_cmov_becomes_a_branch_and_a_phi() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let when_false = copy_from(&mut dag, entry, 10);
    let when_true = copy_from(&mut dag, entry, 11);
    let ne = dag.get_constant(5, EVT::I8);
    let select = machine(&mut dag, "CMOV_GR32", &[EVT::I32], &[when_false, when_true, ne]);
    let root = copy_to(&mut dag, entry, 12, select);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits");
        assert_eq!(blocks.len(), 3, "this, false, sink");
        let (this, fallthrough, sink) = (&blocks[0], &blocks[1], &blocks[2]);
        let branch = this.insns.last().and_then(|one| one.what.as_ref()).expect("a branch ends the first block");
        assert_eq!((branch.name.as_deref(), branch.target), (Some("jne"), Some(sink.at)));
        assert!(fallthrough.insns.is_empty());
        let [phi] = &sink.phis[..] else { panic!("one phi: {:?}", sink.phis) };
        assert_eq!(phi.incoming.len(), 2);
        assert_eq!(phi.incoming[0].0, fallthrough.at, "the false value comes from the false block");
        assert_eq!(phi.incoming[1].0, this.at, "the true value from the block that branched");
        let mov = sink.insns[0].what.as_ref().expect("the copy lands in the join block");
        assert_eq!(mov.sources[0], Loc::Held(crate::model::ir::Held { value: phi.result, width: 32 }));
    });
}
