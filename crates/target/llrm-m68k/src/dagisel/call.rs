//! `M68kTargetLowering`'s calls: `LowerCall`, `LowerCallResult`,
//! `LowerFormalArguments`, `LowerReturn` and `LowerGlobalAddress`.
//!
//! # How it works
//! `CC_M68k_C` puts every argument in a 4-byte stack slot, pushed in order and
//! removed by the caller; a byte or word is promoted to `i32` first. A result
//! is in `d0` (`bd0`, `wd0` for a byte or word) and a pointer in `a0`. Where
//! each goes is `machines/calling.toml`, read through `Convention::place`.
//! The values arrive already split into legal `i8`/`i16`/`i32` parts, as
//! LLVM's `Outs` and `Ins` are.
//!
//! A `byval` argument is `CCPassByVal<4, 4>`: its bytes (at least 4, rounded
//! to 4) in the argument area at the larger of its alignment and 4, copied
//! there by loads and stores (`CreateCopyOfByValArgument`, always inline); the
//! callee's parameter is the address of that area.
//!
//! DEVIATE: not translated are `inreg` (`d0`, `d1`), `nest` (`a1`),
//! varargs, tail calls, the Fast convention and callee-pop. Reason:
//! the slice does not reach them. Exit: M3d.
//!
//! DEVIATE: the DAG's global node carries no target flag (`TF=`): a callee is
//! always the direct form, and a global that PIC would reach through the GOT
//! is refused. Reason: `NodeData::Global` has no flags field. Exit: S2.
//!
//! # See also
//! `llvm/lib/Target/M68k/M68kISelLowering.cpp`, pin d1106deb71cc:
//! `LowerMemArgument` (L454), `LowerMemOpCallTo` (L523), `LowerCall` (L544),
//! `LowerCallResult` (L906), `LowerFormalArguments` (L941), `LowerReturn`
//! (L1095), `LowerGlobalAddress` (L2727).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::target_calling::{ByVal, LiveIn, TargetCalling};
pub use llrm_codegen::backend::selectiondag::target_calling::{Ext, FixedObject, In, Out};
use llrm_codegen::backend::selectiondag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_target::calling::{Convention, Kind, Place};

use super::lowering::m68kisd;
use crate::abi;
use crate::reg;

/// The bytes of the `i32` slot an `i8` or `i16` argument sits in, high end:
/// m68k is big-endian (`LowerMemArgument`).
fn slot_offset(
    vt: EVT,
    offset: i64,
) -> i64 {
    match vt {
        EVT::I8 => offset + 3,
        EVT::I16 => offset + 2,
        _ => offset,
    }
}

/// The m68k C calling convention applied to DAG nodes.
#[derive(Debug)]
pub struct M68kCalling {
    convention: Convention,
}

impl Default for M68kCalling {
    fn default() -> Self {
        Self { convention: abi::c() }
    }
}

impl M68kCalling {
    /// Where each argument is: its offset in the stack arguments, and the bytes
    /// they take (`AnalyzeCallOperands`, `getAlignedCallFrameSize`).
    fn place(
        &self,
        count: usize,
    ) -> (Vec<i64>, i64) {
        let placement = self.convention.place(&vec![Kind::Word; count]);
        let offsets = placement
            .places
            .iter()
            .map(|place| match place {
                Place::Stack(offset) => *offset,
                Place::Registers(_) => unreachable!("CC_M68k_C takes no register argument"),
            })
            .collect();
        (offsets, placement.stack_bytes)
    }

    /// [`Self::place`] for arguments of which some are `byval`
    /// (`CCState::HandleByVal`): a `byval` one takes its size, at least 4 and
    /// rounded to 4, at the larger of its alignment and 4; the rest a word.
    fn place_parts(
        &self,
        parts: &[Option<ByVal>],
    ) -> (Vec<i64>, i64) {
        if parts.iter().all(Option::is_none) {
            return self.place(parts.len());
        }
        let align_up = |value: i64, align: i64| (value + align - 1) / align * align;
        let (mut stack, mut widest) = (0_i64, 4_i64);
        let mut offsets = Vec::with_capacity(parts.len());
        for part in parts {
            let (size, align) = match part {
                Some(byval) => (align_up(i64::from(byval.size).max(4), 4), i64::from(byval.align).max(4)),
                None => (4, 4),
            };
            widest = widest.max(align);
            stack = align_up(stack, align);
            offsets.push(stack);
            stack += size;
        }
        (offsets, align_up(stack, widest))
    }

    /// `CreateCopyOfByValArgument`: the `size` bytes at `src` stored from
    /// `offset` of the outgoing area, by the widest loads and stores its
    /// alignment allows, the loads first. The stores are returned.
    fn copy_bytes(
        dag: &mut SelectionDAG,
        chain: SDValue,
        stack_pointer: SDValue,
        offset: i64,
        src: SDValue,
        byval: ByVal,
    ) -> Vec<SDValue> {
        let mut loads = Vec::new();
        let mut at = 0_i64;
        let size = i64::from(byval.size);
        while at < size {
            let (vt, step) = match (i64::from(byval.align), size - at) {
                (align, left) if align >= 4 && left >= 4 => (EVT::I32, 4),
                (align, left) if align >= 2 && left >= 2 => (EVT::I16, 2),
                _ => (EVT::I8, 1),
            };
            let from = dag.get_constant(at as u128, EVT::I32);
            let from = dag.get_node(ISD::Add, EVT::I32, &[src, from]);
            let info = MemInfo {
                mem_vt: vt,
                align: step as u32,
                volatile: false,
                base: MemBase::Unknown,
                ext: None,
                space: 0,
                ordering: None,
            };
            loads.push((dag.get_load(vt, chain, from, info), vt, at, step));
            at += step;
        }
        let chains: Vec<SDValue> = loads.iter().map(|(load, ..)| SDValue { node: load.node, res: 1 }).collect();
        let after =
            if chains.len() > 1 { dag.get_token_factor(&chains) } else { chains.first().copied().unwrap_or(chain) };
        loads
            .into_iter()
            .map(|(load, vt, at, step)| {
                let slot = dag.get_constant((offset + at) as u128, EVT::I32);
                let to = dag.get_node(ISD::Add, EVT::I32, &[stack_pointer, slot]);
                let info = MemInfo {
                    mem_vt: vt,
                    align: step as u32,
                    volatile: false,
                    base: MemBase::CallArgument((offset + at) as i32),
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                dag.get_store(after, load, to, info)
            })
            .collect()
    }

    /// The register `RetCC_M68k_C` puts a result of `vt` in.
    fn result_register(
        &self,
        vt: EVT,
        pointer: bool,
        earlier: usize,
    ) -> llrm_lir::registers::RegId {
        let names = if pointer {
            self.convention.results.get("address")
        } else {
            self.convention.results.get(&(vt.size_in_bits() / 8).max(1).to_string())
        };
        let names = names.expect("RetCC_M68k_C names a register");
        // `CCAssignToReg<[D0, D1]>`: the k-th result of a class takes the k-th
        // register, the high word of an `i64` first
        abi::register(names.get(earlier).expect("RetCC_M68k_C has a register for every result"))
    }

    /// How many of `earlier` results are of the register class of a result of
    /// `vt`, `pointer`.
    fn results_before(
        dag: &SelectionDAG,
        earlier: impl Iterator<Item = (EVT, bool)>,
        vt: EVT,
        pointer: bool,
    ) -> usize {
        let class = |vt: EVT, pointer: bool| if pointer { 0 } else { (vt.size_in_bits() / 8).max(1) };
        let _ = dag;
        earlier.filter(|&(one, ptr)| class(one, ptr) == class(vt, pointer)).count()
    }

    /// `LowerCall`: the arguments stored in the outgoing stack area, the `CALL`
    /// and the results read back. Returns the chain and the result values.
    pub fn lower_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
        ins: &[In],
    ) -> (SDValue, Vec<SDValue>) {
        let parts: Vec<Option<ByVal>> = outs.iter().map(|out| out.byval).collect();
        let (offsets, bytes) = self.place_parts(&parts);
        let mut chain = dag.get_callseq_start(chain, bytes as u64);
        let mut stack_pointer = None;
        let mut stores = Vec::new();
        for (out, offset) in outs.iter().zip(offsets) {
            if let Some(byval) = out.byval {
                let sp =
                    *stack_pointer.get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, reg::SP, EVT::I32));
                stores.extend(Self::copy_bytes(dag, chain, sp, offset, out.value, byval));
                continue;
            }
            // `CCPromoteToType<i32>`
            let mut arg = out.value;
            if dag.value_type(arg) != EVT::I32 {
                arg = dag.get_node(out.ext.node(), EVT::I32, &[arg]);
            }
            let sp = *stack_pointer.get_or_insert_with(|| dag.get_copy_from_phys_reg_plain(chain, reg::SP, EVT::I32));
            // `LowerMemOpCallTo`
            let slot = dag.get_constant(offset as u128, EVT::I32);
            let address = dag.get_node(ISD::Add, EVT::I32, &[sp, slot]);
            let info = MemInfo {
                mem_vt: EVT::I32,
                align: 2,
                volatile: false,
                base: MemBase::CallArgument(offset as i32),
                ext: None,
                space: 0,
                ordering: None,
            };
            stores.push(dag.get_store(chain, arg, address, info));
        }
        if !stores.is_empty() {
            chain = dag.get_token_factor(&stores);
        }
        let callee = self.callee(dag, callee);
        // The callee leaves the condition codes undefined, but the descriptions
        // list only `SP` and the pin's mask is the preserved set, so
        // the scheduler must be told: a flag live across the call is
        // recomputed after it.
        let mask = dag.get_register_mask_clobbering(&[super::generated::ids::reg::CCR]);
        let call = dag.get_node_multi(m68kisd::CALL, &[EVT::OTHER, EVT::GLUE], &[chain, callee, mask]);
        let end = dag.get_callseq_end(
            SDValue { node: call.node, res: 0 },
            bytes as u64,
            0,
            SDValue { node: call.node, res: 1 },
        );
        let mut chain = SDValue { node: end.node, res: 0 };
        let mut glue = SDValue { node: end.node, res: 1 };
        // `LowerCallResult`
        let mut values = Vec::new();
        for (at, result) in ins.iter().enumerate() {
            let before =
                Self::results_before(dag, ins[..at].iter().map(|one| (one.vt, one.pointer)), result.vt, result.pointer);
            let register = self.result_register(result.vt, result.pointer, before);
            let copy = dag.get_copy_from_phys_reg(chain, register, result.vt, glue);
            chain = SDValue { node: copy.node, res: 1 };
            glue = SDValue { node: copy.node, res: 2 };
            values.push(SDValue { node: copy.node, res: 0 });
        }
        (chain, values)
    }

    /// The callee as `LowerCall` leaves it: a `GlobalAddress` is the
    /// `TargetGlobalAddress` (the direct form, see the DEVIATE above).
    fn callee(
        &self,
        dag: &mut SelectionDAG,
        callee: SDValue,
    ) -> SDValue {
        match (dag.opcode(callee), dag.node(callee).data) {
            (ISD::GlobalAddress, NodeData::Global { symbol, offset }) => {
                dag.get_target_global_address(EVT::I32, symbol, offset)
            }
            _ => callee,
        }
    }

    /// `LowerFormalArguments`: each argument loaded from its slot in the
    /// caller's outgoing area. `fixed` creates the stack object.
    pub fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
    ) -> Vec<SDValue> {
        let parts: Vec<Option<ByVal>> = ins.iter().map(|arg| arg.byval).collect();
        let (offsets, _) = self.place_parts(&parts);
        ins.iter()
            .zip(offsets)
            .map(|(arg, offset)| {
                // `LowerMemArgument`: a `byval` parameter is the address of its
                // bytes in the caller's area
                if let Some(byval) = arg.byval {
                    let index = fixed(byval.size.max(1), offset, false);
                    return dag.get_frame_index(EVT::I32, index);
                }
                let offset = slot_offset(arg.vt, offset);
                let index = fixed(arg.vt.size_in_bits() / 8, offset, true);
                let address = dag.get_frame_index(EVT::I32, index);
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
            })
            .collect()
    }

    /// `LowerReturn`: the values copied to their registers, then
    /// `RET chain, bytes-to-pop, registers.., glue`.
    pub fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        let mut chain = chain;
        let pop = dag.get_constant_with(0, EVT::I32, true, false);
        let mut operands = vec![chain, pop];
        let mut glue = None;
        let mut sent: Vec<(EVT, bool)> = Vec::new();
        for out in outs {
            // `CCIfType<[i1], CCPromoteToType<i8>>`
            let mut value = out.value;
            if dag.value_type(value) == EVT::I1 {
                value = dag.get_node(out.ext.node(), EVT::I8, &[value]);
            }
            let vt = dag.value_type(value);
            let before = Self::results_before(dag, sent.iter().copied(), vt, out.pointer);
            sent.push((vt, out.pointer));
            let register = self.result_register(vt, out.pointer, before);
            let copy = dag.get_copy_to_phys_reg(chain, register, value, glue);
            chain = SDValue { node: copy.node, res: 0 };
            glue = Some(SDValue { node: copy.node, res: 1 });
            operands.push(dag.get_phys_register(vt, register));
        }
        operands[0] = chain;
        operands.extend(glue);
        dag.get_node(m68kisd::RET, EVT::OTHER, &operands)
    }
}

impl TargetCalling for M68kCalling {
    /// A hidden result pointer is the first stack argument, as any pointer.
    fn passes_in_place(
        &self,
        attribute: &str,
    ) -> bool {
        attribute == "sret" || attribute == "byval"
    }

    fn byval_in_area(&self) -> bool {
        true
    }

    /// `ABI require that for returning structs by value we copy the sret
    /// argument into %D0`.
    fn returns_sret_pointer(&self) -> bool {
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
        M68kCalling::lower_call(self, dag, chain, callee, outs, ins)
    }

    /// Every argument is in memory: `live_in` is not used.
    fn lower_formal_arguments(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        ins: &[In],
        fixed: FixedObject,
        _live_in: LiveIn,
    ) -> Vec<SDValue> {
        M68kCalling::lower_formal_arguments(self, dag, chain, ins, fixed)
    }

    /// The variadic arguments start where the named ones' stack words end.
    fn variadic_frame_offset(
        &self,
        fixed: &[In],
    ) -> Option<i64> {
        let parts: Vec<Option<ByVal>> = fixed.iter().map(|arg| arg.byval).collect();
        let (offsets, _) = self.place_parts(&parts);
        let size = |byval: Option<ByVal>| byval.map_or(4, |one| (i64::from(one.size).max(4) + 3) / 4 * 4);
        Some(offsets.last().zip(parts.last()).map_or(0, |(last, byval)| last + size(*byval)))
    }

    fn lower_return(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        outs: &[Out],
    ) -> SDValue {
        M68kCalling::lower_return(self, dag, chain, outs)
    }

    /// `LowerCall`'s sibcall: `TC_RETURN chain, callee, fpdiff`, for a direct
    /// callee whose arguments fit the caller's incoming slots: an argument
    /// that is the caller's own, in the slot the callee reads it from, stays
    /// (`MatchingStackOffset`); any other is stored into that slot.
    ///
    /// DEVIATE: the M68k pin keeps a call whose argument is not already in
    /// place (it stores only under `-tailcallopt`). This is AArch64's sibcall
    /// (`isEligibleForTailCallOptimization`: the callee's stack size within
    /// `getBytesInStackArgArea`; `LowerCall`: `IsTailCall` stores at FPDiff 0,
    /// `addTokenForArgument`), as gcc does. A slot is stored only if the
    /// caller has an immutable 4-byte object there; a caller with a `byval`
    /// parameter keeps the call. Exit: none, the pin's x86 and ARM do the same.
    fn lower_tail_call(
        &self,
        dag: &mut SelectionDAG,
        chain: SDValue,
        callee: SDValue,
        outs: &[Out],
    ) -> Option<SDValue> {
        if outs.iter().any(|out| out.byval.is_some()) {
            return None;
        }
        // a direct callee, already the target form (`callee` above)
        let target = match dag.opcode(callee) {
            ISD::TargetGlobalAddress | ISD::TargetExternalSymbolNode => callee,
            _ => return None,
        };
        let (offsets, _) = self.place(outs.len());
        let mut stores = Vec::new();
        for (out, offset) in outs.iter().zip(offsets) {
            let slot = *dag.fixed_offsets.iter().find(|(_, at)| **at == offset)?.0;
            if dag.fixed_objects.get(&slot) != Some(&(4, true)) {
                return None;
            }
            let own = matches!(dag.node(out.value).data, NodeData::Mem(info) if info.base == MemBase::Stack(slot));
            if own && dag.opcode(out.value) == ISD::Load && dag.value_type(out.value) == EVT::I32 {
                continue;
            }
            stores.push((slot, out.value, out.ext));
        }
        // the caller's incoming loads hang off the entry token, unordered
        // against the stores: each is chained ahead of them, and is no longer
        // immutable (`addTokenForArgument`)
        let mut chains = vec![chain];
        if !stores.is_empty() {
            let entry = dag.entry_node();
            let loads: Vec<u32> = dag
                .users(entry)
                .filter(|&user| {
                    let load = SDValue { node: user, res: 0 };
                    dag.opcode(load) == ISD::Load
                        && matches!(dag.node(load).data,
                            NodeData::Mem(info) if matches!(info.base, MemBase::Stack(index) if index < 0))
                })
                .collect();
            for load in loads {
                dag.invariant_loads.remove(&load);
                chains.push(SDValue { node: load, res: 1 });
            }
        }
        let ordered = if chains.len() > 1 { dag.get_token_factor(&chains) } else { chain };
        let mut done = Vec::new();
        for (slot, value, ext) in stores {
            // `CCPromoteToType<i32>`
            let value = if dag.value_type(value) == EVT::I32 { value } else { dag.get_node(ext.node(), EVT::I32, &[value]) };
            let address = dag.get_frame_index(EVT::I32, slot);
            let info = MemInfo {
                mem_vt: EVT::I32,
                align: 2,
                volatile: false,
                base: MemBase::Stack(slot),
                ext: None,
                space: 0,
                ordering: None,
            };
            done.push(dag.get_store(ordered, value, address, info));
        }
        let chain = if done.is_empty() {
            chain
        } else {
            dag.get_token_factor(&done)
        };
        let fpdiff = dag.get_constant(0, EVT::I32);
        Some(dag.get_node(m68kisd::TC_RETURN, EVT::OTHER, &[chain, target, fpdiff]))
    }
}

/// `LowerJumpTable`: `WrapperPC (TargetJumpTable)`, or `Wrapper` where the code
/// model makes local references absolute (`classifyLocalReference`, the same
/// choice as [`lower_global_address`]). The PIC model's `GLOBAL_BASE_REG` add
/// is not translated: `lower_operation` assumes no PIC.
pub fn lower_jump_table(
    dag: &mut SelectionDAG,
    node: SDValue,
    absolute: bool,
) -> SDValue {
    let NodeData::JumpTable(index) = dag.node(node).data else { unreachable!("JumpTable has a table payload") };
    let table = dag.get_target_jump_table(EVT::I32, index);
    dag.get_node(if absolute { m68kisd::WRAPPER } else { m68kisd::WRAPPER_PC }, EVT::I32, &[table])
}

/// `LowerGlobalAddress`: `WrapperPC (TargetGlobalAddress)`, or `Wrapper` where
/// the code model makes the address absolute (`classifyGlobalReference`), the
/// offset folded into the symbol. A global the PIC model reaches through the
/// GOT is not translated (see the DEVIATE above): `lower_operation` assumes
/// no PIC, where every reference is direct.
pub fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
    absolute: bool,
) -> SDValue {
    let NodeData::Global { symbol, offset } = dag.node(node).data else {
        unreachable!("GlobalAddress has a global payload")
    };
    let address = dag.get_target_global_address(EVT::I32, symbol, offset);
    dag.get_node(if absolute { m68kisd::WRAPPER } else { m68kisd::WRAPPER_PC }, EVT::I32, &[address])
}
