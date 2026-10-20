//! Sets up a procedure's frame and kept registers where they are first needed,
//! not at the entry, and takes them back only on the returns that path reaches.
//!
//! # How it works
//! The frame and the registers a procedure keeps are one piece of work: set up
//! at the top of one Save block, taken back before the return of one Restore
//! block. [`crate::backend::saverestore`] finds the two blocks as LLVM's
//! `ShrinkWrapImpl` does (the nearest blocks that dominate and post-dominate
//! every use, outside any loop, no hotter than the entry); this module says
//! which blocks use the frame or a kept register and lays the result out for
//! `masm`.
//!
//! A path that never reaches Save pays nothing: no push, no frame, a bare
//! `ret`. There is one epilogue, so no return is copied to give a path its own.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   entry["<b>entry</b>"]:::mid
//!   early["<b>early return</b><br/><small>no push, no frame</small>"]:::mid
//!   save["<b>Save</b><br/><small>push si#59; frame</small>"]:::src
//!   loop["<b>loop</b>"]:::mid
//!   ret["<b>Restore</b><br/><small>pop si#59; ret</small>"]:::mid
//!   entry --> early
//!   entry --> save
//!   save --> loop
//!   loop --> ret
//! ```
//!
//! *One Save, one Restore*
//!
//! A wrap is refused where the placement cannot be used as it is:
//!
//! - The Restore does not end in a return (the epilogue would sit before a
//!   jump).
//! - Some block the Save reaches is also reached around it, for the frame and
//!   wherever the stack pointer addresses the frame cells, so a block would be
//!   entered at two stack depths.
//!
//! Where the entry uses the frame, the registers alone are saved late (the
//! frame stays at the entry, before them). No register is set up above the
//! frame, since the frame's cells sit just below the arguments and a push
//! before the reserve would cover them: the frame comes first at Save and last
//! back at Restore.
//!
//! # Example
//! The pass places pieces while masm builds the procedure, after LIR is
//! final, so its result is in the assembly, not in a LIR dump. A function that
//! returns `k` at once when `n` is 0 and otherwise loops over calls to `g`:
//!
//! ```asm
//! f_ proc near
//! L0_0:
//!     or eax, eax
//!     je L0_13          ; early return: no push, no pop
//! L0_2:
//!     push ecx          ; set-up in the home block
//!     push ebx
//!     push esi
//!     ; ... loop with `call g_`
//! L0_14:
//!     mov eax, edx
//!     pop esi           ; taken back only on this return
//!     pop ebx
//!     pop ecx
//!     ret
//! L0_13:
//!     mov eax, edx
//!     ret
//! f_ endp
//! ```
//!
//! The three pushes sit in `L0_2`, which the early return does not reach, and
//! the early return leaves with a bare `ret`.
//!
//! From `-S` of a C function (`if (n == 0) return k;` then a loop calling `g`)
//! at -O2 -m32; the comments are added.
//!
//! # Runs
//! `wrapped` runs when masm builds a procedure's items, for a procedure that
//! saves a register or has a frame, whose blocks are all reachable from the
//! entry, and that returns. The frame is not moved in an interrupt handler, a
//! runtime entry frame, a procedure with a stack check or one with inline code.
//! Registers are not wrapped in an interrupt handler or a runtime entry frame.
//! There is no level switch beyond that.
//!
//! DEVIATE 1: LLVM's `postShrinkWrapping` splits the restore block so that the
//! paths that need nothing skip it; masm makes no new block, so a placement
//! that asks for the split is refused (`saverestore`'s DEVIATE 2).
//!
//! # See also
//! LLVM's
//! [`ShrinkWrap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_mir::module::BlockId;

use crate::analysis::loops::dominance;
use crate::backend::saverestore::{self, Frame, Frequencies};
use crate::liveness::blockfrequency::{Graph, MachineBlockFrequencyInfo};
use crate::model::ir::{self, Loc};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// What a procedure sets up and takes back: the frame, or a register it keeps
/// for its caller.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Piece {
    Frame,
    Saved(RegId),
}

/// Where a procedure sets its pieces up that are not set up at its entry:
/// at the top of the block of `opens`, in that order, and taken back before
/// the return of `closes`, in that order, before what the entry set up.
pub struct Wrap {
    pub opens: BTreeMap<i64, Vec<Piece>>,
    pub closes: BTreeMap<i64, Vec<Piece>>,
    pub wrapped: BTreeSet<Piece>,
}

/// The registers `one` names, and those the call it is disturbs by its
/// convention.
pub fn named(one: &Insn) -> BTreeSet<RegId> {
    let mut found: BTreeSet<RegId> =
        one.call.iter().flat_map(|call| call.disturbs.iter().copied().map(ir::root)).collect();
    let Some(what) = &one.what else { return found };
    for place in what.dests.iter().chain(&what.sources) {
        match place {
            Loc::Reg(ir::Reg { register, .. }) => {
                found.insert(ir::root(*register));
            }
            Loc::Mem(ir::Mem { through, index_through, .. }) => {
                found.extend([*through, *index_through].map(ir::root));
            }
            _ => {}
        }
    }
    found
}

/// Whether `one` reads or writes the frame or the stack pointer: a frame cell
/// or argument, a push or pop, or an instruction the printer cannot read.
fn touches_frame(one: &Insn) -> bool {
    let Some(what) = &one.what else { return false };
    if matches!(
        what.op,
        ir::Operation::Push | ir::Operation::Pop | ir::Operation::Leave | ir::Operation::Escape
    )
        || (what.op == ir::Operation::Nothing
            && what.name.as_deref().is_some_and(|name| name.starts_with("push") || name.starts_with("pop")))
    {
        return true;
    }
    what.dests.iter().chain(&what.sources).any(Loc::in_frame)
}

/// The wrap of `body` for the registers `kept`, and for the frame where `frame`
/// names the frame register and the stack pointer, if one is worth having:
/// `ShrinkWrapImpl`'s one Save and one Restore (`saverestore`), every piece set
/// up at the top of Save and taken back before the return of Restore.
/// No register is set up above the frame, whichever register addresses its
/// cells: they sit directly below the arguments, and a push before the
/// reserve would put the cells over it. With `stack_addressed` the stack
/// pointer addresses them at the depth each block is entered at, and every
/// block must be entered at one, whatever a piece sets up.
pub fn wrapped(
    body: &LirBody,
    kept: &BTreeSet<RegId>,
    frame: Option<(RegId, RegId)>,
    stack_addressed: bool,
) -> Option<Wrap> {
    if body.noreturn || (kept.is_empty() && frame.is_none()) || body.blocks.is_empty() {
        return None;
    }
    let from_entry = dominance(&body.blocks, Some(body.entry));
    // Every block must be reached from the entry: a block only the runtime
    // enters has no dominator.
    if body.blocks.iter().any(|block| !from_entry.reachable(block.at)) {
        return None;
    }
    let graph = Graph::of(body);
    let frequencies = Frequent { info: &MachineBlockFrequencyInfo::of(body) };
    // The frame and the registers at one Save; where the entry uses the frame
    // (its arguments are addressed through it), the registers alone are saved
    // late and the frame stays at the entry, set up before them.
    let framed = frame.is_some();
    let mut attempts = vec![framed];
    if framed {
        attempts.push(false);
    }
    for with_frame in attempts {
        let uses = Uses::of(body, kept, frame.filter(|_| with_frame));
        let Some(placement) = saverestore::run(&graph, &uses, &frequencies) else { continue };
        // DEVIATE 1: the consumer makes no block to split the restore.
        if placement.split.is_some() {
            continue;
        }
        let (save, restore) = (body.blocks[placement.save.0 as usize].at, body.blocks[placement.restore.0 as usize].at);
        // The epilogue goes before a return; a block that jumps on would need
        // the flags and the stack as they are.
        if !uses.is_return(placement.restore) {
            continue;
        }
        // A block the save reaches that something else also reaches is entered
        // at two depths.
        if stack_addressed || with_frame {
            let from_save = dominance(&body.blocks, Some(save));
            if body.blocks.iter().any(|block| from_save.reachable(block.at) && !from_entry.dominates(save, block.at)) {
                continue;
            }
        }
        let mut pieces: Vec<Piece> = Vec::new();
        if with_frame {
            pieces.push(Piece::Frame);
        }
        pieces.extend(kept.iter().map(|&register| Piece::Saved(register)));
        let closes: Vec<Piece> = pieces.iter().rev().copied().collect();
        return Some(Wrap {
            wrapped: pieces.iter().copied().collect(),
            opens: BTreeMap::from([(save, pieces)]),
            closes: BTreeMap::from([(restore, closes)]),
        });
    }
    None
}

/// What `body`'s blocks do with the frame and the kept registers
/// (`useOrDefCSROrFI`).
struct Uses<'a> {
    body: &'a LirBody,
    kept: &'a BTreeSet<RegId>,
    frame_registers: BTreeSet<RegId>,
    framed: bool,
}

impl<'a> Uses<'a> {
    fn of(
        body: &'a LirBody,
        kept: &'a BTreeSet<RegId>,
        frame: Option<(RegId, RegId)>,
    ) -> Self {
        let frame_registers =
            frame.iter().flat_map(|(pointer, stack)| [ir::root(*pointer), ir::root(*stack)]).collect();
        Self { body, kept, frame_registers, framed: frame.is_some() }
    }

    fn insn_needs(
        &self,
        one: &Insn,
    ) -> bool {
        let names = named(one);
        (self.framed && (touches_frame(one) || names.iter().any(|register| self.frame_registers.contains(register))))
            || names.iter().any(|register| self.kept.contains(register))
    }

    fn block(
        &self,
        block: BlockId,
    ) -> &LirBlock {
        &self.body.blocks[block.0 as usize]
    }
}

impl Frame for Uses<'_> {
    fn touches(
        &self,
        block: BlockId,
        _: bool,
    ) -> bool {
        self.block(block).insns.iter().any(|one| self.insn_needs(one))
    }

    fn terminators_touch(
        &self,
        block: BlockId,
    ) -> bool {
        let last = self.block(block).insns.last();
        last.is_some_and(|one| {
            one.what.as_ref().is_some_and(|what| {
                matches!(
                    what.op,
                    ir::Operation::Branch | ir::Operation::Jump | ir::Operation::Return
                )
            }) && self.insn_needs(one)
        })
    }

    fn is_return(
        &self,
        block: BlockId,
    ) -> bool {
        self.block(block)
            .insns
            .last()
            .is_some_and(|one| one.what.as_ref().is_some_and(|what| what.op == ir::Operation::Return))
    }
}

/// `MachineBlockFrequencyInfo` as the placement reads it.
struct Frequent<'a> {
    info: &'a MachineBlockFrequencyInfo,
}

impl Frequencies for Frequent<'_> {
    fn entry(&self) -> u64 {
        self.info.get_entry_freq().get_frequency()
    }

    fn block(
        &self,
        block: BlockId,
    ) -> u64 {
        self.info.get_block_freq(block.0 as usize).get_frequency()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Imm, Reg, Semantics};
    use crate::support::hash::IndexMap;

    fn insn(
        at: i64,
        what: Semantics,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![]))
    }

    /// Writes `register`.
    fn touch(
        at: i64,
        register: RegId,
    ) -> Arc<Insn> {
        let one = Semantics {
            name: Some("mov".to_owned()),
            dests: vec![Loc::Reg(Reg { register, width: 4 })],
            sources: vec![Loc::Imm(Imm { value: 1, width: 2, address: None })],
            ..Semantics::new(ir::Operation::Move)
        };
        insn(at, one)
    }

    fn branch(
        at: i64,
        target: i64,
    ) -> Arc<Insn> {
        insn(
            at,
            Semantics { name: Some("je".to_owned()), target: Some(target), ..Semantics::new(ir::Operation::Branch) },
        )
    }

    fn ret(at: i64) -> Arc<Insn> {
        insn(at, Semantics { name: Some("ret".to_owned()), ..Semantics::new(ir::Operation::Return) })
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }
    }

    fn body(blocks: Vec<LirBlock>) -> LirBody {
        let blocks = crate::model::lir::shared(blocks);
        LirBody::new("wrap", blocks[0].at, blocks, IndexMap::default(), IndexMap::default())
    }

    /// The early exit saved every register the late path uses, and popped them
    /// again: hanoi's `push ecx` before the `n == 0` test.
    #[test]
    fn test_a_register_used_only_below_the_early_exit_is_saved_there_and_nowhere_else() {
        let blocks = vec![
            block(0, vec![branch(2, 1)], vec![1, 2]),
            block(1, vec![ret(10)], vec![]),
            block(2, vec![touch(20, RegId::ESI), ret(22)], vec![]),
        ];
        let wrap = wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false).expect("a wrap");
        let both = vec![Piece::Saved(RegId::EBX), Piece::Saved(RegId::ESI)];
        assert_eq!(wrap.opens, BTreeMap::from([(2, both.clone())]));
        assert_eq!(wrap.closes, BTreeMap::from([(2, both.into_iter().rev().collect())]));
    }

    /// A register the entry uses holds the save at the entry.
    #[test]
    fn test_a_register_the_entry_uses_keeps_the_save_at_the_entry() {
        let blocks = vec![
            block(0, vec![touch(0, RegId::EBX), branch(2, 1)], vec![1, 2]),
            block(1, vec![ret(10)], vec![]),
            block(2, vec![touch(20, RegId::ESI), ret(22)], vec![]),
        ];
        assert!(wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false).is_none());
    }

    /// A return the early exit shares with the late path is reached around the
    /// save: nothing is saved late, and no tail is copied to change that.
    #[test]
    fn test_a_return_the_early_exit_shares_with_the_late_path_keeps_the_save_at_the_entry() {
        let blocks = vec![
            block(0, vec![branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::ESI)], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        assert!(wrapped(&body(blocks), &BTreeSet::from([RegId::ESI]), None, false).is_none());
    }

    /// Two late paths that each use a register were saved and popped per exit
    /// (the sieve rows' extra `pop; ret` tails): one Save, one Restore.
    #[test]
    fn test_one_restore_serves_every_path_below_the_save() {
        let blocks = vec![
            block(0, vec![branch(5, 1)], vec![1, 5]),
            block(1, vec![touch(10, RegId::ESI), branch(3, 2)], vec![2, 3]),
            block(2, vec![touch(20, RegId::EDI)], vec![4]),
            block(3, vec![touch(30, RegId::EBX)], vec![4]),
            block(4, vec![ret(40)], vec![]),
            block(5, vec![ret(50)], vec![]),
        ];
        let kept = BTreeSet::from([RegId::EBX, RegId::ESI, RegId::EDI]);
        let wrap = wrapped(&body(blocks), &kept, None, false).expect("a wrap");
        assert_eq!(wrap.opens.keys().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(wrap.closes.keys().copied().collect::<Vec<_>>(), [4]);
    }
}
