//! The loads and stores of a block that AArch64's load/store optimizer pairs
//! (`ldp`/`stp`), so that a trial prices straight-line memory as the target
//! leaves it: adjacent cells of one object, in the same block, are one
//! instruction, and a loop's rolled access never is.
//!
//! The pin's rule (`tryToPairLdStInst` and `findMatchingInsn`, d1106deb71cc):
//! two plain loads or two plain stores of one size (4, 8 or 16 bytes), at
//! adjacent offsets of one base, the lower a multiple of the size and within
//! the 7-bit scaled offset, with nothing between them that may alias, and at
//! most `LdStLimit` (20) instructions apart.
//!
//! DEVIATE UP1-PAIR (owner w15, exit 2026-11-20): the base register is the
//! object (MIR has no registers), and a load's destination, used in between,
//! is not checked.
//!
//! # See also
//! [`tryToPairLdStInst`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64LoadStoreOptimizer.cpp#L2846),
//! [`findMatchingInsn`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64LoadStoreOptimizer.cpp#L2016),
//! [`inBoundsForPair`](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64LoadStoreOptimizer.cpp#L1581).

use llrm_analysis::memory::{MemRef, Unit};
use llrm_mir::module::{BlockId, Operand};

use super::*;

/// `aarch64-load-store-scan-limit`.
const SCAN_LIMIT: usize = 20;

struct Access {
    store: bool,
    root: Operand,
    disp: i64,
    size: i64,
    at: usize,
}

/// The loads and the stores of `block` that pair.
pub(super) fn paired(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &llrm_mir::passes::Outer,
    block: BlockId,
) -> (i64, i64) {
    let unit = Unit::within(context, layout, function, outer);
    let (mut loads, mut stores) = (0, 0);
    let mut open: Vec<Access> = Vec::new();
    for (at, inst) in function.iter_block(block).enumerate() {
        let one = function.instruction(inst);
        let (store, pointer, ty) = match one.opcode {
            Opcode::Load { volatile: false, .. } => {
                (false, one.operands[0], one.result.map(|result| function.value(result).ty))
            }
            Opcode::Store { volatile: false, .. } => {
                (true, one.operands[1], function.operand_type(context, one.operands[0]))
            }
            Opcode::Call(_) | Opcode::Load { .. } | Opcode::Store { .. } => {
                open.clear();
                continue;
            }
            _ => {
                if llrm_mir::touched::touches_memory(&one.opcode) {
                    open.clear();
                }
                continue;
            }
        };
        let size = ty.map_or(0, |ty| layout.store_size(&context.types, ty) as i64);
        let reference = MemRef::at(&unit, pointer, size.max(1) as u32);
        let root = reference.root.filter(|_| reference.object && reference.base.is_none() && reference.segment.is_none());
        let (Some(root), true) = (root, matches!(size, 4 | 8 | 16)) else {
            open.clear();
            continue;
        };
        let now = Access { store, root, disp: reference.disp, size, at };
        // What may alias ends the wait of the accesses it touches: the other
        // kind, or the same kind overlapping, in the same object.
        let (low, high) = (now.disp, now.disp + now.size);
        open.retain(|before| {
            before.root != now.root
                || (before.store == now.store && (before.disp + before.size <= low || high <= before.disp))
        });
        open.retain(|before| now.at - before.at <= SCAN_LIMIT);
        let partner = open.iter().position(|before| {
            before.store == now.store
                && before.root == now.root
                && before.size == now.size
                && (before.disp + before.size == now.disp || now.disp + now.size == before.disp)
                && {
                    let lower = before.disp.min(now.disp);
                    lower % now.size == 0 && (-64..=63).contains(&(lower / now.size))
                }
        });
        match partner {
            Some(index) => {
                open.remove(index);
                if store {
                    stores += 1;
                } else {
                    loads += 1;
                }
            }
            None => open.push(now),
        }
    }
    (loads, stores)
}

#[cfg(test)]
mod tests {
    use llrm_mir::passes::Outer;

    use super::*;
    use crate::testing::parsed;

    /// The pairs of the entry block of `@f`.
    fn pairs_of(body: &str) -> (i64, i64) {
        let mut module = parsed(&format!(
            "@a = global [64 x i32] zeroinitializer\n@b = global [64 x i32] zeroinitializer\ndeclare void @g()\n\
             define void @f(ptr %p) {{\nentry:\n{body}\n  ret void\n}}\n"
        ));
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap_or("e-p:64:64-n32:64")).expect("layout");
        let outer = Outer::of(&module, None);
        let (context, function) = module.function_mut("f").expect("@f");
        let entry = function.layout()[0];
        paired(context, &layout, function, &outer, entry)
    }

    fn store(
        bytes: i64,
        at: i64,
        object: &str,
    ) -> String {
        format!("  store i32 7, ptr getelementptr inbounds (i8, ptr @{object}, i64 {})\n", at * bytes)
    }

    /// Four adjacent words stored in a row are two `stp`: the trial must not
    /// count them as four stores (lru -O2: +2 B priced where the backend's
    /// stores of the unrolled loop are fewer than the rolled loop's).
    #[test]
    fn adjacent_stores_of_one_object_pair() {
        let body: String = (0..4).map(|at| store(4, at, "a")).collect();
        assert_eq!(pairs_of(&body), (0, 2));
    }

    #[test]
    fn a_call_between_stores_keeps_them_apart() {
        let body = format!("{}  call void @g()\n{}", store(4, 0, "a"), store(4, 1, "a"));
        assert_eq!(pairs_of(&body), (0, 0));
    }

    #[test]
    fn stores_of_two_objects_do_not_pair() {
        let body = format!("{}{}", store(4, 0, "a"), store(4, 1, "b"));
        assert_eq!(pairs_of(&body), (0, 0));
    }

    #[test]
    fn a_pair_must_start_at_a_multiple_of_its_size() {
        let body = format!("{}{}", store(1, 2, "a"), store(1, 6, "a"));
        assert_eq!(pairs_of(&body), (0, 0));
    }
}
