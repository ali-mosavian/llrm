//! The call sequence as a DAG, for a caller that has no builder: a library
//! helper the legalizer calls (`i64` division, `fmod`).
//!
//! # How it works
//! [`lib_call`] makes what `LowerCallTo` and X86's `LowerCall` make for a
//! direct call: `callseq_start` on the root, a store per stack argument
//! through `$esp`, a glued `CopyToReg` per register argument, `X86ISD::CALL`
//! with the register operands and mask, `callseq_end`, then a `CopyFromReg`
//! of each result register, each on the chain and glue of the one before.
//! Where each argument goes is the caller's: a [`Place`] per argument, from
//! the helper's register contract or a
//! [`Convention`](crate::backend::isel::Convention).
//!
//! # Example
//! Two `i32` stack arguments and a result in `eax`:
//!
//! ```text
//! t1: ch,glue = callseq_start t0, 8, 0 ; t2 = CopyFromReg t1, $esp
//! store t1, a, t2 ; store t1, b, add t2, 4 ; TokenFactor
//! X86ISD::CALL chain, TargetExternalSymbol'helper', RegisterMask, ...
//! callseq_end ... ; CopyFromReg ..., $eax, glue
//! ```
//!
//! # Runs
//! Called by the type legalizer's libcall expansion (sdag-b); the builder's
//! `lower_call` makes the same nodes.
//!
//! # See also
//! LLVM's `TargetLowering::LowerCallTo`
//! ([SelectionDAGBuilder.cpp:9256](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9256)).

use llrm_lir::registers::RegId;

use super::dag::{MemBase, MemInfo, SDValue, SelectionDAG};
use super::isd::ISD;
use super::mvt::EVT;
use super::x86isd;

/// Where an argument goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// In the stack slot `offset` bytes from `$esp` at the call.
    Stack(i64),
    Register(RegId),
}

/// What a call result is read from.
#[derive(Clone, Copy, Debug)]
pub struct Returned {
    pub reg: RegId,
    pub vt: EVT,
}

/// The target facts a call needs.
pub struct CallEnv<'a> {
    pub arch: &'a dyn llrm_target::Target,
    pub pointer: EVT,
}

/// The nodes of the call: the chain after it, and the value read from each
/// result register in order.
pub fn lib_call(
    dag: &mut SelectionDAG,
    env: &CallEnv<'_>,
    root: SDValue,
    callee: SDValue,
    args: &[(SDValue, Place)],
    stack_bytes: u64,
    popped: u64,
    returns: &[Returned],
) -> (SDValue, Vec<SDValue>) {
    let start = dag.get_callseq_start(root, stack_bytes);
    let mut chain = SDValue { node: start.node, res: 0 };
    let mut stores = Vec::new();
    let mut copies: Vec<(RegId, SDValue)> = Vec::new();
    let mut stack_pointer = None;
    for (value, place) in args {
        match *place {
            Place::Stack(offset) => {
                let sp = *stack_pointer.get_or_insert_with(|| {
                    let sp = env.arch.register_named(super::builder::stack_pointer_name(env.pointer));
                    dag.get_copy_from_phys_reg_plain(chain, sp, env.pointer)
                });
                let offset = dag.get_constant(offset as u128, env.pointer);
                let address = dag.get_node(ISD::Add, env.pointer, &[sp, offset]);
                let vt = dag.value_type(*value);
                let mem = MemInfo {
                    mem_vt: vt,
                    align: 1,
                    volatile: false,
                    base: MemBase::Unknown,
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                stores.push(dag.get_store(chain, *value, address, mem));
            }
            Place::Register(reg) => copies.push((reg, *value)),
        }
    }
    if !stores.is_empty() {
        chain = dag.get_token_factor(&stores);
    }
    let mut glue: Option<SDValue> = None;
    for (reg, value) in &copies {
        let copy = dag.get_copy_to_phys_reg(chain, *reg, *value, glue);
        chain = SDValue { node: copy.node, res: 0 };
        glue = Some(SDValue { node: copy.node, res: 1 });
    }
    let mut ops = vec![chain, callee];
    for (reg, value) in &copies {
        let vt = dag.value_type(*value);
        ops.push(dag.get_phys_register(vt, *reg));
    }
    ops.push(dag.get_register_mask());
    ops.extend(glue);
    let call = dag.get_node_multi(x86isd::CALL, &[EVT::OTHER, EVT::GLUE], &ops);
    let end = dag.get_callseq_end(
        SDValue { node: call.node, res: 0 },
        stack_bytes,
        popped,
        SDValue { node: call.node, res: 1 },
    );
    let mut chain = SDValue { node: end.node, res: 0 };
    let mut glue = SDValue { node: end.node, res: 1 };
    let mut results = Vec::new();
    for returned in returns {
        let copy = dag.get_copy_from_phys_reg(chain, returned.reg, returned.vt, glue);
        chain = SDValue { node: copy.node, res: 1 };
        glue = SDValue { node: copy.node, res: 2 };
        results.push(copy);
    }
    (chain, results)
}

/// `makeLibCall` for a C routine: `name` called with `args`, each placed where
/// the target's native convention puts an integer of its width, and a result
/// of type `ret` read from where that convention leaves it (a pair of
/// registers, low first, is a `BuildPair`). `None` where an argument needs
/// several registers.
pub fn c_lib_call(
    dag: &mut SelectionDAG,
    env: &CallEnv<'_>,
    name: &str,
    args: &[SDValue],
    ret: EVT,
) -> Option<SDValue> {
    use llrm_target::calling::Place as Where;
    let native = env.arch.calling().native();
    let slot = env.arch.stack_slot_bytes();
    let kinds: Vec<_> = args
        .iter()
        .map(|&arg| {
            let bytes = i64::from(dag.value_type(arg).size_in_bits() / 8);
            native.kind(llrm_mir::target::Argument { bytes, integer: true, pointer: false, floating: false, memory: false }, slot)
        })
        .collect();
    let placement = native.place(&kinds);
    let mut places = Vec::new();
    for (&arg, place) in args.iter().zip(&placement.places) {
        match place {
            Where::Stack(offset) => places.push((arg, Place::Stack(*offset))),
            Where::Registers(names) if names.len() == 1 => {
                places.push((arg, Place::Register(env.arch.register_named(&names[0]))))
            }
            Where::Registers(names) if names.len() == 2 => {
                // `getCopyToParts`: the low half, then the high, each in its register.
                let half = EVT::integer(dag.value_type(arg).size_in_bits() / 2);
                let amount = dag.get_constant(u128::from(half.size_in_bits()), EVT::I8);
                let high = dag.get_node(ISD::Srl, dag.value_type(arg), &[arg, amount]);
                let (low, high) = (dag.get_node(ISD::Truncate, half, &[arg]), dag.get_node(ISD::Truncate, half, &[high]));
                places.push((low, Place::Register(env.arch.register_named(&names[0]))));
                places.push((high, Place::Register(env.arch.register_named(&names[1]))));
            }
            Where::Registers(_) => return None,
        }
    }
    let result_bytes = i64::from(ret.size_in_bits() / 8);
    let result = native.result_registers(result_bytes)?;
    let part = match result.len() {
        1 => ret,
        2 => EVT::integer(ret.size_in_bits() / 2),
        _ => return None,
    };
    let returns: Vec<Returned> =
        result.iter().map(|register| Returned { reg: env.arch.register_named(register), vt: part }).collect();
    let callee = dag.get_target_external_symbol(env.pointer, name);
    let root = dag.entry_node();
    let stack = placement.stack_bytes as u64;
    let (_chain, values) = lib_call(dag, env, root, callee, &places, stack, 0, &returns);
    Some(match values[..] {
        [one] => one,
        [low, high] => dag.get_node(ISD::BuildPair, ret, &[low, high]),
        _ => return None,
    })
}
