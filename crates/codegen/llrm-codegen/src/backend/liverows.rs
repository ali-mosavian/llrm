//! Which values are live where, per block and per instruction row, for the
//! passes that place values (the allocator's consumers).
//!
//! # How it works
//! [`live`] and [`live_out`] give, per block, the values live on entry and on
//! exit. [`LiveRows`] answers the same per instruction row through the
//! [`LiveAt`] trait: a dense table for small bodies, a walk of the blocks of a
//! web for large ones ([`WebRows`]). [`live_reference`] is the plain
//! iteration the others are checked against.
//!
//! # Used by
//! `backend::allocate` and the passes that read liveness of a lowered body
//! (`flow`, `phielim`, `peephole`, `regclass`).
//!
//! # Example
//! `v1 := mov 1; v2 := add v1, 2; ret v2`: `v1` is live from row 0 to row 1,
//! `v2` from row 1 to the `ret`; [`live_out`] of the only block is empty.

#[cfg(test)]
use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::intervals::{self as ranges, Indexes};
use crate::backend::live::LiveRanges;
use crate::model::lir::LirBlock;
use crate::model::lir::LirBody;
use crate::support::hash::IndexMap;

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
