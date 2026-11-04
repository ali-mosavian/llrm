//! What a value's candidate registers cost, from a table of its live blocks.
//!
//! LLVM's `SplitAnalysis` reads a value's `BlockInfo` and `ThroughBlocks` once;
//! `InterferenceCache` answers what a register meets in a block once per
//! (register, block); `calculateRegionSplitCost` prices a candidate from the
//! bundles' states with no region, and a `SplitEditor` is made for the best
//! only. `Frame` is the table, and `price` the candidate's cost, the same sum
//! `_benefit` makes of a `Region` (`LLRM_CHECK_SPLIT` holds the two to it).

use super::*;

const NONE: u32 = u32::MAX;

// What a test counts: the interference asked of a register in a block, and the
// regions made.
#[cfg(test)]
thread_local! {
    pub(super) static ASKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A block of the value's live range, with what each candidate asks of it.
pub(super) struct Row {
    position: usize,
    at: i64,
    len: usize,
    span: (i64, i64),
    entry: usize,
    exit: usize,
    frequency: f64,
    /// First and last position naming the value, and how many do: use blocks.
    uses: Option<(usize, usize, usize)>,
    live_in: bool,
    live_out: bool,
    /// Where the value is named, in `Frame::named`.
    named: std::ops::Range<usize>,
    /// Where the block's closing jumps start (`_last_split`).
    tail: i64,
    /// The successors' rows (`NONE` outside the live range).
    next: std::ops::Range<usize>,
}

/// The value's live blocks: use blocks first, as `Analysis` has them, then the
/// blocks it passes through.
pub(super) struct Frame<'a> {
    blocks: &'a Blocks<'a>,
    value: u32,
    rows: Vec<Row>,
    /// The row of the block at a body position, or `NONE`.
    row_of: Vec<u32>,
    named: Vec<usize>,
    next: Vec<u32>,
    /// The rows by body position: the order the scan sums in.
    in_order: Vec<u32>,
}

/// What each row holds of the value: up to two ranges.
pub(super) struct Held {
    ranges: Vec<[(usize, usize); 2]>,
    count: Vec<u8>,
}

impl Held {
    fn of(rows: usize) -> Self {
        Self { ranges: vec![[(0, 0); 2]; rows], count: vec![0; rows] }
    }

    fn set(
        &mut self,
        row: usize,
        found: [(usize, usize); 2],
        count: usize,
    ) {
        // `Region::add` sorts and joins ranges that touch.
        let (ranges, count) = match count {
            2 => {
                let (first, second) = if found[0] <= found[1] { (found[0], found[1]) } else { (found[1], found[0]) };
                if second.0 <= first.1 { ([(first.0, first.1.max(second.1)), (0, 0)], 1) } else { ([first, second], 2) }
            }
            _ => (found, count),
        };
        self.ranges[row] = ranges;
        self.count[row] = count as u8;
    }

    fn of_row(
        &self,
        row: usize,
    ) -> &[(usize, usize)] {
        &self.ranges[row][..self.count[row] as usize]
    }
}

/// `_ranges_in`, in place: the ranges of a block a piece holds.
fn ranges_in(
    length: usize,
    entry: bool,
    exit: bool,
    named: Option<(usize, usize)>,
    interference: Option<(i64, i64)>,
) -> ([(usize, usize); 2], usize) {
    let length = length as i64;
    let mut out = [(0i64, 0i64); 2];
    let mut count = 0;
    let mut push = |from: i64, to: i64| {
        out[count] = (from, to);
        count += 1;
    };
    match interference {
        None if entry && exit => push(0, length),
        None => match named {
            Some((first, last)) => {
                if entry {
                    push(0, last as i64 + 1);
                }
                if exit {
                    push(first as i64, length);
                }
            }
            None if entry || exit => push(0, length),
            None => {}
        },
        Some((low, high)) => {
            if entry {
                let end = match named {
                    Some((_, last)) if !exit => low.min(last as i64 + 1),
                    _ => low,
                };
                push(0, end);
            }
            if exit {
                let start = match named {
                    Some((first, _)) if !entry => (high + 1).max(first as i64),
                    _ => high + 1,
                };
                push(start, length);
            }
        }
    }
    let mut kept = [(0usize, 0usize); 2];
    let mut made = 0;
    for &(from, to) in &out[..count] {
        if from < to {
            kept[made] = (from as usize, to as usize);
            made += 1;
        }
    }
    (kept, made)
}

impl<'a> Frame<'a> {
    pub(super) fn new(
        blocks: &'a Blocks<'a>,
        analysis: &Analysis,
        placement: &spillplacement::Placement,
        index: &Indexes,
        value: u32,
    ) -> Self {
        let (body, graph) = (blocks.body, &blocks.graph);
        let names = blocks.names(value);
        let mut frame = Self {
            blocks,
            value,
            rows: Vec::with_capacity(analysis.uses.len() + analysis.through.len()),
            row_of: vec![NONE; body.blocks.len()],
            named: Vec::new(),
            next: Vec::new(),
            in_order: Vec::new(),
        };
        let row = |frame: &mut Self, at: i64, uses: Option<(usize, usize, usize)>, live_in: bool, live_out: bool| {
            let position = graph.position[&at];
            let block = &body.blocks[position];
            let (entry, exit) = placement.bundles.of[&at];
            let started = frame.named.len();
            if let Some(found) = names.get(&position) {
                frame.named.extend_from_slice(found);
            }
            frame.row_of[position] = frame.rows.len() as u32;
            frame
                .rows
                .push(
                    Row {
                        position,
                        at,
                        len: block.insns.len(),
                        span: index.span[&at],
                        entry,
                        exit,
                        frequency: placement.frequency[&at],
                        uses,
                        live_in,
                        live_out,
                        named: started..frame.named.len(),
                        tail: _last_split(block),
                        next: 0..0,
                    },
                );
        };
        for one in &analysis.uses {
            row(&mut frame, one.block, Some((one.first, one.last, one.references)), one.live_in, one.live_out);
        }
        for &at in &analysis.through {
            row(&mut frame, at, None, true, true);
        }
        for at in 0..frame.rows.len() {
            let block = &body.blocks[frame.rows[at].position];
            let started = frame.next.len();
            for next in &block.succ {
                frame.next.push(graph.position.get(next).map_or(NONE, |position| frame.row_of[*position]));
            }
            frame.rows[at].next = started..frame.next.len();
        }
        // Both runs are in body order: merged.
        let used = frame.uses();
        let (mut one, mut other) = (0..used as u32, used as u32..frame.rows.len() as u32);
        frame.in_order.reserve(frame.rows.len());
        loop {
            let take = match (one.start < one.end, other.start < other.end) {
                (true, true) => frame.rows[one.start as usize].position < frame.rows[other.start as usize].position,
                (true, false) => true,
                (false, true) => false,
                (false, false) => break,
            };
            let range = if take { &mut one } else { &mut other };
            frame.in_order.push(range.start);
            range.start += 1;
        }
        frame
    }

    pub(super) fn rows(&self) -> usize {
        self.rows.len()
    }

    pub(super) fn uses(&self) -> usize {
        self.rows.iter().take_while(|row| row.uses.is_some()).count()
    }

    fn block(
        &self,
        row: usize,
    ) -> &'a LirBlock {
        &self.blocks.body.blocks[self.rows[row].position]
    }

    /// `_use_constraints`: the use blocks want the value in a register at the
    /// borders it is live across, as far as `blocked` lets them. False when no
    /// bundle wants a register.
    pub(super) fn constrain(
        &self,
        placement: &mut spillplacement::Placement,
        blocked: &mut dyn FnMut(usize) -> Option<(i64, i64)>,
    ) -> bool {
        for (at, row) in self.rows.iter().enumerate() {
            let Some((first, last, references)) = row.uses else { break };
            let mut entry = if row.live_in { Border::PrefReg } else { Border::DontCare };
            let mut exit = if row.live_out { Border::PrefReg } else { Border::DontCare };
            if let Some((low, high)) = blocked(at) {
                if row.live_in {
                    entry = if low < 0 {
                        Border::MustSpill
                    } else if low <= first as i64 {
                        Border::PrefSpill
                    } else {
                        entry
                    };
                }
                if row.live_out {
                    exit = if high >= row.tail {
                        Border::MustSpill
                    } else if high >= last as i64 {
                        Border::PrefSpill
                    } else {
                        exit
                    };
                }
            }
            placement.constrain(row.entry, row.exit, row.frequency * references.max(1) as f64, entry, exit);
        }
        placement.scan()
    }

    /// `_grown`: the through blocks around each bundle that turned positive.
    /// The rows taken in, in the order they were.
    pub(super) fn grow(
        &self,
        placement: &mut spillplacement::Placement,
        mut blocked: Option<&mut dyn FnMut(usize) -> Option<(i64, i64)>>,
    ) -> Vec<u32> {
        let graph = &self.blocks.graph;
        let mut todo: Vec<bool> = self.rows.iter().map(|row| row.uses.is_none()).collect();
        let mut active: Vec<u32> = Vec::new();
        loop {
            let added = active.len();
            for bundle in placement.recent.clone() {
                for block in &placement.bundles.blocks[bundle] {
                    let row = self.row_of[graph.position[block]];
                    if row != NONE && std::mem::take(&mut todo[row as usize]) {
                        active.push(row);
                    }
                }
            }
            if active.len() == added {
                return active;
            }
            let new = &active[added..];
            match blocked.as_mut() {
                Some(blocked) => {
                    let mut open = Vec::new();
                    for &row in new {
                        let info = &self.rows[row as usize];
                        match blocked(row as usize) {
                            None => open.push(row),
                            Some((low, high)) => placement.constrain(
                                info.entry,
                                info.exit,
                                info.frequency,
                                if low < 0 { Border::MustSpill } else { Border::PrefSpill },
                                if high >= info.tail { Border::MustSpill } else { Border::PrefSpill },
                            ),
                        }
                    }
                    for row in open {
                        let info = &self.rows[row as usize];
                        placement.link_bundles(info.entry, info.exit, info.frequency);
                    }
                }
                None => {
                    for &row in new {
                        let info = &self.rows[row as usize];
                        placement.prefer_stack(info.entry, info.exit, info.frequency * 2.0);
                    }
                }
            }
            placement.iterate();
        }
    }

    /// `_held`: what the bundles that settled in a register hold of each row.
    pub(super) fn held(
        &self,
        active: &[u32],
        settled: &[usize],
        blocked: &mut dyn FnMut(usize) -> Option<(i64, i64)>,
    ) -> Held {
        let mut held = Held::of(self.rows.len());
        let borders = |row: &Row, live_in: bool, live_out: bool| {
            (live_in && settled.binary_search(&row.entry).is_ok(), live_out && settled.binary_search(&row.exit).is_ok())
        };
        for (at, row) in self.rows.iter().enumerate() {
            let Some((first, last, _)) = row.uses else { break };
            let (entry, exit) = borders(row, row.live_in, row.live_out);
            let (found, count) = ranges_in(row.len, entry, exit, Some((first, last)), blocked(at));
            held.set(at, found, count);
        }
        for &at in active {
            let row = &self.rows[at as usize];
            let (entry, exit) = borders(row, true, true);
            let (found, count) = ranges_in(row.len, entry, exit, None, blocked(at as usize));
            held.set(at as usize, found, count);
        }
        held
    }

    /// A table that holds nothing.
    pub(super) fn nothing(&self) -> Held {
        Held::of(self.rows.len())
    }

    /// `compact` less the rows `best` holds anything of.
    pub(super) fn take_out(
        &self,
        compact: &mut Held,
        best: &Held,
    ) {
        for at in 0..self.rows.len() {
            if best.count[at] > 0 {
                compact.count[at] = 0;
            }
        }
    }

    /// The region `held` makes: what the best candidate hands on.
    pub(super) fn region(
        &self,
        held: &Held,
    ) -> Region {
        #[cfg(test)]
        MADE.with(|made| made.set(made.get() + 1));
        let mut spans: Vec<(i64, Vec<(usize, usize)>)> = Vec::new();
        for (at, row) in self.rows.iter().enumerate() {
            if held.count[at] > 0 {
                spans.push((row.at, held.of_row(at).to_vec()));
            }
        }
        spans.sort_unstable_by_key(|one| one.0);
        Region { spans: spans.into_iter().collect() }
    }

    /// `_benefit` of the region `held` makes, less nothing: 0 where it holds
    /// nothing or the whole range. The sums are made in the order the scan of
    /// the region's blocks makes them.
    pub(super) fn price(
        &self,
        held: &Held,
        live: &dyn liverows::LiveAt,
        placement: &spillplacement::Placement,
        saving: &dyn Fn(&Insn) -> f64,
    ) -> f64 {
        if held.count.iter().all(|count| *count == 0) {
            return 0.0;
        }
        let whole =
            self.rows.iter().enumerate().all(|(at, row)| held.count[at] == 1 && held.ranges[at][0] == (0, row.len));
        if whole {
            return 0.0;
        }
        let body = self.blocks.body;
        let saved: f64 = self
            .in_order
            .iter()
            .filter(|row| self.rows[**row as usize].uses.is_some())
            .map(|&at| {
                let row = &self.rows[at as usize];
                let ranges = held.of_row(at as usize);
                self.named[row.named.clone()]
                    .iter()
                    .filter(|position| ranges.iter().any(|(from, to)| **position >= *from && **position < *to))
                    .map(|position| saving(&body.blocks[row.position].insns[*position]))
                    .sum::<f64>()
                    * row.frequency
            })
            .sum();
        saved - self.copies(held, live, placement)
    }

    /// The copies carving the region `held` makes would add, each weighted by
    /// its block's frequency: `crossings_in`, summed.
    fn copies(
        &self,
        held: &Held,
        live: &dyn liverows::LiveAt,
        placement: &spillplacement::Placement,
    ) -> f64 {
        let (body, graph) = (self.blocks.body, &self.blocks.graph);
        let value = self.value;
        let written = self
            .rows
            .iter()
            .enumerate()
            .any(
                |(at, row)| {
                    let ranges = held.of_row(at);
                    self.named[row.named.clone()]
                        .iter()
                        .any(
                            |position| body.blocks[row.position].insns[*position].defines.contains(&value)
                                && ranges.iter().any(|(from, to)| *position >= *from && *position < *to),
                        )
                },
            );
        // A block is in a crossing only if the region has it or has a successor
        // of it, in the order the blocks come.
        let mut near: Vec<usize> = Vec::new();
        for (at, row) in self.rows.iter().enumerate() {
            if held.count[at] > 0 {
                near.push(row.position);
                near.extend(graph.parents[row.position].iter().copied());
            }
        }
        near.sort_unstable();
        near.dedup();
        let frequency = |row: u32, at: i64| {
            if row == NONE { placement.frequency[&at] } else { self.rows[row as usize].frequency }
        };
        let enters = |row: u32| row != NONE && held.of_row(row as usize).first().is_some_and(|one| one.0 == 0);
        let mut crossings: Vec<f64> = Vec::new();
        for position in near {
            let block = &body.blocks[position];
            let at = self.row_of[position];
            let here = if at == NONE { &[][..] } else { held.of_row(at as usize) };
            if !here.is_empty() {
                let row = &self.rows[at as usize];
                let exit = row.live_out;
                let named = &self.named[row.named.clone()];
                let before = |position: usize| _live_before_at(block, named, value, exit, position);
                for (from, to) in here {
                    if *from > 0 && before(*from) {
                        crossings.push(row.frequency);
                    }
                    if written && *to < block.insns.len() && before(*to) {
                        crossings.push(row.frequency);
                    }
                }
            }
            let leaving = here.last().is_some_and(|one| one.1 == block.insns.len());
            let from = if at == NONE { placement.frequency[&block.at] } else { self.rows[at as usize].frequency };
            let successors = if at == NONE { None } else { Some(self.rows[at as usize].next.clone()) };
            for (number, next) in block.succ.iter().enumerate() {
                let row = match &successors {
                    Some(range) => self.next[range.start + number],
                    None => graph.position.get(next).map_or(NONE, |place| self.row_of[*place]),
                };
                let entering = enters(row);
                let across = if row == NONE { live.live_in(*next, value) } else { self.rows[row as usize].live_in };
                if leaving != entering && across && (entering || written) {
                    crossings.push(from.min(frequency(row, *next)));
                }
            }
        }
        let entry = body.blocks.iter().find(|block| block.at == body.entry);
        let entry_row = graph.position.get(&body.entry).map_or(NONE, |place| self.row_of[*place]);
        if enters(entry_row)
            && entry.is_some_and(|block| live.live_in(block.at, value) && !block.arrives().contains(&value))
        {
            crossings.push(frequency(entry_row, body.entry));
        }
        crossings.iter().copied().sum()
    }

    /// The interference of a register in a row, asked once: the row's span of
    /// the register's holders and destroyers.
    pub(super) fn interference<'b>(
        &'b self,
        occupied: &'b Occupied,
        register: RegId,
        width: u32,
        index: &'b Indexes,
    ) -> Blocked<'b> {
        Blocked { frame: self, cursor: occupied.cursor(register, width), index, asked: vec![None; self.rows.len()] }
    }
}

/// What a candidate register meets in each row, found when first asked.
pub(super) struct Blocked<'b> {
    frame: &'b Frame<'b>,
    cursor: Cursor<'b>,
    index: &'b Indexes,
    asked: Vec<Option<Option<(i64, i64)>>>,
}

impl Blocked<'_> {
    pub(super) fn at(
        &mut self,
        row: usize,
    ) -> Option<(i64, i64)> {
        let (frame, cursor, index) = (self.frame, &self.cursor, self.index);
        *self
            .asked[row]
            .get_or_insert_with(
                || cursor.at(frame.rows[row].span).map(|slots| _positions(index, frame.block(row), slots)),
            )
    }
}
