//! Calls, returns and formal arguments over the `mos` convention (`machines/calling.toml`), the
//! counterpart of LLVM's `LowerCall`, `LowerCallResult`, `LowerFormalArguments` and `LowerReturn`.
//!
//! # How it works
//! A byte argument takes the first free byte register from `rc2`, a word the first free pair
//! (`rc4:rc5` is `rs2`), as `Convention::place` says. A byte result leaves in `a`, a word in
//! `a:x`. The values arrive already split into legal `i8` and `i16` parts, so an `i32` is two words.
//!
//! An argument that fits no register goes on the soft stack (`rs0`): the caller stores it at `rs0 +
//! offset` inside the call sequence, the callee loads it from a fixed object. The adjustment of `rs0`
//! itself belongs to the frame lowering, as the call-sequence pseudos' expansion.
//!
//! # See also
//! `llvm/lib/Target/AVR/AVRISelLowering.cpp`: `LowerCall` (L1311), `LowerCallResult` (L1448),
//! `LowerFormalArguments` (L1122), `LowerReturn` (L1495).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_target::calling::{Convention, Kind, Place};

use super::generated::ids::isd as ids;
use crate::abi;

/// `MOSISD::CALL` and `MOSISD::RET_GLUE`, numbered by `own_ids.txt`.
pub const CALL: ISD = ISD::Target(ids::CALL);
pub const RET_GLUE: ISD = ISD::Target(ids::RET_GLUE);

pub use llrm_codegen::backend::selectiondag::target_calling::{Ext, FixedObject, In, Out};
use llrm_codegen::backend::selectiondag::target_calling::{LiveIn, TargetCalling};

#[derive(Debug, Eq, PartialEq)]
pub enum Refused {
    /// A type other than `i8` or `i16`.
    Type(EVT),
}

/// Where an argument is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Register(llrm_lir::registers::RegId),
    /// By its offset in the stack arguments.
    Stack(i64),
}

fn kind(vt: EVT) -> Result<Kind, Refused> {
    match vt {
        EVT::I8 => Ok(Kind::Word),
        EVT::I16 => Ok(Kind::Wide),
        other => Err(Refused::Type(other)),
    }
}

/// The location of each argument and the bytes the stack ones take; a word's two bytes are named
/// as their pair.
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
            Place::Registers(names) if vts[at] == EVT::I16 => Location::Register(abi::pair_of(&names[0])),
            Place::Registers(names) => Location::Register(abi::register(&names[0])),
            Place::Stack(offset) => Location::Stack(*offset),
        })
        .collect();
    Ok((places, placement.stack_bytes))
}

/// The registers the parts `vts` of one result leave in, in order: the convention's row for the
/// whole width, `a`, `x`, `rc2`, `rc3` for a long.
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

/// `LowerCall`: the arguments copied to their registers, the `CALL`, and the results read back.
/// Returns the chain and the result values.
pub fn lower_call(
    dag: &mut SelectionDAG,
    chain: SDValue,
    callee: SDValue,
    outs: &[Out],
    ins: &[In],
) -> Result<(SDValue, Vec<SDValue>), Refused> {
    let convention = abi::mos();
    let vts: Vec<EVT> = outs.iter().map(|out| dag.value_type(out.value)).collect();
    let (targets, bytes) = locations(&convention, &vts)?;
    let mut chain = dag.get_callseq_start(chain, bytes as u64);
    // `LowerMemOpCallTo`: the stack arguments are stored at `rs0 + offset`.
    let mut stores = Vec::new();
    let mut stack_pointer = None;
    for ((out, &vt), &target) in outs.iter().zip(&vts).zip(&targets) {
        let Location::Stack(offset) = target else { continue };
        let sp = *stack_pointer.get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, abi::register("rs0"), EVT::I16));
        let slot = dag.get_constant(offset as u128, EVT::I16);
        let address = dag.get_node(ISD::Add, EVT::I16, &[sp, slot]);
        let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::CallArgument(offset as i32), ext: None, space: 0 };
        stores.push(dag.get_store(chain, out.value, address, info));
    }
    if !stores.is_empty() {
        chain = dag.get_token_factor(&stores);
    }
    let mut glue = None;
    for (out, &target) in outs.iter().zip(&targets) {
        let Location::Register(register) = target else { continue };
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
        let Location::Register(register) = target else { continue };
        let vt = dag.value_type(out.value);
        operands.push(dag.get_phys_register(vt, register));
    }
    operands.push(dag.get_register_mask());
    operands.extend(glue);
    let call = dag.get_node_multi(CALL, &[EVT::OTHER, EVT::GLUE], &operands);
    let end = dag.get_callseq_end(
        SDValue { node: call.node, res: 0 },
        bytes as u64,
        0,
        SDValue { node: call.node, res: 1 },
    );
    let mut chain = SDValue { node: end.node, res: 0 };
    let mut glue = SDValue { node: end.node, res: 1 };
    let vts: Vec<EVT> = ins.iter().map(|result| result.vt).collect();
    let mut registers = result_registers(&convention, &vts).into_iter();
    let values = vts.iter().map(|&vt| read_result(dag, vt, &mut registers, &mut chain, &mut glue)).collect();
    Ok((chain, values))
}

/// `LowerFormalArguments`: each argument read from the virtual register its register is live-in
/// to, or loaded from its fixed stack object (`fixed` creates it).
pub fn lower_formal_arguments(
    dag: &mut SelectionDAG,
    chain: SDValue,
    ins: &[In],
    fixed: FixedObject,
    live_in: LiveIn,
) -> Result<Vec<SDValue>, Refused> {
    let convention = abi::mos();
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
            Location::Stack(offset) => {
                // `LowerMemArgument`
                let index = fixed(arg.vt.size_in_bits() / 8, offset, true);
                let address = dag.get_frame_index(EVT::I16, index);
                let info = MemInfo {
                    mem_vt: arg.vt,
                    align: 1,
                    volatile: false,
                    base: MemBase::Stack(index),
                    ext: None,
                    space: 0,
                };
                dag.get_load(arg.vt, chain, address, info)
            }
        })
        .collect())
}

/// `LowerReturn`: the value split into bytes and copied to `a` (and `x`), then `RET_GLUE`.
pub fn lower_return(
    dag: &mut SelectionDAG,
    chain: SDValue,
    outs: &[Out],
) -> Result<SDValue, Refused> {
    let convention = abi::mos();
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

/// The builder's hook: the shared builder splits values into legal parts, so a refusal here is a
/// builder bug.
#[derive(Debug, Default)]
pub struct MosCalling;

impl TargetCalling for MosCalling {
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
