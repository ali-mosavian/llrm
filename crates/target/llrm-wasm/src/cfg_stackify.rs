//! `WebAssemblyCFGStackify`: `BLOCK`, `LOOP` and their ends placed around the
//! sorted blocks, and each branch's target block replaced by the depth of the
//! scope it leaves or repeats.
//!
//! # How it works
//! A loop header gets a `LOOP` at its start and the block after the loop's last
//! block an `END_LOOP`. A block that is branched to from above gets an
//! `END_BLOCK` at its start and the `BLOCK` goes to the nearest common
//! dominator of its predecessors, as late as it can while enclosing nothing it
//! must not cross (`ScopeTops`). Depths count the scopes open at a branch.
//!
//! An EH pad gets a `TRY` at the nearest common dominator of the blocks that
//! unwind to it and an `END_TRY` after the last block of its exception
//! (`placeTryMarker`); a `BLOCK` made redundant by it is removed
//! (`removeUnnecessaryInstrs`).
//!
//! DEVIATE W2: no `TRY_TABLE` or `RETHROW`, and no `fixCatchUnwindMismatches`,
//! which acts only on typed `catch` pads (`CATCH_LEGACY`); the pads here are
//! `catch_all` (W18), which catch everything and never mismatch. Exit: W18.
//!
//! # See also
//! `WebAssemblyCFGStackify.cpp` (`placeBlockMarker` L295, `placeLoopMarker`
//! L446, `rewriteDepthImmediates` L2595, `fixEndsAtEndOfFunction` L2381), pin
//! d1106deb71cc.

use std::collections::HashMap;

use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::BlockId;

use crate::cfg::Cfg;
use crate::dagisel::post_ids::{
    BLOCK, CALL_INDIRECT, CATCH_ALL_LEGACY, DELEGATE, END_BLOCK, END_FUNCTION, END_LOOP, END_TRY, LOOP, RETHROW, THROW,
    TRY,
};
use crate::exception_info::Exceptions;
use crate::mir::{Function, Inst, Operand, ValType};
use crate::sort_region::{Region, Regions};

/// `BlockType::Void`.
const VOID: i64 = 0x40;

/// The block `delegate` names to rethrow to the caller (`getFakeCallerBlock`).
const CALLER: u32 = u32::MAX;

struct Stackify<'a> {
    f: &'a mut Function,
    cfg: Cfg,
    tree: DomTree,
    loops: LoopInfo<BlockId>,
    exceptions: Exceptions,
    /// `isEHPad`, kept from before the markers: a loop's end may come first in
    /// a pad.
    pads: Vec<bool>,
    /// The pad of each `TRY` marker, and the `TRY` of each pad (`TryToEHPad`,
    /// `EHPadToTry`).
    try_to_pad: HashMap<u32, usize>,
    pad_to_try: HashMap<usize, u32>,
    /// `ScopeTops`: the first block of the outermost scope that ends in each
    /// block.
    scope_tops: Vec<Option<usize>>,
    /// Marker tag to the marker at the other end of its scope.
    begin_to_end: HashMap<u32, u32>,
    end_to_begin: HashMap<u32, u32>,
    /// The block each marker is in; markers never move.
    block_of: HashMap<u32, usize>,
    next_tag: u32,
    /// `AppendixBB`: where an `END_LOOP` goes when the loop is last.
    appendix: Option<usize>,
}

impl Stackify<'_> {
    fn new_marker(
        &mut self,
        opcode: u16,
        operands: &[Operand],
        block: usize,
    ) -> Inst {
        self.next_tag += 1;
        self.block_of.insert(self.next_tag, block);
        Inst { tag: self.next_tag, ..Inst::new(opcode, operands) }
    }

    fn register_scope(
        &mut self,
        begin: u32,
        end: u32,
    ) {
        self.begin_to_end.insert(begin, end);
        self.end_to_begin.insert(end, begin);
    }

    /// `updateScopeTops`.
    fn update_scope_tops(
        &mut self,
        begin: usize,
        end: usize,
    ) {
        if self.scope_tops[end].is_none_or(|top| top > begin) {
            self.scope_tops[end] = Some(begin);
        }
    }

    /// `getAppendixBlock`.
    fn appendix_block(&mut self) -> usize {
        if let Some(block) = self.appendix {
            return block;
        }
        let block = self.f.blocks.len();
        self.f.blocks.push(Vec::new());
        self.f.succs.push(vec![block]);
        self.scope_tops.push(None);
        self.appendix = Some(block);
        block
    }

    /// `explicitlyBranchesTo`.
    fn branches_to(
        &self,
        pred: usize,
        block: usize,
    ) -> bool {
        crate::branch::terminators(&self.f.blocks[pred])
            .iter()
            .any(|inst| inst.operands.iter().any(|operand| *operand == Operand::Block(block as u32)))
    }

    fn place_loop_marker(
        &mut self,
        mbb: usize,
    ) {
        let Some(region) = self.loops.get_loop_for(BlockId(mbb as u32)) else { return };
        if region.get_header().0 as usize != mbb {
            return;
        }
        let bottom = Regions { loops: &self.loops, exceptions: &self.exceptions }.bottom(Region::Loop(mbb));
        let after_loop = if bottom + 1 == self.f.blocks.len() { self.appendix_block() } else { bottom + 1 };
        // The `LOOP` goes after any `END_LOOP` the block starts with.
        let at = self.f.blocks[mbb].iter().rposition(|inst| inst.opcode == END_LOOP).map_or(0, |last| last + 1);
        let begin = self.new_marker(LOOP, &[Operand::Imm(VOID)], mbb);
        let begin_tag = begin.tag;
        self.f.blocks[mbb].insert(at, begin);
        let end = self.new_marker(END_LOOP, &[], after_loop);
        let end_tag = end.tag;
        self.f.blocks[after_loop].insert(0, end);
        self.register_scope(begin_tag, end_tag);
        assert!(
            self.scope_tops[after_loop].is_none_or(|top| top < mbb),
            "with block sorting the outermost loop for a block should be first"
        );
        self.update_scope_tops(mbb, after_loop);
    }

    /// `WebAssembly::isChild`: an instruction whose first operand defines a
    /// stackified register.
    fn is_child(
        &self,
        inst: &Inst,
    ) -> bool {
        matches!(
            inst.operands.first(),
            Some(Operand::Def(reg)) if self.f.is_stackified(*reg)
        )
    }

    fn place_block_marker(
        &mut self,
        mbb: usize,
    ) {
        if mbb >= self.cfg_blocks() {
            return;
        }
        let mut header: Option<usize> = None;
        let mut is_branched_to = false;
        for pred in self.cfg.predecessors_of(mbb).collect::<Vec<_>>() {
            if pred < mbb {
                header = Some(match header {
                    Some(at) => {
                        let common = self
                            .tree
                            .find_nearest_common_dominator(&self.cfg, BlockId(at as u32), BlockId(pred as u32))
                            .expect("a forward dominator tree has a root");
                        common.0 as usize
                    }
                    None => pred,
                });
                if self.branches_to(pred, mbb) {
                    is_branched_to = true;
                }
            }
        }
        let Some(mut header) = header else { return };
        if !is_branched_to {
            return;
        }
        assert!(mbb != 0, "header blocks have no predecessors");
        // Do not start the block inside a scope that ends before `mbb`: begin
        // at that scope's top.
        let mut at = mbb - 1;
        while at != header {
            if let Some(top) = self.scope_tops[at] {
                if top > header {
                    at = top;
                    continue;
                }
                header = top;
                break;
            }
            at -= 1;
        }
        let block = &self.f.blocks[header];
        let mut after = vec![false; block.len()];
        for (i, inst) in block.iter().enumerate() {
            if inst.opcode == LOOP {
                let bottom = self.block_of[&self.begin_to_end[&inst.tag]] - 1;
                after[i] = mbb > bottom;
            }
            if inst.opcode == BLOCK || inst.opcode == TRY {
                after[i] = self.block_of[&self.begin_to_end[&inst.tag]] <= mbb;
            }
            if inst.is_terminator() {
                after[i] = true;
            }
        }
        let first_terminator = block.iter().position(Inst::is_terminator).unwrap_or(block.len());
        for i in (1..=first_terminator).rev() {
            if self.is_child(&block[i - 1]) {
                after[i - 1] = true;
            } else {
                break;
            }
        }
        let insert_at = after.iter().position(|&one| one).unwrap_or(block.len());
        let begin = self.new_marker(BLOCK, &[Operand::Imm(VOID)], header);
        let begin_tag = begin.tag;
        self.f.blocks[header].insert(insert_at, begin);
        // The end goes after any `END_LOOP` of a loop that began at or below
        // the header.
        let ends: Vec<bool> = self.f.blocks[mbb]
            .iter()
            .map(|inst| {
                matches!(inst.opcode, END_LOOP | END_TRY) && self.block_of[&self.end_to_begin[&inst.tag]] >= header
            })
            .collect();
        let end_at = ends.iter().rposition(|&one| one).map_or(0, |last| last + 1);
        let end = self.new_marker(END_BLOCK, &[], mbb);
        let end_tag = end.tag;
        self.f.blocks[mbb].insert(end_at, end);
        self.register_scope(begin_tag, end_tag);
        self.update_scope_tops(header, mbb);
    }

    /// `placeTryMarker`: the `TRY` of the exception headed by the pad `mbb`.
    fn place_try_marker(
        &mut self,
        mbb: usize,
    ) {
        let mut header: Option<usize> = None;
        for pred in self.cfg.predecessors_of(mbb).collect::<Vec<_>>() {
            if pred < mbb {
                header = Some(match header {
                    Some(at) => {
                        self.tree
                            .find_nearest_common_dominator(&self.cfg, BlockId(at as u32), BlockId(pred as u32))
                            .expect("a forward dominator tree has a root")
                            .0 as usize
                    }
                    None => pred,
                });
            }
        }
        let Some(mut header) = header else { return };
        let exception = self.exceptions.exception_for(mbb).expect("a pad heads an exception");
        let bottom = Regions { loops: &self.loops, exceptions: &self.exceptions }.bottom(Region::Exception(exception));
        if bottom + 1 == self.f.blocks.len() {
            self.appendix_block();
        }
        let cont = bottom + 1;
        // Do not start the try inside a scope that ends before `bottom`.
        let mut at = bottom;
        while at != header {
            if let Some(top) = self.scope_tops[at] {
                if top > header {
                    at = top;
                    continue;
                }
                header = top;
                break;
            }
            at -= 1;
        }
        let block = &self.f.blocks[header];
        let mut after = vec![false; block.len()];
        for (i, inst) in block.iter().enumerate() {
            // A loop that ends above the pad is nested in this try.
            if inst.opcode == LOOP {
                let loop_bottom = self.block_of[&self.begin_to_end[&inst.tag]] - 1;
                after[i] = mbb > loop_bottom;
            }
            if inst.opcode == BLOCK || inst.opcode == TRY || inst.is_terminator() {
                after[i] = true;
            }
        }
        // The call that unwinds here goes inside the try, with its operands.
        let first_terminator = block.iter().position(Inst::is_terminator).unwrap_or(block.len());
        let mut start = first_terminator;
        if self.f.succs[header].contains(&mbb) {
            if let Some(call) = block.iter().rposition(|inst| self.is_call(inst)) {
                after[call] = true;
                start = call;
            }
        }
        for i in (1..=start).rev() {
            if self.is_child(&block[i - 1]) {
                after[i - 1] = true;
            } else {
                break;
            }
        }
        let insert_at = after.iter().position(|&one| one).unwrap_or(block.len());
        let begin = self.new_marker(TRY, &[Operand::Imm(VOID)], header);
        let begin_tag = begin.tag;
        self.f.blocks[header].insert(insert_at, begin);
        // The end goes after any `END_LOOP` of a loop that began below the
        // header: that loop is in the catch part.
        let ends: Vec<bool> = self.f.blocks[cont]
            .iter()
            .map(|inst| inst.opcode == END_LOOP && self.block_of[&self.end_to_begin[&inst.tag]] > header)
            .collect();
        let end_at = ends.iter().rposition(|&one| one).map_or(0, |last| last + 1);
        let end = self.new_marker(END_TRY, &[], cont);
        let end_tag = end.tag;
        self.f.blocks[cont].insert(end_at, end);
        self.register_scope(begin_tag, end_tag);
        self.try_to_pad.insert(begin_tag, mbb);
        self.pad_to_try.insert(mbb, begin_tag);
        // Markers must not span the `catch` either.
        self.update_scope_tops(header, mbb);
        self.update_scope_tops(header, cont);
    }

    /// `MachineInstr::isCall`.
    fn is_call(
        &self,
        inst: &Inst,
    ) -> bool {
        crate::branch::is_call(inst)
    }

    /// `removeUnnecessaryInstrs`: the `br` that ends the block before a pad and
    /// goes where the `end_try` is, and a `block` that wraps a whole `try`.
    fn remove_unnecessary_instrs(&mut self) {
        for pad in 0..self.f.blocks.len() {
            if !self.pads.get(pad).copied().unwrap_or(false) || pad == 0 {
                continue;
            }
            // The continuation: past the end of the try (and of any try that
            // ends at a pad).
            let mut cont = pad;
            while self.pads.get(cont).copied().unwrap_or(false) {
                let Some(try_tag) = self.pad_to_try.get(&cont) else { break };
                cont = self.block_of[&self.begin_to_end[try_tag]];
            }
            let pred = pad - 1;
            let Some(last) = self.f.blocks[pred].iter().rposition(|inst| inst.is_terminator()) else { continue };
            let terminator = &self.f.blocks[pred][last];
            // Only an unconditional branch after the block's other terminators.
            if terminator.opcode == crate::dagisel::generated::ids::opcode::BR
                && terminator.operands == [Operand::Block(cont as u32)]
            {
                self.f.blocks[pred].remove(last);
            }
        }
        // `block ... try ... end_try ... end_block` with the same type.
        let mut gone: Vec<u32> = Vec::new();
        for b in 0..self.f.blocks.len() {
            for i in 0..self.f.blocks[b].len() {
                let inst = &self.f.blocks[b][i];
                if inst.opcode != TRY {
                    continue;
                }
                let (try_tag, end_tag) = (inst.tag, self.begin_to_end[&inst.tag]);
                let ret = inst.operands[0];
                let cont = self.block_of[&end_tag];
                let Some(end_at) = self.f.blocks[cont].iter().position(|one| one.tag == end_tag) else { continue };
                let (mut before, mut after) = (i, end_at + 1);
                while before > 0
                    && after < self.f.blocks[cont].len()
                    && self.f.blocks[b][before - 1].opcode == BLOCK
                    && self.f.blocks[cont][after].opcode == END_BLOCK
                    && self.f.blocks[b][before - 1].operands[0] == ret
                {
                    gone.push(self.f.blocks[b][before - 1].tag);
                    gone.push(self.f.blocks[cont][after].tag);
                    before -= 1;
                    after += 1;
                }
                let _ = try_tag;
            }
        }
        for tag in &gone {
            if let Some(end) = self.begin_to_end.remove(tag) {
                self.end_to_begin.remove(&end);
            }
        }
        for block in &mut self.f.blocks {
            block.retain(|inst| inst.tag == 0 || !gone.contains(&inst.tag));
        }
    }

    /// `WebAssembly::mayThrow` of instruction `at` of `block`.
    fn may_throw(
        &self,
        block: usize,
        at: usize,
    ) -> bool {
        let inst = &self.f.blocks[block][at];
        match inst.opcode {
            THROW | RETHROW | CALL_INDIRECT => return true,
            _ if !crate::branch::is_call(inst) => return false,
            _ => {}
        }
        match inst.operands.iter().find_map(|operand| if let Operand::Sym(at) = operand { Some(*at) } else { None }) {
            Some(symbol) => !self.f.nothrow.contains(&self.f.symbols[symbol as usize].0),
            None => true,
        }
    }

    fn has_pad_successor(
        &self,
        block: usize,
    ) -> bool {
        self.unwind_pad(block).is_some()
    }

    /// The first successor of `block` that is an EH pad.
    fn unwind_pad(
        &self,
        block: usize,
    ) -> Option<usize> {
        self.f.succs[block].iter().copied().find(|&to| self.pads.get(to).copied().unwrap_or(false))
    }

    /// The (block, position) of the instruction tagged `tag`.
    fn locate(
        &self,
        tag: u32,
    ) -> (usize, usize) {
        self.f
            .blocks
            .iter()
            .enumerate()
            .find_map(|(b, insts)| insts.iter().position(|inst| inst.tag == tag).map(|at| (b, at)))
            .expect("a tagged instruction")
    }

    /// A block with `insts` and `succs` that becomes block `at`: the blocks
    /// from `at` on are numbered one higher (`MF.insert`, with the numbers the
    /// layout gives).
    fn insert_block(
        &mut self,
        at: usize,
        insts: Vec<Inst>,
        succs: Vec<usize>,
    ) {
        let shift = |number: usize| if number >= at { number + 1 } else { number };
        for successors in &mut self.f.succs {
            for to in successors.iter_mut() {
                *to = shift(*to);
            }
        }
        for operand in self.f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
            if let Operand::Block(to) = operand {
                if *to != CALLER && *to as usize >= at {
                    *to += 1;
                }
            }
        }
        for home in self.block_of.values_mut() {
            *home = shift(*home);
        }
        for pad in self.try_to_pad.values_mut() {
            *pad = shift(*pad);
        }
        self.pad_to_try = self.pad_to_try.drain().map(|(pad, try_tag)| (shift(pad), try_tag)).collect();
        for top in self.scope_tops.iter_mut().flatten() {
            *top = shift(*top);
        }
        self.scope_tops.insert(at.min(self.scope_tops.len()), None);
        self.pads.insert(at.min(self.pads.len()), false);
        if let Some(appendix) = &mut self.appendix {
            *appendix = shift(*appendix);
        }
        let succs = succs.into_iter().map(shift).collect();
        for inst in &insts {
            if inst.tag != 0 {
                self.block_of.insert(inst.tag, at);
            }
        }
        self.f.blocks.insert(at, insts);
        self.f.succs.insert(at, succs);
    }

    /// The markers of `block` are in it.
    fn homed(
        &mut self,
        block: usize,
    ) {
        for inst in &self.f.blocks[block] {
            if inst.tag != 0 && self.block_of.contains_key(&inst.tag) {
                self.block_of.insert(inst.tag, block);
            }
        }
    }

    /// `unstackifyVRegsUsedInSplitBB`: a register `pre` defines and `post`
    /// reads can no longer live on the stack, and a `TEE` whose result then
    /// is not on the stack becomes two copies.
    fn unstackify_split(
        &mut self,
        pre: usize,
        post: usize,
    ) {
        let index = self.f.def_use();
        let used: Vec<u32> = self.f.blocks[post]
            .iter()
            .flat_map(|inst| inst.operands.iter())
            .filter_map(|operand| if let Operand::Use(reg) = operand { Some(*reg) } else { None })
            .filter(|&reg| index.def_count(reg) == 1 && index.def(reg).is_some_and(|(block, _)| block == pre))
            .collect();
        for reg in used {
            self.f.unstackify(reg);
        }
        let mut at = 0;
        while at < self.f.blocks[pre].len() {
            let inst = &self.f.blocks[pre][at];
            if crate::mir::is_tee(inst.opcode) {
                let (Operand::Def(tee), Operand::Def(reg), Operand::Use(def)) =
                    (inst.operands[0], inst.operands[1], inst.operands[2])
                else {
                    unreachable!("a tee defines two registers and reads one")
                };
                if !self.f.is_stackified(tee) {
                    self.f.unstackify(def);
                    let copy = self.f.vreg_type(def).copy();
                    let copies = [
                        Inst::new(copy, &[Operand::Def(tee), Operand::Use(def)]),
                        Inst::new(copy, &[Operand::Def(reg), Operand::Use(def)]),
                    ];
                    self.f.blocks[pre].splice(at..=at, copies);
                    at += 1;
                }
            }
            at += 1;
        }
    }

    /// `addNestedTryDelegate`: the instructions from `begin` to `end`
    /// (tags, inclusive) wrapped in a `try` whose `delegate` hands an
    /// exception to `dest`, a pad or [`CALLER`].
    fn add_nested_try_delegate(
        &mut self,
        begin: u32,
        end: u32,
        dest: u32,
    ) {
        let (begin_block, mut begin_at) = self.locate(begin);
        // The range's operands go inside the try too.
        while begin_at > 0 && self.is_child(&self.f.blocks[begin_block][begin_at - 1]) {
            begin_at -= 1;
        }
        let try_marker = self.new_marker(TRY, &[Operand::Imm(VOID)], begin_block);
        let try_tag = try_marker.tag;
        self.f.blocks[begin_block].insert(begin_at, try_marker);
        let (end_block, end_at) = self.locate(end);
        let split = end_at + 1;
        let delegate = {
            self.next_tag += 1;
            Inst { tag: self.next_tag, ..Inst::new(DELEGATE, &[Operand::Block(dest)]) }
        };
        let delegate_tag = delegate.tag;
        let delegate_succs = if dest == CALLER { Vec::new() } else { vec![dest as usize] };
        if split == self.f.blocks[end_block].len() {
            self.insert_block(end_block + 1, vec![delegate], delegate_succs);
            self.f.succs[end_block].push(end_block + 1);
        } else {
            let catch_after_split = self.pads[end_block]
                && self.f.blocks[end_block][split..].iter().any(|inst| inst.opcode == CATCH_ALL_LEGACY);
            let (pre, post) = if !catch_after_split {
                self.insert_block(end_block + 1, vec![delegate], delegate_succs);
                self.insert_block(end_block + 2, Vec::new(), Vec::new());
                let rest = self.f.blocks[end_block].split_off(split);
                self.f.blocks[end_block + 2] = rest;
                self.f.succs[end_block + 2] = std::mem::take(&mut self.f.succs[end_block]);
                self.homed(end_block + 2);
                (end_block, end_block + 2)
            } else {
                self.insert_block(end_block, Vec::new(), Vec::new());
                self.insert_block(end_block + 1, vec![delegate], delegate_succs);
                let head: Vec<Inst> = self.f.blocks[end_block + 2].drain(..split).collect();
                self.f.blocks[end_block] = head;
                self.homed(end_block);
                (end_block, end_block + 2)
            };
            self.unstackify_split(pre, post);
            self.f.succs[pre] = vec![pre + 1, post];
        }
        let delegate_block = self.locate(delegate_tag).0;
        self.block_of.insert(delegate_tag, delegate_block);
        self.register_scope(try_tag, delegate_tag);
    }

    /// `fixCallUnwindMismatches` (legacy EH): a call that may throw inside a
    /// `try` that is not its pad's is wrapped in a `try` / `delegate` to the
    /// pad, or to the caller.
    fn fix_call_unwind_mismatches(&mut self) -> bool {
        // (destination, ranges of (first, last) tags), in the order found.
        let mut ranges: Vec<(u32, Vec<(u32, u32)>)> = Vec::new();
        let note = |dest: u32, range: (u32, u32), ranges: &mut Vec<(u32, Vec<(u32, u32)>)>| match ranges
            .iter_mut()
            .find(|(one, _)| *one == dest)
        {
            Some((_, found)) => found.push(range),
            None => ranges.push((dest, vec![range])),
        };
        let tag_of = |state: &mut Self, block: usize, at: usize| {
            if state.f.blocks[block][at].tag == 0 {
                state.next_tag += 1;
                state.f.blocks[block][at].tag = state.next_tag;
            }
            state.f.blocks[block][at].tag
        };
        // The pads and delegate targets that catch what is thrown here,
        // innermost last.
        let mut stack: Vec<u32> = Vec::new();
        let walk = |state: &Self, stack: &mut Vec<u32>, block: usize, at: usize| match state.f.blocks[block][at].opcode
        {
            TRY => {
                stack.pop();
            }
            DELEGATE => {
                let Operand::Block(dest) = state.f.blocks[block][at].operands[0] else {
                    unreachable!("a delegate names a block")
                };
                stack.push(dest);
            }
            CATCH_ALL_LEGACY => stack.push(block as u32),
            _ => {}
        };
        for block in (0..self.f.blocks.len()).rev() {
            let mut seen = false;
            for at in (0..self.f.blocks[block].len()).rev() {
                walk(self, &mut stack, block, at);
                if seen || !self.has_pad_successor(block) || !self.may_throw(block, at) {
                    continue;
                }
                seen = true;
                let dest = self.unwind_pad(block).expect("an unwind pad") as u32;
                if stack.last() == Some(&dest) {
                    continue;
                }
                let tag = tag_of(self, block, at);
                note(dest, (tag, tag), &mut ranges);
            }
        }
        assert!(stack.is_empty(), "the try markers should be balanced");
        // Calls that unwind to the caller, which the `try`s around them would
        // catch.
        let mut open: Option<(u32, u32)> = None;
        for block in (0..self.f.blocks.len()).rev() {
            let mut seen = false;
            for at in (0..self.f.blocks[block].len()).rev() {
                let may = self.may_throw(block, at);
                let opcode = self.f.blocks[block][at].opcode;
                if self.has_pad_successor(block) && may && !seen {
                    seen = true;
                } else if open.is_some() && matches!(opcode, BLOCK | END_BLOCK | LOOP | END_LOOP | TRY | END_TRY) {
                    note(CALLER, open.take().expect("a range"), &mut ranges);
                } else if stack.is_empty() || stack.last() == Some(&CALLER) || !may {
                } else {
                    let tag = tag_of(self, block, at);
                    open = Some(match open {
                        None => (tag, tag),
                        Some((_, last)) => (tag, last),
                    });
                }
                walk(self, &mut stack, block, at);
            }
            if let Some(range) = open.take() {
                note(CALLER, range, &mut ranges);
            }
        }
        assert!(stack.is_empty(), "the try markers should be balanced");
        if ranges.is_empty() {
            return false;
        }
        for (dest, found) in ranges {
            for (begin, end) in found {
                if dest != CALLER {
                    let block = self.locate(begin).0;
                    if let Some(pad) = self.unwind_pad(block) {
                        self.f.succs[block].retain(|&to| to != pad);
                    }
                }
                self.add_nested_try_delegate(begin, end, dest);
            }
        }
        true
    }

    /// The blocks the analyses were made over, before an appendix block.
    fn cfg_blocks(&self) -> usize {
        self.f.blocks.len() - usize::from(self.appendix.is_some())
    }

    /// `rewriteDepthImmediates`.
    fn rewrite_depths(&mut self) {
        let mut stack: Vec<(usize, u32)> = Vec::new();
        for b in (0..self.f.blocks.len()).rev() {
            for i in (0..self.f.blocks[b].len()).rev() {
                let inst = &self.f.blocks[b][i];
                match inst.opcode {
                    BLOCK | LOOP | TRY => {
                        stack.pop();
                    }
                    END_BLOCK | END_TRY => stack.push((b, inst.tag)),
                    DELEGATE => {
                        let tag = inst.tag;
                        let Operand::Block(dest) = inst.operands[0] else { unreachable!("a delegate names a block") };
                        let depth = if dest == CALLER {
                            stack.len()
                        } else if self.pads.get(dest as usize).copied().unwrap_or(false) {
                            let end = self.begin_to_end[&self.pad_to_try[&(dest as usize)]];
                            stack
                                .iter()
                                .rev()
                                .position(|&(block, one)| one == end && block == self.block_of[&end])
                                .expect("a delegate's try is in scope")
                        } else {
                            stack
                                .iter()
                                .rev()
                                .position(|&(block, _)| block == dest as usize)
                                .expect("a delegate's destination")
                        };
                        self.f.blocks[b][i].operands[0] = Operand::Imm(depth as i64);
                        stack.push((b, tag));
                    }
                    END_LOOP => stack.push((self.block_of[&self.end_to_begin[&inst.tag]], inst.tag)),
                    _ if inst.is_terminator() => {
                        for operand in &mut self.f.blocks[b][i].operands {
                            if let Operand::Block(target) = *operand {
                                let depth =
                                    stack.iter().rev().position(|&(block, _)| block == target as usize).unwrap_or_else(
                                        || panic!("a branch destination is in scope: bb{target} from bb{b}"),
                                    );
                                *operand = Operand::Imm(depth as i64);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        assert!(stack.is_empty(), "control flow should be balanced");
    }

    /// `fixEndsAtEndOfFunction`: scopes that end the function carry its result
    /// type. A `try` also passes it to the scopes that end before its `catch`.
    fn fix_ends(&mut self) {
        let Some(&result) = self.f.results.first() else { return };
        assert!(self.f.results.len() == 1, "multivalue results are not translated");
        let code = match result {
            ValType::I32 => 0x7f,
            ValType::I64 => 0x7e,
            ValType::F32 => 0x7d,
            ValType::F64 => 0x7c,
        };
        let last = self.f.blocks.len() - 1;
        let mut work = vec![(last, self.f.blocks[last].len())];
        while let Some((mut b, mut i)) = work.pop() {
            'walk: loop {
                while i > 0 {
                    i -= 1;
                    let inst = &self.f.blocks[b][i];
                    if !matches!(inst.opcode, END_BLOCK | END_LOOP | END_TRY | DELEGATE) {
                        break 'walk;
                    }
                    let begin = self.end_to_begin[&inst.tag];
                    if inst.opcode == END_TRY {
                        let pad = self.try_to_pad[&begin];
                        if let Some(catch) = self.f.blocks[pad].iter().position(|one| one.opcode == CATCH_ALL_LEGACY) {
                            if catch > 0 {
                                work.push((pad, catch));
                            }
                        }
                    }
                    let home = self.block_of[&begin];
                    let marker =
                        self.f.blocks[home].iter_mut().find(|marker| marker.tag == begin).expect("the begin marker");
                    marker.operands[0] = Operand::Imm(code);
                }
                if b == 0 {
                    break;
                }
                b -= 1;
                i = self.f.blocks[b].len();
            }
        }
    }
}

/// Whether anything changed (always); an error names what is not translated.
pub fn cfg_stackify(f: &mut Function) -> Result<bool, String> {
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    let exceptions = Exceptions::of(f, &cfg, &tree);
    let pads: Vec<bool> = (0..f.blocks.len()).map(|block| f.is_eh_pad(block)).collect();
    let n = f.blocks.len();
    let mut state = Stackify {
        f,
        cfg,
        tree,
        loops,
        exceptions,
        pads,
        try_to_pad: HashMap::new(),
        pad_to_try: HashMap::new(),
        scope_tops: vec![None; n + 1],
        begin_to_end: HashMap::new(),
        end_to_begin: HashMap::new(),
        block_of: HashMap::new(),
        next_tag: 0,
        appendix: None,
    };
    for b in 0..n {
        state.place_loop_marker(b);
    }
    for b in 0..state.f.blocks.len() {
        if state.pads.get(b).copied().unwrap_or(false) {
            if state.f.personality {
                state.place_try_marker(b);
            }
        } else {
            state.place_block_marker(b);
        }
    }
    if state.f.personality {
        state.fix_call_unwind_mismatches();
        state.remove_unnecessary_instrs();
    }
    state.rewrite_depths();
    state.fix_ends();
    let last = state.f.blocks.len() - 1;
    state.f.blocks[last].push(Inst::new(END_FUNCTION, &[]));
    for marker in state.f.blocks.iter_mut().flatten() {
        marker.tag = 0;
    }
    Ok(true)
}
