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
//! DEVIATE W2: no `TRY_TABLE`, `DELEGATE` or `RETHROW`, so no
//! `fixCallUnwindMismatches` and `fixCatchUnwindMismatches`: a call inside a
//! `try` that does not unwind to it is refused by name. Reason: QB's calls
//! unwind to one pad per procedure. Exit: the next EH stage.
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
use crate::dagisel::post_ids::{BLOCK, CATCH_ALL_LEGACY, END_BLOCK, END_FUNCTION, END_LOOP, END_TRY, LOOP, TRY};
use crate::exception_info::Exceptions;
use crate::mir::{Function, Inst, Operand, ValType};
use crate::sort_region::{Region, Regions};

/// `BlockType::Void`.
const VOID: i64 = 0x40;

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

    /// A call in the body of a `try` that does not unwind to its pad would be
    /// caught by it: the pin delegates it to the caller
    /// (`fixCallUnwindMismatches`), which is not translated.
    fn check_unwind_mismatches(&self) -> Result<(), String> {
        // (the try's marker, its pad, whether the walk is past its `catch`)
        let mut open: Vec<(u32, usize, bool)> = Vec::new();
        for (block, insts) in self.f.blocks.iter().enumerate() {
            let pad_of_block =
                self.f.succs[block].iter().copied().find(|&to| self.pads.get(to).copied().unwrap_or(false));
            // Only the last call of a block that ends in an `invoke` unwinds.
            let invoke = insts.iter().rposition(|inst| crate::branch::is_call(inst));
            for (at, inst) in insts.iter().enumerate() {
                let unwinds_to = pad_of_block.filter(|_| Some(at) == invoke);
                match inst.opcode {
                    TRY => open.push((inst.tag, self.try_to_pad[&inst.tag], false)),
                    CATCH_ALL_LEGACY => {
                        if let Some(one) = open.iter_mut().rev().find(|one| one.1 == block) {
                            one.2 = true;
                        }
                    }
                    END_TRY => {
                        let begin = self.end_to_begin[&inst.tag];
                        open.retain(|one| one.0 != begin);
                    }
                    _ if crate::branch::is_call(inst) => {
                        if let Some(&(_, pad, _)) = open.iter().rev().find(|one| !one.2) {
                            if unwinds_to != Some(pad) {
                                return Err(format!(
                                    "@{}: a call inside a try that does not unwind to it \
                                     (fixCallUnwindMismatches) is not translated; make it an invoke",
                                    self.f.name
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
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
                    END_LOOP => stack.push((self.block_of[&self.end_to_begin[&inst.tag]], inst.tag)),
                    _ if inst.is_terminator() => {
                        for operand in &mut self.f.blocks[b][i].operands {
                            if let Operand::Block(target) = *operand {
                                let depth = stack
                                    .iter()
                                    .rev()
                                    .position(|&(block, _)| block == target as usize)
                                    .expect("a branch destination is in scope");
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
                    if !matches!(inst.opcode, END_BLOCK | END_LOOP | END_TRY) {
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
        state.check_unwind_mismatches()?;
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
