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
//! `ret`. A path that falls into a return the early paths share takes its
//! registers back at its own end, before the jump or where it falls in
//! (`tryToSplitRestore`'s split block, without the block).
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
//! - The Restore does not end in a return and does not leave by one jump (the
//!   epilogue would sit before a branch that reads the flags).
//! - The Restore is split, and a block that takes it back does not jump to it
//!   or fall into it with no branch of its own to it.
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
//! paths that need nothing skip it; masm makes no new block. The epilogue goes
//! at the end of each block that was to get the split block: before its jump to
//! the restore, or where it falls into it. A block that reaches the restore any
//! other way refuses the placement (`saverestore`'s DEVIATE 2).
//!
//! DEVIATE 2: under `size` (the m16 `-Os` targets) a restore that needs the
//! epilogue on an edge, or that does not end in a return, is refused; the
//! registers stay saved at the entry. The pin wraps those and duplicates the
//! `ret` into each early exit, which costs a byte per exit, and the branch that
//! skips the epilogue grows past its short form (QCport m16 -Os +9 B, nothing
//! smaller). Exit: the placement is made as the pin does once jumps are sized
//! after the epilogue is placed.
//!
//! # See also
//! LLVM's
//! [`ShrinkWrap`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/ShrinkWrap.cpp).

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use llrm_lir::registers::{RegId, Regs};
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
    /// Taken back at the end of the block, before its jump to the restore or
    /// where it falls into it (the edge `tryToSplitRestore` puts the epilogue
    /// on), in that order.
    pub closes_after: BTreeMap<i64, Vec<Piece>>,
    /// Taken back on the taken edge of the block's conditional branch to the
    /// restore: `masm` makes the block `tryToSplitRestore` splits the edge
    /// with, after the procedure's last.
    pub closes_taken: BTreeMap<i64, Vec<Piece>>,
    pub wrapped: BTreeSet<Piece>,
}

/// The registers `one` names, and those the call it is disturbs by its
/// convention.
pub fn named(
    regs: Regs,
    one: &Insn,
) -> BTreeSet<RegId> {
    let mut found = BTreeSet::new();
    any_named(regs, one, |register| {
        found.insert(register);
        false
    });
    found
}

/// Whether `test` holds of a register `one` names (`named`'s, as it finds
/// them, none collected): what `useOrDefCSROrFI` asks of each operand.
fn any_named(
    regs: Regs,
    one: &Insn,
    mut test: impl FnMut(RegId) -> bool,
) -> bool {
    if one.call.iter().flat_map(|call| call.disturbs.iter()).any(|register| test(regs.root(*register))) {
        return true;
    }
    let Some(what) = &one.what else { return false };
    what.dests
        .iter()
        .chain(&what.sources)
        .any(
            |place| match place {
                Loc::Reg(ir::Reg { register, .. }) => test(regs.root(*register)),
                Loc::Mem(ir::Mem { through, index_through, .. }) => {
                    test(regs.root(*through)) || test(regs.root(*index_through))
                }
                _ => false,
            },
        )
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
/// block must be entered at one, whatever a piece sets up. With `size` a
/// restore that needs the epilogue placed on an edge is refused.
pub fn wrapped(
    body: &LirBody,
    kept: &BTreeSet<RegId>,
    frame: Option<(RegId, RegId)>,
    stack_addressed: bool,
    size: bool,
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
        // DEVIATE 1: the consumer makes no block to split the restore; it puts
        // the epilogue at the end of each predecessor, before its jump to the
        // restore or where it falls into it, and refuses any other.
        let split = placement.split.as_deref().unwrap_or_default();
        // Under `size` the split changes where the early exit lands, and a
        // branch over the epilogue grows past its short form (QCport m16 -Os
        // +9 B, nothing smaller): the registers stay saved at the entry. The
        // caller states `size` only for a target that says so (the pin has no
        // such check).
        if size && (!split.is_empty() || !uses.is_return(placement.restore)) {
            continue;
        }
        if !split.iter().all(|&one| {
            ends_on_edge_into(body, one, placement.restore) || ends_on_taken_edge_into(body, one, placement.restore)
        }) {
            continue;
        }
        let (save, restore) = (body.blocks[placement.save.0 as usize].at, body.blocks[placement.restore.0 as usize].at);
        // The epilogue goes before a return. A restore that does not end in
        // one has it before its one jump, or at its end: nothing else may leave
        // it, for the flags and the stack as the jump finds them.
        let returns = uses.is_return(placement.restore);
        if !returns && !split.is_empty() {
            continue;
        }
        let leaves = (!returns).then(|| body.blocks[placement.restore.0 as usize].succ.to_vec());
        if leaves.as_ref().is_some_and(|leaves| !leaves_by_one_jump(body, placement.restore, leaves)) {
            continue;
        }
        // A block the save reaches that something else also reaches is entered
        // at two depths; the edges the epilogue is on are where it has run.
        if stack_addressed || with_frame {
            let torn_down: Vec<(i64, i64)> = match &leaves {
                Some(leaves) => leaves.iter().map(|&next| (restore, next)).collect(),
                None => split.iter().map(|one| (body.blocks[one.0 as usize].at, restore)).collect(),
            };
            let from_save = reached_from(body, save, &torn_down);
            if body.blocks.iter().any(|block| from_save.contains(&block.at) && !from_entry.dominates(save, block.at)) {
                continue;
            }
        }
        let mut pieces: Vec<Piece> = Vec::new();
        if with_frame {
            pieces.push(Piece::Frame);
        }
        pieces.extend(kept.iter().map(|&register| Piece::Saved(register)));
        let closes: Vec<Piece> = pieces.iter().rev().copied().collect();
        let mut closes_taken = BTreeMap::new();
        let (closes, closes_after) = if !split.is_empty() {
            let mut after = BTreeMap::new();
            for &one in split {
                let at = body.blocks[one.0 as usize].at;
                if ends_on_taken_edge_into(body, one, placement.restore) {
                    closes_taken.insert(at, closes.clone());
                } else {
                    after.insert(at, closes.clone());
                }
            }
            (BTreeMap::new(), after)
        } else if returns {
            (BTreeMap::from([(restore, closes)]), BTreeMap::new())
        } else {
            (BTreeMap::new(), BTreeMap::from([(restore, closes)]))
        };
        return Some(Wrap {
            wrapped: pieces.iter().copied().collect(),
            opens: BTreeMap::from([(save, pieces)]),
            closes,
            closes_after,
            closes_taken,
        });
    }
    None
}

/// Whether the end of `from` is on its one edge into `to` and no other: it
/// jumps to `to` (the epilogue goes before the jump), or falls into `to`, the
/// block laid out after it, with no jump.
fn ends_on_edge_into(
    body: &LirBody,
    from: BlockId,
    to: BlockId,
) -> bool {
    let (Some(block), Some(restore)) = (body.blocks.get(from.0 as usize), body.blocks.get(to.0 as usize)) else {
        return false;
    };
    let next = from.0 + 1 == to.0;
    let only = block.succ.len() == 1 && block.succ.contains(&restore.at);
    let Some(what) =
        block.insns.iter().rev().find(|one| crate::backend::masm::prints(one)).and_then(|one| one.what.as_ref())
    else {
        return next && only;
    };
    match what.op {
        ir::Operation::Jump => only && what.target == Some(restore.at),
        // The edge not taken falls through.
        ir::Operation::Branch => next && block.succ.contains(&restore.at) && what.target != Some(restore.at),
        ir::Operation::Return => false,
        _ => next && only,
    }
}

/// Whether `from` ends in a conditional branch that takes it to `to` and
/// falls into another block: the edge `tryToSplitRestore` splits is the taken
/// one.
fn ends_on_taken_edge_into(
    body: &LirBody,
    from: BlockId,
    to: BlockId,
) -> bool {
    let (Some(block), Some(restore)) = (body.blocks.get(from.0 as usize), body.blocks.get(to.0 as usize)) else {
        return false;
    };
    let Some(what) =
        block.insns.iter().rev().find(|one| crate::backend::masm::prints(one)).and_then(|one| one.what.as_ref())
    else {
        return false;
    };
    what.op == ir::Operation::Branch && what.target == Some(restore.at) && from.0 + 1 != to.0
}

/// Whether `from` leaves by one jump, or falls out by none, and so can take the
/// epilogue at its end.
fn leaves_by_one_jump(
    body: &LirBody,
    from: BlockId,
    leaves: &[i64],
) -> bool {
    let Some(block) = body.blocks.get(from.0 as usize) else { return false };
    let last = block.insns.iter().rev().find(|one| crate::backend::masm::prints(one)).and_then(|one| one.what.as_ref());
    leaves.len() == 1
        && match last.map(|what| what.op) {
            None => true,
            Some(ir::Operation::Jump) => true,
            Some(ir::Operation::Branch | ir::Operation::Return) => false,
            Some(_) => true,
        }
}

/// The blocks reached from `from` without crossing the `cut` edges.
fn reached_from(
    body: &LirBody,
    from: i64,
    cut: &[(i64, i64)],
) -> BTreeSet<i64> {
    let mut seen = BTreeSet::from([from]);
    let mut work = vec![from];
    while let Some(at) = work.pop() {
        let Some(block) = body.blocks.iter().find(|one| one.at == at) else { continue };
        for &next in block.succ.iter() {
            if !cut.contains(&(at, next)) && seen.insert(next) {
                work.push(next);
            }
        }
    }
    seen
}

/// What `body`'s blocks do with the frame and the kept registers
/// (`useOrDefCSROrFI`).
struct Uses<'a> {
    body: &'a LirBody,
    kept: &'a BTreeSet<RegId>,
    frame_registers: BTreeSet<RegId>,
    framed: bool,
    /// Per block, whether it touches: asked again by every pass over the
    /// blocks, scanned once.
    touched: std::cell::RefCell<Vec<Option<bool>>>,
    /// How many blocks were scanned.
    scans: std::cell::Cell<usize>,
}

impl<'a> Uses<'a> {
    fn of(
        body: &'a LirBody,
        kept: &'a BTreeSet<RegId>,
        frame: Option<(RegId, RegId)>,
    ) -> Self {
        let regs = body.regs();
        let frame_registers =
            frame.iter().flat_map(|(pointer, stack)| [regs.root(*pointer), regs.root(*stack)]).collect();
        Self {
            body,
            kept,
            frame_registers,
            framed: frame.is_some(),
            touched: std::cell::RefCell::new(vec![None; body.blocks.len()]),
            scans: std::cell::Cell::new(0),
        }
    }

    fn insn_needs(
        &self,
        one: &Insn,
    ) -> bool {
        let regs = self.body.regs();
        (self.framed
            && (touches_frame(one) || any_named(regs, one, |register| self.frame_registers.contains(&register))))
            || any_named(regs, one, |register| self.kept.contains(&register))
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
        if let Some(known) = self.touched.borrow()[block.0 as usize] {
            return known;
        }
        self.scans.set(self.scans.get() + 1);
        let found = self.block(block).insns.iter().any(|one| self.insn_needs(one));
        self.touched.borrow_mut()[block.0 as usize] = Some(found);
        found
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
            name: Some("mov".into()),
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
        insn(at, Semantics { name: Some("je".into()), target: Some(target), ..Semantics::new(ir::Operation::Branch) })
    }

    fn ret(at: i64) -> Arc<Insn> {
        insn(at, Semantics { name: Some("ret".into()), ..Semantics::new(ir::Operation::Return) })
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

    /// The placement asks of each block twice (the walk, then the dirty
    /// set): the second ask rescanned its instructions, 4596 scans of d_faces
    /// where the blocks held half.
    #[test]
    fn test_a_block_asked_twice_is_scanned_once() {
        let blocks = vec![block(0, vec![touch(0, RegId::EBX), ret(2)], vec![])];
        let kept = BTreeSet::from([RegId::EBX]);
        let body = body(blocks);
        let uses = Uses::of(&body, &kept, None);
        assert!(uses.touches(BlockId(0), false) && uses.touches(BlockId(0), true));
        assert_eq!(uses.scans.get(), 1);
    }

    /// A register named only as the base or the index of a memory operand is
    /// a use: `any_named` must find both, as `named`'s set did.
    #[test]
    fn test_a_register_named_as_an_address_base_or_index_is_found() {
        let place = Loc::Mem(ir::Mem { through: RegId::ESI, index_through: RegId::EBX, ..ir::Mem::new(None, 4) });
        let one =
            insn(0, Semantics { name: Some("mov".into()), dests: vec![place], ..Semantics::new(ir::Operation::Move) });
        let regs = body(vec![block(0, vec![], vec![])]).regs();
        for register in [RegId::ESI, RegId::EBX] {
            assert!(any_named(regs, &one, |named| named == regs.root(register)), "{register:?} not found");
            assert!(named(regs, &one).contains(&regs.root(register)));
        }
        assert!(!any_named(regs, &one, |named| named == regs.root(RegId::EDI)));
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
        let wrap =
            wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false, false).expect("a wrap");
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
        assert!(wrapped(&body(blocks), &BTreeSet::from([RegId::EBX, RegId::ESI]), None, false, false).is_none());
    }

    /// A return the early exit shares with the late path is reached around the
    /// save: the late path takes its register back where it falls into the
    /// return, and the early exit jumps over that.
    #[test]
    fn test_a_return_the_early_exit_shares_with_the_late_path_is_split_on_the_late_edge() {
        let blocks = vec![
            block(0, vec![branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::ESI)], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        let wrap = wrapped(&body(blocks), &BTreeSet::from([RegId::ESI]), None, false, false).expect("a wrap");
        let esi = vec![Piece::Saved(RegId::ESI)];
        assert_eq!(wrap.opens, BTreeMap::from([(2, esi.clone())]));
        assert!(wrap.closes.is_empty());
        assert_eq!(wrap.closes_after, BTreeMap::from([(2, esi)]));
    }

    /// Under `size` the late edge's epilogue moves the early exit past the pops
    /// and a short branch grows (QCport m16 -Os +9 B): the save stays at the
    /// entry.
    #[test]
    fn test_a_split_restore_is_refused_under_size() {
        let blocks = vec![
            block(0, vec![branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::ESI)], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        assert!(wrapped(&body(blocks), &BTreeSet::from([RegId::ESI]), None, false, true).is_none());
    }

    /// The late path that does not fall into the return but is laid out away
    /// from it jumps there: its pops go before the jump.
    #[test]
    fn test_a_late_path_that_jumps_to_the_shared_return_pops_before_the_jump() {
        let jump =
            insn(25, Semantics { name: Some("jmp".into()), target: Some(4), ..Semantics::new(ir::Operation::Jump) });
        let blocks = vec![
            block(0, vec![branch(1, 5)], vec![5, 2]),
            block(2, vec![touch(20, RegId::ESI), jump], vec![4]),
            block(5, vec![touch(50, RegId::EAX)], vec![4]),
            block(4, vec![ret(40)], vec![]),
        ];
        let wrap = wrapped(&body(blocks), &BTreeSet::from([RegId::ESI]), None, false, false);
        assert!(wrap.is_some_and(|wrap| wrap.closes_after.contains_key(&2)));
    }

    /// A late path whose conditional branch takes it to the shared return
    /// pops on that edge: the early exit still skips the prologue (x_mergesort
    /// `msort` pushed four registers before its early `jl`).
    #[test]
    fn test_a_late_path_that_branches_to_the_shared_return_pops_on_the_taken_edge() {
        let jump =
            insn(42, Semantics { name: Some("jmp".into()), target: Some(3), ..Semantics::new(ir::Operation::Jump) });
        let blocks = vec![
            block(0, vec![branch(2, 3)], vec![3, 2]),
            block(2, vec![touch(20, RegId::ESI), branch(22, 3)], vec![3, 4]),
            block(4, vec![touch(40, RegId::ESI), jump], vec![3]),
            block(3, vec![ret(30)], vec![]),
        ];
        let wrap = wrapped(&body(blocks), &BTreeSet::from([RegId::ESI]), None, false, false).expect("a wrap");
        assert!(
            wrap.closes_after.contains_key(&4)
                && wrap.closes_taken.contains_key(&2)
                && !wrap.closes_after.contains_key(&2)
        );
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
        let wrap = wrapped(&body(blocks), &kept, None, false, false).expect("a wrap");
        assert_eq!(wrap.opens.keys().copied().collect::<Vec<_>>(), [1]);
        assert_eq!(wrap.closes.keys().copied().collect::<Vec<_>>(), [4]);
    }
}
