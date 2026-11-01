//! Calls, returns and formal arguments over the `z80` convention
//! (`machines/calling.toml`), the counterpart of LLVM's `LowerCall`,
//! `LowerCallResult`, `LowerFormalArguments` and `LowerReturn`.
//!
//! # How it works
//! A byte argument takes the first free of `a l h e d c b`, a word the first
//! free pair of `hl de bc`, as `Convention::place` says. A byte result leaves
//! in `a`, a word in `e:d` (DE), a long in `e:d:l:h`. The values arrive already
//! split into legal `i8` and `i16` parts, so an `i32` is two words.
//!
//! DEVIATE: an argument that fits no register is refused. Reason: the stack
//! arguments are pushed, and the `PUSH` node and the frame object a callee
//! reads them from are not written. Exit: the frame lowering.
//!
//! # See also
//! `llvm/lib/Target/AVR/AVRISelLowering.cpp`: `LowerCall` (L1311),
//! `LowerCallResult` (L1448), `LowerFormalArguments` (L1122), `LowerReturn`
//! (L1495).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{NodeData, SDValue, SelectionDAG};
use llrm_target::calling::{Convention, Kind, Place};

use super::generated::ids::isd as ids;
use crate::abi;

/// `Z80ISD::CALL` and `Z80ISD::RET_GLUE`, numbered by `own_ids.txt`.
pub const CALL: ISD = ISD::Target(ids::CALL);
pub const RET_GLUE: ISD = ISD::Target(ids::RET_GLUE);

pub use llrm_codegen::backend::selectiondag::target_calling::{Ext, FixedObject, In, Out};
use llrm_codegen::backend::selectiondag::target_calling::{LiveIn, TargetCalling};

#[derive(Debug, Eq, PartialEq)]
pub enum Refused {
    /// A type other than `i8` or `i16`.
    Type(EVT),
    /// An argument that goes on the stack.
    Stack,
}

/// Where an argument is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Register(llrm_lir::registers::RegId),
}

fn kind(vt: EVT) -> Result<Kind, Refused> {
    match vt {
        EVT::I8 => Ok(Kind::Word),
        EVT::I16 => Ok(Kind::Wide),
        other => Err(Refused::Type(other)),
    }
}

/// The location of each argument and the bytes the stack ones take; a word's
/// two bytes are named as their pair.
fn locations(
    convention: &Convention,
    vts: &[EVT],
) -> Result<(Vec<Location>, i64), Refused> {
    let kinds = vts.iter().map(|&vt| kind(vt)).collect::<Result<Vec<_>, _>>()?;
    let placement = convention.place(&kinds);
    let places = placement
        .places
        .iter()
        .enumerate()
        .map(|(at, place)| match place {
            Place::Registers(names) if vts[at] == EVT::I16 => Ok(Location::Register(abi::pair_of(&names[0]))),
            Place::Registers(names) => Ok(Location::Register(abi::register(&names[0]))),
            Place::Stack(_) => Err(Refused::Stack),
        })
        .collect::<Result<_, _>>()?;
    Ok((places, placement.stack_bytes))
}

/// The registers the parts `vts` of one result leave in, in order: the
/// convention's row for the whole width, `e`, `d`, `l`, `h` for a long.
fn result_registers(
    convention: &Convention,
    vts: &[EVT],
) -> Vec<llrm_lir::registers::RegId> {
    let width: u32 = vts.iter().map(|vt| vt.size_in_bits() / 8).sum();
    convention.result_registers(i64::from(width)).expect("calling.toml has a result row")[..width as usize]
        .iter()
        .map(|name| abi::register(name))
        .collect()
}

/// A result part: each byte read from its register, joined into a word.
fn read_result(
    dag: &mut SelectionDAG,
    vt: EVT,
    registers: &mut impl Iterator<Item = llrm_lir::registers::RegId>,
    chain: &mut SDValue,
    glue: &mut SDValue,
) -> SDValue {
    let mut bytes = Vec::new();
    for register in registers.take(vt.size_in_bits() as usize / 8) {
        let copy = dag.get_copy_from_phys_reg(*chain, register, EVT::I8, *glue);
        *chain = SDValue { node: copy.node, res: 1 };
        *glue = SDValue { node: copy.node, res: 2 };
        bytes.push(SDValue { node: copy.node, res: 0 });
    }
    match bytes[..] {
        [one] => one,
        [lo, hi] => dag.get_node(ISD::BuildPair, vt, &[lo, hi]),
        _ => unreachable!("a result part is a byte or a word"),
    }
}

/// `LowerCall`: the arguments copied to their registers, the `CALL`, and the
/// results read back. Returns the chain and the result values.
pub fn lower_call(
    dag: &mut SelectionDAG,
    chain: SDValue,
    callee: SDValue,
    outs: &[Out],
    ins: &[In],
) -> Result<(SDValue, Vec<SDValue>), Refused> {
    let convention = abi::z80();
    let vts: Vec<EVT> = outs.iter().map(|out| dag.value_type(out.value)).collect();
    let (targets, bytes) = locations(&convention, &vts)?;
    let mut chain = dag.get_callseq_start(chain, bytes as u64);
    let mut glue = None;
    for (out, &target) in outs.iter().zip(&targets) {
        let Location::Register(register) = target;
        let copy = dag.get_copy_to_phys_reg(chain, register, out.value, glue);
        chain = SDValue { node: copy.node, res: 0 };
        glue = Some(SDValue { node: copy.node, res: 1 });
    }
    let callee = match (dag.opcode(callee), dag.node(callee).data) {
        (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
            dag.get_target_global_address(EVT::I16, symbol, offset)
        }
        _ => callee,
    };
    let mut operands = vec![chain, callee];
    for (out, &target) in outs.iter().zip(&targets) {
        let Location::Register(register) = target;
        let vt = dag.value_type(out.value);
        operands.push(dag.get_phys_register(vt, register));
    }
    operands.push(dag.get_register_mask());
    operands.extend(glue);
    let call = dag.get_node_multi(CALL, &[EVT::OTHER, EVT::GLUE], &operands);
    let end =
        dag.get_callseq_end(SDValue { node: call.node, res: 0 }, bytes as u64, 0, SDValue { node: call.node, res: 1 });
    let mut chain = SDValue { node: end.node, res: 0 };
    let mut glue = SDValue { node: end.node, res: 1 };
    let vts: Vec<EVT> = ins.iter().map(|result| result.vt).collect();
    let mut registers = result_registers(&convention, &vts).into_iter();
    let values = vts.iter().map(|&vt| read_result(dag, vt, &mut registers, &mut chain, &mut glue)).collect();
    Ok((chain, values))
}

/// `LowerFormalArguments`: each argument read from the virtual register its
/// register is live-in to, or loaded from its fixed stack object (`fixed`
/// creates it).
pub fn lower_formal_arguments(
    dag: &mut SelectionDAG,
    chain: SDValue,
    ins: &[In],
    _fixed: FixedObject,
    live_in: LiveIn,
) -> Result<Vec<SDValue>, Refused> {
    let convention = abi::z80();
    let vts: Vec<EVT> = ins.iter().map(|arg| arg.vt).collect();
    let (sources, _) = locations(&convention, &vts)?;
    Ok(ins
        .iter()
        .zip(sources)
        .map(|(arg, location)| match location {
            Location::Register(register) => {
                let copy = dag.get_copy_from_reg(chain, live_in(register), arg.vt);
                SDValue { node: copy.node, res: 0 }
            }
        })
        .collect())
}

/// `LowerReturn`: the value split into bytes and copied to `a` (and `d`), then
/// `RET_GLUE`.
pub fn lower_return(
    dag: &mut SelectionDAG,
    chain: SDValue,
    outs: &[Out],
) -> Result<SDValue, Refused> {
    let convention = abi::z80();
    let mut chain = chain;
    let mut glue = None;
    let mut operands = vec![chain];
    let vts: Vec<EVT> = outs.iter().map(|out| dag.value_type(out.value)).collect();
    for &vt in &vts {
        kind(vt)?;
    }
    let mut registers = result_registers(&convention, &vts).into_iter();
    for (out, &vt) in outs.iter().zip(&vts) {
        let bytes = match vt {
            EVT::I8 => vec![out.value],
            _ => {
                let shift = dag.get_constant(8, EVT::I8);
                let high = dag.get_node(ISD::Srl, vt, &[out.value, shift]);
                vec![dag.get_node(ISD::Truncate, EVT::I8, &[out.value]), dag.get_node(ISD::Truncate, EVT::I8, &[high])]
            }
        };
        for byte in bytes {
            let register = registers.next().expect("a register for every byte");
            let copy = dag.get_copy_to_phys_reg(chain, register, byte, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            operands.push(dag.get_phys_register(EVT::I8, register));
        }
    }
    operands[0] = chain;
    operands.extend(glue);
    Ok(dag.get_node(RET_GLUE, EVT::OTHER, &operands))
}

/// The builder's hook: the shared builder splits values into legal parts, so a
/// refusal here is a builder bug.
#[derive(Debug, Default)]
pub struct Z80Calling;

impl TargetCalling for Z80Calling {
    fn passes_in_place(
        &self,
        name: &str,
    ) -> bool {
        // a hidden result pointer is the first parameter, a `byval` one the
        // address of the caller's copy
        matches!(name, "sret" | "byval")
    }

    fn byval_by_reference(&self) -> bool {
        true
    }

    fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        lower_call(dag, chain, callee, outs, ins).expect("legal parts")
    }

    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        live_in: LiveIn,
    ) -> Vec<SDValue> {
        lower_formal_arguments(dag, chain, ins, fixed, live_in).expect("legal parts")
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        lower_return(dag, chain, outs).expect("legal parts")
    }
}
