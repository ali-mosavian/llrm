//! Takes LIR out of SSA: each phi becomes parallel copies on its incoming
//! edges.
//!
//! # How it works
//! For each phi `r = phi [p1: v1, p2: v2]` the pass puts `r := v1` at the end
//! of `p1` and `r := v2` at the end of `p2`, before the terminator. The copies
//! of one edge carry one group number, so [`crate::backend::parcopy`] later
//! orders them as a single simultaneous assignment. A phi with an incoming edge
//! from outside the body is left alone.
//!
//! A copy on a branch's edge instead of in its predecessor is made only when
//! it costs the target at least a tenth less (`EdgePrices::cheaper_on_edge`:
//! the edge's chance times its copies and the `jmp`, against the copies every
//! time), and never in a build for size: the odds are a guess, and an edge
//! that wins by less costs a block and a jump the clocks do not show.
//!
//! Before copying, three shortcuts avoid a copy altogether. A one-input phi
//! renames its result to its input. A loop-exit one-input phi is the exception:
//! it becomes a copy opening the exit, so a call after the loop does not pin
//! the loop's register to a call-preserved one. On a critical edge, where every
//! incoming value is defined once in its own predecessor, is read by nothing
//! but this phi (or only past the loop the edge closes), and the phi's result
//! is not live after that definition, the definitions are renamed to define the
//! result outright. That is what gcc's out-of-SSA does with a phi argument that
//! does not interfere with the result.
//!
//! Otherwise a predecessor with several successors makes the edge critical. A
//! copy there may run in the predecessor only if no other path reads either end
//! of it (`_observed`: whether a block that reads it is reachable from the
//! predecessor's other successors without entering the successor, searched
//! from both ends so that the smaller region is walked) and, where block odds
//! are known, the edge is not cheaper
//! (`EdgePrices::cheaper_on_edge`: the edge's chance times copies plus a `jmp`
//! against the copies run every time). Otherwise the copy goes in a new block
//! on the edge, which holds its copies and a jump to the successor:
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   p["<b>P</b>"]:::mid
//!   q["<b>Q</b>"]:::mid
//!   s["<b>S</b><br/><small>r = phi[P: v]</small>"]:::mid
//!   p --> q
//!   p --> s
//! ```
//!
//! *Before: edge P to S is critical (r = phi[P: v])*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   p["<b>P</b>"]:::mid
//!   q["<b>Q</b>"]:::mid
//!   e["<b>E</b><br/><small>r := v #59; jmp S</small>"]:::src
//!   s["<b>S</b>"]:::mid
//!   p --> q
//!   p --> e
//!   e --> s
//! ```
//!
//! *After: the copy goes in a new block E on the edge*
//!
//! The new block's label is above every physical offset (`(entry + 1) << 32`),
//! in a namespace of its own per body entry.
//!
//! `LLRM_PHIELIM_LLVM=1` lowers the phis that are not x87 by the translation of
//! LLVM's `PHIElimination` ([`crate::phielimination`]) first, and this pass the
//! rest. Off by default: it leaves a cycle of loop-carried values to be broken
//! by copies through a register, where the parallel copy here breaks it with an
//! `xchg`, and the i64 Fibonacci loop
//! (`test_an_i64_is_a_dword_pair_through_phis_calls_and_returns`)
//! spills a dword with it.
//!
//! `unsplit` undoes a split whose copies coalescing removed: a block of only a
//! jump is bypassed, so the jump is not taken on every pass. The peephole calls
//! it first.
//!
//! # Example
//! A loop carrying three values has three phis at its head:
//!
//! Before:
//!
//! ```lir
//! block 0x0000
//!   0x0000  v2 := mov 0x1  ; inserted
//!   0x0000  v3 := mov 0x0  ; inserted
//!   0x0000  jmp   ; inserted
//! block 0x0001
//!   v5 := phi 0x0000:v2 0x0009:v4
//!   v6 := phi 0x0000:v3 0x0009:v5
//!   v8 := phi 0x0000:v1 0x0009:v7
//!   ; ...
//! block 0x0009
//!   0x0009  v4 := add v6, v5  ; inserted
//!   0x000a  jmp   ; inserted
//! ```
//!
//! After:
//!
//! ```lir
//! block 0x0000
//!   0x0000  v2 := mov 0x1  ; inserted
//!   0x0000  v3 := mov 0x0  ; inserted
//!   0x0000  v5 := mov v2  ; inserted; parallel-copy 1
//!   0x0000  v6 := mov v3  ; inserted; parallel-copy 1
//!   0x0000  v8 := mov v1  ; inserted; parallel-copy 1
//!   0x0000  jmp   ; inserted
//! block 0x0001
//!   ; ...
//! block 0x0009
//!   0x0009  v4 := add v6, v5  ; inserted
//!   0x000a  v5 := mov v4  ; inserted; parallel-copy 2
//!   0x000a  v6 := mov v5  ; inserted; parallel-copy 2
//!   0x000a  v8 := mov v7  ; inserted; parallel-copy 2
//!   0x000a  jmp   ; inserted
//! ```
//!
//! Each phi became one copy at the end of each predecessor, one group per edge.
//! The copies of group 2 read values (`v5`) that others write, which
//! [`crate::backend::parcopy`] orders after allocation.
//!
//! From `ISEL_DUMP=1` of a C Fibonacci loop (`t = a + b; a = b; b = t;`)
//! compiled with `llrm-c -O2 -m16`: the stage before `PhiElimination`, and
//! `PhiElimination`.
//!
//! # Runs
//! A machine phase after `ssaspill` (spiller route only) and before
//! `machinesched`, in every machine (`flow::machine_with`); `EdgePrices` come
//! from the CPU profile, and with none a copy stays in its predecessor. Also
//! `placed_on_edges`, which [`crate::backend::floatalloc`] uses to put its
//! selected transfers on exact edges.
//!
//! # Keeps
//! Result is out of SSA except for phis it refused. Value identities are
//! unified where a phi was trivial, so the body's widths and operands are
//! renamed throughout.
//!
//! # See also
//! - LLVM `PHIElimination`:
//!   <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/PHIElimination.cpp>

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

/// What the target charges for a copy and for the `jmp` ending an edge's block,
/// in clocks.
#[derive(Clone, Copy, Debug)]
pub struct EdgePrices {
    pub copy: f64,
    pub jump: f64,
}

impl EdgePrices {
    /// None for a build for size: a copy on its edge is a block of its own and
    /// a jump, and the copy twice where the edge's other end is reached
    /// another way, so the copies stay in their predecessor.
    pub fn of(cpu: &crate::backend::cpu::Profile) -> Option<Self> {
        if cpu.size {
            return None;
        }
        Some(Self { copy: cpu.cost("mov_rr").ok()? as f64, jump: cpu.cost("jmp_short").ok()? as f64 })
    }

    /// Whether `copies` copies cost less in a block of their own on an edge
    /// taken `chance` of the times its predecessor runs (the copies and the
    /// jump that ends it) than in the predecessor (the copies, every time),
    /// by a tenth at least: the odds are a guess, and an edge that wins by
    /// less costs a block and a jump for nothing the clocks show.
    pub fn cheaper_on_edge(
        &self,
        chance: f64,
        copies: usize,
    ) -> bool {
        let copies = copies as f64 * self.copy;
        chance * (copies + self.jump) < copies * EDGE_MARGIN
    }
}

/// What an edge copy must cost, of the predecessor's, to be made there.
const EDGE_MARGIN: f64 = 0.9;

/// The PHI elimination pass (`phielimination::lowered`).
#[derive(Default)]
pub struct PhiElimination;

impl LIRTransform for PhiElimination {
    fn class_name(&self) -> &'static str {
        "PhiElimination"
    }

    fn name(&self) -> &str {
        "phielim"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        crate::phielimination::lowered(&body)
    }
}

/// What `_split_edges` places on one edge: phi `(result, value)` pairs, or
/// transfers already selected (`selected=True`).
#[derive(Clone, Debug)]
pub enum Split {
    Pairs(Vec<(u32, u32)>),
    Selected(Vec<Arc<Insn>>),
}

pub(crate) fn move_of(
    into: Held,
    out_of: Held,
) -> Semantics {
    Semantics {
        name: Some("mov".into()),
        dests: vec![Loc::Held(into)],
        sources: vec![Loc::Held(out_of)],
        ..Semantics::new(Operation::Move)
    }
}

fn jump_to(target: i64) -> Semantics {
    Semantics { name: Some("jmp".into()), target: Some(target), ..Semantics::new(Operation::Jump) }
}

/// Past this many blocks the matrix is not built (N^2 bits) and every query
/// walks.
const MAX_REACHING: usize = 8192;

thread_local! {
    static WALKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks this thread's `_observed` walks have visited, for a test
/// that a value nothing reads asks no walk.
pub fn walked() -> usize {
    WALKED.with(std::cell::Cell::get)
}

/// Which blocks reach which, and where each value is read: a walk for every
/// (value, edge) asked was linear in the blocks, so quadratic in a function of
/// N diamonds (`branches` 1024: 2 Ginstr of 4 runs).
pub struct Reaches {
    index: IndexMap<i64, usize>,
    /// Per block, the blocks it reaches, itself included.
    reach: Vec<Vec<u64>>,
    /// Per value, the blocks that read it, or a phi on an edge out of them
    /// does.
    readers: IndexMap<u32, Vec<usize>>,
    /// A successor with no block: the walk's answer there is "read".
    open: bool,
    /// Each block's successors and predecessors, by place; and marks for the
    /// searches, made once.
    next: Vec<Vec<usize>>,
    before: Vec<Vec<usize>>,
    marks: std::cell::RefCell<Marks>,
}

/// Which blocks a search from each end has been to: a mark is the search's
/// number, so no search clears what an earlier one left.
#[derive(Default)]
struct Marks {
    search: u32,
    forward: Vec<u32>,
    backward: Vec<u32>,
}

impl Reaches {
    pub fn new(
        body: &LirBody,
        at_of: &IndexMap<i64, &Block>,
    ) -> Self {
        let index: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
        let count = index.len();
        let words = if count > MAX_REACHING { 0 } else { count.div_ceil(64) };
        let open =
            count > MAX_REACHING || body.blocks.iter().any(|block| block.succ.iter().any(|at| !at_of.contains_key(at)));
        let successors: Vec<Vec<usize>> = body
            .blocks
            .iter()
            .map(|block| block.succ.iter().filter_map(|at| index.get(at).copied()).collect())
            .collect();
        let mut before: Vec<Vec<usize>> = vec![Vec::new(); count];
        for (at, list) in successors.iter().enumerate() {
            for &to in list {
                before[to].push(at);
            }
        }
        let marks = std::cell::RefCell::new(Marks { search: 0, forward: vec![0; count], backward: vec![0; count] });
        if open {
            return Self {
                index,
                reach: Vec::new(),
                readers: IndexMap::default(),
                open,
                next: successors,
                before,
                marks,
            };
        }
        let mut reach = vec![vec![0_u64; words]; count];
        for (at, row) in reach.iter_mut().enumerate() {
            row[at / 64] |= 1 << (at % 64);
        }
        // Later blocks first settles an acyclic function in one sweep; loops
        // take more.
        let mut changed = true;
        while changed {
            changed = false;
            for at in (0..count).rev() {
                for &next in &successors[at] {
                    if next == at {
                        continue;
                    }
                    let (from, to) = (reach[next].clone(), &mut reach[at]);
                    for (word, other) in to.iter_mut().zip(&from) {
                        if *word | other != *word {
                            *word |= other;
                            changed = true;
                        }
                    }
                }
            }
        }
        let mut readers: IndexMap<u32, Vec<usize>> = IndexMap::default();
        for (at, block) in body.blocks.iter().enumerate() {
            for one in &block.insns {
                for &value in one.uses.iter().chain(one.requires.iter().map(|(held, _)| &held.value)) {
                    readers.entry(value).or_default().push(at);
                }
            }
            for phi in &block.phis {
                for &(from, value) in &phi.incoming {
                    if let Some(&pred) = index.get(&from) {
                        readers.entry(value).or_default().push(pred);
                    }
                }
            }
        }
        Self { index, reach, readers, open, next: successors, before, marks }
    }

    /// Whether no block reachable from `starts` reads `value`.
    /// Whether a block that reads `value` can be reached from `starts` without
    /// entering `barrier`, searched from both ends a block at a time: the
    /// answer is found, or shown not to be, in the smaller of the two regions.
    /// `None` where a successor has no block (the caller walks).
    fn read_beyond(
        &self,
        starts: &[i64],
        barrier: i64,
        value: u32,
    ) -> Option<bool> {
        if self.open {
            return None;
        }
        let barrier = self.index.get(&barrier).copied();
        let begin: Vec<usize> = starts.iter().filter_map(|at| self.index.get(at).copied()).collect();
        let Some(readers) = self.readers.get(&value) else { return Some(false) };
        let mut marks = self.marks.borrow_mut();
        marks.search += 1;
        let search = marks.search;
        let (mut forward, mut backward): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for &at in &begin {
            if Some(at) != barrier && marks.forward[at] != search {
                marks.forward[at] = search;
                forward.push(at);
            }
        }
        for &at in readers {
            if Some(at) != barrier && marks.backward[at] != search {
                marks.backward[at] = search;
                backward.push(at);
            }
        }
        // Met at a block either end has reached.
        if forward.iter().any(|&at| marks.backward[at] == search) {
            return Some(true);
        }
        let (mut ahead, mut behind) = (0, 0);
        while ahead < forward.len() && behind < backward.len() {
            // One block from the end whose frontier is the smaller.
            if forward.len() - ahead <= backward.len() - behind {
                let at = forward[ahead];
                ahead += 1;
                for &to in &self.next[at] {
                    if Some(to) == barrier || marks.forward[to] == search {
                        continue;
                    }
                    marks.forward[to] = search;
                    if marks.backward[to] == search {
                        return Some(true);
                    }
                    forward.push(to);
                }
            } else {
                let at = backward[behind];
                behind += 1;
                for &from in &self.before[at] {
                    if Some(from) == barrier || marks.backward[from] == search {
                        continue;
                    }
                    marks.backward[from] = search;
                    if marks.forward[from] == search {
                        return Some(true);
                    }
                    backward.push(from);
                }
            }
        }
        Some(false)
    }

    fn unread_from(
        &self,
        starts: &[i64],
        value: u32,
    ) -> bool {
        if self.open {
            return false;
        }
        let Some(readers) = self.readers.get(&value) else { return true };
        starts
            .iter()
            .all(
                |at| self.index
                    .get(at)
                    .is_some_and(
                        |&start| readers.iter().all(|&reader| self.reach[start][reader / 64] >> (reader % 64) & 1 == 0),
                    ),
            )
    }
}

/// Whether `value` can be read after leaving `where` other than into `into`.
///
/// `into` defines it, so a path through `into` reads a new one.
pub fn _observed(
    reaches: &Reaches,
    at_of: &IndexMap<i64, &Block>,
    where_: i64,
    into: i64,
    value: u32,
) -> bool {
    let pending: Vec<i64> = at_of[&where_].succ.iter().copied().filter(|&at| at != into).collect();
    // A phi reads on the incoming edge, before any instruction in its block.
    if pending.iter().any(|at| {
        at_of.get(at).is_some_and(|block| block.phis.iter().any(|phi| phi.incoming.contains(&(where_, value))))
    }) {
        return true;
    }
    // Nothing a successor reaches reads it, `into` or not: no walk.
    if reaches.unread_from(&pending, value) {
        return false;
    }
    if let Some(answer) = reaches.read_beyond(&pending, into, value) {
        if llrm_support::env_flag!("LLRM_CHECK_OBSERVED") {
            assert_eq!(
                answer,
                _observed_walk(at_of, &pending, where_, into, value),
                "LLRM_CHECK_OBSERVED: b{where_} to b{into}, v{value}"
            );
        }
        return answer;
    }
    _observed_walk(at_of, &pending, where_, into, value)
}

/// The blocks reachable from `pending` without entering `into`, walked for a
/// read of `value`.
fn _observed_walk(
    at_of: &IndexMap<i64, &Block>,
    pending: &[i64],
    _where: i64,
    into: i64,
    value: u32,
) -> bool {
    let mut pending: Vec<i64> = pending.to_vec();
    let mut seen: BTreeSet<i64> = pending.iter().copied().collect();
    while let Some(at) = pending.pop() {
        WALKED.with(|walked| walked.set(walked.get() + 1));
        let Some(block) = at_of.get(&at) else {
            return true;
        };
        if block
            .insns
            .iter()
            .any(|one| one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value))
        {
            return true;
        }
        for &successor in &block.succ {
            let follower = at_of.get(&successor);
            if follower
                .is_some_and(|follower| follower.phis.iter().any(|phi| phi.incoming.contains(&(block.at, value))))
            {
                return true;
            }
            if successor != into && !seen.contains(&successor) {
                seen.insert(successor);
                pending.push(successor);
            }
        }
    }
    false
}

/// How many times each value is read, phi edges included.
fn _read_once(body: &LirBody) -> IndexMap<u32, i64> {
    let mut out: IndexMap<u32, i64> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for &value in &one.uses {
                *out.entry(value).or_insert(0) += 1;
            }
        }
        for phi in &block.phis {
            for &(_where, value) in &phi.incoming {
                *out.entry(value).or_insert(0) += 1;
            }
        }
    }
    out
}

/// The move of an immediate into `value` where it is `block`'s one definition
/// of it and nothing reads it there: a constant a phi takes from its
/// predecessor.
pub(crate) fn _constant_made_in(
    block: &LirBlock,
    value: u32,
) -> Option<Semantics> {
    let mut defining = block.insns.iter().filter(|one| one.defines.contains(&value));
    let (Some(one), None) = (defining.next(), defining.next()) else { return None };
    let what = one.what.as_ref()?;
    let ([Loc::Held(into)], [Loc::Imm(_)]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    (what.op == Operation::Move && into.value == value && one.uses.is_empty()).then(|| what.clone())
}

/// One instruction defining the phi's result where it defined its own.
fn _renamed(
    one: &Arc<Insn>,
    rename: &IndexMap<u32, u32>,
) -> Result<Arc<Insn>, String> {
    if rename.is_empty() {
        return Ok(Arc::clone(one));
    }
    one.try_renamed(&|value| _name(value, rename))
}

/// The final identity after chained trivial phis are unified.
fn _name(
    mut value: u32,
    rename: &IndexMap<u32, u32>,
) -> Result<u32, String> {
    let mut seen = BTreeSet::new();
    while let Some(&next) = rename.get(&value) {
        if next == value {
            break;
        }
        if seen.contains(&value) {
            return Err("cyclic phi rename".to_owned());
        }
        seen.insert(value);
        value = next;
    }
    Ok(value)
}

/// A surviving phi with every trivial-phi identity made final.
fn _renamed_phi(
    phi: &Phi,
    rename: &IndexMap<u32, u32>,
) -> Result<Phi, String> {
    Ok(Phi {
        result: _name(phi.result, rename)?,
        incoming: phi
            .incoming
            .iter()
            .map(|&(where_, value)| Ok((where_, _name(value, rename)?)))
            .collect::<Result<_, String>>()?,
    })
}

pub(crate) fn _widths(body: &LirBody) -> IndexMap<u32, u32> {
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for op in &block.insns {
            let mut operands: Vec<Held> = op.requires.iter().chain(&op.delivers).map(|(held, _)| *held).collect();
            if let Some(what) = &op.what {
                operands.extend(what.dests.iter().chain(&what.sources).flat_map(ir::values));
            }
            for (value, width) in op.widths.iter().copied().chain(operands.iter().map(|held| (held.value, held.width)))
            {
                let known = widths.get(&value).copied().unwrap_or(0);
                widths.insert(value, known.max(width));
            }
        }
    }
    let phis: Vec<&Phi> = body.blocks.iter().flat_map(|block| &block.phis).collect();
    let mut changing = true;
    while changing {
        changing = false;
        for phi in &phis {
            let values: Vec<u32> =
                std::iter::once(phi.result).chain(phi.incoming.iter().map(|&(_, value)| value)).collect();
            let width =
                values.iter().map(|value| widths.get(value).copied().unwrap_or(2)).max().expect("a phi has a result");
            for value in values {
                if widths.get(&value).copied() != Some(width) {
                    widths.insert(value, width);
                    changing = true;
                }
            }
        }
    }
    widths
}

/// The move a phi becomes, at the end of the block it arrives from.
///
/// Placed on the predecessor's last instruction's address and claiming none
/// of its bytes.
pub(crate) fn _copy(
    where_: &LirBlock,
    into: u32,
    out_of: u32,
    group: Option<i64>,
    width: u32,
) -> Arc<Insn> {
    let last = where_.insns.last();
    let at = last.map_or(where_.at, |last| last.at);
    let edge = _nothing(last.map(|last| &**last), at);
    let mut one = Insn::new(
        at,
        Some(edge),
        Some(move_of(Held { value: into, width }, Held { value: out_of, width })),
        vec![into],
        vec![out_of],
    );
    one.group = group;
    one.call = last.and_then(|last| last.call.clone());
    Arc::new(one)
}

/// An empty span: an inserted instruction claims no original bytes.
///
/// Never None: that would mean "ask the node", whose bytes the neighbour
/// already claims.
pub(crate) fn _nothing(
    beside: Option<&Insn>,
    at: i64,
) -> (i64, i64) {
    let start = beside.and_then(|beside| beside.covers).map_or(at, |covers| covers.0);
    (start, start)
}

/// The copies at the end of the block, but ahead of what leaves it.
pub(crate) fn _before_the_terminator(
    block: &LirBlock,
    added: &[Arc<Insn>],
) -> Vec<Arc<Insn>> {
    if added.is_empty() {
        return block.insns.to_vec();
    }
    let insns = &block.insns;
    let mut cut = insns.len();
    while cut > 0 && _leaves(&insns[cut - 1]) {
        cut -= 1;
    }
    insns[..cut].iter().chain(added).chain(&insns[cut..]).cloned().collect()
}

/// Whether this instruction ends the block.
fn _leaves(one: &Insn) -> bool {
    one.what
        .as_ref()
        .is_some_and(
            |what| matches!(
                what.op,
                Operation::Jump | Operation::Branch | Operation::Return
            ),
        )
}

/// Place already selected parallel transfers on their exact CFG edges.
pub fn placed_on_edges(
    body: &LirBody,
    transfers: &IndexMap<(i64, i64), Vec<Arc<Insn>>>,
) -> LirBody {
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut copies: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut split: IndexMap<(i64, i64), Split> = IndexMap::default();
    for (&(where_, into), insns) in transfers {
        if at_of[&where_].succ.len() > 1 {
            split.insert((where_, into), Split::Selected(insns.clone()));
        } else {
            copies.entry(where_).or_default().extend(insns.iter().cloned());
        }
    }
    if split.is_empty() {
        let mut out = body.clone();
        for block in &mut out.blocks {
            block.insns = _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice)).into();
        }
        return out;
    }
    let kept: IndexMap<i64, Vec<Phi>> = body.blocks.iter().map(|block| (block.at, block.phis.to_vec())).collect();
    _split_edges(body, &split, &copies, &IndexMap::default(), &kept, &IndexMap::default(), true, &|_, _| false)
        .expect("an empty rename cannot cycle")
}

/// A block of its own on each critical edge, holding that edge's copies.
///
/// Synthetic labels are above physical offsets, in a namespace per body
/// entry. Layout emits the blocks out of line and each jumps to its
/// successor.
pub fn _split_edges(
    body: &LirBody,
    split: &IndexMap<(i64, i64), Split>,
    copies: &IndexMap<i64, Vec<Arc<Insn>>>,
    rename: &IndexMap<u32, u32>,
    kept: &IndexMap<i64, Vec<Phi>>,
    widths: &IndexMap<u32, u32>,
    selected: bool,
    cheaper_on_edge: &dyn Fn(i64, i64) -> bool,
) -> Result<LirBody, String> {
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let mut made: IndexMap<(i64, i64), LirBlock> = IndexMap::default();
    // The constants moved to their edges: the block they were made in and the
    // register they were made into.
    let mut moved: BTreeSet<(i64, u32)> = BTreeSet::new();
    let read_by_phis = _read_once(body);
    let mut landing: IndexMap<(i64, i64), i64> = IndexMap::default();
    let mut edges: Vec<(&(i64, i64), &Split)> = split.iter().collect();
    edges.sort_by_key(|(key, _)| **key);
    let highest = at_of.keys().copied().max().expect("a body has a block");
    for (number, (&(where_, into), pairs)) in (1_i64..).zip(edges) {
        let at = ((body.entry + 1) << 32).max(highest) + number;
        landing.insert((where_, into), at);
        let beside = &at_of[&where_].insns[at_of[&where_].insns.len() - 1];
        let mut insns: Vec<Arc<Insn>> = match (selected, pairs) {
            (true, Split::Selected(transfers)) => transfers
                .iter()
                .map(|one| {
                    let mut placed = (**one).clone();
                    placed.at = at;
                    placed.covers = Some((at, at));
                    Arc::new(placed)
                })
                .collect(),
            (false, Split::Pairs(pairs)) => pairs
                .iter()
                .map(|&(a, b)| {
                    // A constant the predecessor made for this phi alone is
                    // made on the edge instead: once, not
                    // every trip.
                    let constant = (read_by_phis.get(&b).copied() == Some(1) && cheaper_on_edge(where_, into))
                        .then(|| _constant_made_in(at_of[&where_], b))
                        .flatten();
                    let mut copy = match constant {
                        Some(what) => {
                            moved.insert((where_, b));
                            let into = Loc::Held(Held { value: a, width: widths[&a] });
                            (*_made(beside, at, Semantics { dests: vec![into], ..what }, vec![a], vec![])).clone()
                        }
                        None => (*_made(
                            beside,
                            at,
                            move_of(Held { value: a, width: widths[&a] }, Held { value: b, width: widths[&a] }),
                            vec![a],
                            vec![b],
                        ))
                        .clone(),
                    };
                    copy.group = Some(number);
                    Arc::new(copy)
                })
                .collect(),
            _ => unreachable!("`selected` says which kind the split holds"),
        };
        insns.push(_made(beside, at, jump_to(into), vec![], vec![]));
        // These instructions live outside `body.blocks` until the end, so
        // the ordinary rename walk below cannot see them.
        made.insert(
            (where_, into),
            LirBlock {
                at,
                insns: insns.iter().map(|one| _renamed(one, rename)).collect::<Result<_, _>>()?,
                succ: vec![into].into(),
                phis: vec![].into(),
                cold: false,
                align: 0,
                max_skip: 0,
            },
        );
    }

    let mut blocks: Vec<Block> = Vec::new();
    let entered: BTreeSet<i64> = landing.keys().map(|&(_, into)| into).collect();
    let left: BTreeSet<i64> = landing.keys().map(|&(where_, _)| where_).collect();
    for block in &body.blocks {
        // A block no split edge touches is the block it was.
        if rename.is_empty()
            && !left.contains(&block.at)
            && !entered.contains(&block.at)
            && !copies.contains_key(&block.at)
        {
            blocks.push(block.clone());
            continue;
        }
        let succ: Vec<i64> =
            block.succ.iter().map(|&one| landing.get(&(block.at, one)).copied().unwrap_or(one)).collect();
        let mut insns: Vec<Arc<Insn>> =
            _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice))
                .iter()
                .filter(|one| !one.defines.iter().any(|value| moved.contains(&(block.at, *value))))
                .map(|one| _retargeted(one, &landing, block.at))
                .collect();
        let last = block.insns.last();
        if let Some(last) = last {
            if let Some(what) = last.what.as_ref().filter(|what| what.op == Operation::Branch) {
                let fallthrough = block.succ.iter().copied().find(|&into| Some(into) != what.target);
                if let Some(&edge) = fallthrough.and_then(|fallthrough| landing.get(&(block.at, fallthrough))) {
                    insns.push(_made(last, last.at, jump_to(edge), vec![], vec![]));
                }
            }
        }
        blocks.push(
            LirBlock {
                at: block.at,
                insns: insns.iter().map(|one| _renamed(one, rename)).collect::<Result<_, _>>()?,
                succ: succ.into(),
                phis: kept[&block.at]
                    .iter()
                    .map(|phi| {
                        _renamed_phi(
                            &Phi {
                                result: phi.result,
                                incoming: phi
                                    .incoming
                                    .iter()
                                    .map(|&(where_, value)| {
                                        (landing.get(&(where_, block.at)).copied().unwrap_or(where_), value)
                                    })
                                    .collect(),
                            },
                            rename,
                        )
                    })
                    .collect::<Result<_, _>>()?,
                cold: block.cold,
                align: block.align,
                max_skip: block.max_skip,
            }
            .into(),
        );
    }
    let mut out = body.clone();
    for (&(where_, into), &at) in &landing {
        let succ = &body.blocks.iter().find(|one| one.at == where_).expect("a split edge's block").succ;
        out.odds.rerouted(where_, succ, into, &[(at, 1.0)]);
    }
    blocks.extend(made.into_values().map(Block::from));
    out.blocks = blocks;
    Ok(out)
}

/// One instruction in a split block, claiming none of BC's own bytes.
fn _made(
    beside: &Insn,
    at: i64,
    what: Semantics,
    defines: Vec<u32>,
    uses: Vec<u32>,
) -> Arc<Insn> {
    let mut one = Insn::new(at, Some((at, at)), Some(what), defines, uses);
    one.call = beside.call.clone();
    Arc::new(one)
}

/// A branch or jump pointing at the split block instead of the successor.
fn _retargeted(
    one: &Arc<Insn>,
    landing: &IndexMap<(i64, i64), i64>,
    here: i64,
) -> Arc<Insn> {
    let Some(what) = &one.what else {
        return Arc::clone(one);
    };
    let Some(moved) = what.remapped(|to| landing.get(&(here, to)).copied().unwrap_or(to)) else {
        return Arc::clone(one);
    };
    let mut out = (**one).clone();
    out.what = Some(moved);
    Arc::new(out)
}

/// A split edge whose copies all went away is the edge again.
///
/// Coalescing leaves the block `_split_edges` made holding only its jump,
/// and that jump, placed out of line, was taken on every pass.
#[must_use]
pub fn unsplit(body: &LirBody) -> LirBody {
    let floor = (body.entry + 1) << 32;
    let mut bypass: IndexMap<i64, i64> = IndexMap::default();
    for block in &body.blocks {
        if block.at < floor || !block.phis.is_empty() || block.succ.len() != 1 {
            continue;
        }
        let live: Vec<&Arc<Insn>> = block
            .insns
            .iter()
            .filter(|one| {
                !(one.what.as_ref().is_some_and(|what| {
                    what.op == Operation::Nothing && what.name.as_deref().is_none_or(str::is_empty)
                }) && one.defines.is_empty()
                    && one.uses.is_empty())
            })
            .collect();
        if live.len() == 1
            && live[0]
                .what
                .as_ref()
                .is_some_and(|what| what.op == Operation::Jump && what.target == Some(block.succ[0]))
        {
            bypass.insert(block.at, block.succ[0]);
        }
    }
    if bypass.is_empty() {
        return body.clone();
    }

    let where_ = |mut at: i64| -> i64 {
        let mut seen = BTreeSet::new();
        while let Some(&next) = bypass.get(&at) {
            if seen.contains(&at) {
                break;
            }
            seen.insert(at);
            at = next;
        }
        at
    };

    let mut blocks: Vec<Block> = Vec::new();
    for block in &body.blocks {
        if bypass.contains_key(&block.at) {
            continue;
        }
        let insns = block
            .insns
            .iter()
            .map(|one| match one.what.as_ref().and_then(|what| what.remapped(where_)) {
                Some(what) => {
                    let mut out = (**one).clone();
                    out.what = Some(what);
                    Arc::new(out)
                }
                None => Arc::clone(one),
            })
            .collect();
        let phis = block
            .phis
            .iter()
            .map(|phi| Phi {
                result: phi.result,
                incoming: phi.incoming.iter().map(|&(at, value)| (where_(at), value)).collect(),
            })
            .collect();
        blocks.push(
            LirBlock {
                at: block.at,
                insns,
                succ: block.succ.iter().map(|&at| where_(at)).collect(),
                phis,
                cold: block.cold,
                align: block.align,
                max_skip: block.max_skip,
            }
            .into(),
        );
    }
    let mut out = body.clone();
    out.blocks = blocks;
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{_split_edges, Split, unsplit};
    use crate::backend::verify;
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
    use crate::support::hash::IndexMap;

    fn what(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Option<Semantics> {
        Some(Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) })
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
        phis: Vec<Phi>,
    ) -> LirBlock {
        LirBlock { at, insns: insns.into(), succ: succ.into(), phis: phis.into(), cold: false, align: 0, max_skip: 0 }
    }

    fn body(
        name: &str,
        blocks: Vec<Block>,
    ) -> LirBody {
        LirBody::new(name, 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn imm(
        value: i64,
        width: u32,
    ) -> Loc {
        Loc::Imm(Imm { value, width, address: None })
    }

    fn branch(
        at: i64,
        covers: Option<(i64, i64)>,
        name: &str,
        target: i64,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(at, covers, what(Operation::Branch, name, vec![], vec![], Some(target)), vec![], vec![]))
    }

    #[test]
    fn test_unsplit_keeps_virtual_edge_copy_anchors() {
        // qcport's snd_mix lost value#259 when unsplit treated two no-byte
        // edge-copy anchors as empty and bypassed the blocks defining the phi.
        let jump_branch = branch(1, None, "je", 1 << 33);
        let anchor =
            Arc::new(Insn::new(2, None, what(Operation::Nothing, "", vec![], vec![], None), vec![259], vec![85]));
        let jump = Arc::new(Insn::new(3, None, what(Operation::Jump, "jmp", vec![], vec![], Some(33)), vec![], vec![]));
        let compare = Arc::new(Insn::new(
            4,
            None,
            what(
                Operation::Compare,
                "cmp",
                vec![],
                vec![Loc::Reg(Reg { register: RegId::CX, width: 2 }), imm(8, 2)],
                None,
            ),
            vec![],
            vec![259],
        ));
        let mut anchored = body(
            "anchored-split-edge",
            vec![
                block(0, vec![jump_branch], vec![1 << 32, 1 << 33], vec![]).into(),
                block(1 << 32, vec![Arc::clone(&anchor), Arc::clone(&jump)], vec![33], vec![]).into(),
                block(1 << 33, vec![anchor, jump], vec![33], vec![]).into(),
                block(33, vec![compare], vec![], vec![]).into(),
            ],
        );
        anchored.inputs = BTreeSet::from([85]);

        let done = unsplit(&anchored);

        assert_eq!(verify::verify(&done, false), Vec::<String>::new());
    }

    #[test]
    fn test_split_fallthrough_gets_an_explicit_jump() {
        // EVTRAP's split exit edge was emitted unreachable after the handler.
        let jz = branch(0, Some((0, 2)), "jz", 20);
        let edge_body = body("edge", vec![block(0, vec![Arc::clone(&jz)], vec![10, 20], vec![]).into()]);
        let done = _split_edges(
            &edge_body,
            &IndexMap::from_iter([((0, 10), Split::Pairs(vec![(2, 1)]))]),
            &IndexMap::default(),
            &IndexMap::default(),
            &IndexMap::from_iter([(0, vec![])]),
            &IndexMap::from_iter([(2, 2)]),
            false,
            &|_, _| false,
        )
        .unwrap();
        let edge = &done.blocks[done.blocks.len() - 1];
        let last = &done.blocks[0].insns[done.blocks[0].insns.len() - 1];
        let last_what = last.what.as_ref().unwrap();
        assert_eq!(last_what.op, Operation::Jump);
        assert_eq!(last_what.target, Some(edge.at));
        assert_eq!(last.at, jz.at);
    }

    #[test]
    fn test_a_block_no_split_edge_touches_is_not_rebuilt() {
        // Every block was rebuilt, retargeting each of its instructions:
        // 1.5 M Ir on d_faces.
        let jz = branch(0, Some((0, 2)), "jz", 20);
        let far = body(
            "far",
            vec![
                block(0, vec![Arc::clone(&jz)], vec![10, 20], vec![]).into(),
                block(10, vec![branch(10, Some((10, 12)), "jz", 40)], vec![40], vec![]).into(),
                block(20, vec![], vec![], vec![]).into(),
                block(40, vec![], vec![], vec![]).into(),
            ],
        );
        let done = _split_edges(
            &far,
            &IndexMap::from_iter([((0, 10), Split::Pairs(vec![(2, 1)]))]),
            &IndexMap::default(),
            &IndexMap::default(),
            &IndexMap::from_iter([(0, vec![]), (10, vec![]), (20, vec![]), (40, vec![])]),
            &IndexMap::from_iter([(2, 2)]),
            false,
            &|_, _| false,
        )
        .unwrap();
        assert!(done.blocks[2].same_as(&far.blocks[2]));
        assert!(done.blocks[3].same_as(&far.blocks[3]));
    }
}
