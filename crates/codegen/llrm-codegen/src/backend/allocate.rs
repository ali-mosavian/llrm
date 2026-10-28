//! Assigns a register to every value of a lowered body, splitting and
//! spilling the values that do not fit.
//!
//! # How it works
//! The allocator is greedy with eviction, after LLVM's `RegAllocGreedy`.
//! Values are taken from a priority queue one at a time and each is given a
//! register, or is moved, split or spilled. There is no interference graph:
//! two values clash when their live intervals overlap while in the same
//! register.
//!
//! ## Inputs
//! The body is machine-level LIR after `phielim`, `twoaddr` and `coalesce`,
//! over virtual values with widths. Besides it the allocator reads:
//!
//! - pins: values already bound to a register (arguments, results, what an
//!   instruction requires, see `constrain`);
//! - the register classes (`classes`, `regclass`): which registers a value may
//!   take, from how its operands use it;
//! - the CPU profile, block frequencies and the frame (spill slots).
//!
//! ## What stands for interference
//! Three facts are kept for the body and recomputed incrementally whenever a
//! rewrite changes it (`Facts`):
//!
//! ```text
//!  live intervals     value -> segments of instruction slots, + spill weight
//!  Masks              points that destroy a register without naming it:
//!                     calls (clobbers), 16-bit callees (clobbers_high:
//!                     the upper half only), points that need DS (the
//!                     data group)
//! ```
//!
//! After a rewrite the values that compete again are those it changed, those it
//! brought to life that no one has queued, and those the queue gave up on and
//! has not got back (`popped`): the others are placed, spilled or waiting, so
//! the live values are not all looked at (`LLRM_CHECK_IDLE` compares with the
//! scan of all of them).
//!
//! A value may use a register when no value placed there overlaps its
//! interval (`LiveIntervalUnion`) and no mask point lies inside it
//! (`_clobbered`). Registers are compared by root, so `AX` and `EAX` are one
//! register; a mask on the upper half matters only to values wider than a
//! word. The weight is the references to the value, weighted by block
//! frequency (10 per loop level) and divided by its size plus a grace, less
//! what its copies and folds would save; a value that must not be spilled
//! (`unspillable`, `protected`) weighs infinity.
//!
//! ## Preparing the body
//! `RegAlloc::transform` first changes the body so that the greedy loop can
//! treat every value alike, in this order:
//!
//! 1. `constrain::distinct_classes` copies a value that needs two incompatible
//!    register sets at once (both a byte and an index) into one value per role;
//!    isel already gives a word that is both base and index of one address two
//!    values.
//! 2. `explicit_selectors` reaches a far cell through ES where its selector is
//!    also read as a number.
//! 3. A pin outside the value's class is dropped where an instruction delivers
//!    the value in that register, else it is an error; a value both pinned and
//!    required in two registers is an error.
//! 4. `constrain::constrained` gives each register an instruction requires a
//!    value of its own, live across that one instruction and copied in and out,
//!    and returns those pins; `required` adds the rest.
//! 5. A value read once from a cell that holds, by an instruction that takes
//!    the cell, is not held at all: the read stays a memory operand.
//!
//! `unallocatable` then refuses a body no assignment can fit: a value no
//! register may hold, two pinned values live together in one register, or
//! more unspillable values live at a point than there are registers.
//!
//! ## The loop
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef src fill:#2563eb,stroke:#1e40af,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef out fill:#16a34a,stroke:#166534,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef data fill:#7c3aed,stroke:#5b21b6,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   classDef warn fill:#dc2626,stroke:#991b1b,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   pop["<b>pop a value v, stage S</b><br/><small>Assign, Split or Spill</small>"]:::src
//!   order["<b>register order</b><br/><small>copy-partner votes first#59;<br/>neighbour-wanted later#59;<br/>DS last</small>"]:::mid
//!   free(["<b>free register?</b><br/><small>not busy, not clobbered</small>"]):::mid
//!   evict["<b>S = Assign or unspillable</b><br/><small>evict cheapest register<br/>holders' weight below v's<br/>(0 if a holder can move)#59;<br/>holders requeue at Assign</small>"]:::mid
//!   split["<b>S = Split, v not a piece</b><br/><small>split v (splitkit): one<br/>block, by region over edge<br/>bundles, else per block#59;<br/>kept only if it pays</small>"]:::mid
//!   recolor["<b>recolor</b><br/><small>move v's holders#59; depth 5,<br/>under 8 holders, 256 tries</small>"]:::mid
//!   force["<b>v pinned or unspillable</b><br/><small>evict by force#59;<br/>victims go to Spill</small>"]:::mid
//!   spill["<b>spill v and siblings</b><br/><small>sharing its slot (spiller)<br/>stores after defs, reloads<br/>before uses, unspillable</small>"]:::data
//!   placed["<b>place, Done</b>"]:::out
//!   changed["<b>body changed</b><br/><small>requeue</small>"]:::warn
//!   pop -->|"queue: fixed, narrowest<br/>class, past Assign, longest"| order
//!   order --> free
//!   free -->|"yes"| placed
//!   free -->|"no"| evict
//!   evict --> placed
//!   evict -->|"none: S := Split"| split
//!   split -->|"pieces requeue at Assign,<br/>the rest at Spill"| changed
//!   split -->|"else"| recolor
//!   recolor --> placed
//!   recolor -->|"else"| force
//!   force --> placed
//!   force -->|"else"| spill
//!   spill --> changed
//! ```
//!
//! *One value through the allocator loop*
//!
//! An eviction records a cascade number so a value evicted by one that was
//! itself evicted cannot ping-pong. A value that cannot be spilled may break
//! the cascade of a spillable one.
//!
//! When a split or a spill changes the body, the facts are recomputed from
//! what changed, pins are found again for the changed blocks, the union is
//! refreshed, any placed value that now overlaps another or has a clobber in
//! its range is unplaced, and every new, moved or unplaced value re-enters
//! the queue. The loop stops when the queue is empty (`BUDGET` visits at
//! most, 200000).
//!
//! The last chance before a value is spilled is recoloring
//! (`greedy::greedy`'s `try_last_chance_recoloring`).
//!
//! Last, `_recolored_hints` moves a value into a copy partner's register when
//! that register is free over its whole interval and joins more copies than
//! the one it has, so each joined copy becomes a deleted move.
//!
//! ## Applying
//! `applied` replaces every operand's value by its register at its width
//! (selector and address registers included), deletes identity moves and
//! dead allocator copies, and refuses a body with a spill left. `datagroup`
//! then restores DS where a value held it.
//!
//! # Example
//! A dword multiply by 5 (`Times5& = x * 5`), before and after. `; inserted`
//! notes are dropped. `v4` and `v5` are pinned to EAX and EDX (`requires`) and
//! everything else is free.
//!
//! Before (`Coalescer`):
//!
//! ```lir
//! 0x0000  v1 := mov [[bp+0x6]]
//! 0x0000  v2 := mov [None v1@unplaced]
//! 0x0001  v4 := mov v2
//! 0x0001  v4 := shl v4, 0x2
//! 0x0001  v4 := add v4, v2
//! 0x0002  v5 := mov v4
//! 0x0002  v5 := shr v5, 0x10
//! 0x0002     ; requires v4@EAX,v5@EDX
//! ```
//!
//! After (`RegAlloc`):
//!
//! ```lir
//! 0x0000  BX := mov [[bp+0x6]]
//! 0x0000  EBX := mov [None v1@BX]
//! 0x0001  EAX := mov EBX
//! 0x0001  EAX := shl EAX, 0x2
//! 0x0001  EAX := add EAX, EBX
//! 0x0002  EDX := mov EAX
//! 0x0002  EDX := shr EDX, 0x10
//! 0x0002     ; ...
//! 0x0002     ; requires v6@EAX,v7@EDX
//! ```
//!
//! Every value is a register; the pinned values are fresh ones (`v6`, `v7`),
//! see `constrain`.
//!
//! When registers run out, values get frame cells. Eight LONG parameters
//! multiplied in pairs (`Mix&`), the first lines, before (`Coalescer`):
//!
//! ```lir
//! 0x0000  v1 := mov [[bp+0x14]]
//! 0x0000  v9 := mov [None v1@unplaced]
//! 0x0001  v2 := mov [[bp+0x12]]
//! 0x0001  v10 := mov [None v2@unplaced]
//! 0x0002  v11 := mov v9
//! 0x0002  v11 := imul v11, v10
//! ; ...
//! ```
//!
//! After (`RegAlloc`): `v9` and `v11` are stored to frame cells, and `v11` is
//! read back before its update (`spill reload`):
//!
//! ```lir
//! 0x0000  BX := mov [[bp+0x14]]
//! 0x0000  ECX := mov [None v1@BX]
//! 0x0001  BX := mov [[bp+0x12]]
//! 0x0001  EAX := mov [None v2@BX]
//! 0x0002  EBX := mov ECX
//! 0x0002  [[bp-0x18]] := mov EBX
//! 0x0002     ; ...
//! 0x0002  [[bp-0x4]] := mov ECX
//! 0x0002  EBX := mov [[bp-0x18]]  ; spill reload
//! 0x0002  EBX := imul EBX, EAX
//! 0x0002  [[bp-0x18]] := mov EBX
//! ; ...
//! ```
//!
//! From `ISEL_DUMP=1 llrm-qb -O2 -march=i386` of `Times5&` and of `Mix&`, first
//! route.
//!
//! # Runs
//! Once per function and route in the machine phases (`regalloc`), after
//! `coalesce` and before `parcopy`, at every -O level; `search` only widens
//! the candidates tried. `assemble.rs` runs the whole machine list twice per
//! function (allocator alone, and with `ssaspill`) and keeps the cheaper.
//! The file also holds the per-block liveness rows (`live`, `live_rows*`)
//! that other phases read.
//!
//! # Needs
//! [`crate::backend::live`],
//! [`crate::backend::regclass`], [`crate::backend::constrain`],
//! [`crate::backend::splitkit`], [`crate::backend::spillplacement`],
//! [`crate::backend::spiller`], [`crate::backend::datagroup`] and
//! [`crate::backend::frame`].
//!
//! # See also
//! - LLVM's `RegAllocGreedy`:
//!   <https://github.com/llvm/llvm-project/blob/llvmorg-18.1.8/llvm/lib/CodeGen/RegAllocGreedy.cpp>
//! - The phase list: [`crate::backend`].

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::analysis::frequency::Frequency;
use crate::analysis::intervals::{self as ranges, Indexes, Interval, key};
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::frame::{self as frames, Frame, Refused};
use crate::backend::live::LiveRanges;
use crate::backend::target::{self, Segments};
use crate::backend::{constrain, datagroup};
use crate::model::ir::{self, Addr, Held, Loc, Mem, Operation, Semantics, Space};
use crate::model::lir::{self, Insn, LirBlock, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::hash::{IndexMap, IndexSet};

/// Most queue visits an allocation may take: more means it does not converge, a
/// bug.
pub const BUDGET: usize = 200_000;

/// A value has no register, or no register it may take is free of values that
/// cannot be spilled: the body asks for more registers at one point than the
/// machine has, or names a register no value can be in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unplaced(pub String);

/// A value the allocator chose to spill, and nothing writes the spill.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spilled(pub String);

impl fmt::Display for Unplaced {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Display for Spilled {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Unplaced {}
impl std::error::Error for Spilled {}

/// Every exception that may leave the register-allocation modules.
///
/// Python raises these through one another's frames; Rust says which one
/// it was so `except Unplaced` can catch exactly that one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Unplaced(Unplaced),
    Spilled(Spilled),
    Refused(Refused),
    /// `ValueError`, or a call into a module not yet ported.
    Value(String),
}

impl fmt::Display for Error {
    fn fmt(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Unplaced(one) => one.fmt(formatter),
            Self::Spilled(one) => one.fmt(formatter),
            Self::Refused(one) => one.fmt(formatter),
            Self::Value(one) => formatter.write_str(one),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Python's class name and message, for a caller that catches by class.
    pub fn raised(&self) -> Exception {
        let (module, kind) = match self {
            Self::Unplaced(_) => ("qbopt.backend.allocate", "Unplaced"),
            Self::Spilled(_) => ("qbopt.backend.allocate", "Spilled"),
            Self::Refused(_) => ("qbopt.backend.frame", "Refused"),
            Self::Value(_) => ("builtins", "ValueError"),
        };
        Exception::defined_in(module, kind, self.to_string())
    }
}

impl From<Unplaced> for Error {
    fn from(one: Unplaced) -> Self {
        Self::Unplaced(one)
    }
}

impl From<Spilled> for Error {
    fn from(one: Spilled) -> Self {
        Self::Spilled(one)
    }
}

impl From<Refused> for Error {
    fn from(one: Refused) -> Self {
        Self::Refused(one)
    }
}

/// Where each value lives, what it cost, and whether that is the best.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub r#where: IndexMap<u32, RegId>,
    pub spilled: BTreeSet<u32>,
    pub cost: f64,
    pub optimal: bool,
    pub why: String,
}

/// The `group` run ending at `index`: where it starts.
pub fn _group_start(
    block: &LirBlock,
    index: usize,
) -> usize {
    let one = &block.insns[index];
    let mut first = index;
    if one.group.is_some() {
        while first > 0 && block.insns[first - 1].group == one.group {
            first -= 1;
        }
    }
    first
}

pub type Live = IndexMap<i64, llrm_mir::dense::IdSet<u32>>;

/// What is live at each block's entry and exit, to a fixed point.
///
/// Dense: the values numbered by their order, each block's sets one row of bits
/// in one array, the fixed point a worklist over the rows. No set is built but
/// the answer.
pub fn live(body: &LirBody) -> (Live, Live) {
    let rows = live_rows(body);
    (rows.sets(true), rows.sets(false))
}

/// `live`'s exit sets alone: one pass over the rows, not two.
pub fn live_out(body: &LirBody) -> Live {
    live_rows(body).sets(false)
}

/// `live` for the values `keep` names only: no other value is numbered,
/// followed or put in a set.
pub fn live_among(
    body: &LirBody,
    keep: impl Fn(u32) -> bool,
) -> (Live, Live) {
    let rows = live_rows_by(body, keep);
    (rows.sets(true), rows.sets(false))
}

/// What is live at each block's entry and exit as the fixed point leaves it:
/// rows of bits, which a reader that only walks the values need not turn into
/// sets.
pub struct LiveRows(Found);

// Times a body's liveness was turned into a set of values for each block.
#[cfg(test)]
thread_local! {
    pub(crate) static SETS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

enum Found {
    /// Rows of bits, one for each block: the fixed point over every value.
    Dense(DenseRows),
    /// The values each block has live, found from where the values occur.
    Web(WebRows),
}

struct DenseRows {
    numbered: Vec<u32>,
    words: usize,
    position: IndexMap<i64, usize>,
    blocks: Vec<i64>,
    into: Vec<u64>,
    out: Vec<u64>,
}

impl LiveRows {
    /// How many values the rows number.
    pub fn numbered(&self) -> usize {
        match &self.0 {
            Found::Dense(dense) => dense.numbered.len(),
            Found::Web(web) => web.numbered(),
        }
    }

    /// The values live at the entry of the block at `at`, in order.
    pub fn entering(
        &self,
        at: i64,
    ) -> Box<dyn Iterator<Item = u32> + '_> {
        match &self.0 {
            Found::Dense(dense) => Box::new(dense.values(&dense.into, dense.position[&at])),
            Found::Web(web) => Box::new(web.entering(at)),
        }
    }

    /// The values live at the exit of the block at `at`, in order.
    pub fn leaving(
        &self,
        at: i64,
    ) -> Box<dyn Iterator<Item = u32> + '_> {
        match &self.0 {
            Found::Dense(dense) => Box::new(dense.values(&dense.out, dense.position[&at])),
            Found::Web(web) => Box::new(web.leaving(at)),
        }
    }

    /// Each block's entry (`entering`) or exit values, as sets, in the order of
    /// the blocks.
    fn sets(
        &self,
        entering: bool,
    ) -> Live {
        #[cfg(test)]
        SETS_BUILT.with(|one| one.set(one.get() + 1));
        match &self.0 {
            Found::Dense(dense) => dense.sets(if entering { &dense.into } else { &dense.out }),
            Found::Web(web) => web
                .graph
                .position
                .keys()
                .map(|&block| {
                    (block, if entering { web.entering(block).collect() } else { web.leaving(block).collect() })
                })
                .collect(),
        }
    }
}

impl DenseRows {
    fn holds(
        &self,
        rows: &[u64],
        block: i64,
        value: u32,
    ) -> bool {
        let (Some(&at), Ok(bit)) = (self.position.get(&block), self.numbered.binary_search(&value)) else {
            return false;
        };
        rows[at * self.words + bit / 64] >> (bit % 64) & 1 == 1
    }

    fn values<'a>(
        &'a self,
        rows: &'a [u64],
        block: usize,
    ) -> impl Iterator<Item = u32> + 'a {
        rows[block * self.words..(block + 1) * self.words].iter().enumerate().flat_map(move |(word, &bits)| {
            std::iter::successors((bits != 0).then_some(bits), |left| Some(left & (left - 1)).filter(|rest| *rest != 0))
                .map(move |left| self.numbered[word * 64 + left.trailing_zeros() as usize])
        })
    }

    fn sets(
        &self,
        rows: &[u64],
    ) -> Live {
        self.blocks.iter().enumerate().map(|(at, block)| (*block, self.values(rows, at).collect())).collect()
    }
}

/// How many times this body's facts have walked every instruction for liveness
/// rows (`live_rows_by`), for a test that a web of a few values does not.
pub fn live_rows_walks(body: &LirBody) -> usize {
    body.facts.0.counted("live-rows-walks")
}

/// What is live at each block's entry and exit for a few values, found from
/// where they occur: a value is live into a block that reads it before writing
/// it and out of a block a successor has it live into, up the predecessors
/// until one writes it. The rows of `values` as `live_rows_by` finds them in a
/// body with no phis, at the cost of the occurrences and the blocks each is
/// live in, not of every instruction of the body.
pub struct WebRows {
    graph: Arc<crate::analysis::graph::Graph>,
    into: Vec<Vec<u32>>,
    out: Vec<Vec<u32>>,
    numbered: usize,
}

impl LiveAt for WebRows {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.graph.position.get(&block).is_some_and(|at| self.into[*at].binary_search(&value).is_ok())
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.graph.position.get(&block).is_some_and(|at| self.out[*at].binary_search(&value).is_ok())
    }
}

impl WebRows {
    pub fn numbered(&self) -> usize {
        self.numbered
    }

    pub fn entering(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.into[self.graph.position[&at]].iter().copied()
    }

    pub fn leaving(
        &self,
        at: i64,
    ) -> impl Iterator<Item = u32> + '_ {
        self.out[self.graph.position[&at]].iter().copied()
    }
}

/// `WebRows` of `values` (ascending); `places[value]` are the instructions that
/// name it, by block then position, once each, and whether each defines and
/// reads it.
pub fn live_rows_among(
    body: &LirBody,
    values: &[u32],
    places: &IndexMap<u32, Vec<ranges::Occurrence>>,
) -> WebRows {
    live_rows_within(body, values, places, &Phis::default(), usize::MAX).expect("no budget to run out of")
}

/// Where the phis of a body define and read the values asked of: the blocks (by
/// position) that define each as a phi's result, and the predecessors (by
/// position) that hand it to a phi, at their ends.
#[derive(Default)]
pub struct Phis {
    defined: IndexMap<u32, Vec<usize>>,
    handed: IndexMap<u32, Vec<usize>>,
}

impl Phis {
    fn of(
        body: &LirBody,
        keep: &impl Fn(u32) -> bool,
    ) -> Self {
        let graph = crate::analysis::graph::Graph::of(body);
        let mut found = Self::default();
        for (at, block) in body.blocks.iter().enumerate() {
            for phi in &block.phis {
                if keep(phi.result) {
                    found.defined.entry(phi.result).or_default().push(at);
                }
                for (from, value) in &phi.incoming {
                    // A predecessor the body does not hold hands nothing.
                    if let Some(&from) = graph.position.get(from).filter(|_| keep(*value)) {
                        found.handed.entry(*value).or_default().push(from);
                    }
                }
            }
        }
        found
    }
}

/// `live_rows_among`, given up (None) once the values have been followed
/// through more than `budget` blocks in all: where the values are live in most
/// blocks the rows of bits are the cheaper way.
pub fn live_rows_within(
    body: &LirBody,
    values: &[u32],
    places: &IndexMap<u32, Vec<ranges::Occurrence>>,
    phis: &Phis,
    budget: usize,
) -> Option<WebRows> {
    let mut spent = 0usize;
    let count = body.blocks.len();
    let graph = crate::analysis::graph::Graph::of(body);
    let predecessors = &graph.parents;
    let (mut in_mark, mut out_mark, mut written_mark, mut run_mark) =
        (vec![0u32; count], vec![0u32; count], vec![0u32; count], vec![0u32; count]);
    // The first run of a block that names the value, as (defined, read): a read
    // decides, a write beside it too.
    let mut first_run: Vec<(usize, bool, bool)> = vec![(0, false, false); count];
    let mut into: Vec<Vec<u32>> = vec![Vec::new(); count];
    let mut out: Vec<Vec<u32>> = vec![Vec::new(); count];
    let mut turn = 0u32;
    let mut numbered = 0;
    for &value in values {
        let found = places.get(&value).map(Vec::as_slice).unwrap_or_default();
        let (defined_by, handed_by) = (
            phis.defined.get(&value).map(Vec::as_slice).unwrap_or_default(),
            phis.handed.get(&value).map(Vec::as_slice).unwrap_or_default(),
        );
        if found.is_empty() && defined_by.is_empty() && handed_by.is_empty() {
            continue;
        }
        numbered += 1;
        turn += 1;
        let mut reached: Vec<usize> = Vec::new();
        let mut work: Vec<usize> = Vec::new();
        for &((block_index, at), defined, used) in found {
            let end = ranges::_group_end(&body.blocks[block_index], at);
            if run_mark[block_index] != turn {
                run_mark[block_index] = turn;
                first_run[block_index] = (end, defined, used);
                reached.push(block_index);
            } else if first_run[block_index].0 == end {
                first_run[block_index].1 |= defined;
                first_run[block_index].2 |= used;
            }
            if defined {
                written_mark[block_index] = turn;
            }
        }
        // A phi's result is written before the block's first instruction.
        for &block_index in defined_by {
            if run_mark[block_index] != turn {
                run_mark[block_index] = turn;
                reached.push(block_index);
            }
            first_run[block_index] = (0, true, false);
            written_mark[block_index] = turn;
        }
        for &block_index in &reached {
            if first_run[block_index].2 {
                in_mark[block_index] = turn;
                work.push(block_index);
            }
        }
        let mut live_out: Vec<usize> = Vec::new();
        // A phi's argument is read at the end of the predecessor it comes from,
        // and live into it unless that block writes the value first.
        for &from in handed_by {
            if out_mark[from] != turn {
                out_mark[from] = turn;
                live_out.push(from);
            }
            if run_mark[from] != turn && in_mark[from] != turn {
                in_mark[from] = turn;
                reached.push(from);
                work.push(from);
            }
        }
        while let Some(block_index) = work.pop() {
            for &before in &predecessors[block_index] {
                if out_mark[before] != turn {
                    out_mark[before] = turn;
                    live_out.push(before);
                    if written_mark[before] != turn && in_mark[before] != turn {
                        in_mark[before] = turn;
                        reached.push(before);
                        work.push(before);
                    }
                }
            }
        }
        spent += reached.len() + live_out.len();
        if spent > budget {
            return None;
        }
        for &block_index in &reached {
            if in_mark[block_index] == turn {
                into[block_index].push(value);
            }
        }
        for &block_index in &live_out {
            out[block_index].push(value);
        }
    }
    Some(WebRows { graph, into, out, numbered })
}

/// Whether a value is live at the entry or exit of a block: what a caller that
/// asks of one value at a time reads, whether it has the sets of every block
/// (`Live`) or the rows.
pub trait LiveAt {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool;
    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool;
}

impl LiveAt for LiveRows {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        match &self.0 {
            Found::Dense(dense) => dense.holds(&dense.into, block, value),
            Found::Web(web) => web.live_in(block, value),
        }
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        match &self.0 {
            Found::Dense(dense) => dense.holds(&dense.out, block, value),
            Found::Web(web) => web.live_out(block, value),
        }
    }
}

/// Where a value is live, from its segments: live into a block when a segment
/// covers the block's first slot, out of it when one reaches its end (a use at
/// the last instruction ends a slot before it). The facts the allocator keeps
/// already say it, so a split asks them and no rows are made of the body.
pub struct IntervalLive<'a> {
    pub live: &'a LiveRanges,
    pub index: &'a Indexes,
}

impl LiveAt for IntervalLive<'_> {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        let first = self.index.span[&block].0;
        self.live
            .get(&value)
            .is_some_and(
                |one| {
                    let at = one.segments.partition_point(|seg| seg.start <= first);
                    at > 0 && one.segments[at - 1].end > first
                },
            )
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        let last = self.index.span[&block].1;
        self.live
            .get(&value)
            .is_some_and(
                |one| {
                    let at = one.segments.partition_point(|seg| seg.start < last);
                    at > 0 && one.segments[at - 1].end >= last
                },
            )
    }
}

impl LiveAt for (&Live, &Live) {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.0.get(&block).is_some_and(|live| live.contains(&value))
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.1.get(&block).is_some_and(|live| live.contains(&value))
    }
}

/// `live`, as rows.
pub fn live_rows(body: &LirBody) -> LiveRows {
    live_rows_by(body, |_| true)
}

/// Blocks below which the rows are a few words each and are always the way.
const WEB_BLOCKS: usize = 256;

/// Words in the rows of bits (blocks times the words of the largest value
/// number) from which following values may beat them: below it solving the
/// rows costs less than finding where the values occur (d_faces' largest
/// body has 9,000).
const WEB_ROWS: usize = 65_536;

/// `live_rows` of the values `keep` says only: each is live where it is as in
/// the whole, the others are not numbered, so a caller that asks of a few
/// values pays for rows of those.
pub fn live_rows_by(
    body: &LirBody,
    keep: impl Fn(u32) -> bool,
) -> LiveRows {
    llrm_mir::counts::bump(llrm_mir::counts::Event::LiveRows);
    // Each value is followed from where it occurs up the predecessors to where
    // it is written, so the work is the size of the live ranges and not the
    // blocks times the values (gcc's `calculate_live_on_exit`, LLVM's
    // `LiveVariables`). A body with phis is solved over the rows.
    // Under the check every body is followed, so that the walk is held to the
    // rows on all of them.
    let checking = llrm_support::env_flag!("LLRM_CHECK_LIVE");
    let mut numbered = None;
    if checking || body.blocks.len() >= WEB_BLOCKS {
        let values = numbered_by(body, &keep);
        let rows = body.blocks.len() * (values.len() / 64 + 1);
        if checking || rows >= WEB_ROWS {
            body.facts.0.bump("live-rows-walks");
            let found = crate::analysis::occurrences::Occurrences::scan(body, &keep);
            let places = found.occurrences();
            // Following the values costs a step for each block a value is live
            // in; where that comes to more than the rows, the rows
            // are the cheaper way, and the walk is given up at the budget.
            // (Estimated from where the values first and last occur, it took a
            // value whose uses lie far apart in the layout for one
            // live in the blocks between: 2.4 M steps against 0.04 M on
            // branches(1024) with the header copy, which then solved the rows,
            // 3,457 blocks times 190 words.)
            let web = if checking {
                Some(
                    live_rows_within(body, &values, &places, &Phis::of(body, &keep), usize::MAX)
                        .expect("no budget to run out of"),
                )
            } else {
                live_rows_within(body, &values, &places, &Phis::of(body, &keep), rows)
            };
            if let Some(web) = web {
                let found = LiveRows(Found::Web(web));
                if llrm_support::env_flag!("LLRM_CHECK_LIVE") {
                    let walk = live_rows_dense(body, &keep);
                    for block in &body.blocks {
                        assert!(
                            found.entering(block.at).eq(walk.entering(block.at))
                                && found.leaving(block.at).eq(walk.leaving(block.at))
                                && found.numbered() == walk.numbered(),
                            "{}: the liveness found from the occurrences differs from the rows in block {:#x}",
                            body.name,
                            block.at
                        );
                    }
                }
                return found;
            }
        }
        numbered = Some(values);
    }
    if let Some(numbered) = numbered {
        return live_rows_numbered(body, keep, numbered);
    }
    live_rows_dense(body, keep)
}

/// `live_rows_by` solved over rows of bits for every block: the oracle for the
/// walk from the occurrences, and what a body with phis is solved by.
pub fn live_rows_dense(
    body: &LirBody,
    keep: impl Fn(u32) -> bool,
) -> LiveRows {
    let numbered = numbered_by(body, &keep);
    live_rows_numbered(body, keep, numbered)
}

/// Every value `keep` says that the body names, ascending.
fn numbered_by(
    body: &LirBody,
    keep: &impl Fn(u32) -> bool,
) -> Vec<u32> {
    let mut numbered: Vec<u32> = Vec::new();
    for block in &body.blocks {
        numbered.extend(
            block
                .phis
                .iter()
                .flat_map(|phi| std::iter::once(phi.result).chain(phi.incoming.iter().map(|(_, value)| *value)))
                .filter(|value| keep(*value)),
        );
        for one in &block.insns {
            numbered.extend(one.defines.iter().chain(&one.uses).copied().filter(|value| keep(*value)));
        }
    }
    numbered.sort_unstable();
    numbered.dedup();
    numbered
}

/// `live_rows_dense` of the values `numbered`, which `keep` says.
fn live_rows_numbered(
    body: &LirBody,
    keep: impl Fn(u32) -> bool,
    numbered: Vec<u32>,
) -> LiveRows {
    body.facts.0.bump("live-rows-walks");
    // Every value the body names, numbered by order. Ids can be far apart, so
    // the number of a value is found by a table over the ids where they are
    // dense enough, else by search.
    let largest = numbered.last().copied().unwrap_or(0) as usize;
    let table: Option<Vec<u32>> = (largest <= 8 * numbered.len() + 64).then(|| {
        let mut table = vec![u32::MAX; largest + 1];
        for (at, value) in numbered.iter().enumerate() {
            table[*value as usize] = at as u32;
        }
        table
    });
    let number = |value: u32| -> Option<usize> {
        if !keep(value) {
            return None;
        }
        Some(match &table {
            Some(table) => table[value as usize] as usize,
            None => numbered.binary_search(&value).expect("every value is numbered"),
        })
    };
    let words = numbered.len() / 64 + 1;
    let count = body.blocks.len();
    let set = |row: &mut [u64], value: u32| {
        if let Some(at) = number(value) {
            row[at / 64] |= 1 << (at % 64);
        }
    };
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    // Rows of `words` each, a block's at `block * words`.
    let mut defined = vec![0u64; count * words];
    let mut generated = vec![0u64; count * words];
    // A phi's argument is read at the end of the predecessor it comes from.
    let mut handed = vec![0u64; count * words];
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, value) in &phi.incoming {
                if let Some(&at) = position.get(from) {
                    set(&mut handed[at * words..(at + 1) * words], *value);
                } else {
                    // A predecessor the body does not hold: its row is none, as
                    // its set was dropped.
                }
            }
        }
    }
    for (at, block) in body.blocks.iter().enumerate() {
        let row = at * words..(at + 1) * words;
        for phi in &block.phis {
            set(&mut defined[row.clone()], phi.result);
        }
        for one in &block.insns {
            for value in &one.defines {
                set(&mut defined[row.clone()], *value);
            }
        }
        let mut alive: Vec<u64> = handed[row.clone()].to_vec();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let first = _group_start(block, index as usize);
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    if let Some(at) = number(*value) {
                        alive[at / 64] &= !(1 << (at % 64));
                    }
                }
            }
            for item in group {
                for value in &item.uses {
                    set(&mut alive, *value);
                }
            }
            index = first as i64 - 1;
        }
        for phi in &block.phis {
            if let Some(at) = number(phi.result) {
                alive[at / 64] &= !(1 << (at % 64));
            }
        }
        generated[row].copy_from_slice(&alive);
    }
    let mut predecessors = vec![Vec::new(); count];
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); count];
    for (at, block) in body.blocks.iter().enumerate() {
        for successor in &block.succ {
            if let Some(&to) = position.get(successor) {
                predecessors[to].push(at);
                successors[at].push(to);
            }
        }
    }
    // The least fixed point of a backward problem: a worklist over the rows.
    let mut into = generated.clone();
    let mut out = vec![0u64; count * words];
    let mut pending: Vec<usize> = (0..count).collect();
    let mut queued = vec![true; count];
    let mut now = vec![0u64; words];
    while let Some(at) = pending.pop() {
        queued[at] = false;
        now.iter_mut().for_each(|word| *word = 0);
        for &from in &successors[at] {
            for (word, bits) in now.iter_mut().zip(&into[from * words..(from + 1) * words]) {
                *word |= bits;
            }
        }
        out[at * words..(at + 1) * words].copy_from_slice(&now);
        let mut changed = false;
        for word in 0..words {
            let entering = generated[at * words + word] | (now[word] & !defined[at * words + word]);
            if entering != into[at * words + word] {
                into[at * words + word] = entering;
                changed = true;
            }
        }
        if changed {
            for &pred in &predecessors[at] {
                if !queued[pred] {
                    queued[pred] = true;
                    pending.push(pred);
                }
            }
        }
    }
    // A phi's argument is read at the end of its predecessor, leaving it.
    for (at, row) in out.chunks_mut(words).enumerate() {
        for (word, bits) in row.iter_mut().zip(&handed[at * words..(at + 1) * words]) {
            *word |= bits;
        }
    }
    LiveRows(Found::Dense(DenseRows {
        numbered,
        words,
        position,
        blocks: body.blocks.iter().map(|block| block.at).collect(),
        into,
        out,
    }))
}

/// `live` as it was written over sorted sets, which the tests hold the dense
/// one to.
#[cfg(test)]
pub fn live_reference(body: &LirBody) -> (Live, Live) {
    let defines: IndexMap<i64, BTreeSet<u32>> = body
        .blocks
        .iter()
        .map(|block| {
            let mut found: BTreeSet<u32> = block.arrives().into_iter().collect();
            found.extend(block.insns.iter().flat_map(|one| one.defines.iter().copied()));
            (block.at, found)
        })
        .collect();
    let mut exposed: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    // A phi's argument is read at the end of the predecessor it comes from.
    let mut handed: IndexMap<i64, BTreeSet<u32>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            for (from, value) in &phi.incoming {
                handed.entry(*from).or_default().insert(*value);
            }
        }
    }
    for block in &body.blocks {
        let mut alive: BTreeSet<u32> = handed.get(&block.at).cloned().unwrap_or_default();
        let mut index = block.insns.len() as i64 - 1;
        while index >= 0 {
            let first = _group_start(block, index as usize);
            let group = &block.insns[first..=index as usize];
            for item in group {
                for value in &item.defines {
                    alive.remove(value);
                }
            }
            for item in group {
                alive.extend(item.uses.iter().copied());
            }
            index = first as i64 - 1;
        }
        let arrives: BTreeSet<u32> = block.arrives().into_iter().collect();
        exposed.insert(block.at, alive.difference(&arrives).copied().collect());
    }

    // The least fixed point of a backward problem, found by a worklist over
    // dense bit sets: the round-robin over sorted sets it replaces reached
    // the same sets. Values numbered densely: ids can be far apart.
    let numbered: Vec<u32> = body
        .blocks
        .iter()
        .flat_map(|block| defines[&block.at].iter().chain(&exposed[&block.at]).copied())
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .collect();
    let number: crate::support::hash::HashMap<u32, usize> =
        numbered.iter().enumerate().map(|(at, value)| (*value, at)).collect();
    let words = numbered.len() / 64 + 1;
    let bits = |values: &BTreeSet<u32>| {
        let mut set = vec![0u64; words];
        for value in values {
            let at = number[value];
            set[at / 64] |= 1 << (at % 64);
        }
        set
    };
    let position: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(at, block)| (block.at, at)).collect();
    let mut predecessors = vec![Vec::new(); body.blocks.len()];
    for (at, block) in body.blocks.iter().enumerate() {
        for successor in &block.succ {
            if let Some(&to) = position.get(successor) {
                predecessors[to].push(at);
            }
        }
    }
    let kept: Vec<Vec<u64>> =
        body.blocks.iter().map(|block| bits(&defines[&block.at]).iter().map(|word| !word).collect()).collect();
    let generated: Vec<Vec<u64>> = body.blocks.iter().map(|block| bits(&exposed[&block.at])).collect();
    let mut into: Vec<Vec<u64>> = generated.clone();
    let mut out: Vec<Vec<u64>> = vec![vec![0u64; words]; body.blocks.len()];
    let mut pending: Vec<usize> = (0..body.blocks.len()).collect();
    let mut queued = vec![true; body.blocks.len()];
    while let Some(at) = pending.pop() {
        queued[at] = false;
        let mut now = vec![0u64; words];
        for successor in &body.blocks[at].succ {
            if let Some(&from) = position.get(successor) {
                for (word, bits) in now.iter_mut().zip(&into[from]) {
                    *word |= bits;
                }
            }
        }
        let entering: Vec<u64> = (0..words).map(|word| generated[at][word] | (now[word] & kept[at][word])).collect();
        out[at] = now;
        if entering != into[at] {
            into[at] = entering;
            for &pred in &predecessors[at] {
                if !queued[pred] {
                    queued[pred] = true;
                    pending.push(pred);
                }
            }
        }
    }
    let numbered = &numbered;
    let values = |set: &[u64]| -> llrm_mir::dense::IdSet<u32> {
        set.iter()
            .enumerate()
            .flat_map(|(word, bits)| {
                (0..64).filter(move |bit| bits >> bit & 1 == 1).map(move |bit| numbered[word * 64 + bit])
            })
            .collect()
    };
    let live_in: Live = body.blocks.iter().zip(&into).map(|(block, set)| (block.at, values(set))).collect();
    let live_out: Live = body
        .blocks
        .iter()
        .zip(&out)
        .map(|(block, set)| {
            let mut leaving = values(set);
            leaving.extend(handed.get(&block.at).into_iter().flatten().copied());
            (block.at, leaving)
        })
        .collect();
    (live_in, live_out)
}

/// How far a range has got, and therefore what may still be tried on it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Stage {
    Assign = 0,
    Split = 1,
    Spill = 2,
    Done = 3,
}

use crate::backend::classes::RegisterClasses;
use crate::backend::regclass::classes;

/// A plain move into one value; whether anything reads it is the caller's.
pub fn _unread_move(one: &Insn) -> bool {
    let Some(what) = &one.what else {
        return false;
    };
    what.op == Operation::Move
        && what.dests.len() == 1
        && what.sources.len() == 1
        && matches!(what.dests[0], Loc::Held(_))
        && matches!(what.sources[0], Loc::Imm(_) | Loc::Held(_))
        && matches!(&what.dests[0], Loc::Held(dest) if one.defines == [dest.value])
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.clobbers.is_empty()
        && one.group.is_none()
        && one.symbol != Some(true)
}

/// Each far cell whose selector is also read as a number, or pinned to a
/// general register, reached through ES.
pub fn explicit_selectors(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    segments: &Segments,
    registers: &RegisterClasses,
) -> LirBody {
    let confined = classes(body, &BTreeSet::new(), segments, registers);
    explicit_selectors_given(body, pinned, segments, &confined).unwrap_or_else(|| body.clone())
}

/// `explicit_selectors`, given the body's classes: None where no cell is
/// rewritten.
pub fn explicit_selectors_given(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    segments: &Segments,
    confined: &crate::backend::regclass::Classes,
) -> Option<LirBody> {
    let regs = body.regs();
    let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
    let empty = IndexMap::default();
    let pinned = pinned.unwrap_or(&empty);
    let mut conflicted: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(what) = &one.what else {
                continue;
            };
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Mem(cell) = place {
                    if let Some(selector) = cell.selector {
                        if confined.get(&selector.value) != Some(&selectors)
                            || !regs.is_segment(*pinned.get(&selector.value).unwrap_or(&regs.far()))
                        {
                            conflicted.insert(selector.value);
                        }
                    }
                }
            }
        }
    }
    if conflicted.is_empty() {
        return None;
    }

    let through_es = |place: &Loc| -> Loc {
        if let Loc::Mem(cell) = place {
            if cell.selector.is_some_and(|selector| conflicted.contains(&selector.value)) {
                let addr = cell.addr.expect("a far cell has an address");
                return Loc::Mem(Mem {
                    selector: None,
                    addr: Some(Addr { segment: regs.far().iced(), ..addr }),
                    ..cell.clone()
                });
            }
        }
        place.clone()
    };

    let mut blocks = Vec::new();
    for block in &body.blocks {
        let mut insns = Vec::new();
        for one in &block.insns {
            let named: Vec<Held> = match &one.what {
                None => Vec::new(),
                Some(what) => what
                    .dests
                    .iter()
                    .chain(&what.sources)
                    .filter_map(|place| match place {
                        Loc::Mem(cell) => cell.selector.filter(|selector| conflicted.contains(&selector.value)),
                        _ => None,
                    })
                    .collect(),
            };
            if named.is_empty() {
                insns.push(Arc::clone(one));
                continue;
            }
            let what = one.what.as_ref().expect("named is empty without semantics");
            let what = Semantics {
                dests: what.dests.iter().map(&through_es).collect(),
                sources: what.sources.iter().map(&through_es).collect(),
                ..what.clone()
            };
            let requires: IndexSet<(Held, RegId)> =
                one.requires.iter().copied().chain(named.iter().map(|held| (*held, regs.far()))).collect();
            let uses: IndexSet<u32> = one.uses.iter().copied().chain(named.iter().map(|held| held.value)).collect();
            let mut made = (**one).clone();
            made.what = Some(what);
            made.requires = requires.into_iter().collect();
            made.uses = uses.into_iter().collect();
            insns.push(Arc::new(made));
        }
        blocks.push(block.with_insns(insns));
    }
    Some(body.with_blocks(blocks))
}

/// How many copies away a fixed register still reaches a value as its hint: the
/// allocator's copy-hint propagation depth, as gcc exposes its limits as
/// `--param`s. A chain of copies longer than this carries no hint further. The
/// corpus (136 files at -O1, -O2, -O3, -Os and -Omax, 141,519 propagations)
/// reaches 74 at the most (crc, an unrolled loop), 17 everywhere else, and 96%
/// stop within 8; 128 is above the most. Without a bound a copy chain of N
/// values is walked end to end after every spill (`cells`, N=448: 221 levels),
/// N rounds of N steps.
pub const HINT_DEPTH: usize = 128;

/// A register for every value, by the greedy allocator, for the tests of the
/// passes that need an assignment of a body that does not spill.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn allocate(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    _preferred: Option<&IndexMap<u32, RegId>>,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
    classes: &RegisterClasses,
) -> Result<Assignment, Error> {
    let mut frame = Frame::new(0);
    Ok(greedy_rewritten(body, pinned, unspillable, protected, cpu, segments, classes, &[], &mut frame)?.0)
}

thread_local! {
    /// How many allocations the translated greedy allocator made, and in how
    /// many it fell back to this one (a range it could not place).
    static GREEDY_RUNS: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}

/// The allocation by the translated greedy allocator (`crate::greedy`): it
/// decides which values to spill and where the rest go, and writes the spill
/// code into the body as it goes (`InlineSpiller`). The body returned holds
/// every value in a register; the set is what went to the stack. An error
/// where the allocator cannot place a range: there is no other allocator.
pub fn greedy_rewritten(
    body: &LirBody,
    pinned: Option<&IndexMap<u32, RegId>>,
    unspillable: Option<&BTreeSet<u32>>,
    protected: Option<&BTreeSet<u32>>,
    cpu: ProfileOrName<'_>,
    segments: &Segments,
    classes: &RegisterClasses,
    saved: &[RegId],
    frame: &mut Frame,
) -> Result<(Assignment, LirBody, BTreeSet<u32>), Error> {
    targets::profile(cpu).map_err(Error::Value)?;
    let none = IndexMap::default();
    let pins = pinned.unwrap_or(&none);
    let empty = BTreeSet::new();
    let mut unspill: BTreeSet<u32> = unspillable.cloned().unwrap_or_default();
    unspill.extend(protected.unwrap_or(&empty).iter().copied());
    if llrm_support::env_flag!("LLRM_GREEDY_TRACE") {
        eprintln!("greedy-entry {} phis={}", body.name, body.blocks.iter().any(|block| !block.phis.is_empty()));
    }
    if body.blocks.iter().any(|block| !block.phis.is_empty()) {
        return Err(Error::Value(format!("{}: a phi reached the allocator", body.name)));
    }
    let current = body.clone();
    // The classes are all the greedy allocator reads of `Facts`, as
    // `MachineRegisterInfo::getRegClass` is all `RAGreedy` reads of isel.
    let confined = crate::backend::regclass::classes(&current, &empty, segments, classes);
    let sets = crate::backend::regclass::Sets::of(classes);
    let inputs = crate::greedy::shadow::ShadowInputs {
        body: &current,
        registers: classes.registers,
        order: &|value| target::order(confined.get(&value), segments, classes),
        pins,
        unspillable: &unspill,
        data: Some(segments.data),
        sets: Some(&sets),
        stable_loads: targets::profile(cpu).map_or(true, |profile| profile.stable_loads),
        saved,
    };
    let mode = crate::greedy::shadow::Mode::Inline { frame: &mut *frame, classes };
    let decided = crate::greedy::shadow::decide_with(&inputs, mode);
    if llrm_support::env_flag!("LLRM_GREEDY_TRACE") {
        if let Some(d) = &decided {
            eprintln!("greedy-inline {}: failed {} spill_failed {}", body.name, d.report.stats.failed, d.spill_failed);
        }
    }
    if let (Some(d), true) = (&decided, llrm_support::env_var("LLRM_EDITED_DUMP").is_some_and(|n| n == &*body.name)) {
        if let Some(edited) = &d.edited {
            for (b, block) in edited.blocks.iter().enumerate() {
                for (at, one) in block.insns.iter().enumerate() {
                    eprintln!(
                        "ED b{b}[{at}] #{} d{:?} u{:?} g{:?} {:?}",
                        one.id(),
                        one.defines,
                        one.uses,
                        one.group,
                        one.what.as_ref().map(|w| (&w.name, &w.dests, &w.sources))
                    );
                }
            }
        }
    }
    let Some(decided) = decided.filter(|decided| decided.report.stats.failed == 0 && !decided.spill_failed) else {
        return Err(Error::Value(format!("{}: the greedy allocator could not place every range", body.name)));
    };
    let Some(edited) = decided.edited else {
        return Err(Error::Value(format!("{}: the greedy allocator wrote no body", body.name)));
    };
    if llrm_support::env_flag!("LLRM_GREEDY_TRACE") {
        let r = &decided.report;
        eprintln!(
            "greedy-inline {}: values {} dequeued {} assigned {} evicted {} waited {} spilled {} new {} last-chance {} reloads {} stores {} folded {}",
            body.name,
            r.values,
            r.stats.dequeued,
            r.stats.assigned,
            r.stats.evicted,
            r.stats.waited,
            r.stats.spilled_ranges,
            r.stats.new_queued,
            r.stats.last_chance,
            r.reloads,
            r.stores,
            r.folded
        );
    }
    GREEDY_RUNS.with(|runs| runs.set((runs.get().0 + 1, runs.get().1, runs.get().2)));
    let r#where: IndexMap<u32, RegId> = decided.placed.into_iter().collect();
    Ok((
        Assignment { r#where, spilled: BTreeSet::new(), cost: 0.0, optimal: false, why: "llvm greedy".to_owned() },
        edited,
        decided.spilled.into_iter().collect(),
    ))
}

thread_local! {
    static FACTS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times `Facts::of` has run on this thread.
#[cfg(test)]
fn facts_built() -> usize {
    FACTS_BUILT.with(std::cell::Cell::get)
}

/// (allocations the greedy allocator made, allocations it left to this one).
pub fn greedy_runs() -> (usize, usize) {
    GREEDY_RUNS.with(|runs| (runs.get().0, runs.get().1))
}

thread_local! {
    /// The last allocation's (splits made, pieces, pieces spilled, values
    /// spilled), for the trial log.
    static REQUIRED_SKIPPED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LAST_STATS: std::cell::Cell<(usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0)) };
}

fn _values(body: &LirBody) -> Vec<u32> {
    let mut out: BTreeSet<u32> = BTreeSet::new();
    for block in &body.blocks {
        out.extend(block.arrives());
        for one in &block.insns {
            out.extend(one.defines.iter().copied());
            out.extend(one.uses.iter().copied());
        }
    }
    out.into_iter().collect()
}

/// How wide each value is anywhere it is read or written.
pub fn _widest(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            for (value, width) in _named_widths(one) {
                let widest = out.entry(value).or_insert(0);
                *widest = (*widest).max(width);
            }
        }
    }
    out
}

/// The (value, width) each place an instruction reads is: what its definition
/// writes does not count. A destination in memory reads its address; a
/// destination the instruction also uses (a partial write) is read at its own
/// width.
pub(crate) fn _read_widths(one: &Insn) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if let Some(what) = &one.what {
        for place in what.sources.iter().flat_map(ir::values) {
            out.push((place.value, place.width));
        }
        for dest in &what.dests {
            for place in ir::values(dest) {
                if !matches!(dest, Loc::Held(_)) || one.uses.contains(&place.value) {
                    out.push((place.value, place.width));
                }
            }
        }
    }
    out.extend(one.requires.iter().map(|(place, _)| (place.value, place.width)));
    out.extend(one.widths.iter().copied());
    out
}

/// The widest each value is read (bytes): a value no instruction reads wider
/// than 2 bytes never needs its high half, however wide its definition is.
pub fn _widest_read(body: &LirBody) -> IndexMap<u32, u32> {
    let mut out: IndexMap<u32, u32> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| block.insns.iter()) {
        for (value, width) in _read_widths(one) {
            let widest = out.entry(value).or_insert(0);
            *widest = (*widest).max(width);
        }
    }
    out
}

/// Each (value, width) an instruction names.
pub(crate) fn _named_widths(one: &Insn) -> impl Iterator<Item = (u32, u32)> + '_ {
    one.what
        .iter()
        .flat_map(|what| what.dests.iter().chain(&what.sources))
        .flat_map(ir::values)
        .chain(one.requires.iter().chain(&one.delivers).map(|(place, _register)| *place))
        .map(|place| (place.value, place.width))
        .chain(one.widths.iter().copied())
}

/// A point that destroys registers without naming them: `during` the
/// instruction, their high halves only, or `before` it reads its operands.
pub struct Mask {
    pub slot: i64,
    pub during: BTreeSet<RegId>,
    pub high: BTreeSet<RegId>,
    pub before: BTreeSet<RegId>,
}

/// The points that destroy registers, and, for each register, where, in order:
/// whether a value is live across one is asked of every value and every
/// register, over and over, and answered from the slots by bisection, not by a
/// look at every point.
#[derive(Default)]
pub struct Masks {
    list: Vec<Arc<Mask>>,
    /// The instruction each point is of, where the points came from a body:
    /// what `after` finds the points of the instructions a rewrite removed
    /// by.
    owners: Vec<usize>,
    /// Points handed out by `iter`, for a test that a query by span does not
    /// walk them all.
    walked: std::cell::Cell<usize>,
    reaching: std::cell::OnceCell<crate::support::hash::HashMap<RegId, Reaching>>,
    /// The points a rewrite added, where `after` made these from the ones
    /// before it: what a register held across the rewrite may now be
    /// destroyed at.
    added: Option<Vec<Arc<Mask>>>,
}

/// The points that destroy one register, as `Masks::destroyed_within` asks.
pub struct Destroyers<'a> {
    lists: [&'a [i64]; 3],
}

impl Destroyers<'_> {
    /// The first and last point in `lo..hi`.
    pub fn within(
        &self,
        lo: i64,
        hi: i64,
    ) -> Option<(i64, i64)> {
        let mut found: Option<(i64, i64)> = None;
        for list in self.lists {
            let from = list.partition_point(|slot| *slot < lo);
            let to = from + list[from..].partition_point(|slot| *slot < hi);
            if from < to {
                let (first, last) = (list[from], list[to - 1]);
                found = Some(found.map_or((first, last), |(low, high)| (low.min(first), high.max(last))));
            }
        }
        found
    }
}

/// Where one register is destroyed, sorted: before the point's own reads
/// (`read`), during it (`during`), or only its high half (`high`).
#[derive(Clone, Default)]
struct Reaching {
    read: Vec<i64>,
    during: Vec<i64>,
    high: Vec<i64>,
}

impl Masks {
    pub fn new(list: Vec<Mask>) -> Self {
        Self {
            list: list.into_iter().map(Arc::new).collect(),
            owners: Vec::new(),
            walked: std::cell::Cell::new(0),
            reaching: std::cell::OnceCell::new(),
            added: None,
        }
    }

    /// The points of `body`, which is the body of `before` with the
    /// instructions of `changes` removed and added, in the same numbering:
    /// the points of the instructions kept are kept, as LLVM's `SlotIndexes`
    /// keep an instruction's index across an edit. None where `before` did
    /// not come from a body.
    pub fn after(
        before: Masks,
        gone_slots: &IndexMap<usize, i64>,
        index: &Indexes,
        segments: &Segments,
        changes: &ranges::Changes,
    ) -> Option<Masks> {
        if before.owners.len() != before.list.len() {
            return None;
        }
        let Masks { mut list, mut owners, reaching, .. } = before;
        let owner_of = |slot: i64, owner: usize, list: &[Arc<Mask>], owners: &[usize]| -> Option<usize> {
            let from = list.partition_point(|mask| mask.slot < slot);
            (from..list.len()).take_while(|at| list[*at].slot == slot).find(|at| owners[*at] == owner)
        };
        let mut removed: Vec<usize> = Vec::new();
        for one in &changes.gone {
            let Some(slot) = gone_slots.get(&ranges::key(one)) else { continue };
            if let Some(at) = owner_of(*slot, ranges::key(one), &list, &owners) {
                removed.push(at);
            }
        }
        removed.sort_unstable();
        let mut added: Vec<(i64, usize, Arc<Mask>)> = Vec::new();
        for one in &changes.added {
            if let Some(mask) = _mask_of(one, index.at.get(&ranges::key(one)).copied()?, segments) {
                added.push((mask.slot, ranges::key(one), Arc::new(mask)));
            }
        }
        added.sort_by_key(|(slot, owner, _)| (*slot, *owner));
        let put: Vec<Arc<Mask>> = added.iter().map(|(_, _, mask)| Arc::clone(mask)).collect();
        // The points are edited where they stand: the ones the rewrite removed
        // go, the ones it added go in after the points of their slot, and the
        // rest are not touched, where a copy of the list cost a look at every
        // point at each of a function's rewrites.
        let mut taken: Vec<Arc<Mask>> = Vec::with_capacity(removed.len());
        for at in removed.iter().rev() {
            owners.remove(*at);
            taken.push(list.remove(*at));
        }
        taken.reverse();
        for (slot, owner, mask) in added {
            let at = list.partition_point(|point| point.slot <= slot);
            list.insert(at, mask);
            owners.insert(at, owner);
        }
        let found = Masks {
            list,
            owners,
            walked: std::cell::Cell::new(0),
            reaching: std::cell::OnceCell::new(),
            added: Some(put.clone()),
        };
        if let Some(mut reaching) = reaching.into_inner() {
            for mask in &taken {
                Self::unreach(&mut reaching, mask);
            }
            for mask in &put {
                Self::reach(&mut reaching, mask);
            }
            let _ = found.reaching.set(reaching);
        }
        Some(found)
    }

    fn reach(
        found: &mut crate::support::hash::HashMap<RegId, Reaching>,
        mask: &Mask,
    ) {
        for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
            let one = found.entry(*register).or_default();
            let list = if mask.before.contains(register) {
                &mut one.read
            } else if mask.during.contains(register) {
                &mut one.during
            } else {
                &mut one.high
            };
            let at = list.partition_point(|slot| *slot < mask.slot);
            list.insert(at, mask.slot);
        }
    }

    fn unreach(
        found: &mut crate::support::hash::HashMap<RegId, Reaching>,
        mask: &Mask,
    ) {
        for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
            let Some(one) = found.get_mut(register) else { continue };
            let list = if mask.before.contains(register) {
                &mut one.read
            } else if mask.during.contains(register) {
                &mut one.during
            } else {
                &mut one.high
            };
            let at = list.partition_point(|slot| *slot < mask.slot);
            if list.get(at) == Some(&mask.slot) {
                list.remove(at);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The points the rewrite this was made after added; None where it was made
    /// from a body.
    pub fn added(&self) -> Option<&[Arc<Mask>]> {
        self.added.as_deref()
    }

    /// Whether these are the same points as `other`, in the same order, and
    /// the registers are reached at the same slots.
    pub fn same_points(
        &self,
        other: &Masks,
    ) -> bool {
        let key = |mask: &Mask| (mask.slot, mask.during.clone(), mask.high.clone(), mask.before.clone());
        let mut mine: Vec<_> = self.list.iter().zip(&self.owners).map(|(mask, owner)| (key(mask), *owner)).collect();
        let mut theirs: Vec<_> =
            other.list.iter().zip(&other.owners).map(|(mask, owner)| (key(mask), *owner)).collect();
        mine.sort_by_key(|(key, owner)| (key.0, *owner));
        theirs.sort_by_key(|(key, owner)| (key.0, *owner));
        let reached = |found: &Masks| -> Vec<(RegId, Vec<i64>, Vec<i64>, Vec<i64>)> {
            let mut all: Vec<_> = found
                .list
                .iter()
                .flat_map(|mask| mask.before.iter().chain(&mask.during).chain(&mask.high))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter_map(|register| {
                    found
                        .reaching(*register)
                        .map(|one| (*register, one.read.clone(), one.during.clone(), one.high.clone()))
                })
                .collect();
            all.sort_by_key(|one| format!("{:?}", one.0));
            all
        };
        mine == theirs && reached(self) == reached(other)
    }

    /// Room for `more` points, and the room there is, for a test that `after`
    /// edits the list it is given and does not build another.
    #[cfg(test)]
    pub fn reserve_points(
        &mut self,
        more: usize,
    ) {
        self.list.reserve(more);
        self.owners.reserve(more);
    }

    #[cfg(test)]
    pub fn point_capacity(&self) -> usize {
        self.list.capacity().min(self.owners.capacity())
    }

    /// How many points `iter` has handed out.
    pub fn walked(&self) -> usize {
        self.walked.get()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mask> + '_ {
        self.walked.set(self.walked.get() + self.list.len());
        self.list.iter().map(|mask| &**mask)
    }

    /// The first and last point in `lo..hi` that destroys `register` (its high
    /// half only counts for a register wider than a word): the points of one
    /// register are sorted, so a block's span asks two searches, not every
    /// point of the function.
    pub fn destroyed_within(
        &self,
        register: RegId,
        width: u32,
        lo: i64,
        hi: i64,
    ) -> Option<(i64, i64)> {
        self.destroyers(register, width).within(lo, hi)
    }

    /// Where `register` is destroyed, found once for the many spans a
    /// candidate asks of it.
    pub fn destroyers(
        &self,
        register: RegId,
        width: u32,
    ) -> Destroyers<'_> {
        match self.reaching(register) {
            Some(one) => Destroyers { lists: [&one.read, &one.during, if width > 2 { &one.high } else { &[] }] },
            None => Destroyers { lists: [&[], &[], &[]] },
        }
    }

    fn reaching(
        &self,
        register: RegId,
    ) -> Option<&Reaching> {
        self.reaching
            .get_or_init(|| {
                let mut found: crate::support::hash::HashMap<RegId, Reaching> =
                    crate::support::hash::HashMap::default();
                for mask in self.list.iter().map(|mask| &**mask) {
                    for register in mask.before.iter().chain(&mask.during).chain(&mask.high) {
                        let one = found.entry(*register).or_default();
                        if mask.before.contains(register) {
                            one.read.push(mask.slot);
                        } else if mask.during.contains(register) {
                            one.during.push(mask.slot);
                        } else {
                            one.high.push(mask.slot);
                        }
                    }
                }
                for one in found.values_mut() {
                    one.read.sort_unstable();
                    one.during.sort_unstable();
                    one.high.sort_unstable();
                }
                found
            })
            .get(&register)
    }
}

/// Every point a register is destroyed without being named, and which. The
/// data segment register is reloaded ahead of a point that needs the data
/// group, so it holds none of that point's operands either.
pub fn _masks(
    body: &LirBody,
    index: &Indexes,
    segments: &Segments,
) -> Masks {
    let mut list: Vec<Arc<Mask>> = Vec::new();
    let mut owners: Vec<usize> = Vec::new();
    for block in &body.blocks {
        for one in &block.insns {
            if let Some(mask) = _mask_of(one, index.at[&ranges::key(one)], segments) {
                list.push(Arc::new(mask));
                owners.push(ranges::key(one));
            }
        }
    }
    Masks { list, owners, walked: std::cell::Cell::new(0), reaching: std::cell::OnceCell::new(), added: None }
}

/// The point `one` is at `slot`, if it destroys a register without naming it.
fn _mask_of(
    one: &Arc<Insn>,
    slot: i64,
    segments: &Segments,
) -> Option<Mask> {
    let during: BTreeSet<RegId> = one.clobbers.iter().map(|register| _whole(*register)).collect();
    let high: BTreeSet<RegId> = one.clobbers_high.iter().map(|register| _whole(*register)).collect();
    let before: BTreeSet<RegId> = target::needs_data_group(one).then_some(segments.data).into_iter().collect();
    (!during.is_empty() || !high.is_empty() || !before.is_empty()).then_some(Mask { slot, during, high, before })
}

/// The 32-bit register this one is part of.
pub fn _whole(register: RegId) -> RegId {
    ir::root(register)
}

/// Whether this range is live across a point that destroys the register, or
/// into one that destroys it before reading.
pub fn _clobbered(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let answer = _clobbered_from(one, register, masks, width);
    if check_clobbered() {
        assert_eq!(
            answer,
            _clobbered_reference(one, register, masks, width),
            "LLRM_CHECK_CLOBBERED: {register:?} over {:?}",
            one.segments
        );
    }
    answer
}

fn check_clobbered() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_flag!("LLRM_CHECK_CLOBBERED"))
}

/// `_clobbered`, from where each register is destroyed: a segment meets a point
/// where the point is after its start and, by `reaches`, no later than its end.
fn _clobbered_from(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let Some(places) = masks.reaching(_whole(register)) else { return false };
    // The first slot after `start` is the best candidate: any later one reaches
    // no further.
    let after = |slots: &[i64], start: i64| slots.get(slots.partition_point(|slot| *slot <= start)).copied();
    one.segments
        .iter()
        .any(
            |seg| after(&places.read, seg.start).is_some_and(|slot| seg.end >= ranges::def_point(slot))
                || after(&places.during, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))
                || (width > 2 && after(&places.high, seg.start).is_some_and(|slot| seg.end > ranges::def_point(slot))),
        )
}

/// What `_clobbered` was: a look at every point.
pub fn _clobbered_reference(
    one: &Interval,
    register: RegId,
    masks: &Masks,
    width: u32,
) -> bool {
    let mine = _whole(register);
    for mask in masks.iter() {
        let slot = mask.slot;
        let read = mask.before.contains(&mine);
        if !read && !mask.during.contains(&mine) && (!mask.high.contains(&mine) || width <= 2) {
            continue;
        }
        // A use keeps its value alive to `slot + DEF`.
        let reaches = |end: i64| if read { end >= ranges::def_point(slot) } else { end > ranges::def_point(slot) };
        if one.segments.iter().any(|seg| seg.start < slot && reaches(seg.end)) {
            return true;
        }
    }
    false
}

/// Assign, then rewrite. LLVM's two halves, in one phase.
pub struct RegAlloc {
    pub pinned: IndexMap<u32, RegId>,
    /// One frame, shared with the phases around this one, as Python shares it.
    pub frame: Option<Rc<RefCell<Frame>>>,
    pub cpu: Profile,
    pub segments: Segments,
    pub classes: Rc<RegisterClasses>,
    /// The registers the function's convention keeps, whole: the first use of
    /// one costs a `push` and a `pop` (`tryAssignCSRFirstTime`). None: it
    /// costs nothing.
    pub saved: Vec<RegId>,
}

impl RegAlloc {
    pub const NAME: &'static str = "regalloc";

    pub fn new(
        pinned: Option<&IndexMap<u32, RegId>>,
        frame: Option<Rc<RefCell<Frame>>>,
        cpu: ProfileOrName<'_>,
        segments: &Segments,
        classes: &Rc<RegisterClasses>,
    ) -> Result<Self, String> {
        Ok(Self {
            pinned: pinned.cloned().unwrap_or_default(),
            frame,
            cpu: targets::profile(cpu)?.clone(),
            segments: segments.clone(),
            classes: Rc::clone(classes),
            saved: Vec::new(),
        })
    }

    /// With the registers the function's convention keeps.
    pub fn with_saved(
        mut self,
        saved: Vec<RegId>,
    ) -> Self {
        self.saved = saved;
        self
    }

    /// Assign; where that spills, make the spill real and assign again.
    pub fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Error> {
        llrm_mir::counts::bump(llrm_mir::counts::Event::Allocations);
        let regs = body.regs();
        let prepare = llrm_support::debug::span("regalloc prepare");
        if self.frame.is_none() {
            self.frame = Some(Rc::new(RefCell::new(frames::of(&body, None, "", None)?)));
        }
        let cpu = self.cpu.clone();
        // Only a body that never names the data segment register itself may
        // find it holding one of its values.
        let segments = self.segments.clone();
        let data_free = !datagroup::names_data_segment(&body, &segments);
        if crate::support::debug::enabled("regclass") {
            let skip = crate::backend::regclass::untouchable(&body);
            let found = crate::backend::regclass::violations(&body, &segments, &self.classes, &skip);
            let crowded =
                found.iter().filter(|one| matches!(one.why, crate::backend::regclass::Why::Crowded { .. })).count();
            llrm_support::debug!(
                "regclass",
                "{}: {} points do not fit entering RegAlloc ({} crowded, {} unmatched)",
                body.name,
                found.len(),
                crowded,
                found.len() - crowded
            );
        }
        let floor = self.pinned.keys().copied().max().map_or(0, |one| one + 1);
        // One scan of the body's classes serves the three steps when none of
        // them changes it.
        let (uses, mut confined) = crate::backend::regclass::confining(&body, &segments, &self.classes);
        let mut body = match constrain::distinct_classes_given(&body, floor, &uses) {
            Some(peeled) => {
                confined = classes(&peeled, &BTreeSet::new(), &segments, &self.classes);
                peeled
            }
            None => body,
        };
        if let Some(rewritten) = explicit_selectors_given(&body, Some(&self.pinned), &segments, &confined) {
            confined = classes(&rewritten, &BTreeSet::new(), &segments, &self.classes);
            body = rewritten;
        }
        let incompatible: BTreeSet<u32> = self
            .pinned
            .iter()
            .filter(|(value, register)| {
                confined.get(*value).is_some_and(|choices| {
                    !choices.iter().map(|choice| _whole(*choice)).any(|choice| choice == _whole(**register))
                })
            })
            .map(|(value, _register)| *value)
            .collect();
        if !incompatible.is_empty() {
            let delivered: BTreeSet<u32> = body
                .blocks
                .iter()
                .flat_map(|block| &block.insns)
                .flat_map(|one| one.delivers.iter().map(|(held, _register)| held.value))
                .collect();
            let missing: BTreeSet<u32> = incompatible.difference(&delivered).copied().collect();
            if let Some(value) = missing.first() {
                return Err(Unplaced(format!(
                    "value#{value} is pinned outside its register class and has no defining occurrence to split"
                ))
                .into());
            }
            self.pinned.retain(|value, _register| !incompatible.contains(value));
        }
        let selectors: BTreeSet<RegId> = segments.selectors.iter().copied().collect();
        self.pinned.retain(|value, register| !(regs.is_segment(*register) && confined.get(value) == Some(&selectors)));
        let (constrained_body, fixed) = constrain::constrained(&body, Some(&self.pinned), &self.classes);
        body = constrain::hoisted(&constrained_body, &fixed);
        let clash: BTreeSet<u32> = fixed
            .keys()
            .copied()
            .filter(|one| self.pinned.get(one).is_some_and(|register| *register != fixed[one]))
            .collect();
        if let Some(first) = clash.first() {
            return Err(Unplaced(format!("value#{first} is pinned and required in different registers")).into());
        }
        let mut prefer = self.pinned.clone();
        prefer.extend(fixed.iter().map(|(value, register)| (*value, *register)));
        let reloads: BTreeSet<u32> = fixed.keys().copied().collect();
        let abandoned: BTreeSet<usize> = body
            .blocks
            .iter()
            .flat_map(|block| &block.insns)
            .filter(|one| _unread_move(one))
            .map(ranges::key)
            .collect();
        body = removed_unread(&body, &abandoned);
        // Asked once: the body is the same at the next use.
        let required = constrain::required(&body, &self.classes);
        self.pinned = prefer.clone();
        self.pinned.extend(required.iter().map(|(value, register)| (*value, *register)));
        let frame = Rc::clone(self.frame.as_ref().expect("made above"));
        drop(prepare);
        let base = llrm_support::debug::span("regalloc base");
        let mut pins = prefer.clone();
        pins.extend(required);
        // Each spill gets a slot of its own; they are shared once, over the
        // allocation (`crate::backend::slots::slots_colored`).
        frame.borrow_mut().defer_colours = true;
        let allocated = greedy_rewritten(
            &body,
            Some(&pins),
            Some(&reloads),
            Some(&BTreeSet::new()),
            (&cpu).into(),
            &segments,
            &self.classes,
            &self.saved,
            &mut frame.borrow_mut(),
        );
        frame.borrow_mut().defer_colours = false;
        let (got, out, spilled) = allocated?;
        drop(base);
        llrm_support::debug!("regalloc", "{}: {} insns, {} spilled", body.name, out.insns().len(), spilled.len());
        let colored = llrm_support::debug::timed("spill slots shared", || {
            crate::backend::slots::slots_colored(&out, &mut frame.borrow_mut())
        });
        let _apply = llrm_support::debug::span("regalloc apply");
        crate::backend::rewriter::rewrite(&colored, &got, &self.classes)
            .map(|placed| datagroup::restored(&placed, data_free, &segments))
    }
}

impl LIRTransform for RegAlloc {
    fn class_name(&self) -> &'static str {
        "RegAlloc"
    }

    fn name(&self) -> &str {
        Self::NAME
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        RegAlloc::transform(self, body).map_err(|error| error.to_string())
    }

    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        RegAlloc::transform(self, body).map_err(|error| error.raised())
    }
}

/// Frame traffic inside loops by cause, weighted by loop depth: the spiller's
/// reloads, stores and rematerializations, and the frame operands of x87 and
/// other instructions. The allocator can reach only the first three.
pub fn traffic_by_cause(body: &LirBody) -> std::collections::BTreeMap<&'static str, f64> {
    let deep = ranges::depths(body);
    let busy = Frequency::of(body);
    let mut out = std::collections::BTreeMap::new();
    for block in &body.blocks {
        let depth = deep.get(&block.at).copied().unwrap_or(0);
        if depth == 0 {
            continue;
        }
        for one in &block.insns {
            let Some(what) = &one.what else { continue };
            let frame = what
                .dests
                .iter()
                .chain(&what.sources)
                .any(
                    |place| matches!(
                        place,
                        Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                    ),
                );
            let float = what.op.is_x87();
            let cause = match () {
                _ if one.spill_reload => "reload",
                _ if one.spill_store => "store",
                _ if one.rematerialized => "remat",
                _ if frame && float => "x87-frame",
                _ if frame => "int-frame",
                _ => continue,
            };
            *out.entry(cause).or_insert(0.0) += busy.block(block.at);
        }
    }
    out
}

/// Python `format(value, "g")`.
pub(crate) fn _g(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_owned();
    }
    let scientific = format!("{value:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation");
    let exponent: i32 = exponent.parse().expect("an exponent");
    let trimmed = |text: String| -> String {
        if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_owned() } else { text }
    };
    if !(-4..6).contains(&exponent) {
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{}e{sign}{:02}", trimmed(mantissa.to_owned()), exponent.abs());
    }
    trimmed(format!("{value:.*}", (5 - exponent) as usize))
}

/// `body` without the abandoned moves (an instruction nothing reads) in
/// `abandoned`, keeping the bytes each owned.
fn removed_unread(
    body: &LirBody,
    abandoned: &BTreeSet<usize>,
) -> LirBody {
    if abandoned.is_empty() {
        return body.clone();
    }
    let every = || body.blocks.iter().flat_map(|block| &block.insns);
    let mut used: BTreeSet<u32> = every().flat_map(|one| one.uses.iter().copied()).collect();
    used.extend(every().flat_map(|one| one.requires.iter().map(|(held, _)| held.value)));
    used.extend(body.blocks.iter().flat_map(|one| one.arrives()));
    used.extend(
        body.blocks.iter().flat_map(|block| &block.phis).flat_map(|phi| phi.incoming.iter().map(|(_, value)| *value)),
    );

    let removable =
        |one: &Arc<Insn>| abandoned.contains(&key(one)) && !one.defines.iter().any(|value| used.contains(value));

    let anchor = |one: Arc<Insn>| -> Arc<Insn> {
        if !removable(&one) {
            return one;
        }
        crate::greedy::tii::_with(&one, |made| {
            made.what = Some(lir::inert());
            made.defines = Vec::new();
            made.uses = Vec::new();
            made.widths = Vec::new();
        })
    };

    body.with_blocks(
        body.blocks
            .iter()
            .map(|block| {
                block.with_insns(
                    lir::without(&block.insns, removable, None::<fn(&Arc<Insn>) -> Arc<Insn>>)
                        .into_iter()
                        .map(anchor)
                        .collect(),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    //! Ports of `tests/test_allocation.py`, `tests/test_allocation_hints.py`,
    //! `tests/test_interval_redefinitions.py`, and the allocator tests of
    //! `test_address_roles`, `test_lir`, `test_dead_call_deliveries`,
    //! `test_lir_verify`, `test_lower_arguments` and `test_pointer_memory`.

    use std::collections::BTreeSet;
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::*;
    use crate::backend::rewriter::{_dead_insertions, _placed, _settled};
    use crate::backend::{parcopy, select, verify};
    use crate::model::ir::{Imm, Reg};
    use crate::support::hash::IndexMap;

    fn semantics(
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Semantics {
        Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
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

    fn reg(
        register: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn block(
        at: i64,
        insns: Vec<Insn>,
    ) -> LirBlock {
        LirBlock::new(at, insns.into_iter().map(Arc::new).collect())
    }

    fn body_of(
        name: &str,
        entry: i64,
        insns: Vec<Insn>,
    ) -> LirBody {
        LirBody::new(name, entry, vec![block(entry, insns).into()], IndexMap::default(), IndexMap::default())
    }

    /// A body the greedy allocator cannot place is an error naming the
    /// function: the round allocator that used to take it (`rewritten`, the
    /// rounds loop, `LLRM_GREEDY_ROUNDS`, `LLRM_FORCE_FALLBACK`) is gone. Two
    /// values live together and both pinned to AX have no allocation; the old
    /// path fell back to an allocator that placed something else or looped.
    #[test]
    fn test_a_body_greedy_cannot_place_is_an_error_naming_the_function() {
        let at = |at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>| {
            Insn::new(at, Some((at, at + 1)), Some(what), defines, uses)
        };
        let one = at(0, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(1, 2)]), vec![1], vec![]);
        let two = at(1, semantics(Operation::Move, "mov", vec![held(2, 2)], vec![imm(2, 2)]), vec![2], vec![]);
        let add = at(
            2,
            semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(2, 2)]),
            vec![1],
            vec![1, 2],
        );
        let body = body_of("cannot_place", 0, vec![one, two, add]);
        let pinned = IndexMap::from_iter([(1, RegId::AX), (2, RegId::AX)]);
        let unspillable = BTreeSet::from([1, 2]);
        let got = greedy_rewritten(
            &body,
            Some(&pinned),
            Some(&unspillable),
            None,
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
            &[],
            &mut Frame::new(0),
        );
        let message = got.err().map(|error| error.to_string()).expect("an error, not a fallback allocation");
        assert!(message.contains("cannot_place"), "{message}");
        assert_eq!(greedy_runs().1, 0, "nothing falls back");
    }

    /// lru bas `BENCHLRU&` at -Os: an argument loaded in the entry block and
    /// pushed after two loops sat in ax the whole way (`mov ax,[bp+6]` ...
    /// `push ax`), where re-reading its cell at the push is the same memory
    /// operand and one instruction fewer; the registers it held took the
    /// constants' (+1 instruction, +3 B).
    #[test]
    #[ignore = "AM-CD-1: the spiller route that read the load at the push is gone"]
    fn test_a_load_read_once_by_a_push_is_read_at_the_push() {
        let cell = Mem { offset: 0, disp_width: 1, ..Mem::new(Some(Addr::new(Space::Frame, 6)), 2) };
        let at = |at: i64, what: Semantics, defines: Vec<u32>, uses: Vec<u32>| {
            Insn::new(at, Some((at, at + 1)), Some(what), defines, uses)
        };
        let load = at(0, semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(cell)]), vec![1], vec![]);
        let enter = at(1, semantics(Operation::Jump, "jmp", vec![], vec![]), vec![], vec![]);
        // A counted loop between the load and the push: the push's block runs
        // as often as the load's.
        let count = at(
            10,
            semantics(Operation::Binary, "add", vec![held(2, 2)], vec![held(2, 2), imm(1, 2)]),
            vec![2],
            vec![2],
        );
        let test = at(11, semantics(Operation::Compare, "cmp", vec![], vec![held(2, 2), imm(3, 2)]), vec![], vec![2]);
        let back = at(12, semantics(Operation::Branch, "jne", vec![], vec![]), vec![], vec![]);
        let push = at(20, semantics(Operation::Push, "push", vec![], vec![held(1, 2)]), vec![], vec![1]);
        let ret = at(21, semantics(Operation::Return, "retf", vec![], vec![]), vec![], vec![]);
        let zero = at(2, semantics(Operation::Move, "mov", vec![held(2, 2)], vec![imm(0, 2)]), vec![2], vec![]);
        let (count_again, test_again, back_again) = (
            at(
                15,
                semantics(Operation::Binary, "add", vec![held(2, 2)], vec![held(2, 2), imm(1, 2)]),
                vec![2],
                vec![2],
            ),
            at(16, semantics(Operation::Compare, "cmp", vec![], vec![held(2, 2), imm(7, 2)]), vec![], vec![2]),
            at(17, semantics(Operation::Branch, "jne", vec![], vec![]), vec![], vec![]),
        );
        let (mut first, mut looping, mut again, mut last) = (
            block(0, vec![load, zero, enter]),
            block(10, vec![count, test, back]),
            block(15, vec![count_again, test_again, back_again]),
            block(20, vec![push, ret]),
        );
        first.succ = vec![10].into();
        looping.succ = vec![10, 15].into();
        again.succ = vec![15, 20].into();
        last.succ = vec![].into();
        let mut body = LirBody::new(
            "t",
            0,
            vec![first.into(), looping.into(), again.into(), last.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        body.loop_trip_counts = vec![(10, 3), (15, 7)];
        let got = _through_regalloc(body, &[]);
        let loads_in = |at: i64| {
            got.blocks.iter().find(|one| one.at == at).expect("a block").insns.iter().any(|one| {
                one.what
                    .as_ref()
                    .is_some_and(|what| matches!(what.sources.as_slice(), [Loc::Mem(_)]) && what.op == Operation::Move)
            })
        };
        assert!(!loads_in(0), "the load stayed in the entry block: {:?}", got.blocks[0].insns);
        assert!(loads_in(20), "the load is not at the push");
    }

    /// The greedy allocator reads only the register classes of `Facts`, but
    /// built all of it, the old allocator's liveness included, for every body:
    /// 11.3 M of 1977 M instructions on d_faces.
    #[test]
    fn test_the_greedy_allocator_builds_no_facts_of_the_old_one() {
        let insns: Vec<Insn> = vec![
            _mov(1, 2, 4),
            Insn::new(
                8,
                Some((8, 9)),
                Some(semantics(Operation::Binary, "add", vec![held(1, 2)], vec![held(1, 2), held(1, 2)])),
                vec![1],
                vec![1],
            ),
        ];
        let (runs, built) = (greedy_runs().0, facts_built());
        _through_regalloc(_one_block(insns), &[]);
        assert_eq!(greedy_runs().0 - runs, 1, "premise: the greedy allocator placed the body");
        assert_eq!(facts_built() - built, 0);
    }

    fn _one_block(insns: Vec<Insn>) -> LirBody {
        body_of("one", 0, insns)
    }

    fn _mov(
        into: u32,
        value: i64,
        at: i64,
    ) -> Insn {
        Insn::new(
            at,
            Some((at, at + 2)),
            Some(semantics(Operation::Move, "mov", vec![held(into, 2)], vec![imm(value, 2)])),
            vec![into],
            vec![],
        )
    }

    fn _shl(
        result: u32,
        count: u32,
        at: i64,
    ) -> Insn {
        let what = semantics(Operation::Binary, "shl", vec![held(result, 2)], vec![held(result, 2), held(count, 2)]);
        Insn::new(at, Some((at, at + 2)), Some(what), vec![result], vec![result, count])
    }

    fn _named(
        body: &LirBody,
        name: &str,
    ) -> Vec<Semantics> {
        body.insns()
            .iter()
            .filter_map(|one| one.what.clone())
            .filter(|what| what.name.as_deref() == Some(name) && !matches!(what.sources[0], Loc::Imm(_)))
            .collect()
    }

    /// Python's phases share one frame object; an owned copy lost every
    /// spill slot RegAlloc made, so the prologue reserved too little.
    #[test]
    fn test_spill_slots_land_in_the_shared_frame() {
        let mut insns = vec![];
        let mut at = 0x100;
        for value in 1..=8u32 {
            insns.push(_mov(value, i64::from(value), at));
            at += 2;
        }
        for value in 1..=8u32 {
            let what = semantics(Operation::Binary, "add", vec![held(value, 2)], vec![held(value, 2), held(value, 2)]);
            insns.push(Insn::new(at, Some((at, at + 2)), Some(what), vec![value], vec![value]));
            at += 2;
        }
        for value in 1..=8u32 {
            let what = semantics(Operation::Push, "push", vec![], vec![held(value, 2)]);
            insns.push(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![value]));
            at += 2;
        }
        let body = _one_block(insns);
        let frame = Rc::new(RefCell::new(frames::of(&body, None, "", None).expect("a frame")));
        let mut phase = RegAlloc::new(
            None,
            Some(Rc::clone(&frame)),
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates");
        assert!(!frame.borrow().slots.is_empty());
    }

    /// RegAlloc scanned a body for its classes three times before it changed
    /// anything (`distinct_classes`, `explicit_selectors`, the classes it
    /// keeps): 5 scans of d_faces's functions' worth of reads a function, 7
    /// M Ir. A body they leave alone is scanned once for the three.
    #[test]
    fn a_body_the_class_steps_leave_alone_is_scanned_once_for_them() {
        let mut insns = Vec::new();
        for value in 1..=3u32 {
            insns.push(_mov(value, i64::from(value), 0x100 + 2 * i64::from(value)));
        }
        for value in 1..=3u32 {
            let at = 0x200 + 2 * i64::from(value);
            let what = semantics(Operation::Push, "push", vec![], vec![held(value, 2)]);
            insns.push(Insn::new(at, Some((at, at + 2)), Some(what), vec![], vec![value]));
        }
        let before = crate::backend::regclass::SCANS.with(std::cell::Cell::get);
        let _ = _through_regalloc(_one_block(insns), &[]);
        let scans = crate::backend::regclass::SCANS.with(std::cell::Cell::get) - before;
        assert!(scans <= 2, "{scans} scans of the classes in one allocation");
    }

    fn _through_regalloc(
        body: LirBody,
        pinned: &[(u32, RegId)],
    ) -> LirBody {
        let pinned = pins(pinned);
        let frame = frames::of(&body, None, "", None).expect("a frame");
        let mut phase = RegAlloc::new(
            Some(&pinned),
            Some(Rc::new(RefCell::new(frame))),
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("a cpu");
        RegAlloc::transform(&mut phase, body).expect("allocates")
    }

    fn pins(pairs: &[(u32, RegId)]) -> IndexMap<u32, RegId> {
        pairs.iter().copied().collect()
    }

    fn values(items: &[u32]) -> BTreeSet<u32> {
        items.iter().copied().collect()
    }

    fn allocated(
        body: &LirBody,
        pinned: Option<&IndexMap<u32, RegId>>,
    ) -> Result<Assignment, Error> {
        allocate(
            body,
            pinned,
            None,
            None,
            None,
            ProfileOrName::Name("386"),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
    }

    fn registers(places: &[Loc]) -> Vec<RegId> {
        places.iter().map(|x| if let Loc::Reg(one) = x { one.register } else { RegId::None }).collect()
    }

    fn assignment(r#where: IndexMap<u32, RegId>) -> Assignment {
        Assignment { r#where, spilled: BTreeSet::new(), cost: 0.0, optimal: true, why: String::new() }
    }

    fn cell_of(one: &Insn) -> Mem {
        match &one.what.as_ref().expect("semantics").sources[0] {
            Loc::Mem(cell) => cell.clone(),
            other => panic!("not a cell: {other:?}"),
        }
    }

    // ------------------------------------------------ tests/test_allocation.py

    #[test]
    fn test_a_widening_multiply_puts_its_halves_in_ax_and_dx() {
        let what = semantics(Operation::Multiply, "imul", vec![held(1, 2), held(2, 2)], vec![held(1, 2), held(3, 2)]);
        let imul = Insn::new(0x100, Some((0x100, 0x102)), Some(what), vec![1, 2], vec![1, 3]);
        let got = _through_regalloc(_one_block(vec![_mov(1, 3, 0xFC), _mov(3, 5, 0xFE), imul]), &[]);
        let made = _named(&got, "imul");
        assert_eq!(made.len(), 1);
        let made = &made[0];
        assert_eq!(ir::root(registers(&made.dests)[0]), RegId::EAX, "{:?}", made.dests);
        assert_eq!(ir::root(registers(&made.dests)[1]), RegId::EDX, "{:?}", made.dests);
        assert_eq!(made.sources[0], made.dests[0], "the tie was broken");
    }

    #[test]
    fn test_a_half_register_and_its_whole_are_the_same_register() {
        let mut insns: Vec<Insn> =
            [3, 4, 5].iter().zip(0..).map(|(one, index)| _mov(*one, i64::from(*one), 0xF0 + 2 * index)).collect();
        insns.push(Insn::new(
            0x100,
            Some((0x100, 0x104)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 4)], vec![imm(7, 4)])),
            vec![1],
            vec![],
        ));
        insns.push(_mov(2, 3, 0x104));
        insns.push(Insn::new(
            0x106,
            Some((0x106, 0x10A)),
            Some(semantics(Operation::Binary, "add", vec![held(1, 4)], vec![held(1, 4), held(2, 2)])),
            vec![1],
            vec![1, 2],
        ));
        insns.push(Insn::new(
            0x10A,
            Some((0x10A, 0x10C)),
            Some(semantics(Operation::Binary, "add", vec![held(3, 2)], vec![held(3, 2), held(4, 2)])),
            vec![3],
            vec![3, 4, 5],
        ));
        let got = allocated(&_one_block(insns), Some(&pins(&[(2, RegId::DX)]))).expect("allocates");
        let (long, pinned) = (got.r#where.get(&1), got.r#where.get(&2));
        assert!(long.is_some() || pinned.is_some(), "{:?}", got.r#where);
        if let (Some(long), Some(pinned)) = (long, pinned) {
            assert_ne!(
                ir::root(*long),
                ir::root(*pinned),
                "the long was given {long:?} and the pinned value {pinned:?}, which are one register"
            );
        }
    }

    #[test]
    fn test_a_fixed_source_that_is_not_the_multiply_pair_is_honoured() {
        let got = _through_regalloc(_one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104)]), &[]);
        let made = _named(&got, "shl");
        assert_eq!(made.len(), 1);
        assert_eq!(ir::root(registers(&made[0].sources)[1]), RegId::ECX, "{:?}", made[0].sources);
    }

    /// An operand read as undef names a value with no register: the rewrite
    /// gives it the first register of the class where it used to stop with
    /// `Unplaced`.
    #[test]
    fn test_an_operand_read_as_undef_is_placed_in_any_register() {
        let what = Semantics {
            name: Some("mov".into()),
            dests: vec![Loc::Held(Held { value: 2, width: 2 })],
            sources: vec![Loc::Held(Held { value: 9, width: 2 })],
            ..Semantics::new(Operation::Move)
        };
        let mut one = Insn::new(0, None, Some(what), vec![2], Vec::new());
        one.undefs = vec![9];
        let held: IndexMap<u32, RegId> = IndexMap::from_iter([(2, RegId::BX)]);
        let classes = RegisterClasses::m16();
        let placed = _placed(&Arc::new(one), &held, &classes).expect("placed");
        let sources = &placed.what.as_ref().expect("semantics").sources;
        assert!(matches!(&sources[0], Loc::Reg(_)), "{sources:?}");
    }

    /// A byte read as undef beside a def in `si`, which has no byte form: the
    /// rewrite named `si` for the byte and the instruction printed
    /// `movzx si, si`. The byte takes a register that has one.
    #[test]
    fn test_a_byte_read_as_undef_does_not_take_a_register_without_a_byte() {
        let what = Semantics {
            name: Some("movzx".into()),
            dests: vec![Loc::Held(Held { value: 2, width: 2 })],
            sources: vec![Loc::Held(Held { value: 9, width: 1 })],
            ..Semantics::new(Operation::Extend)
        };
        let mut one = Insn::new(0, None, Some(what), vec![2], Vec::new());
        one.undefs = vec![9];
        let held: IndexMap<u32, RegId> = IndexMap::from_iter([(2, RegId::SI)]);
        let placed = _placed(&Arc::new(one), &held, &RegisterClasses::m16()).expect("placed");
        let sources = &placed.what.as_ref().expect("semantics").sources;
        let Loc::Reg(read) = &sources[0] else { panic!("{sources:?}") };
        assert!(!matches!(read.register, RegId::SI | RegId::ESI), "{sources:?}");
    }

    /// `sbb x, x` reads its flags and nothing else: both sources are undef. The
    /// rewrite gave them the first register of the class (`ax`) beside a
    /// destination `dx`, `sbb dx, ax`: the CRC-32 mask came out 1309022537, not
    /// -873187034 (tests/sbb_undef_source.rs, whose fixture the pin-shaped
    /// `shl`/`sar` mask no longer turns into an `sbb`).
    #[test]
    fn test_an_undef_sbb_source_is_the_destination_register() {
        let held = |value: u32| Loc::Held(Held { value, width: 2 });
        let what = Semantics {
            name: Some("sbb".into()),
            dests: vec![held(2)],
            sources: vec![held(9), held(9)],
            ..Semantics::new(Operation::Binary)
        };
        let mut one = Insn::new(0, None, Some(what), vec![2], Vec::new());
        one.undefs = vec![9];
        let placed =
            _placed(&Arc::new(one), &IndexMap::from_iter([(2, RegId::DX)]), &RegisterClasses::m16()).expect("placed");
        let sources = &placed.what.as_ref().expect("semantics").sources;
        let read = |at: usize| match &sources[at] {
            Loc::Reg(read) => read.register,
            other => panic!("{other:?}"),
        };
        assert_eq!((read(0), read(1)), (RegId::DX, RegId::DX), "{sources:?}");
    }

    #[test]
    fn test_a_value_required_in_two_registers_gets_one_fresh_value_per_site() {
        let cwd = Insn::new(
            0x106,
            Some((0x106, 0x107)),
            Some(semantics(Operation::Extend, "cwd", vec![held(8, 2)], vec![held(7, 2)])),
            vec![8],
            vec![7],
        );
        let got =
            _through_regalloc(_one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104), cwd]), &[]);
        let shift = _named(&got, "shl");
        let extend = _named(&got, "cwd");
        assert_eq!((shift.len(), extend.len()), (1, 1));
        assert_eq!(ir::root(registers(&shift[0].sources)[1]), RegId::ECX, "{:?}", shift[0].sources);
        assert_eq!(ir::root(registers(&extend[0].sources)[0]), RegId::EAX, "{:?}", extend[0].sources);
    }

    #[test]
    fn test_an_origin_pin_does_not_override_what_the_instruction_requires() {
        let body = _one_block(vec![_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104)]);
        let got = _through_regalloc(body, &[(7, RegId::EBX)]);
        let made = _named(&got, "shl");
        assert_eq!(made.len(), 1);
        assert_eq!(ir::root(registers(&made[0].sources)[1]), RegId::ECX, "{:?}", made[0].sources);
    }

    fn _based_cell() -> Insn {
        let r#where = Addr { base: (RegId::SI).iced(), ..Addr::new(Space::Segment, 0x10) };
        let cell = Mem { disp_width: 2, base: Some(Held { value: 21, width: 2 }), ..Mem::new(Some(r#where), 2) };
        let what = semantics(Operation::Move, "mov", vec![held(30, 2)], vec![Loc::Mem(cell)]);
        Insn::new(0x100, Some((0x100, 0x104)), Some(what), vec![30], vec![21])
    }

    #[test]
    fn test_a_placed_cell_reaches_memory_by_the_register_its_value_got() {
        let regs = crate::backend::registerinfo::test_regs();
        for register in [RegId::EBX, RegId::ESI] {
            let was = cell_of(&_based_cell());
            let got = _settled(
                &Loc::Mem(was.clone()),
                &pins(&[(21, register)]),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("placed");
            let Loc::Mem(got) = got else { panic!("not a cell: {got:?}") };
            assert_eq!(got.through, regs.named(register, 2), "{register:?}: {:?}", got.through);
            assert_eq!(got.base, Some(Held { value: 21, width: 2 }), "the cell stopped naming its value");
            assert_eq!(got.addr, was.addr, "addr");
            assert_eq!(got.width, was.width, "width");
            assert_eq!(got.offset, was.offset, "offset");
            assert_eq!(got.disp_width, was.disp_width, "disp_width");
        }
    }

    /// nbodys placed `fld [si]`'s base in AX, where the call left it: the
    /// allocator released the whole-range AX pin, constrain found it again in
    /// `body.pins`, and `[ax]`, which has no 16-bit encoding, was emitted.
    #[test]
    fn test_a_call_result_used_as_a_base_leaves_the_register_it_was_delivered_in() {
        let mut call = Insn::new(
            0xF0,
            Some((0xF0, 0xF3)),
            Some(semantics(Operation::Call, "call", vec![], vec![])),
            vec![21],
            vec![],
        );
        call.delivers = vec![(Held { value: 21, width: 2 }, RegId::AX)];
        let mut body = _one_block(vec![call, _based_cell()]);
        body.pins = pins(&[(21, RegId::EAX)]);
        let placed = _through_regalloc(body, &[(21, RegId::EAX)]);
        let load = placed
            .insns()
            .into_iter()
            .find(|one| one.at == 0x100 && matches!(
                one.what.as_ref().map(|what| &what.sources[0]),
                Some(Loc::Mem(_))
            ));
        let through = cell_of(&load.expect("the load")).through;
        assert!(crate::backend::classes::RegisterClasses::m16().addressing.contains(&through), "{through:?}");
    }

    #[test]
    fn test_a_based_cell_keeps_the_register_the_allocation_gave_its_base() {
        let r#where = Addr { base: (RegId::SI).iced(), ..Addr::new(Space::Literal, 0x2) };
        let cell =
            Mem { offset: 2, disp_width: 1, base: Some(Held { value: 17, width: 2 }), ..Mem::new(Some(r#where), 2) };
        let load = Insn::new(
            0xA2,
            Some((0xA2, 0xA4)),
            Some(semantics(Operation::Move, "mov", vec![reg(RegId::AX, 2)], vec![Loc::Mem(cell)])),
            vec![],
            vec![17],
        );
        let got = crate::backend::rewriter::rewrite(
            &_one_block(vec![load]),
            &assignment(pins(&[(17, RegId::BX)])),
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("applies");
        let read = cell_of(&got.blocks[0].insns[0]);
        assert_eq!(read.through, RegId::BX, "the cell is still reached through {:?}", read.through);
        assert_eq!(read.base, Some(Held { value: 17, width: 2 }), "the cell stopped saying which value reached it");
        assert_eq!((read.addr, read.width, read.offset, read.disp_width), (Some(r#where), 2, 2, 1));
    }

    #[test]
    fn test_a_fixed_call_argument_reaches_its_register_through_the_whole_phase() {
        let made = Insn::new(
            0,
            Some((0, 3)),
            Some(semantics(
                Operation::Move,
                "mov",
                vec![held(2, 2)],
                vec![Loc::Mem(Mem::new(Some(Addr::new(Space::Segment, 0x10)), 2))],
            )),
            vec![2],
            vec![],
        );
        let mut call =
            Insn::new(3, Some((3, 8)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![], vec![2]);
        call.requires = vec![(Held { value: 2, width: 2 }, RegId::CX)];
        let got = _through_regalloc(_one_block(vec![made, call]), &[(2, RegId::SI)]);
        let insns = &got.blocks[0].insns;
        assert_eq!(insns.len(), 3, "the pre-call copy is missing: {} instructions", insns.len());
        let copy = insns[1].what.as_ref().expect("semantics");
        assert_eq!(copy.dests[0], reg(RegId::CX, 2), "the fresh value went to {:?}", copy.dests[0]);
        assert_eq!(copy.sources[0], reg(RegId::SI, 2), "the argument moved off si: {:?}", copy.sources[0]);
        assert_eq!(
            insns[0].what.as_ref().expect("semantics").dests[0],
            reg(RegId::SI, 2),
            "the original was pinned away from si"
        );
    }

    /// A phi's argument is read at the end of the predecessor it names: a value
    /// only a phi reads was live nowhere, and a next-use analysis saw it dead.
    #[test]
    fn test_a_phi_argument_is_live_out_of_its_predecessor() {
        let define = Insn::new(
            0,
            Some((0, 2)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(5, 2)])),
            vec![1],
            vec![],
        );
        let mut first = block(0, vec![define]);
        first.succ = vec![0x10].into();
        let mut second = block(0x10, vec![]);
        second.phis = vec![crate::model::lir::Phi { result: 2, incoming: vec![(0, 1)] }].into();
        let body = LirBody::new("phi", 0, vec![first.into(), second.into()], IndexMap::default(), IndexMap::default());
        let (live_in, live_out) = live(&body);
        assert!(live_out[&0].contains(&1), "the argument leaves its predecessor: {live_out:?}");
        assert!(!live_in[&0x10].contains(&1) && !live_in[&0x10].contains(&2));
    }
    // ----------------------------------------- tests/test_allocation_hints.py

    fn call_copy() -> LirBody {
        let define = Insn::new(
            0,
            Some((0, 3)),
            Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![imm(42, 2)])),
            vec![1],
            vec![],
        );
        let copy = Insn::new(
            3,
            Some((3, 3)),
            Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
            vec![2],
            vec![1],
        );
        let used = Insn::new(
            3,
            Some((3, 4)),
            Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])),
            vec![],
            vec![2],
        );
        LirBody { pins: pins(&[(2, RegId::EDI)]), ..body_of("call-copy", 0, vec![define, copy, used]) }
    }

    /// Excess call copies grew SCREEN and contributed to E1M1's BASIC error 14.
    #[test]
    fn test_call_input_is_computed_in_its_available_required_register() {
        let body = call_copy();
        let assignment = allocated(&body, Some(&body.pins)).expect("allocates");
        let result =
            crate::backend::rewriter::rewrite(&body, &assignment, &crate::backend::classes::RegisterClasses::m16())
                .expect("applies");
        assert!(assignment.spilled.is_empty());
        let mut emitted: Vec<u8> = Vec::new();
        for one in result.insns() {
            let what = one.what.as_ref().expect("semantics");
            let encoded = select::emit(what, 0, None, false, false, None).expect("encodes");
            emitted.extend(encoded.code);
        }
        assert_eq!(emitted, [0xBF, 0x2A, 0x00, 0x57]);
    }

    #[test]
    fn test_fixed_result_can_remain_in_its_return_register() {
        let body = call_copy();
        let got = allocated(&body, Some(&pins(&[(1, RegId::EDI)]))).expect("allocates");
        assert!(got.spilled.is_empty());
        assert_eq!(got.r#where[&1], RegId::EDI);
        assert_eq!(got.r#where[&2], RegId::EDI);
    }

    #[test]
    fn test_copy_hint_cannot_override_a_register_requirement() {
        for barrier in ["overlap", "clobber", "class", "fixed"] {
            let mut body = call_copy();
            let insns: Vec<Insn> = body.iter_insns().map(|one| (**one).clone()).collect();
            let (define, copy, used) = (insns[0].clone(), insns[1].clone(), insns[2].clone());
            let mut pinned = body.pins.clone();
            match barrier {
                "overlap" => {
                    let what = Semantics { sources: vec![held(1, 2)], ..used.what.clone().expect("semantics") };
                    let extra = Insn { at: 4, covers: Some((4, 5)), what: Some(what), uses: vec![1], ..used.clone() };
                    body.blocks = vec![block(0, vec![define, copy, used, extra]).into()];
                }
                "clobber" => {
                    let mut extra = Insn::new(2, Some((2, 2)), None, vec![], vec![]);
                    extra.clobbers = BTreeSet::from([RegId::EDI]);
                    body.blocks = vec![block(0, vec![define, extra, copy, used]).into()];
                }
                "class" => {
                    let byte_use = Insn::new(
                        2,
                        Some((2, 2)),
                        Some(semantics(Operation::Compare, "cmp", vec![], vec![held(1, 1), imm(0, 1)])),
                        vec![],
                        vec![1],
                    );
                    body.blocks = vec![block(0, vec![define, byte_use, copy, used]).into()];
                }
                _ => {
                    pinned.insert(1, RegId::ECX);
                }
            }
            let got = allocated(&body, Some(&pinned)).expect("allocates");
            assert!(got.spilled.is_empty(), "{barrier}");
            // A value that overlaps the pinned copy of itself may share its
            // register: the overlap starts at the copy
            // (`LiveRange::overlaps(Other, CP, Indexes)`), both hold one value.
            if barrier != "overlap" {
                assert_ne!(got.r#where[&1], RegId::EDI, "{barrier}");
            }
            assert_eq!(got.r#where[&2], RegId::EDI, "{barrier}");
        }
    }

    // ------------------------------------ tests/test_interval_redefinitions.py

    #[test]
    #[ignore = "fails in Python too: the surviving case raises Unplaced, not a spill"]
    fn test_call_redefinition_is_not_a_value_surviving_the_clobber() {
        let one = Loc::Held(Held { value: 1, width: 2 });
        let before = Insn::new(
            0,
            Some((0, 1)),
            Some(semantics(Operation::Move, "mov", vec![one.clone()], vec![imm(7, 2)])),
            vec![1],
            vec![],
        );
        let mut call = Insn::new(
            1,
            Some((1, 6)),
            Some(semantics(Operation::Call, "call", vec![], vec![one.clone()])),
            vec![1],
            vec![1],
        );
        call.clobbers = BTreeSet::from([RegId::ESI]);
        let after =
            Insn::new(6, Some((6, 7)), Some(semantics(Operation::Push, "push", vec![], vec![one])), vec![], vec![1]);
        let body = body_of("call", 0, vec![before.clone(), call.clone(), after.clone()]);
        let si = pins(&[(1, RegId::SI)]);
        let assigned = allocate(
            &body,
            Some(&si),
            Some(&values(&[1])),
            None,
            None,
            "386".into(),
            &target::BUILT_IN,
            &crate::backend::classes::RegisterClasses::m16(),
        )
        .expect("allocates");
        assert!(assigned.spilled.is_empty() && assigned.r#where[&1] == RegId::SI);

        let surviving = body_of("call", 0, vec![before, Insn { defines: vec![], ..call }, after]);
        assert_eq!(allocated(&surviving, Some(&si)).expect("allocates").spilled, values(&[1]));
    }

    // --------------------------------------------- tests/test_address_roles.py

    fn _instruction(
        at: i64,
        what: Semantics,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Insn {
        Insn::new(at, Some((at, at)), Some(what), defines, uses)
    }

    fn _frame_load(
        at: i64,
        value: u32,
        displacement: i64,
    ) -> Insn {
        let cell = Mem { through: RegId::BP, ..Mem::new(Some(Addr::new(Space::Frame, displacement)), 2) };
        _instruction(
            at,
            semantics(Operation::Move, "mov", vec![held(value, 2)], vec![Loc::Mem(cell)]),
            vec![value],
            vec![],
        )
    }

    fn _load(
        at: i64,
        into: u32,
        cell: Mem,
        uses: Vec<u32>,
    ) -> Insn {
        _instruction(at, semantics(Operation::Move, "mov", vec![held(into, 2)], vec![Loc::Mem(cell)]), vec![into], uses)
    }

    /// QCport's far_put could not allocate two live pointer bases.
    #[test]
    fn test_shared_word_index_takes_bx_instead_of_spilling_a_base() {
        let index =
            _instruction(3, semantics(Operation::Move, "mov", vec![held(3, 2)], vec![imm(1, 2)]), vec![3], vec![]);
        let source = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 3, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
        };
        let destination = Mem {
            base: Some(Held { value: 2, width: 2 }),
            index: Some(Held { value: 3, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr { segment: (RegId::ES).iced(), ..Addr::new(Space::Far, 0) }), 2)
        };
        let read = _load(4, 4, source, vec![1, 3]);
        let write = _instruction(
            5,
            semantics(Operation::Move, "mov", vec![Loc::Mem(destination)], vec![held(4, 2)]),
            vec![],
            vec![2, 3, 4],
        );
        let body = body_of("shared-index", 0, vec![_frame_load(1, 1, 4), _frame_load(2, 2, 6), index, read, write]);

        let assignment = allocated(&body, None).expect("allocates");
        assert!(assignment.spilled.is_empty(), "{assignment:?}");
        let placed =
            crate::backend::rewriter::rewrite(&body, &assignment, &crate::backend::classes::RegisterClasses::m16())
                .expect("applies");
        let accesses: Vec<Semantics> =
            placed.iter_insns().filter(|one| [4, 5].contains(&one.at)).filter_map(|one| one.what.clone()).collect();
        assert!(accesses.iter().all(|what| select::emit(what, 0, None, false, false, None).is_some()), "{accesses:?}");
    }

    #[test]
    fn test_word_address_role_keeps_a_call_crossing_base_out_of_bx() {
        let mut call = Insn::new(
            2,
            Some((2, 2)),
            Some(Semantics { name: Some("call".into()), ..Semantics::new(Operation::Call) }),
            vec![],
            vec![],
        );
        call.clobbers = BTreeSet::from([RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX]);
        call.clobbers_high = BTreeSet::from([RegId::ESI, RegId::EDI]);
        let index =
            _instruction(3, semantics(Operation::Move, "mov", vec![held(2, 2)], vec![imm(1, 2)]), vec![2], vec![]);
        let cell = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 2, width: 2 }),
            scale: 1,
            ..Mem::new(Some(Addr::new(Space::Literal, 0)), 2)
        };
        let read = _load(4, 3, cell, vec![1, 2]);
        let body = body_of("call-crossing-base", 0, vec![_frame_load(1, 1, 4), call, index, read]);

        let found =
            classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());

        assert_eq!(found[&1], crate::backend::classes::RegisterClasses::m16().word_indexes);
        assert_eq!(found[&2], crate::backend::classes::RegisterClasses::m16().word_bases);
    }

    #[test]
    fn test_retained_owner_influences_commutative_address_roles_before_allocation() {
        let cell = Mem {
            base: Some(Held { value: 1, width: 2 }),
            index: Some(Held { value: 2, width: 2 }),
            ..Mem::new(Some(Addr::new(Space::Far, 0)), 2)
        };
        let read = _load(3, 3, cell, vec![1, 2]);
        let body = body_of("retained-address-role", 0, vec![_frame_load(1, 1, 6), _frame_load(2, 2, 8), read]);

        let confined =
            classes(&body, &values(&[1]), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());

        assert_eq!(confined[&1], crate::backend::classes::RegisterClasses::m16().word_indexes);
        assert_eq!(confined[&2], crate::backend::classes::RegisterClasses::m16().word_bases);
    }

    #[test]
    fn test_32_bit_secondary_base_is_not_confined_to_16_bit_address_registers() {
        let cell = Mem { base: Some(Held { value: 1, width: 4 }), ..Mem::new(Some(Addr::new(Space::Far, 0)), 2) };
        let body = body_of("secondary-base-class", 0, vec![_load(2, 2, cell, vec![1])]);

        assert!(
            !classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())
                .contains_key(&1)
        );
    }

    // ------------------------------------------------------ tests/test_lir.py

    // Python's cell has the string address "[bx]", which Rust cannot hold.
    fn _celled(
        base: Option<Held>,
        through: RegId,
    ) -> LirBody {
        let at = 0x100;
        let cell = Mem { through, base, ..Mem::new(None, 2) };
        let what = semantics(Operation::Move, "mov", vec![held(30, 2)], vec![Loc::Mem(cell)]);
        let uses = base.map(|one| vec![one.value]).unwrap_or_default();
        _one_block(vec![Insn::new(at, Some((at, at + 2)), Some(what), vec![30], uses)])
    }

    #[test]
    fn test_an_address_value_takes_the_class_a_base_register_must_be_in() {
        for through in [RegId::BX, RegId::SI] {
            let got = classes(
                &_celled(Some(Held { value: 21, width: 2 }), through),
                &BTreeSet::new(),
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16(),
            );
            assert_eq!(
                got.get(&21),
                Some(&crate::backend::classes::RegisterClasses::m16().addressing),
                "through={through:?}: {:?}",
                got.get(&21)
            );
        }
    }

    #[test]
    fn test_an_unbased_cell_confines_no_value() {
        assert!(
            !classes(
                &_celled(None, RegId::BX),
                &BTreeSet::new(),
                &target::BUILT_IN,
                &crate::backend::classes::RegisterClasses::m16()
            )
            .contains_key(&21)
        );
    }

    // ------------------------------------- tests/test_dead_call_deliveries.py

    #[test]
    fn test_dead_call_result_does_not_create_an_undefined_copy() {
        let mut call =
            Insn::new(0, Some((0, 3)), Some(semantics(Operation::Call, "call", vec![], vec![])), vec![1], vec![]);
        call.delivers = vec![(Held { value: 1, width: 2 }, RegId::AX)];
        let (lowered, _fixed) = constrain::constrained(
            &body_of("dead result", 0, vec![call]),
            Some(&pins(&[(1, RegId::EAX)])),
            &crate::backend::classes::RegisterClasses::m16(),
        );
        let insns = lowered.insns();
        assert_eq!(insns.len(), 1, "a copy was made of a result nothing reads");
    }

    // --------------------------------------------- tests/test_lir_verify.py

    #[test]
    fn test_register_allocation_keeps_the_definition_of_an_elided_identity() {
        for covers in [None, Some((1, 1)), Some((1, 3))] {
            let mut identity = Insn::new(
                1,
                covers,
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            );
            identity.symbol = Some(covers == Some((1, 3)));
            let mut returning =
                Insn::new(2, None, Some(semantics(Operation::Return, "", vec![], vec![])), vec![], vec![2]);
            returning.requires = vec![(Held { value: 2, width: 2 }, RegId::AX)];
            let body = LirBody { inputs: values(&[1]), ..body_of("fixed-identity", 1, vec![identity, returning]) };

            let placed = crate::backend::rewriter::rewrite(
                &body,
                &assignment(pins(&[(1, RegId::AX), (2, RegId::AX)])),
                &crate::backend::classes::RegisterClasses::m16(),
            )
            .expect("applies");

            let wrong = verify::verify(&placed, false);
            assert!(wrong.is_empty(), "{covers:?}: {wrong:?}");
            let first = &placed.insns()[0];
            assert_eq!(first.what.as_ref().expect("semantics").op, Operation::Nothing, "{covers:?}");
            assert_ne!(first.symbol, Some(true), "{covers:?}");
        }
    }

    /// C sieve read value#126 after its identity phi copy disappeared.
    #[test]
    fn test_parallel_copy_identity_keeps_its_virtual_definition() {
        let identity = Insn {
            group: Some(7),
            ..Insn::new(
                1,
                Some((1, 1)),
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            )
        };
        let consumed = Insn::new(
            2,
            Some((2, 2)),
            Some(semantics(Operation::Push, "push", vec![], vec![held(2, 2)])),
            vec![],
            vec![2],
        );
        let body = LirBody { inputs: values(&[1]), ..body_of("parallel-identity", 1, vec![identity, consumed]) };
        let assignment = assignment(pins(&[(1, RegId::AX), (2, RegId::AX)]));

        let placed =
            crate::backend::rewriter::rewrite(&body, &assignment, &crate::backend::classes::RegisterClasses::m16())
                .expect("applies");
        let scheduled = parcopy::scheduled(&placed).expect("schedules");

        assert!(verify::verify(&placed, false).is_empty(), "{:?}", verify::verify(&placed, false));
        assert!(verify::verify(&scheduled, false).is_empty(), "{:?}", verify::verify(&scheduled, false));
    }

    // ------------------------------------------ tests/test_lower_arguments.py

    #[test]
    fn test_dead_inserted_copy_chain_does_not_erase_observable_values() {
        for keep in ["none", "read", "original", "source_load", "opaque"] {
            let slot = Frame::new(0).cell(1u32, 2).expect("a slot");
            let mut load = Insn::new(
                0,
                Some((0, 0)),
                Some(semantics(Operation::Move, "mov", vec![held(1, 2)], vec![Loc::Mem(slot)])),
                vec![1],
                vec![],
            );
            load.spill_reload = keep != "source_load";
            let copy = Insn::new(
                1,
                Some(if keep == "original" { (1, 2) } else { (1, 1) }),
                Some(semantics(Operation::Move, "mov", vec![held(2, 2)], vec![held(1, 2)])),
                vec![2],
                vec![1],
            );
            let mut insns = vec![load.clone(), copy];
            if keep == "read" || keep == "opaque" {
                insns.push(Insn::new(2, Some((2, 3)), None, vec![], if keep == "read" { vec![2] } else { vec![] }));
            }
            let result = _dead_insertions(&body_of("dead", 0, insns.clone()));
            let expected: Vec<Insn> = match keep {
                "none" => vec![],
                "source_load" => vec![load],
                _ => insns,
            };
            let got: Vec<Insn> = result.iter_insns().map(|one| (**one).clone()).collect();
            assert_eq!(got, expected, "{keep}");
        }
    }

    // ------------------------------------------ tests/test_pointer_memory.py

    #[test]
    fn test_generated_byte_value_cannot_be_allocated_to_edi() {
        let what = semantics(Operation::Move, "mov", vec![held(1, 1)], vec![imm(12, 1)]);
        let body = body_of("byte", 0, vec![Insn::new(0, Some((0, 0)), Some(what), vec![1], vec![])]);
        assert_eq!(
            classes(&body, &BTreeSet::new(), &target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16())[&1],
            BTreeSet::from([RegId::AX, RegId::BX, RegId::CX, RegId::DX])
        );
    }
    /// An x87 value placed in FP0..FP6 is named by the register, with no value
    /// left on the instruction (`fpregs` numbers it again): it stayed a value
    /// the stackifier could not tell from another in the same register.
    #[test]
    fn test_an_x87_value_in_a_register_is_the_register_and_no_value() {
        let load = semantics(
            Operation::FloatLoad,
            "fld",
            vec![Loc::Held(Held { value: 7, width: 10 })],
            vec![Loc::Mem(Mem::new(Some(Addr::new(Space::Frame, -8)), 8))],
        );
        let one = Arc::new(Insn::new(1, Some((1, 1)), Some(load), vec![7], vec![]));
        let held = pins(&[(7, llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO).rfp_registers()[3])]);
        let placed = _placed(&one, &held, &crate::backend::classes::RegisterClasses::m16()).expect("placed");
        let what = placed.what.as_ref().expect("semantics");
        assert_eq!(
            what.dests,
            vec![Loc::Reg(Reg {
                register: llrm_lir::registers::Regs(&llrm_x86_m16::REGISTER_INFO).rfp_registers()[3],
                width: 10
            })]
        );
        assert!(placed.defines.is_empty(), "{:?}", placed.defines);
    }
}
