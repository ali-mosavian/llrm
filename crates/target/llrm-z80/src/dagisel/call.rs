//! Calls, returns and formal arguments over the `z80` convention
//! (`machines/calling.toml`), the counterpart of LLVM's `LowerCall`,
//! `LowerCallResult`, `LowerFormalArguments` and `LowerReturn`.
//!
//! # How it works
//! A byte argument takes the first free of `a l h e d c b`, a word the first
//! free pair of `hl de bc`, as `Convention::place` says. A byte result leaves
//! in `a`, a word in the pair `de`, a long in `hl` then `de` (the order of the
//! argument registers). The values arrive already split into legal `i8` and
//! `i16` parts, so an `i32` is two words.
//!
//! An argument that fits no register goes on the stack at the offset
//! `Convention::place` gives it, counted from the first stack argument. The
//! caller stores it at SP + offset inside the call frame its `CALLSEQ_START`
//! opens (`STSP`, as AVR's `LowerCall` stores through `STDSPQRr`); the callee
//! loads it from a fixed object at the same offset. The four bytes of return
//! address and saved IX in front of the first argument
//! (`first_argument_offset`) are the frame resolver's, added in one place.
//!
//! DEVIATE Z80-ARGPUSH (owner z80-argpush, exit 2026-11-14): the stores are not
//! `PUSH`. Reason: the pin turns stores into pushes in a separate pass
//! (`X86CallFrameOptimization`); `expand::push_arguments` is that pass, after
//! allocation, and the stores are chained top word first (`LowerCall` joins
//! them in a `TokenFactor`) so that every push stands at its store.
//!
//! # See also
//! `llvm/lib/Target/AVR/AVRISelLowering.cpp`: `LowerCall` (L1311),
//! `LowerCallResult` (L1448), `LowerFormalArguments` (L1122), `LowerReturn`
//! (L1495).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_target::calling::{Convention, Kind, Place};

use super::generated::ids::isd as ids;
use crate::abi;

/// `Z80ISD::CALL` and `Z80ISD::RET_GLUE`, numbered by `own_ids.txt`.
pub const CALL: ISD = ISD::Target(ids::CALL);
pub const RET_GLUE: ISD = ISD::Target(ids::RET_GLUE);
/// A store to SP + offset in the call frame.
pub const STSP: ISD = ISD::Target(ids::STSP);
/// `ldir` behind a test of the count.
pub const LDIRG: ISD = ISD::Target(ids::LDIRG);
/// The byte stored, then `ldir` over the rest: a fill of a constant count.
pub const LDFILL: ISD = ISD::Target(ids::LDFILL);
/// A 64-bit value in memory shifted in place, a bit a trip: address, count,
/// kind (0 shl, 1 lshr, 2 ashr).
pub const WSHIFT: ISD = ISD::Target(ids::WSHIFT);

pub use llrm_codegen::backend::selectiondag::target_calling::{Ext, FixedObject, In, Out};
use llrm_codegen::backend::selectiondag::target_calling::{LiveIn, Site, TargetCalling};

#[derive(Debug, Eq, PartialEq)]
pub enum Refused {
    /// A type other than `i8` or `i16`.
    Type(EVT),
    /// A result of this many bytes, which the convention has no registers for.
    ResultWidth(u32),
}

/// Where an argument is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Register(llrm_lir::registers::RegId),
    /// Bytes from the first stack argument.
    Stack(i64),
}

fn kind(vt: EVT) -> Result<Kind, Refused> {
    match vt {
        EVT::I8 => Ok(Kind::Word),
        EVT::I16 => Ok(Kind::Wide),
        other => Err(Refused::Type(other)),
    }
}

/// The location of each argument and the bytes the stack ones take; a word's
/// two bytes are named as their pair. The arguments from `fixed` on are a
/// variadic call's unnamed ones: on the stack after the named ones.
fn locations(
    convention: &Convention,
    vts: &[EVT],
    fixed: usize,
) -> Result<(Vec<Location>, i64), Refused> {
    let kinds = vts.iter().map(|&vt| kind(vt)).collect::<Result<Vec<_>, _>>()?;
    let placement = convention.place_variadic(&kinds, &[], fixed);
    let places = placement
        .places
        .iter()
        .enumerate()
        .map(|(at, place)| match place {
            Place::Registers(names) if vts[at] == EVT::I16 => Ok(Location::Register(abi::pair_of(&names[0]))),
            Place::Registers(names) => Ok(Location::Register(abi::register(&names[0]))),
            Place::Stack(offset) => Ok(Location::Stack(*offset)),
        })
        .collect::<Result<_, _>>()?;
    Ok((places, placement.stack_bytes))
}

/// The register each part of one result leaves in, in order: the convention's
/// row for the whole width, a word in the pair that holds its two bytes (`e`,
/// `d` is `de`; a long is `de` then `hl`).
fn result_registers(
    convention: &Convention,
    vts: &[EVT],
) -> Result<Vec<llrm_lir::registers::RegId>, Refused> {
    let width: u32 = vts.iter().map(|vt| vt.size_in_bits() / 8).sum();
    let names = convention.result_registers(i64::from(width)).ok_or(Refused::ResultWidth(width))?;
    let mut at = 0;
    Ok(vts
        .iter()
        .map(|vt| {
            let register = match *vt {
                EVT::I16 => abi::pair_of(&names[at]),
                _ => abi::register(&names[at]),
            };
            at += vt.size_in_bits() as usize / 8;
            register
        })
        .collect())
}

/// A result part, read from its register.
fn read_result(
    dag: &mut SelectionDAG,
    vt: EVT,
    registers: &mut impl Iterator<Item = llrm_lir::registers::RegId>,
    chain: &mut SDValue,
    glue: &mut SDValue,
) -> SDValue {
    let register = registers.next().expect("a register for every part");
    let copy = dag.get_copy_from_phys_reg(*chain, register, vt, *glue);
    *chain = SDValue { node: copy.node, res: 1 };
    *glue = SDValue { node: copy.node, res: 2 };
    SDValue { node: copy.node, res: 0 }
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
    let fixed = outs.iter().position(|out| out.variadic).unwrap_or(outs.len());
    let (targets, bytes) = locations(&convention, &vts, fixed)?;
    let mut chain = dag.get_callseq_start(chain, bytes as u64);
    // The stores run in the order of the pushes that replace them, the top word
    // first; one chain keeps the scheduler from reordering them.
    let mut stores: Vec<_> = outs
        .iter()
        .zip(&targets)
        .filter_map(|(out, &target)| match target {
            Location::Stack(offset) => Some((offset, out.value)),
            Location::Register(_) => None,
        })
        .collect();
    stores.sort_by_key(|&(offset, _)| std::cmp::Reverse(offset));
    for (offset, value) in stores {
        let at = dag.get_constant(offset as u128, EVT::I16);
        let store = dag.get_node_multi(STSP, &[EVT::OTHER], &[chain, value, at]);
        chain = SDValue { node: store.node, res: 0 };
    }
    let in_registers: Vec<_> = outs
        .iter()
        .zip(&targets)
        .filter_map(|(out, &target)| match target {
            Location::Register(register) => Some((out, register)),
            Location::Stack(_) => None,
        })
        .collect();
    let mut glue = None;
    // `call (iy)` takes its target in IY: copied first, the pointer is not a
    // pair-sized value live while the arguments fill HL, DE and BC
    let callee = match (dag.opcode(callee), dag.node(callee).data) {
        (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
            dag.get_target_global_address(EVT::I16, symbol, offset)
        }
        // `X86TargetLowering::LowerCall`: a libcall's `ExternalSymbol` becomes
        // the `TargetExternalSymbol` the call selects on
        (ISD::ExternalSymbol, NodeData::Symbol(id)) => {
            let name = dag.symbol_names[id as usize].clone();
            // the runtime's own name for a C library routine, as the builder's
            // `libcalls` table gives it
            let symbol = super::function::LIBCALLS
                .iter()
                .find(|(c_name, _)| *c_name == name)
                .map_or(name.as_str(), |(_, symbol)| symbol);
            dag.get_target_external_symbol(EVT::I16, symbol)
        }
        (ISD::TargetGlobalAddress | ISD::TargetExternalSymbolNode, _) => callee,
        _ => {
            let copy = dag.get_copy_to_phys_reg(chain, crate::reg::IY, callee, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            dag.get_phys_register(EVT::I16, crate::reg::IY)
        }
    };
    for &(out, register) in &in_registers {
        let copy = dag.get_copy_to_phys_reg(chain, register, out.value, glue);
        chain = SDValue { node: copy.node, res: 0 };
        glue = Some(SDValue { node: copy.node, res: 1 });
    }
    let mut operands = vec![chain, callee];
    for &(out, register) in &in_registers {
        let vt = dag.value_type(out.value);
        operands.push(dag.get_phys_register(vt, register));
    }
    // DEVIATE SCM1 (as x86's `call_clobbered_flags`): only the flags, the one
    // live physical register that crosses a call in this DAG; the scheduler
    // recomputes a flags value after the call. Exit: with the per-target
    // clobber lists.
    operands.push(dag.get_register_mask_clobbering(&[super::generated::ids::reg::F]));
    operands.extend(glue);
    let call = dag.get_node_multi(CALL, &[EVT::OTHER, EVT::GLUE], &operands);
    let end =
        dag.get_callseq_end(SDValue { node: call.node, res: 0 }, bytes as u64, 0, SDValue { node: call.node, res: 1 });
    let mut chain = SDValue { node: end.node, res: 0 };
    let mut glue = SDValue { node: end.node, res: 1 };
    let vts: Vec<EVT> = ins.iter().map(|result| result.vt).collect();
    let mut registers = result_registers(&convention, &vts)?.into_iter();
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
    fixed: FixedObject,
    live_in: LiveIn,
) -> Result<Vec<SDValue>, Refused> {
    let convention = abi::z80();
    let vts: Vec<EVT> = ins.iter().map(|arg| arg.vt).collect();
    let (sources, _) = locations(&convention, &vts, vts.len())?;
    Ok(ins
        .iter()
        .zip(sources)
        .map(|(arg, location)| match location {
            Location::Register(register) => {
                let copy = dag.get_copy_from_reg(chain, live_in(register), arg.vt);
                SDValue { node: copy.node, res: 0 }
            }
            // `LowerMemArgument`: the argument is its fixed object, loaded
            Location::Stack(offset) => {
                let index = fixed(arg.vt.size_in_bits() / 8, offset, true);
                let address = dag.get_frame_index(EVT::I16, index);
                let info = MemInfo {
                    mem_vt: arg.vt,
                    align: 1,
                    volatile: false,
                    base: MemBase::Stack(index),
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                dag.get_load(arg.vt, chain, address, info)
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
    let mut registers = result_registers(&convention, &vts)?.into_iter();
    for (out, &vt) in outs.iter().zip(&vts) {
        let register = registers.next().expect("a register for every part");
        let copy = dag.get_copy_to_phys_reg(chain, register, out.value, glue);
        chain = SDValue { node: copy.node, res: 0 };
        glue = Some(SDValue { node: copy.node, res: 1 });
        operands.push(dag.get_phys_register(vt, register));
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
    fn can_lower_return(
        &self,
        parts: &[EVT],
    ) -> bool {
        let bytes: u32 = parts.iter().map(|vt| vt.size_in_bits() / 8).sum();
        bytes == 0 || abi::z80().result_registers(i64::from(bytes)).is_some()
    }

    /// `CanLowerReturn`: a result of a width the convention names no
    /// registers for is refused with the reason, where lowering it would
    /// have no place to leave it.
    fn accepts(
        &self,
        dag: &SelectionDAG,
        site: Site,
        outs: &[Out],
        ins: &[In],
    ) -> Result<(), &'static str> {
        // the builder splits integers into bytes and words; a float part has
        // no registers
        let parts = outs.iter().map(|out| dag.value_type(out.value)).chain(ins.iter().map(|part| part.vt));
        if parts.into_iter().any(|vt| kind(vt).is_err()) {
            return Err("a part that is not a byte or a word");
        }
        let bytes: u32 = match site {
            Site::Arguments => return Ok(()),
            Site::Return => outs.iter().map(|out| dag.value_type(out.value).size_in_bits() / 8).sum(),
            Site::Call => ins.iter().map(|result| result.vt.size_in_bits() / 8).sum(),
        };
        match bytes {
            0 => Ok(()),
            width if abi::z80().result_registers(i64::from(width)).is_some() => Ok(()),
            _ => Err("a result the convention has no registers for"),
        }
    }

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

    /// The unnamed arguments follow the named ones' stack bytes.
    fn variadic_frame_offset(
        &self,
        named: &[In],
    ) -> Option<i64> {
        let vts: Vec<EVT> = named.iter().map(|arg| arg.vt).collect();
        locations(&abi::z80(), &vts, vts.len()).ok().map(|(_, bytes)| bytes)
    }

    /// `EmitTargetCodeForMemcpy`, as X86's is `rep movs`: `ldir` behind a test
    /// of the count (7 B and 19 T before the copy), where `__memcpy` is a
    /// call (3 B) and a push, a swap, the same test, the same `ldir`, a pop and
    /// a return, about 60 T more. A function tuned for size keeps the call:
    /// 4 B less at each site.
    fn emit_memcpy_sized(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        src: SDValue,
        size: SDValue,
        opt_size: bool,
    ) -> Option<SDValue> {
        if opt_size {
            return None;
        }
        let size = dag.get_zext_or_trunc(size, EVT::I16);
        Some(dag.get_node(LDIRG, EVT::OTHER, &[chain, dst, src, size]))
    }

    /// `EmitTargetCodeForMemset`, as X86's is `rep stos`: the byte stored at
    /// the destination and `ldir` copying it forward over the next `n - 1`
    /// bytes (`ld (hl), v; ld e, l; ld d, h; inc de; ld bc, n - 1; ldir`,
    /// 13 B at the site and no routine) where `__memseta` is a call (7 B at
    /// the site) and a 21 B routine linked once. Only for a constant count
    /// of two or more, and only in the selection that asks for it
    /// (`INLINE_FILLS`): the module price decides whether the routine goes
    /// (`compile::decomposed_functions`).
    fn emit_memset(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        dst: SDValue,
        value: SDValue,
        size: SDValue,
    ) -> Option<SDValue> {
        if !super::pipeline::INLINE_FILLS.with(std::cell::Cell::get) {
            return None;
        }
        let bytes = u64::try_from(dag.constant(size)?).ok().filter(|bytes| (2..=0x1_0000).contains(bytes))?;
        let count = dag.get_constant(u128::from(bytes - 1), EVT::I16);
        Some(dag.get_node(LDFILL, EVT::OTHER, &[chain, dst, value, count]))
    }

    /// Not `ldir`: a `memmove` whose destination is above its source overlaps
    /// the wrong way for a forward copy, and the runtime's `__memmove`
    /// picks the direction.
    fn emit_memmove(
        &self,
        _dag: &mut SelectionDAG,
        _chain: SDValue,
        _dst: SDValue,
        _src: SDValue,
        _size: SDValue,
    ) -> Option<SDValue> {
        None
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
