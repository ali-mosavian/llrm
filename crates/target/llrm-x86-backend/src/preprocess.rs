//! The rewrites `X86DAGToDAGISel` makes of the DAG before selection (the pin's
//! `PreprocessISelDAG`), for the ones that need no matcher.
//!
//! # How it works
//! Without SSE a `float` or `double` is held in an x87 register at the
//! register's own width, so an `fp_round` (the `FPTrunc` of the IR) that
//! drops precision is done by storing the value in the narrower type to a
//! stack slot and loading it back: the store rounds. [`preprocess`] rewrites
//! every such node, `fp_round x, 0` between x87 types, into that store and
//! load. An `fp_extend` between x87 types and an `fp_round` with the
//! "value is unchanged" flag set stay: the register holds either exactly.
//! With SSE2 `f32` and `f64` are in xmm registers, and a conversion between
//! one and an x87 type (`fpext float to x86_fp80`) goes through the slot
//! too: the store in the narrower type, then an extending load.
//!
//! # Runs
//! Before selection, from the `--isel=dag` driver, which owns the frame
//! objects ([`FrameObjects`]).
//!
//! # See also
//! LLVM's `X86DAGToDAGISel::PreprocessISelDAG`
//! ([L988](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L988)),
//! its `FP_ROUND`/`FP_EXTEND` case at L1454 and the callee-load move at L1425.

use llrm_codegen::backend::selectiondag::dag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::dag_target::FrameObjects;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};

fn is_x87(vt: EVT) -> bool {
    matches!(vt, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80))
}

/// `isScalarFPTypeInSSEReg`: with SSE2, `f32` and `f64` live in an xmm
/// register.
fn in_sse(
    vt: EVT,
    sse2: bool,
) -> bool {
    sse2 && matches!(vt, EVT::Simple(MVT::F32 | MVT::F64))
}

/// DEVIATE SD226: a store of an x87 `fp_round` that nothing else reads is one
/// truncating store (`fst`) of the rounded value. The pin's `visitSTORE` makes
/// the same fold but only where the truncating store is legal, and X86 marks
/// `f64 -> f32` Expand, so it keeps the round trip through a stack temporary
/// (`fstps (%esp); flds (%esp); fstps (%eax)`); the old selector stores once.
/// Returns the `fp_round`s it folded.
fn truncating_stores(
    dag: &mut SelectionDAG,
    sse2: bool,
) -> Vec<u32> {
    let mut folded = Vec::new();
    for node in dag.node_ids().collect::<Vec<_>>() {
        let value = SDValue { node, res: 0 };
        if dag.opcode(value) != ISD::FpRound || dag.node(value).ops.len() != 2 || !dag.has_one_use(node) {
            continue;
        }
        let source = dag.operand(value, 0);
        let (from, to) = (dag.value_type(source), dag.value_type(value));
        if !is_x87(from) || !is_x87(to) || in_sse(from, sse2) || in_sse(to, sse2) {
            continue;
        }
        let Some(user) = dag.users(value).next() else { continue };
        let store = SDValue { node: user, res: 0 };
        if dag.opcode(store) != ISD::Store || dag.operand(store, 1) != value {
            continue;
        }
        let NodeData::Mem(info) = dag.node(store).data else { continue };
        if info.volatile || info.mem_vt != to {
            continue;
        }
        let (chain, ptr) = (dag.operand(store, 0), dag.operand(store, 2));
        let truncating = dag.get_store(chain, source, ptr, info);
        dag.replace_all_uses_with(store, truncating);
        folded.push(node);
    }
    folded
}

/// Rewrite the `fp_round`s and `fp_extend`s that cross between the x87 stack
/// and the SSE registers, or round on the stack, through memory; returns how
/// many. The x87 has an extending load and a truncating store; SSE folds a
/// direct load into other operations.
pub fn preprocess(
    dag: &mut SelectionDAG,
    frame: &mut dyn FrameObjects,
    sse2: bool,
) -> usize {
    let folded = truncating_stores(dag, sse2);
    let mut rewritten = folded.len();
    // The pin walks the nodes from the last made; a node made on the way
    // (the store and load below) is neither.
    for node in dag.node_ids().rev() {
        let value = SDValue { node, res: 0 };
        if folded.contains(&node) {
            continue;
        }
        let round = match dag.opcode(value) {
            ISD::FpRound if dag.node(value).ops.len() == 2 => true,
            ISD::FpExtend => false,
            _ => continue,
        };
        let source = dag.operand(value, 0);
        let (from, to) = (dag.value_type(source), dag.value_type(value));
        if !is_x87(from) && !in_sse(from, sse2) || !is_x87(to) && !in_sse(to, sse2) {
            continue;
        }
        let (from_sse, to_sse) = (in_sse(from, sse2), in_sse(to, sse2));
        // Both in xmm registers: a legal conversion. Neither: an extension is a
        // no-op, and so is `fp_round x, 1` (the value is exact in the narrower
        // type).
        if from_sse && to_sse || !from_sse && !to_sse && (!round || dag.constant(dag.operand(value, 1)) != Some(0)) {
            continue;
        }
        let mem = if round { to } else { from };
        let size = mem.size_in_bits().div_ceil(8);
        let index = frame.create_stack_temporary(size, size);
        let slot = dag.get_frame_index(dag.config().pointer, index);
        let info = MemInfo {
            mem_vt: mem,
            align: size,
            volatile: false,
            base: MemBase::Stack(index),
            ext: None,
            space: 0,
            ordering: None,
        };
        let chain = dag.entry_node();
        let store = dag.get_store(chain, source, slot, info);
        let reloaded = if mem == to {
            dag.get_load(to, store, slot, info)
        } else {
            dag.get_ext_load(llrm_codegen::backend::selectiondag::actions::LoadExt::ExtLoad, to, store, slot, info)
        };
        dag.replace_all_uses_with(value, reloaded);
        rewritten += 1;
    }
    rewritten
}

/// What `PreprocessISelDAG` reads of the subtarget for its call block.
#[derive(Clone, Copy, Debug)]
pub struct CalleeLoadFacts {
    /// `OptLevel != CodeGenOptLevel::None`.
    pub optimizing: bool,
    pub is_64: bool,
    /// `TM.isPositionIndependent()`: none of this crate's targets has it.
    pub position_independent: bool,
    pub slow_two_mem_ops: bool,
    pub slow_indirect_call: bool,
    /// `Subtarget->useIndirectThunkCalls()`.
    pub indirect_thunks: bool,
}

/// How many times `value` is read (`SDNode::hasNUsesOfValue`).
fn uses_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> usize {
    dag.users(value)
        .map(|user| dag.node(SDValue { node: user, res: 0 }).ops.iter().filter(|op| **op == value).count())
        .sum()
}

fn has_one_use(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    uses_of(dag, value) == 1
}

/// `isCalleeLoad`
/// ([L892](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L892)):
/// the callee is a simple load that nothing else reads, that can be moved below
/// the call sequence and the chains leading up to the call. `chain` ends at the
/// call sequence's start.
fn is_callee_load(
    dag: &SelectionDAG,
    callee: SDValue,
    chain: &mut SDValue,
    has_call_seq: bool,
) -> bool {
    if callee.node == chain.node || !has_one_use(dag, callee) {
        return false;
    }
    let simple = matches!(
        dag.node(callee).data,
        NodeData::Mem(info) if !info.volatile && info.ordering.is_none() && info.ext.is_none()
    );
    // unindexed: a load of a value and a chain
    if dag.opcode(callee) != ISD::Load || dag.node(callee).values.len() != 2 || !simple {
        return false;
    }
    let load_chain = SDValue { node: callee.node, res: 1 };
    if !has_one_use(dag, load_chain) {
        return false;
    }
    while has_call_seq && dag.opcode(*chain) != ISD::CallSeqStart {
        if !has_one_use(dag, *chain) {
            return false;
        }
        *chain = dag.operand(*chain, 0);
    }
    loop {
        if dag.node(*chain).ops.is_empty() {
            return false;
        }
        // not safe to move the load across a store: only these
        if !matches!(
            dag.opcode(*chain),
            ISD::CallSeqStart | ISD::CopyToReg | ISD::Load
        ) {
            return false;
        }
        let before = dag.operand(*chain, 0);
        if before.node == callee.node {
            return true;
        }
        if dag.opcode(before) == ISD::TokenFactor
            && has_one_use(dag, SDValue { node: before.node, res: 0 })
            && dag.node(before).ops.contains(&load_chain)
            && has_one_use(dag, load_chain)
        {
            return true;
        }
        // look past CopyToRegs; only one path is walked, so the chain must not
        // branch
        if dag.opcode(before) == ISD::CopyToReg && has_one_use(dag, SDValue { node: before.node, res: 0 }) {
            *chain = before;
            continue;
        }
        return false;
    }
}

/// `moveBelowOrigChain`
/// ([L857](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L857)):
/// the original chain reads the load's chain instead, and the load moves below
/// the chain: it reads what the call read, and the call reads the load.
fn move_below_orig_chain(
    dag: &mut SelectionDAG,
    load: SDValue,
    call: SDValue,
    orig_chain: SDValue,
) {
    let chain = dag.operand(orig_chain, 0);
    let mut ops = Vec::new();
    if chain.node == load.node {
        ops.push(dag.operand(load, 0));
    } else {
        debug_assert_eq!(dag.opcode(chain), ISD::TokenFactor, "unexpected chain operand");
        for at in 0..dag.node(chain).ops.len() {
            let one = dag.operand(chain, at);
            ops.push(if one.node == load.node { dag.operand(load, 0) } else { one });
        }
        let new_chain = dag.get_node(ISD::TokenFactor, EVT::OTHER, &ops);
        ops.clear();
        ops.push(new_chain);
    }
    ops.extend(dag.node(orig_chain).ops[1..].iter().copied());
    dag.update_node_operands(orig_chain.node, &ops);
    let load_ops = [dag.operand(call, 0), dag.operand(load, 1), dag.operand(load, 2)];
    dag.update_node_operands(load.node, &load_ops);
    let mut call_ops = vec![SDValue { node: load.node, res: 1 }];
    call_ops.extend(dag.node(call).ops[1..].iter().copied());
    dag.update_node_operands(call.node, &call_ops);
}

/// The call block of `PreprocessISelDAG`
/// ([L1425](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1425)):
/// the address of a call or a tail call that is a load moves from before the
/// call sequence to just before the call, where the call or the jump can fold
/// it (`call [mem]`, `jmp [mem]`). `enough_regs` is `checkTCRetEnoughRegs`.
/// Returns how many it moved.
pub fn move_callee_loads(
    dag: &mut SelectionDAG,
    facts: CalleeLoadFacts,
    enough_regs: &mut dyn FnMut(&mut SelectionDAG, u32) -> bool,
) -> usize {
    if !facts.optimizing || facts.indirect_thunks {
        return 0;
    }
    let mut moved = 0;
    for node in dag.node_ids().collect::<Vec<_>>() {
        let call = SDValue { node, res: 0 };
        if dag.is_deleted(node) {
            continue;
        }
        let opcode = dag.opcode(call);
        let call_ok = opcode == super::x86isd::CALL
            && !facts.slow_two_mem_ops
            && !facts.slow_indirect_call;
        let tail_ok = opcode == super::x86isd::TC_RETURN
            && (facts.is_64 || !facts.position_independent);
        if !call_ok && !tail_ok || dag.node(call).ops.len() < 2 {
            continue;
        }
        let mut chain = dag.operand(call, 0);
        let load = dag.operand(call, 1);
        if !is_callee_load(dag, load, &mut chain, opcode == super::x86isd::CALL) {
            continue;
        }
        if opcode == super::x86isd::TC_RETURN && !enough_regs(dag, node) {
            continue;
        }
        move_below_orig_chain(dag, load, call, chain);
        moved += 1;
    }
    moved
}
