use llrm_dagisel::generated::x86_i486::MACHINE_OPCODES;
use llrm_dagisel::x86_addr::AddrMode;
use llrm_lir::registers::RegId;
use llrm_support::hash::HashMap;
use llrm_target::Target;

use super::emit::{EmitContext, EmitError, emit};
use super::opcodes::OpcodeMap;
use super::schedule::{SchedInfo, schedule};
use super::*;
use crate::model::ir::{Loc, Reg};

const EAX: u32 = u32::MAX;
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

struct NoGlobals;

impl super::emit::GlobalMap for NoGlobals {
    fn segment(
        &self,
        _symbol: Option<u32>,
    ) -> crate::model::ir::Addr {
        crate::model::ir::Addr::new(crate::model::ir::Space::Group, 0)
    }

    fn address(
        &self,
        symbol: u32,
        offset: i64,
    ) -> crate::model::ir::Addr {
        crate::model::ir::Addr {
            index: i64::from(symbol),
            ..crate::model::ir::Addr::new(crate::model::ir::Space::Segment, offset)
        }
    }
}

struct NoFrame;

impl super::emit::FrameMap for NoFrame {
    fn cell(
        &self,
        _index: i32,
        _width: u32,
    ) -> Option<crate::model::ir::Mem> {
        None
    }
}

fn opcode(name: &str) -> u32 {
    u32::from(MACHINE_OPCODES.iter().find(|(_, n)| *n == name).unwrap_or_else(|| panic!("{name}")).0)
}

fn context(f: impl FnOnce(&EmitContext<'_>)) {
    context_in(AddrMode::Flat32, f)
}

fn context_in(
    mode: AddrMode,
    f: impl FnOnce(&EmitContext<'_>),
) {
    let target = llrm_x86_m32::M32;
    let map = OpcodeMap::new(llrm_target::instr::parse(&target.forms_text(), target.dialect()).expect("x86.instr"));
    let names: HashMap<u16, &str> = MACHINE_OPCODES.iter().copied().collect();
    let no_blocks = HashMap::default();
    f(&EmitContext {
        tables: &[],
        caller_saved: &Default::default(),
        conventions: &super::emit::NoConventions,
        constants: &super::emit::NoConstants,
        stack_pointer: RegId::SP,
        strings: None,
        registers: llrm_target::Target::registers(&target),
        globals: &NoGlobals,
        block_at: &no_blocks,
        frame: &NoFrame,
        address_mode: mode,
        opt_size: false,
        call_alignment: 1,
        map: &map,
        names: &names,
    });
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
    let register =
        if reg == EAX { dag.get_phys_register(EVT::I32, RegId::EAX) } else { dag.get_register(EVT::I32, reg) };
    dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER], &[chain, register, value])
}

/// `r3 = r1 + r2` through `ADD32rr` and a copy: the add defines r3 itself
/// (`InstrEmitter::CreateVirtualRegisters`); a `mov` after it was a phi copy
/// per loop edge in x_isqrt that the pin does not have.
#[test]
fn an_add_copied_to_a_virtual_register_defines_it() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = copy_from(&mut dag, entry, 10);
    let b = copy_from(&mut dag, entry, 11);
    let sum = machine(&mut dag, "ADD32rr", &[EVT::I32], &[a, b]);
    let root = copy_to(&mut dag, entry, 12, sum);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits").blocks;
        let insns = &blocks[0].insns;
        let lines = text(insns);
        assert_eq!(insns.len(), 1, "{lines:?}");
        assert!(lines[0].contains("add"), "{lines:?}");
        assert_eq!(insns[0].what.as_ref().unwrap().sources.len(), 2);
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
    let root = copy_to(&mut dag, entry, EAX, five);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits").blocks;
        let insns = &blocks[0].insns;
        let mov = insns[0].what.as_ref().unwrap();
        assert!(matches!(mov.dests[0], Loc::Reg(Reg { register: RegId::EAX, .. })), "{mov:?}");
        assert!(matches!(mov.sources[0], Loc::Imm(ref imm) if imm.value == 5), "{mov:?}");
    });
}

/// An opcode with no form (an atomic exchange-add) is refused, not guessed.
#[test]
fn an_unmapped_opcode_is_an_error() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = copy_from(&mut dag, entry, 10);
    let b = copy_from(&mut dag, entry, 11);
    let cmov = machine(&mut dag, "LXADD32", &[EVT::I32], &[a, b]);
    let root = copy_to(&mut dag, entry, 12, cmov);
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
        let blocks = emit(&dag, &units, ctx).expect("emits").blocks;
        assert_eq!(blocks.len(), 3, "this, false, sink");
        let (this, fallthrough, sink) = (&blocks[0], &blocks[1], &blocks[2]);
        let branch = this.insns.last().and_then(|one| one.what.as_ref()).expect("a branch ends the first block");
        assert_eq!((branch.name.as_deref(), branch.target), (Some("jne"), Some(sink.at)));
        assert!(fallthrough.insns.is_empty());
        let [phi] = &sink.phis[..] else { panic!("one phi: {:?}", sink.phis) };
        assert_eq!(phi.incoming.len(), 2);
        assert_eq!(phi.incoming[0].0, fallthrough.at, "the false value comes from the false block");
        assert_eq!(phi.incoming[1].0, this.at, "the true value from the block that branched");
        // The result is made in %12 itself (`CreateVirtualRegisters`): no
        // export copy follows the select.
        assert!(sink.insns.is_empty(), "{:?}", sink.insns);
    });
}

/// In real mode a base is BX or BP and an index SI or DI; the pin's
/// `constrainRegClass` is what tells the allocator. A flat address
/// constrains nothing. Without the tags the allocator may put the base in AX,
/// which no real-mode address can use.
#[test]
fn real_mode_address_components_are_confined_to_their_classes() {
    for (mode, want) in [(AddrMode::Real16, vec![("base", 0), ("index", 1)]), (AddrMode::Flat32, vec![])] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let base = copy_from(&mut dag, entry, 10);
        let index = copy_from(&mut dag, entry, 11);
        let scale = dag.get_constant(1, EVT::I8);
        let disp = dag.get_constant(4, EVT::I32);
        let noreg = dag.get_register(EVT::I16, 0);
        let load = machine(&mut dag, "MOV16rm", &[EVT::I16, EVT::OTHER], &[base, scale, index, disp, noreg, entry]);
        let root = copy_to(&mut dag, entry, 12, load);
        context_in(mode, |ctx| {
            let units = schedule(&dag, root, &Info);
            let emitted = emit(&dag, &units, ctx).expect("emits");
            let value = |loc: &Loc| match loc {
                Loc::Held(held) => held.value,
                other => panic!("{other:?}"),
            };
            let load = emitted.blocks[0]
                .insns
                .iter()
                .find_map(|i| i.what.as_ref().filter(|w| w.name.as_deref() == Some("mov")))
                .unwrap();
            let Loc::Mem(cell) = &load.sources[0] else { panic!("{load:?}") };
            let ids = [cell.base.unwrap().value, cell.index.unwrap().value];
            let got: Vec<(&str, usize)> =
                emitted.classes.iter().map(|(v, t)| (*t, ids.iter().position(|i| i == v).unwrap())).collect();
            assert_eq!(got, want, "{mode:?} {}", value(&Loc::Held(cell.base.unwrap())));
        });
    }
}

/// `SHL32rCL` reads `CL` with no operand: the amount is copied to `CL` by
/// the `CopyToReg` glued to it. The emitter looked for a second source in the
/// node's operands and stopped at "SHL32rCL: missing source" (9 m32 programs).
#[test]
fn a_shift_by_cl_reads_cl() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let value = copy_from(&mut dag, entry, 10);
    let register = dag.get_register(EVT::I8, 11);
    let amount = dag.get_node_multi(ISD::CopyFromReg, &[EVT::I8, EVT::OTHER], &[entry, register]);
    let cl = dag.get_phys_register(EVT::I8, RegId::CL);
    let set = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &[entry, cl, amount]);
    let glue = SDValue { node: set.node, res: 1 };
    let shifted = machine(&mut dag, "SHL32rCL", &[EVT::I32, EVT::I32], &[value, glue]);
    let root = copy_to(&mut dag, entry, 12, shifted);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits").blocks;
        let lines = text(&blocks[0].insns);
        let shl =
            blocks[0].insns.iter().find(|one| one.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("shl")));
        let shl = shl.unwrap_or_else(|| panic!("{lines:?}")).what.as_ref().unwrap();
        // the amount is a value the form pins to `cx`, not a physical register
        // nobody defines (the LIR verifier: "value defined N times")
        assert!(matches!(shl.sources.last(), Some(Loc::Held(_))), "{lines:?}");
        assert!(lines.iter().all(|line| !line.contains("CL")), "{lines:?}");
    });
}

/// `lea ax, [index*4 + 160]` at m16 was refused ('a lea of a symbol and a
/// scaled index at m16': QCport's menu.c and sbar.c under the DAG default):
/// 16-bit addressing has no scale, so the index is shifted first.
#[test]
fn a_lea_of_a_constant_and_a_scaled_index_at_m16_shifts_the_index() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let index = copy_from(&mut dag, entry, 11);
    let scale = dag.get_constant(4, EVT::I8);
    let disp = dag.get_constant(160, EVT::I32);
    let noreg = dag.get_register(EVT::I16, 0);
    let lea = machine(&mut dag, "LEA32r", &[EVT::I32], &[noreg, scale, index, disp, noreg]);
    let root = copy_to(&mut dag, entry, 12, lea);
    context_in(AddrMode::Real16, |ctx| {
        let units = schedule(&dag, root, &Info);
        let emitted = emit(&dag, &units, ctx).expect("emits");
        let names: Vec<_> = emitted.blocks[0].insns.iter().filter_map(|i| i.what.as_ref()?.name.clone()).collect();
        assert_eq!(names.iter().map(|n| &**n).collect::<Vec<_>>(), ["shl", "lea", "mov"]);
    });
}

/// `lea edx, [edx+edx*4]` at m16 carried a dword displacement of zero: nine
/// bytes against the five of mod 00 (lru -O2 +27 B). Only an index alone needs
/// the disp32.
#[test]
fn a_lea_of_a_base_and_a_scaled_dword_index_at_m16_has_no_displacement() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let base = copy_from(&mut dag, entry, 10);
    let index = copy_from(&mut dag, entry, 11);
    let scale = dag.get_constant(4, EVT::I8);
    let disp = dag.get_constant(0, EVT::I32);
    let noreg = dag.get_register(EVT::I16, 0);
    let lea = machine(&mut dag, "LEA32r", &[EVT::I32], &[base, scale, index, disp, noreg]);
    let root = copy_to(&mut dag, entry, 12, lea);
    context_in(AddrMode::Real16, |ctx| {
        let units = schedule(&dag, root, &Info);
        let emitted = emit(&dag, &units, ctx).expect("emits");
        let lea = emitted.blocks[0].insns.iter().find(|i| i.what.as_ref().is_some_and(|w| w.name.as_deref() == Some("lea"))).expect("a lea");
        let Some(llrm_lir::Loc::Mem(cell)) = lea.what.as_ref().and_then(|w| w.sources.first()) else { panic!("{lea:?}") };
        assert_eq!(cell.disp_width, 0, "{cell:?}");
    });
}

/// `r3 = r1 + r2` on the x87 stack through `ADD_Fp64` and a copy: the add
/// defines r3 itself, as `CreateVirtualRegisters` has it. Made apart with a
/// `mov` after it, a loop's old-value copy stayed where the pin coalesces it
/// (`recfloat` -O2 m32: 10 instructions a trip against the pin's 9).
#[test]
fn an_x87_add_copied_to_a_virtual_register_defines_it() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a_reg = dag.get_register(EVT::Simple(MVT::F64), 10);
    let b_reg = dag.get_register(EVT::Simple(MVT::F64), 11);
    let a = dag.get_node_multi(ISD::CopyFromReg, &[EVT::Simple(MVT::F64), EVT::OTHER], &[entry, a_reg]);
    let b = dag.get_node_multi(ISD::CopyFromReg, &[EVT::Simple(MVT::F64), EVT::OTHER], &[entry, b_reg]);
    let sum = machine(&mut dag, "ADD_Fp64", &[EVT::Simple(MVT::F64)], &[a, b]);
    let out = dag.get_register(EVT::Simple(MVT::F64), 12);
    let root = dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER], &[entry, out, sum]);
    context(|ctx| {
        let units = schedule(&dag, root, &Info);
        let blocks = emit(&dag, &units, ctx).expect("emits").blocks;
        let lines = text(&blocks[0].insns);
        assert_eq!(blocks[0].insns.len(), 1, "{lines:?}");
        assert!(lines[0].contains("fadd"), "{lines:?}");
    });
}
