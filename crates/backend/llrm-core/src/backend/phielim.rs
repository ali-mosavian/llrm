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
//! `pressuresink`, in every machine (`flow::machine_with`); `EdgePrices` come
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

/// `prices`: where none are known, a copy stays in its predecessor.
#[derive(Default)]
pub struct PhiElimination {
    pub prices: Option<EdgePrices>,
}

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
        if !OLD_LOWERING.with(std::cell::Cell::get) && !llrm_support::env_flag!("LLRM_NO_PHIELIM_LLVM") {
            let lowered = crate::phielimination::lowered(&body)?;
            return eliminated_priced(&lowered, self.prices);
        }
        eliminated_priced(&body, self.prices)
    }
}

thread_local! {
    static OLD_LOWERING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `work` with every phi lowered by this pass's own parallel copies, on this
/// thread: for a test of what takes parallel copies (the allocator's and
/// `parcopy`'s) and needs some.
pub fn with_old_lowering<R>(work: impl FnOnce() -> R) -> R {
    let before = OLD_LOWERING.with(|flag| flag.replace(true));
    let out = work();
    OLD_LOWERING.with(|flag| flag.set(before));
    out
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
        name: Some("mov".to_owned()),
        dests: vec![Loc::Held(into)],
        sources: vec![Loc::Held(out_of)],
        ..Semantics::new(Operation::Move)
    }
}

fn jump_to(target: i64) -> Semantics {
    Semantics { name: Some("jmp".to_owned()), target: Some(target), ..Semantics::new(Operation::Jump) }
}

/// `body` with every phi it can lower replaced by copies.
pub fn eliminated(body: &LirBody) -> Result<LirBody, String> {
    eliminated_priced(body, None)
}

/// `eliminated`, a copy put on its edge where `prices` say that costs less.
pub fn eliminated_priced(
    body: &LirBody,
    prices: Option<EdgePrices>,
) -> Result<LirBody, String> {
    let body = &_exits_copied(body);
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let successors: IndexMap<i64, usize> = body.blocks.iter().map(|block| (block.at, block.succ.len())).collect();
    let reaches = Reaches::new(body, &at_of);
    let widths = _widths(body);
    // Renaming a phi whose copies the predecessor could hold saves nothing
    // (only an edge block); an x87 value has no copy to save.
    let floating = crate::backend::ssaspill::floating(body);

    // `copies` copies on the edge from `from` to `into` cost less than in
    // `from`.
    let on_edge = |from: i64, into: i64, copies: usize| {
        prices.is_some_and(|prices| prices.cheaper_on_edge(body.odds.chance(from, &at_of[&from].succ, into), copies))
    };

    let once = _read_once(body);
    let flow = Flow::of(body);
    // One per predecessor->successor edge. Every move a phi becomes on
    // that edge is simultaneous with the others and with none outside it.
    let mut groups: IndexMap<(i64, i64), i64> = IndexMap::default();
    let mut edge_group = |where_: i64, into: i64| -> i64 {
        let next = groups.len() as i64 + 1;
        *groups.entry((where_, into)).or_insert(next)
    };

    let mut split: IndexMap<(i64, i64), Split> = IndexMap::default();
    let mut copies: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut rename: IndexMap<u32, u32> = IndexMap::default();
    let mut kept: IndexMap<i64, Vec<Phi>> = IndexMap::default();
    for block in &body.blocks {
        let mut stays = Vec::new();
        let mut crossing: IndexMap<i64, Vec<(u32, u32)>> = IndexMap::default();
        for phi in &block.phis {
            let edges: Vec<(i64, u32)> =
                phi.incoming.iter().copied().filter(|(where_, _)| at_of.contains_key(where_)).collect();
            if edges.len() != phi.incoming.len() {
                stays.push(phi.clone()); // an edge from outside this body
                continue;
            }
            // A one-input phi computes its input. Unify the identities before
            // allocation instead of creating a copy.
            if edges.len() == 1 {
                rename.insert(phi.result, edges[0].1);
                continue;
            }
            if edges.iter().any(|(where_, _)| successors.get(where_).copied().unwrap_or(0) > 1) {
                // A critical edge. Where every incoming value is defined in
                // its own predecessor and read by nothing but this phi, having
                // it define the phi's result outright says what the phi said.
                if edges.iter().all(|&(w, v)| {
                    _defined_in(at_of[&w], v)
                        // A constant made in a loop to leave it runs every
                        // trip.
                        && !(on_edge(w, block.at, 1) && _constant_made_in(at_of[&w], v).is_some())
                        && (once.get(&v).copied() == Some(1)
                            || !floating.contains(&phi.result)
                                && (_observed(&reaches, &at_of, w, block.at, phi.result)
                                    || _observed(&reaches, &at_of, w, block.at, v))
                                && _read_past_loop(&flow, block.at, w, v))
                        && _defined_in_place(at_of[&w], v, phi.result)
                        && !_live_after(&at_of, block.at, w, v, phi.result)
                }) {
                    for &(_where, value) in &edges {
                        rename.insert(value, phi.result);
                    }
                    continue;
                }
                // Split the edge, unless nothing on the predecessor's other
                // paths can read what the copies write (decided per edge
                // below).
                for &(where_, value) in &edges {
                    if successors.get(&where_).copied().unwrap_or(0) > 1 {
                        crossing.entry(where_).or_default().push((phi.result, value));
                    } else {
                        let group = edge_group(where_, block.at);
                        copies
                            .entry(where_)
                            .or_default()
                            .push(
                                _copy(
                                    at_of[&where_],
                                    phi.result,
                                    value,
                                    Some(group),
                                    widths[&phi.result],
                                ),
                            );
                    }
                }
                continue;
            }
            for &(where_, value) in &edges {
                let group = edge_group(where_, block.at);
                copies
                    .entry(where_)
                    .or_default()
                    .push(
                        _copy(
                            at_of[&where_],
                            phi.result,
                            value,
                            Some(group),
                            widths[&phi.result],
                        ),
                    );
            }
        }
        for (&where_, pairs) in &crossing {
            // A copy may run in the predecessor only when neither end needs
            // a distinct value on its other paths. Otherwise put it on its
            // actual edge.
            // A copy goes where it costs least.
            let leaves = on_edge(where_, block.at, pairs.len());
            if leaves
                || pairs.iter().any(|&(result, value)| {
                    _observed(&reaches, &at_of, where_, block.at, result)
                        || _observed(&reaches, &at_of, where_, block.at, value)
                })
            {
                match split.entry((where_, block.at)).or_insert_with(|| Split::Pairs(Vec::new())) {
                    Split::Pairs(existing) => existing.extend(pairs.iter().copied()),
                    Split::Selected(_) => unreachable!("eliminated only splits pairs"),
                }
                continue;
            }
            for &(result, value) in pairs {
                let group = edge_group(where_, block.at);
                copies
                    .entry(where_)
                    .or_default()
                    .push(
                        _copy(
                            at_of[&where_],
                            result,
                            value,
                            Some(group),
                            widths[&result],
                        ),
                    );
            }
        }
        kept.insert(block.at, stays);
    }

    if copies.is_empty() && rename.is_empty() && split.is_empty() {
        return Ok(body.clone());
    }
    if !split.is_empty() {
        return _split_edges(body, &split, &copies, &rename, &kept, &widths, false, &|from, into| {
            on_edge(from, into, 1)
        });
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        let placed = _before_the_terminator(block, copies.get(&block.at).map_or(&[][..], Vec::as_slice));
        block.insns = placed.iter().map(|one| _renamed(one, &rename)).collect::<Result<_, _>>()?;
        block.phis = kept[&block.at].iter().map(|phi| _renamed_phi(phi, &rename)).collect::<Result<_, _>>()?;
    }
    Ok(out)
}

/// `body` with a loop exit's one-input phis as copies opening the exit.
///
/// Renamed away, the value computed in the loop would be the one read after
/// it, and a call there would confine the loop's register to one the call
/// preserves. The copy splits the range where the loop ends.
fn _exits_copied(body: &LirBody) -> LirBody {
    let depths = crate::analysis::intervals::depths(body);
    let mut entering = IndexMap::<i64, usize>::default();
    for block in &body.blocks {
        for to in &block.succ {
            *entering.entry(*to).or_default() += 1;
        }
    }
    let leaves = |block: &LirBlock, phi: &Phi| match phi.incoming.as_slice() {
        [(from, _value)] => entering.get(&block.at) == Some(&1) && depths.get(from) > depths.get(&block.at),
        _ => false,
    };
    if !body.blocks.iter().any(|block| block.phis.iter().any(|phi| leaves(block, phi))) {
        return body.clone();
    }
    let widths = _widths(body);
    body.with_blocks(
        body.blocks
            .iter()
            .map(|block| {
                let (copied, phis): (Vec<&Phi>, Vec<&Phi>) = block.phis.iter().partition(|phi| leaves(block, phi));
                let at = block.insns.first().map_or(block.at, |first| first.at);
                let head = copied.iter().map(|phi| {
                    let (result, value) = (phi.result, phi.incoming[0].1);
                    let width = widths.get(&result).copied().unwrap_or(0).max(widths.get(&value).copied().unwrap_or(0));
                    let mut one = Insn::new(
                        at,
                        Some((at, at)),
                        Some(move_of(Held { value: result, width }, Held { value, width })),
                        vec![result],
                        vec![value],
                    );
                    one.call = block.insns.first().and_then(|first| first.call.clone());
                    Arc::new(one)
                });
                LirBlock {
                    phis: phis.into_iter().cloned().collect(),
                    ..block.with_insns(head.chain(block.insns.iter().cloned()).collect()).into_inner()
                }
            })
            .collect(),
    )
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

/// Whether `result`, the phi in `phi_block`'s, is still read after `value`
/// is defined in predecessor `from`: later there, by another phi on the edge
/// into `phi_block`, or down another path. Defining `result` where `value`
/// was would then overwrite what that read wants.
fn _live_after(
    at_of: &IndexMap<i64, &Block>,
    phi_block: i64,
    from: i64,
    value: u32,
    result: u32,
) -> bool {
    let block = at_of[&from];
    let defined = block.insns.iter().position(|one| one.defines.contains(&value)).expect("defined here");
    if block.insns[defined + 1..].iter().any(|one| one.uses.contains(&result)) {
        return true;
    }
    let read_on_edge =
        |from: i64, into: i64| at_of[&into].phis.iter().any(|phi| phi.incoming.contains(&(from, result)));
    // Live into a block: read before redefined, or passed on.
    let mut seen = BTreeSet::new();
    let mut work: Vec<(i64, i64)> = block.succ.iter().map(|&into| (from, into)).collect();
    while let Some((edge_from, into)) = work.pop() {
        if read_on_edge(edge_from, into) {
            return true;
        }
        // The phi block's own phi defines `result` afresh.
        if into == phi_block || !seen.insert(into) {
            continue;
        }
        let Some(next) = at_of.get(&into) else { continue };
        if next.phis.iter().any(|phi| phi.result == result) {
            continue;
        }
        for one in &next.insns {
            if one.uses.contains(&result) {
                return true;
            }
            if one.defines.contains(&result) {
                break;
            }
        }
        if !next.insns.iter().any(|one| one.defines.contains(&result)) {
            work.extend(next.succ.iter().map(|&after| (into, after)));
        }
    }
    false
}

/// Whether `value`, which `from` defines on the back edge into `header`, is
/// read besides by the phi only outside the loop that edge closes (or on edges
/// from `from`): the loop's variable then holds it wherever it is read, and the
/// phi can define the variable outright, as gcc's out-of-SSA coalesces a phi's
/// result with an argument they do not interfere (`tree-ssa-coalesce`) whoever
/// else reads the argument.
fn _read_past_loop(
    flow: &Flow,
    header: i64,
    from: i64,
    value: u32,
) -> bool {
    // The loop: what reaches `from` without passing the header. A header that
    // does not dominate `from` is no loop.
    let mut inside: BTreeSet<i64> = BTreeSet::from([header]);
    let mut pending = vec![from];
    while let Some(at) = pending.pop() {
        if !inside.insert(at) {
            continue;
        }
        let preds = flow.preds.get(&at).map_or(&[][..], Vec::as_slice);
        if preds.is_empty() && at != header {
            return false;
        }
        pending.extend(preds.iter().copied());
    }
    // Only the reads of the value are looked at, not every block of the body.
    let phis_ok = flow
        .phi_reads
        .get(&value)
        .into_iter()
        .flatten()
        .all(|&(block, edge)| edge == from && (block == header || !inside.contains(&block)));
    let insns_ok =
        flow.insn_reads.get(&value).into_iter().flatten().all(|&block| !(inside.contains(&block) && block != from));
    phis_ok && insns_ok
}

/// Where the values are read and which blocks lead to which: found once for
/// the body, for the walks of `_read_past_loop`, each of which looked at every
/// block of the body.
struct Flow {
    preds: IndexMap<i64, Vec<i64>>,
    /// Per value, the blocks whose instructions read it (or require it).
    insn_reads: IndexMap<u32, Vec<i64>>,
    /// Per value, the (block, edge) of each phi that reads it.
    phi_reads: IndexMap<u32, Vec<(i64, i64)>>,
}

impl Flow {
    fn of(body: &LirBody) -> Self {
        let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
        let mut insn_reads: IndexMap<u32, Vec<i64>> = IndexMap::default();
        let mut phi_reads: IndexMap<u32, Vec<(i64, i64)>> = IndexMap::default();
        for block in &body.blocks {
            for &to in &block.succ {
                let list = preds.entry(to).or_default();
                if !list.contains(&block.at) {
                    list.push(block.at);
                }
            }
            for one in &block.insns {
                for &value in one.uses.iter().chain(one.requires.iter().map(|(held, _)| &held.value)) {
                    let list = insn_reads.entry(value).or_default();
                    if list.last() != Some(&block.at) {
                        list.push(block.at);
                    }
                }
            }
            for phi in &block.phis {
                for &(edge, read) in &phi.incoming {
                    phi_reads.entry(read).or_default().push((block.at, edge));
                }
            }
        }
        Self { preds, insn_reads, phi_reads }
    }
}

/// Whether the instruction defining `value` may define `result` instead: not
/// where it is a two-address operation that reads `result` as other than its
/// first source, which the destination would then overwrite (`1 - cur` became
/// `sub cur, cur`; a commutative one is swapped).
fn _defined_in_place(
    block: &LirBlock,
    value: u32,
    result: u32,
) -> bool {
    let Some(one) = block.insns.iter().find(|one| one.defines.contains(&value)) else { return true };
    if !one.uses.contains(&result) {
        return true;
    }
    let Some(what) = one.what.as_ref().filter(|what| crate::backend::twoaddr::ties(what)) else { return true };
    let reads = |at: usize| {
        matches!(
            what.sources.get(at),
            Some(Loc::Held(held)) if held.value == result
        )
    };
    let commutes =
        matches!(
            what.name.as_deref(),
            Some("add" | "and" | "or" | "xor" | "imul")
        ) && what.sources.len() == 2;
    commutes || !(1..what.sources.len()).any(reads)
}

/// Whether exactly one instruction in this block defines the value.
fn _defined_in(
    block: &LirBlock,
    value: u32,
) -> bool {
    block.insns.iter().filter(|one| one.defines.contains(&value)).count() == 1
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

/// One operand with every value it names put through the rename.
///
/// Through `ir::mapped` rather than a case per operand shape: a cell names
/// the value that computed its address.
pub fn _settled(
    where_: &Loc,
    rename: &IndexMap<u32, u32>,
) -> Result<Loc, String> {
    let mut error = None;
    let out = ir::mapped(where_, |one| match _name(one.value, rename) {
        Ok(value) => Held { value, width: one.width },
        Err(refused) => {
            error.get_or_insert(refused);
            *one
        }
    });
    error.map_or(Ok(out), Err)
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
    for block in &body.blocks {
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

    use super::{_observed, _settled, _split_edges, Reaches, Split, eliminated, unsplit};
    use crate::analysis::frequency::Frequency;
    use crate::backend::verify;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Block, BlockOdds, Insn, LirBlock, LirBody, Phi};
    use crate::support::hash::IndexMap;

    fn what(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Option<Semantics> {
        Some(Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) })
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

    fn held(
        value: u32,
        width: u32,
    ) -> Loc {
        Loc::Held(Held { value, width })
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

    fn values(
        done: &LirBody,
        pick: fn(&Insn) -> &Vec<u32>,
    ) -> BTreeSet<u32> {
        done.insns().iter().flat_map(|one| pick(one).clone()).collect()
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
    fn test_a_phi_read_by_its_sibling_is_not_renamed_into_the_latch() {
        // runtime.nib's print_q4 hung once simplifycfg made its loop's entry
        // edge critical: `b` took the latch's `b + 1` as its own register, so
        // `a`'s copy on the back edge read the new `b`, not the old one.
        let mov = |at: i64, value: u32, n: i64| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Move, "mov", vec![held(value, 2)], vec![imm(n, 2)], None),
                vec![value],
                vec![],
            ))
        };
        let test = Arc::new(Insn::new(
            10,
            Some((10, 11)),
            what(Operation::Compare, "cmp", vec![], vec![held(4, 2), imm(9, 2)], None),
            vec![],
            vec![4],
        ));
        let step = Arc::new(Insn::new(
            20,
            Some((20, 21)),
            what(Operation::Binary, "add", vec![held(5, 2)], vec![held(4, 2), imm(1, 2)], None),
            vec![5],
            vec![4],
        ));
        let looped = body(
            "looped",
            vec![
                block(0, vec![mov(0, 1, 0), mov(1, 2, 1)], vec![10, 30], vec![]).into(),
                block(
                    10,
                    vec![test],
                    vec![20, 30],
                    vec![
                        Phi { result: 3, incoming: vec![(0, 1), (20, 4)] },
                        Phi { result: 4, incoming: vec![(0, 2), (20, 5)] },
                    ],
                )
                .into(),
                block(20, vec![step], vec![10], vec![]).into(),
                block(30, vec![], vec![], vec![]).into(),
            ],
        );
        let done = eliminated(&looped).unwrap();
        let latch = done.blocks.iter().find(|one| one.at == 20).unwrap();
        let redefined = latch.insns.iter().position(|one| one.defines.contains(&4));
        let copied = latch.insns.iter().position(|one| one.defines.contains(&3) && one.uses.contains(&4));
        assert!(copied.is_some() && redefined.is_none_or(|at| at > copied.unwrap()), "{:?}", latch.insns);
    }

    #[test]
    fn test_phi_elimination_copies_the_whole_scalar() {
        // VBDOS nbody printed PX0=285219921 for 1258: phi copies truncated
        // 32-bit accumulators.
        for critical in [false, true] {
            let load = |at: i64, value: u32| {
                Arc::new(Insn::new(
                    at,
                    Some((at, at + 1)),
                    what(Operation::Move, "mov", vec![held(value, 4)], vec![imm(0x1234_5678, 4)], None),
                    vec![value],
                    vec![],
                ))
            };
            let read = Arc::new(Insn::new(
                2,
                Some((2, 3)),
                what(Operation::Binary, "add", vec![held(4, 4)], vec![held(3, 4), held(1, 4)], None),
                vec![4],
                vec![3, 1],
            ));
            let wide = body(
                "wide",
                vec![
                    block(0, vec![load(0, 1)], if critical { vec![2, 1] } else { vec![2] }, vec![]).into(),
                    block(1, vec![load(1, 2)], vec![2], vec![]).into(),
                    block(2, vec![read], vec![], vec![Phi { result: 3, incoming: vec![(0, 1), (1, 2)] }]).into(),
                ],
            );
            let done = eliminated(&wide).unwrap();
            let copies: Vec<Arc<Insn>> = done.insns().into_iter().filter(|op| op.group.is_some()).collect();
            assert_eq!(copies.len(), 2, "critical={critical}");
            assert!(copies.iter().all(|op| {
                let what = op.what.as_ref().unwrap();
                what.dests.iter().chain(&what.sources).all(|arg| matches!(arg, Loc::Held(Held { width: 4, .. })))
            }));
        }
    }

    #[test]
    fn test_phi_on_a_single_predecessor_exit_does_not_split_the_edge() {
        // HARR gained an empty jump trampoline after LCSSA closed its loop
        // exit.
        let mut use_ =
            Insn::new(2, Some((2, 3)), what(Operation::Move, "mov", vec![], vec![held(3, 2)], None), vec![], vec![3]);
        use_.widths = vec![(3, 2)];
        let exit = body(
            "exit",
            vec![
                block(0, vec![branch(0, Some((0, 2)), "jz", 2)], vec![1, 2], vec![]).into(),
                block(1, vec![], vec![], vec![]).into(),
                block(2, vec![Arc::new(use_)], vec![], vec![Phi { result: 3, incoming: vec![(0, 1)] }]).into(),
            ],
        );

        let done = eliminated(&exit).unwrap();

        assert_eq!(done.blocks.len(), exit.blocks.len());
        let exit_block = done.blocks.iter().find(|block| block.at == 2).unwrap();
        assert!(exit_block.phis.is_empty());
        assert!(done.insns().iter().all(|insn| insn.group.is_none()));
        assert_eq!(exit_block.insns[0].uses, vec![1]);
        assert_eq!(exit_block.insns[0].widths, vec![(1, 2)]);
    }

    #[test]
    fn test_phi_source_live_on_the_other_branch_gets_an_edge_copy() {
        // nbody spilled its inner counter after exit copies extended both
        // accumulators.
        let push = |at: i64, value: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Push, "push", vec![], vec![held(value, 2)], None),
                vec![],
                vec![value],
            ))
        };
        let live = body(
            "live-source",
            vec![
                block(0, vec![branch(0, Some((0, 2)), "jz", 2)], vec![1, 2], vec![]).into(),
                block(1, vec![push(1, 1)], vec![], vec![]).into(),
                block(2, vec![push(2, 3)], vec![], vec![Phi { result: 3, incoming: vec![(0, 1), (4, 5)] }]).into(),
                block(4, vec![], vec![2], vec![]).into(),
            ],
        );

        let done = eliminated(&live).unwrap();

        assert_eq!(done.blocks.len(), live.blocks.len() + 1);
        let edge = &done.blocks[done.blocks.len() - 1];
        assert!(edge.insns[0].group.is_some());
        assert_eq!(edge.insns[0].defines, vec![3]);
        assert_eq!(edge.insns[0].uses, vec![1]);
    }

    /// A branch with both of its edges split took the even odds of an edge
    /// isel never estimated: tally's outer loop read 16x its frequency once
    /// the splits were made, and every consumer of the odds read it.
    #[test]
    fn test_splitting_both_edges_of_a_branch_keeps_its_odds() {
        let define = |at: i64, result: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Move, "mov", vec![held(result, 4)], vec![imm(1, 4)], None),
                vec![result],
                vec![],
            ))
        };
        let both = |value: u32| vec![Phi { result: value, incoming: vec![(0, 1), (4, 2)] }];
        let mut branching = body(
            "both-edges",
            vec![
                block(0, vec![define(0, 1), branch(1, Some((1, 2)), "jz", 2)], vec![1, 2], vec![]).into(),
                block(1, vec![], vec![], both(10)).into(),
                block(2, vec![], vec![], both(11)).into(),
                block(4, vec![define(4, 2), branch(5, Some((5, 6)), "jz", 2)], vec![1, 2], vec![]).into(),
            ],
        );
        for (to, probability) in [(1, 0.9), (2, 0.1)] {
            branching.odds.taken.insert((0, to), (probability * BlockOdds::CERTAIN).round() as u32);
        }

        let done = eliminated(&branching).unwrap();

        assert!(done.blocks.len() > branching.blocks.len() + 1, "both edges of block 0 are split");
        let (before, after) = (Frequency::of(&branching), Frequency::of(&done));
        for at in [1, 2] {
            assert!(
                (before.block(at) - after.block(at)).abs() < 1e-6,
                "block {at}: {} split into {}",
                before.block(at),
                after.block(at)
            );
        }
    }

    #[test]
    fn test_critical_edge_copy_uses_the_final_trivial_phi_name() {
        // C crc32 returned -1141145971 instead of 778214622: the synthetic
        // edge copy read the removed intermediate phi result.
        let move_ = |at: i64, result: u32, source: Loc| {
            let uses = match &source {
                Loc::Held(held) => vec![held.value],
                _ => vec![],
            };
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Move, "mov", vec![held(result, 4)], vec![source], None),
                vec![result],
                uses,
            ))
        };
        let trivial = body(
            "trivial-before-critical",
            vec![
                block(0, vec![move_(0, 1, imm(7, 4))], vec![1], vec![]).into(),
                block(
                    1,
                    vec![branch(1, Some((1, 2)), "jz", 3)],
                    vec![2, 3],
                    vec![Phi { result: 2, incoming: vec![(0, 1)] }],
                )
                .into(),
                // Reading the source down the other arm requires a real edge
                // copy.
                block(2, vec![move_(2, 4, held(2, 4))], vec![], vec![]).into(),
                block(
                    3,
                    vec![move_(3, 7, held(6, 4))],
                    vec![],
                    vec![Phi { result: 6, incoming: vec![(1, 2), (4, 5)] }],
                )
                .into(),
                block(4, vec![move_(4, 5, imm(9, 4))], vec![3], vec![]).into(),
            ],
        );

        let done = eliminated(&trivial).unwrap();

        let edge = done.blocks.iter().find(|block| block.at > 4).unwrap();
        let copy = &edge.insns[0];
        assert_eq!(copy.uses, vec![1]);
        assert_eq!(copy.what.as_ref().unwrap().sources, vec![held(1, 4)]);
        let defined = values(&done, |one| &one.defines);
        let used = values(&done, |one| &one.uses);
        assert!(used.is_subset(&defined), "phi elimination left undefined values {:?}", used.difference(&defined));
    }

    /// A value nothing after the edge reads walked every block after it, for
    /// each (value, edge) asked: N diamonds, N^2 visits (`branches` 1024:
    /// `lir phielim` 2087 Minstr, the slope 2).
    #[test]
    fn test_a_value_nothing_reads_after_the_edge_asks_no_walk() {
        let tail = 200;
        let mut blocks = vec![
            block(0, vec![], vec![1, 2], vec![]),
            block(1, vec![], vec![], vec![Phi { result: 3, incoming: vec![(0, 7)] }]),
        ];
        blocks.extend((2..tail).map(|at| block(at, vec![], vec![at + 1], vec![])));
        blocks.push(block(tail, vec![], vec![], vec![]));
        let long = body("long", blocks.into_iter().collect());
        let at_of: IndexMap<i64, &Block> = long.blocks.iter().map(|block| (block.at, block)).collect();
        let before = super::walked();
        assert!(!_observed(&Reaches::new(&long, &at_of), &at_of, 0, 1, 7));
        assert_eq!(super::walked() - before, 0, "a walk down {tail} blocks that read nothing");
    }

    /// `_observed` as it was, a walk out from the successors for a read.
    fn walked(
        at_of: &IndexMap<i64, &Block>,
        where_: i64,
        into: i64,
        value: u32,
    ) -> bool {
        let mut pending: Vec<i64> = at_of[&where_].succ.iter().copied().filter(|&at| at != into).collect();
        if pending.iter().any(|at| {
            at_of.get(at).is_some_and(|block| block.phis.iter().any(|phi| phi.incoming.contains(&(where_, value))))
        }) {
            return true;
        }
        let mut seen: BTreeSet<i64> = pending.iter().copied().collect();
        while let Some(at) = pending.pop() {
            let Some(block) = at_of.get(&at) else { return true };
            if block
                .insns
                .iter()
                .any(|one| one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value))
            {
                return true;
            }
            for &successor in &block.succ {
                if at_of
                    .get(&successor)
                    .is_some_and(|follower| follower.phis.iter().any(|phi| phi.incoming.contains(&(block.at, value))))
                {
                    return true;
                }
                if successor != into && seen.insert(successor) {
                    pending.push(successor);
                }
            }
        }
        false
    }

    /// `_read_past_loop` as it was: every block of the body looked at for each
    /// ask (a nest of 128 loops: 417 M instructions for 263 asks).
    fn read_past_loop_by_scan(
        at_of: &IndexMap<i64, &Block>,
        header: i64,
        from: i64,
        value: u32,
    ) -> bool {
        let mut inside: BTreeSet<i64> = BTreeSet::from([header]);
        let mut pending = vec![from];
        while let Some(at) = pending.pop() {
            if !inside.insert(at) {
                continue;
            }
            let preds: Vec<i64> =
                at_of.values().filter(|block| block.succ.contains(&at)).map(|block| block.at).collect();
            if preds.is_empty() && at != header {
                return false;
            }
            pending.extend(preds);
        }
        at_of
            .values()
            .all(
                |block| {
                    let in_loop = inside.contains(&block.at);
                    let phis_ok = block.phis.iter().all(|phi| {
                        phi.incoming
                            .iter()
                            .all(|&(edge, read)| read != value || (edge == from && (block.at == header || !in_loop)))
                    });
                    let insns_ok = !block.insns.iter().any(|one| {
                        (one.uses.contains(&value) || one.requires.iter().any(|(held, _)| held.value == value))
                            && in_loop
                            && block.at != from
                    });
                    phis_ok && insns_ok
                },
            )
    }

    /// The indexes of reads and of predecessors give what looking at every
    /// block gave, on graphs of every shape.
    #[test]
    fn test_a_read_past_a_loop_is_found_from_the_reads_as_the_scan_finds_it() {
        let mut seed = 0x1234_5678_9abc_def1_u64;
        let mut next = move |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        let push = |at: i64, value: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Push, "push", vec![], vec![held(value, 2)], None),
                vec![],
                vec![value],
            ))
        };
        for _ in 0..300 {
            let size = 2 + next(9) as i64;
            let succs: Vec<Vec<i64>> =
                (0..size).map(|_| (0..next(3)).map(|_| next(size as u64) as i64).collect()).collect();
            let blocks: Vec<Block> = (0..size)
                .map(|at| {
                    let insns = if next(3) == 0 { vec![push(at, 7 + next(3) as u32)] } else { Vec::new() };
                    let preds: Vec<i64> = (0..size).filter(|from| succs[*from as usize].contains(&at)).collect();
                    let phis = match preds.get(next(4) as usize) {
                        Some(&from) => vec![Phi { result: 20 + at as u32, incoming: vec![(from, 7 + next(3) as u32)] }],
                        None => Vec::new(),
                    };
                    Block::from(block(at, insns, succs[at as usize].clone(), phis))
                })
                .collect();
            let graph = body("random", blocks);
            let at_of: IndexMap<i64, &Block> = graph.blocks.iter().map(|block| (block.at, block)).collect();
            let flow = super::Flow::of(&graph);
            for from in 0..size {
                for &header in &succs[from as usize] {
                    for value in 7..10 {
                        assert_eq!(
                            super::_read_past_loop(&flow, header, from, value),
                            read_past_loop_by_scan(&at_of, header, from, value),
                            "b{from} to b{header}, v{value} in {succs:?}"
                        );
                    }
                }
            }
        }
    }

    /// A read reachable only through the block the copy goes into is no read:
    /// the answer was found by walking all of the blocks the start reaches (a
    /// chain of any length before it joins `into`); from the reader's end there
    /// is nothing to walk.
    #[test]
    fn test_a_read_only_through_the_block_copied_into_is_no_walk() {
        let push = |at: i64, value: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Push, "push", vec![], vec![held(value, 2)], None),
                vec![],
                vec![value],
            ))
        };
        let into = 100_000;
        let chain = 2000;
        let mut blocks: Vec<Block> = vec![block(0, vec![], vec![into, 1], vec![]).into()];
        blocks.extend(
            (1..=chain).map(|at| Block::from(block(at, vec![], vec![if at == chain { into } else { at + 1 }], vec![]))),
        );
        blocks.push(block(into, vec![push(into, 7)], vec![], vec![]).into());
        let graph = body("chain", blocks);
        let at_of: IndexMap<i64, &Block> = graph.blocks.iter().map(|block| (block.at, block)).collect();
        let reaches = Reaches::new(&graph, &at_of);
        let before = super::walked();
        assert!(!_observed(&reaches, &at_of, 0, into, 7));
        assert_eq!(super::walked(), before, "the chain was walked");
    }

    /// The walk visited every block a start reaches before it said no: a read
    /// that only a path through `into` reaches, or none, cost the whole
    /// region (nest of 128 loops: 224 M instructions for 1,516 asks). Searched
    /// from both ends it is the smaller region, and the answer is the same on
    /// every graph.
    #[test]
    fn test_a_read_beyond_a_block_is_found_from_either_end_as_the_walk_finds_it() {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |bound: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % bound
        };
        let push = |at: i64, value: u32| {
            Arc::new(Insn::new(
                at,
                Some((at, at + 1)),
                what(Operation::Push, "push", vec![], vec![held(value, 2)], None),
                vec![],
                vec![value],
            ))
        };
        for _ in 0..300 {
            let size = 2 + next(9) as i64;
            let succs: Vec<Vec<i64>> =
                (0..size).map(|_| (0..next(3)).map(|_| next(size as u64) as i64).collect()).collect();
            let blocks: Vec<Block> = (0..size)
                .map(|at| {
                    let insns = if next(3) == 0 { vec![push(at, 7 + next(3) as u32)] } else { Vec::new() };
                    // A phi reads on an edge there is.
                    let preds: Vec<i64> = (0..size).filter(|from| succs[*from as usize].contains(&at)).collect();
                    let phis = match preds.get(next(4) as usize) {
                        Some(&from) => vec![Phi { result: 20 + at as u32, incoming: vec![(from, 7 + next(3) as u32)] }],
                        None => Vec::new(),
                    };
                    Block::from(block(at, insns, succs[at as usize].clone(), phis))
                })
                .collect();
            let graph = body("random", blocks);
            let at_of: IndexMap<i64, &Block> = graph.blocks.iter().map(|block| (block.at, block)).collect();
            let reaches = Reaches::new(&graph, &at_of);
            for block in &graph.blocks {
                for &into in &block.succ {
                    for value in 7..10 {
                        assert_eq!(
                            _observed(&reaches, &at_of, block.at, into, value),
                            walked(&at_of, block.at, into, value),
                            "b{} to b{into}, v{value} in {:?}",
                            block.at,
                            graph
                                .blocks
                                .iter()
                                .map(|one| (
                                    one.at,
                                    one.succ.to_vec(),
                                    one.insns.iter().map(|i| i.uses.clone()).collect::<Vec<_>>(),
                                    one.phis.iter().map(|p| p.incoming.clone()).collect::<Vec<_>>()
                                ))
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_phi_observation_on_an_immediate_alternate_edge_is_visible() {
        // nbody's latch passed an accumulator to phis on both of its edges.
        let alternate = body(
            "alternate-phi",
            vec![
                block(0, vec![], vec![1, 2], vec![]).into(),
                block(1, vec![], vec![], vec![Phi { result: 3, incoming: vec![(0, 7)] }]).into(),
                block(2, vec![], vec![], vec![]).into(),
            ],
        );
        let at_of: IndexMap<i64, &Block> = alternate.blocks.iter().map(|block| (block.at, block)).collect();
        assert!(_observed(&Reaches::new(&alternate, &at_of), &at_of, 0, 2, 7));
    }

    #[test]
    fn test_phi_elimination_renames_a_value_a_cell_is_reached_by() {
        // Port of tests/test_lir.py: `_settled` looked for a Held in
        // `Mem.through`, which is a register now, so a based cell kept the old
        // id.
        let where_ = Addr { base: (RegId::SI).iced(), ..Addr::new(Space::Segment, 0x10) };
        let cell = Mem {
            through: RegId::None,
            offset: 0,
            disp_width: 2,
            base: Some(Held { value: 21, width: 2 }),
            ..Mem::new(Some(where_), 2)
        };
        let Loc::Mem(got) = _settled(&Loc::Mem(cell.clone()), &IndexMap::from_iter([(21, 99)])).unwrap() else {
            panic!("a cell stays a cell");
        };
        assert_eq!(got.base, Some(Held { value: 99, width: 2 }), "the cell kept {:?}", got.base);
        assert_eq!(got.through, RegId::None, "the rename placed it");
        assert_eq!(got.addr, cell.addr);
        assert_eq!(got.width, cell.width);
        assert_eq!(got.offset, cell.offset);
        assert_eq!(got.disp_width, cell.disp_width);
    }

    /// A build for size kept a loop's guard-exit constants on an edge of their
    /// own (a block, a `jmp` and the copies twice: 4 bytes in 25 of 67 programs
    /// once the guard's odds moved); only a build for speed prices an edge
    /// copy.
    #[test]
    fn test_a_build_for_size_keeps_its_phi_copies_in_the_predecessor() {
        use super::EdgePrices;
        use crate::backend::cpu;
        assert!(EdgePrices::of(cpu::tuned("486", false).unwrap()).is_some());
        assert!(EdgePrices::of(cpu::tuned("486", true).unwrap()).is_none());
    }

    /// An edge taken 24% of the time won by 0.06 clocks of 1 (`24% * (1 + 3) <
    /// 1`): x_binsearch grew a block and a `jmp` for clocks the model could not
    /// tell, and 25 of the 67 programs +1% in code once the guards' odds moved
    /// to it.
    #[test]
    fn test_an_edge_that_wins_by_less_than_a_tenth_keeps_its_copy_in_the_predecessor() {
        use super::EdgePrices;
        let prices = EdgePrices { copy: 1.0, jump: 3.0 };
        assert!(!prices.cheaper_on_edge(0.24, 1));
        assert!(prices.cheaper_on_edge(0.1, 1));
        assert!(prices.cheaper_on_edge(0.33, 2));
    }
}
