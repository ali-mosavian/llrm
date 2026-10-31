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
//! DEVIATE W2: no `TRY`, `TRY_TABLE`, `DELEGATE` or `RETHROW`, so none of
//! `placeTryMarker`, `fixCallUnwindMismatches`, `fixCatchUnwindMismatches` and
//! `removeUnnecessaryInstrs`. Reason: no exception handling in QB. Exit: with
//! wasm exception handling.
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
use crate::dagisel::post_ids::{BLOCK, END_BLOCK, END_FUNCTION, END_LOOP, LOOP};
use crate::mir::{Function, Inst, Operand, ValType};

/// `BlockType::Void`.
const VOID: i64 = 0x40;

struct Stackify<'a> {
    f: &'a mut Function,
    cfg: Cfg,
    tree: DomTree,
    loops: LoopInfo<BlockId>,
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
        let bottom = region.get_blocks().iter().map(|block| block.0 as usize).max().expect("a loop has blocks");
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
            if inst.opcode == BLOCK {
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
            .map(|inst| inst.opcode == END_LOOP && self.block_of[&self.end_to_begin[&inst.tag]] >= header)
            .collect();
        let end_at = ends.iter().rposition(|&one| one).map_or(0, |last| last + 1);
        let end = self.new_marker(END_BLOCK, &[], mbb);
        let end_tag = end.tag;
        self.f.blocks[mbb].insert(end_at, end);
        self.register_scope(begin_tag, end_tag);
        self.update_scope_tops(header, mbb);
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
                    BLOCK | LOOP => {
                        stack.pop();
                    }
                    END_BLOCK => stack.push((b, inst.tag)),
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
    /// type.
    fn fix_ends(&mut self) {
        let Some(&result) = self.f.results.first() else { return };
        assert!(self.f.results.len() == 1, "multivalue results are not translated");
        let code = match result {
            ValType::I32 => 0x7f,
            ValType::I64 => 0x7e,
            ValType::F32 => 0x7d,
            ValType::F64 => 0x7c,
        };
        let mut b = self.f.blocks.len() - 1;
        let mut i = self.f.blocks[b].len();
        loop {
            while i > 0 {
                i -= 1;
                let inst = &self.f.blocks[b][i];
                if inst.opcode != END_BLOCK && inst.opcode != END_LOOP {
                    return;
                }
                let begin = self.end_to_begin[&inst.tag];
                let home = self.block_of[&begin];
                let marker =
                    self.f.blocks[home].iter_mut().find(|marker| marker.tag == begin).expect("the begin marker");
                marker.operands[0] = Operand::Imm(code);
            }
            if b == 0 {
                return;
            }
            b -= 1;
            i = self.f.blocks[b].len();
        }
    }
}

/// Whether anything changed (always).
pub fn cfg_stackify(f: &mut Function) -> bool {
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    let n = f.blocks.len();
    let mut state = Stackify {
        f,
        cfg,
        tree,
        loops,
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
        state.place_block_marker(b);
    }
    state.rewrite_depths();
    state.fix_ends();
    let last = state.f.blocks.len() - 1;
    state.f.blocks[last].push(Inst::new(END_FUNCTION, &[]));
    for marker in state.f.blocks.iter_mut().flatten() {
        marker.tag = 0;
    }
    true
}
