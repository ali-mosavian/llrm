//! Repeated instruction sequences of a module as called routines, when that is
//! fewer bytes.
//!
//! # Switches
//! Off: the `Request` pass `-fno-outline` (tests; `llrm-cc` and `llrm-6502-llc`
//! do not take it) or `LLRM_6502_NO_OUTLINE=1` in the environment (an A/B of
//! one binary). `LLRM_6502_OUTLINE_PLAIN=0` drops only the variant outlined
//! before known constants move.
//!
//! # How it works
//! Translation of the pin's `MachineOutliner` (`llvm/lib/CodeGen/
//! MachineOutliner.cpp`), over the finished text of the module:
//!
//! 1. *InstructionMapper*: each instruction is a number by its opcode and
//!    operand; a line that may not be outlined (a label, a branch, a stack
//!    instruction, a raw line) is a number of its own, so no repeat contains
//!    it.
//! 2. *SuffixTree*: Ukkonen's construction over the numbers, in time linear in
//!    their count. Each internal node is a repeated substring, its leaves the
//!    places it starts.
//! 3. *findCandidates*, *getOutliningCandidateInfo*: a substring's candidates
//!    are its starts that do not overlap; its benefit is the bytes of the
//!    candidates less the call at each, the body and its frame. A body that
//!    ends in `rts` is the *tail call* form (`MachineOutlinerTailCall`): the
//!    candidates are `jmp`s and the body needs no frame of its own.
//! 4. *outline*: the best benefit first, a candidate that overlaps one already
//!    outlined is dropped, and a function that is left without a benefit is not
//!    made.
//!
//! The pin outlines for bytes alone, and so does this pass at `-Os`. At the
//! speed levels a place inside a loop may be given up: a call costs 12 clocks
//! (`jsr` and `rts`, a `jmp` 3) a run, a loop runs its body [`LOOP_TRIPS`]
//! times for each entry (the pin's `BranchProbabilityInfo` loop scale, which
//! `MachineBlockFrequencyInfo` applies; a back edge here is a backward branch
//! of the text), and the place goes when each byte it saves would cost more
//! than `clocks_per_byte` clocks (`alloc::Tuning`, the rate every size-reducing
//! choice of the target is made at). A place outside every loop is never given
//! up.
//!
//! The pin's targets must keep a register the call clobbers (the link register
//! of AArch64) live across it. A `jsr` and its `rts` change no register, flag
//! or cell, so no boundary has anything to save: the target hook
//! (`getOutliningTypeImpl`) refuses what a call does touch, the stack. A
//! sequence has no stack instruction (`pha`, `pla`, `php`, `plp`, `tsx`, `txs`,
//! `phx`, `phy`, `plx`, `ply`), no label, branch or jump, no `rts` but its last
//! instruction, and no call but to a runtime routine other than the one that
//! pushes a frame (`__csave`): a call to a function of the program may recurse,
//! and every level would hold two more bytes of the return stack.

use std::collections::HashMap;

use crate::asm::{Inst, Line, Op, Operand};

/// What a call and its return cost.
const CALL: usize = 3;
/// The `rts` of a body that does not end in one.
const FRAME: usize = 1;

/// Clocks a call and its return add to a run of the sequence it replaces.
const CALL_CLOCKS: u64 = 12;
/// Clocks a jump to a body that ends in its own `rts` adds.
const JUMP_CLOCKS: u64 = 3;
/// Runs of a loop's body for one entry: a back edge taken 31 times in 32
/// (`BranchProbabilityInfo`'s loop heuristic, the scale of the pin's
/// `MachineBlockFrequencyInfo`).
const LOOP_TRIPS: u64 = 32;
/// The loops a place counts as being in, at most.
const MOST_LOOPS: u32 = 3;

/// [`run_at`] at the rate of the speed levels.
pub fn run(text: &str) -> String {
    run_at(text, Some(crate::alloc::Tuning::SPEED.clocks_per_byte))
}

/// `text` with its repeated sequences outlined, and the same text when none
/// pays. With a rate, a place in a loop is given up where each byte it saves
/// would cost more than `clocks_per_byte` clocks; with none, only bytes count.
pub fn run_at(
    text: &str,
    clocks_per_byte: Option<u32>,
) -> String {
    run_priced(text, clocks_per_byte, None)
}

/// [`run_at`], and with a `share` (percent) a place is given
/// up where the share of the module's clocks its calls add is more than
/// `share` times the share of the module's bytes it saves (a call and its
/// return add 12 clocks a run).
pub fn run_priced(
    text: &str,
    clocks_per_byte: Option<u32>,
    share: Option<u64>,
) -> String {
    let lines: Vec<Line> = text.lines().map(Line::parse).collect();
    if lines.iter().filter(|line| matches!(line, Line::Inst(_))).count() < 2 * (CALL + FRAME) {
        return text.to_owned();
    }
    let mapped = Mapper::new(&lines);
    let weights = weights(&lines);
    let share = share.map(|percent| {
        let clocks: u64 = lines
            .iter()
            .zip(&weights)
            .filter_map(|(line, weight)| {
                let inst = line.inst()?;
                let cycles = crate::timing::of(crate::dagisel::hooks::Cpu::Mos6502, inst)
                    .map_or(3, |cost| u64::from(cost.cycles));
                Some(weight * cycles)
            })
            .sum();
        (percent, bytes_of(&lines) as u64, clocks)
    });
    let chosen = select(&lines, &mapped, &weights, clocks_per_byte.map(u64::from), share);
    if chosen.is_empty() {
        return text.to_owned();
    }
    rewrite(&lines, &chosen, text.ends_with('\n'))
}

/// Whether an instruction may be inside an outlined body
/// (`getOutliningTypeImpl`).
///
/// No register is saved at a boundary because none needs it: a `jsr` and the
/// `rts` that returns from it change no `a`, `x`, `y`, flag or cell, so the
/// state after the call is the state after the sequence it replaces. What a
/// call does touch is the stack (the return address, two bytes of the 256 of
/// the return stack) and the program counter, so those are what is refused:
/// the stack instructions (a `pha` whose `pla` is in the caller would be
/// popped as half of the return address), labels, branches and jumps (the body
/// is entered at its top and left at its end), an `rts` that is not the last
/// instruction, `__csave` and `__crest` (a frame kept on the stack), a call to
/// a function of the program (it may recurse, and each level would hold two
/// more bytes), and a call to an outlined body (the chain would be as deep as
/// the nesting, which nothing bounds). With these, a body adds two bytes to the
/// stack of the code it replaces, at most, and no more at any recursion depth.
fn legal(
    inst: &Inst,
    last: bool,
) -> bool {
    use Op::*;
    match inst.op {
        Pha | Php | Phx | Phy | Pla | Plp | Plx | Ply | Tsx | Txs => false,
        Jmp | Sei | Sed | Cld => false,
        op if op.is_branch() => false,
        Rts => last,
        Jsr => matches!(
            &inst.operand,
            Operand::Direct(name) if name.starts_with("__")
                && !name.starts_with("__outl_")
                && !name.contains("csave")
                && !name.contains("crest")
        ),
        _ => match inst.operand.name() {
            // `*+3`, a local label, an expression with a location counter
            Some(name) => !name.contains(['*', '@']),
            None => true,
        },
    }
}

/// How many times each line runs for one run of its function, by the loops
/// around it: a backward branch makes a loop of the lines from its target to
/// itself.
fn weights(lines: &[Line]) -> Vec<u64> {
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for (at, line) in lines.iter().enumerate() {
        if let Line::Label(name) = line {
            labels.insert(name, at);
        }
    }
    // a line's depth is the loops that begin at or before it, less those that
    // ended before it
    let mut change = vec![0i32; lines.len() + 1];
    for (at, line) in lines.iter().enumerate() {
        let Line::Inst(Inst { op, operand, .. }) = line else { continue };
        if !(op.is_branch() || *op == Op::Jmp) {
            continue;
        }
        if let Some(&target) = operand.name().and_then(|name| labels.get(name)).filter(|&&target| target < at) {
            change[target] += 1;
            change[at + 1] -= 1;
        }
    }
    let mut depth = 0i32;
    let in_function: Vec<u64> = change[..lines.len()]
        .iter()
        .map(|delta| {
            depth += delta;
            LOOP_TRIPS.pow((depth as u32).min(MOST_LOOPS))
        })
        .collect();
    scaled_by_callers(lines, in_function)
}

/// `in_function` weights each line by the loops of its own function; a
/// function called from a loop runs as often as that loop does, so its lines
/// are scaled by the weight of its call sites (`staticlocals::call_weights`:
/// each `jsr` counts the run of its own line, the callers' entry runs
/// included). A call of a function by itself is one more loop level of its
/// own lines: the recursion runs the body again for each depth.
fn scaled_by_callers(
    lines: &[Line],
    in_function: Vec<u64>,
) -> Vec<u64> {
    let calls = |line: &Line| match line {
        Line::Inst(Inst { op: Op::Jsr, operand, .. }) => operand.name().map(str::to_owned),
        _ => None,
    };
    // a function is a label something calls or the module exports
    let mut callees: std::collections::HashSet<String> = lines.iter().filter_map(calls).collect();
    for line in lines {
        if let Line::Raw(text) = line {
            if let Some(name) = text.trim().strip_prefix(".export ") {
                callees.insert(name.trim().to_owned());
            }
        }
    }
    // the function a line is in: the last label that something calls
    let mut owner: Vec<Option<usize>> = Vec::with_capacity(lines.len());
    let mut names: Vec<&str> = Vec::new();
    let mut at_name: HashMap<&str, usize> = HashMap::new();
    let mut current = None;
    for line in lines {
        if let Line::Label(name) = line {
            if callees.contains(name.as_str()) {
                names.push(name);
                at_name.insert(name, names.len() - 1);
                current = Some(names.len() - 1);
            }
        }
        owner.push(current);
    }
    if names.is_empty() {
        return in_function;
    }
    let most = LOOP_TRIPS.pow(MOST_LOOPS);
    let mut entry = vec![1u64; names.len()];
    let mut recursive = vec![false; names.len()];
    for _ in 0..4 {
        let mut next = vec![0u64; names.len()];
        for (at, line) in lines.iter().enumerate() {
            let Some(callee) = calls(line).and_then(|name| at_name.get(name.as_str()).copied()) else { continue };
            if owner[at] == Some(callee) {
                recursive[callee] = true;
                continue;
            }
            let caller = owner[at].map_or(1, |function| entry[function]);
            next[callee] = (next[callee] + in_function[at] * caller).min(most);
        }
        for (one, sum) in entry.iter_mut().zip(next) {
            *one = sum.max(1);
        }
    }
    for (one, recursion) in entry.iter_mut().zip(&recursive) {
        if *recursion {
            *one = (*one * LOOP_TRIPS).min(most);
        }
    }
    in_function
        .iter()
        .zip(&owner)
        .map(|(weight, function)| (weight * function.map_or(1, |one| entry[one])).min(most))
        .collect()
}

/// The numbers of the instructions (`InstructionMapper`).
struct Mapper {
    ids: Vec<u32>,
    /// Whether a line is an instruction that may be in a body.
    legal: Vec<bool>,
}

impl Mapper {
    fn new(lines: &[Line]) -> Mapper {
        let mut by_text: HashMap<String, u32> = HashMap::new();
        let (mut ids, mut legal_at) = (Vec::with_capacity(lines.len() + 1), Vec::with_capacity(lines.len()));
        let mut unique = u32::MAX;
        let mut in_macro = false;
        for line in lines {
            let mapped = match line {
                Line::Raw(text) => {
                    let code = text.split(';').next().unwrap_or("").trim();
                    if code.starts_with(".macro") {
                        in_macro = true;
                    }
                    if code.starts_with(".endmacro") {
                        in_macro = false;
                    }
                    None
                }
                Line::Inst(inst) if !in_macro && legal(inst, true) => {
                    let fresh = by_text.len() as u32;
                    Some(*by_text.entry(inst.to_string()).or_insert(fresh))
                }
                _ => None,
            };
            legal_at.push(mapped.is_some());
            ids.push(mapped.unwrap_or_else(|| {
                unique -= 1;
                unique
            }));
        }
        // the terminator the tree is built to need
        ids.push(u32::MAX);
        Mapper { ids, legal: legal_at }
    }
}

// ---- the suffix tree ----

const LEAF: usize = usize::MAX;

struct Node {
    /// The edge into the node is `text[start..end]`; a leaf's `end` is `LEAF`
    /// until the construction is over.
    start: usize,
    end: usize,
    link: usize,
    children: HashMap<u32, usize>,
}

/// Ukkonen's online construction (`SuffixTree::SuffixTree`).
struct SuffixTree<'a> {
    text: &'a [u32],
    nodes: Vec<Node>,
}

impl<'a> SuffixTree<'a> {
    fn new(text: &'a [u32]) -> SuffixTree<'a> {
        let mut tree = SuffixTree { text, nodes: vec![Node { start: 0, end: 0, link: 0, children: HashMap::new() }] };
        let (mut active_node, mut active_edge, mut active_len) = (0usize, 0usize, 0usize);
        let mut remainder = 0usize;
        for i in 0..text.len() {
            remainder += 1;
            let mut last_new: Option<usize> = None;
            while remainder > 0 {
                if active_len == 0 {
                    active_edge = i;
                }
                let edge = text[active_edge];
                let Some(&next) = tree.nodes[active_node].children.get(&edge) else {
                    let leaf = tree.leaf(i);
                    tree.nodes[active_node].children.insert(edge, leaf);
                    if let Some(open) = last_new.take() {
                        tree.nodes[open].link = active_node;
                    }
                    remainder -= 1;
                    Self::advance(&mut tree, &mut active_node, &mut active_edge, &mut active_len, i, remainder);
                    continue;
                };
                let length = tree.edge_length(next, i + 1);
                if active_len >= length {
                    active_edge += length;
                    active_len -= length;
                    active_node = next;
                    continue;
                }
                if text[tree.nodes[next].start + active_len] == text[i] {
                    if let Some(open) = last_new.take().filter(|_| active_node != 0) {
                        tree.nodes[open].link = active_node;
                    }
                    active_len += 1;
                    break;
                }
                let split = tree.nodes.len();
                let start = tree.nodes[next].start;
                tree.nodes.push(Node { start, end: start + active_len, link: 0, children: HashMap::new() });
                tree.nodes[active_node].children.insert(edge, split);
                let leaf = tree.leaf(i);
                tree.nodes[split].children.insert(text[i], leaf);
                tree.nodes[next].start += active_len;
                let moved = text[tree.nodes[next].start];
                tree.nodes[split].children.insert(moved, next);
                if let Some(open) = last_new {
                    tree.nodes[open].link = split;
                }
                last_new = Some(split);
                remainder -= 1;
                Self::advance(&mut tree, &mut active_node, &mut active_edge, &mut active_len, i, remainder);
            }
        }
        tree
    }

    /// The rule 1 and 3 steps after a suffix is added.
    fn advance(
        tree: &mut SuffixTree,
        active_node: &mut usize,
        active_edge: &mut usize,
        active_len: &mut usize,
        i: usize,
        remainder: usize,
    ) {
        if *active_node == 0 && *active_len > 0 {
            *active_len -= 1;
            *active_edge = i + 1 - remainder;
        } else if *active_node != 0 {
            *active_node = tree.nodes[*active_node].link;
        }
    }

    fn leaf(
        &mut self,
        start: usize,
    ) -> usize {
        self.nodes.push(Node { start, end: LEAF, link: 0, children: HashMap::new() });
        self.nodes.len() - 1
    }

    fn edge_length(
        &self,
        node: usize,
        now: usize,
    ) -> usize {
        let node = &self.nodes[node];
        (if node.end == LEAF { now } else { node.end }) - node.start
    }

    /// The internal nodes below the root, each as its string length and the
    /// places the string starts (`RepeatedSubstringIterator`).
    fn repeats(&self) -> Vec<(usize, Vec<usize>)> {
        let n = self.text.len();
        let mut found = Vec::new();
        // (node, string length at it, children done)
        let mut stack: Vec<(usize, usize, bool)> = vec![(0, 0, false)];
        let mut starts: HashMap<usize, Vec<usize>> = HashMap::new();
        while let Some((node, depth, done)) = stack.pop() {
            let me = &self.nodes[node];
            if me.children.is_empty() {
                starts.insert(node, vec![n - depth]);
                continue;
            }
            if !done {
                stack.push((node, depth, true));
                for &child in me.children.values() {
                    let length = self.edge_length(child, n);
                    stack.push((child, depth + length, false));
                }
                continue;
            }
            let mut mine = Vec::new();
            for child in me.children.values() {
                if let Some(mut theirs) = starts.remove(child) {
                    mine.append(&mut theirs);
                }
            }
            if node != 0 {
                mine.sort_unstable();
                found.push((depth, mine.clone()));
            }
            starts.insert(node, mine);
        }
        found
    }
}

// ---- the candidates ----

/// A body and the places it replaces.
struct Chosen {
    /// The first line of the body in the text.
    body: usize,
    length: usize,
    /// Where each call goes.
    places: Vec<usize>,
    tail: bool,
}

fn bytes_of(lines: &[Line]) -> usize {
    lines.iter().filter_map(Line::inst).map(Inst::bytes).sum()
}

/// The best benefit first, a sequence that overlaps one taken is not
/// (`MachineOutliner::outline`, `pruneOverlaps`).
fn select(
    lines: &[Line],
    mapped: &Mapper,
    weights: &[u64],
    clocks_per_byte: Option<u64>,
    share: Option<(u64, u64, u64)>,
) -> Vec<Chosen> {
    let tree = SuffixTree::new(&mapped.ids);
    // lines that end a body: an `rts`, and only when nothing follows it in the
    // string
    let rts_before: Vec<usize> = {
        let mut sum = 0;
        let mut counts = Vec::with_capacity(lines.len() + 1);
        for line in lines {
            counts.push(sum);
            if matches!(line, Line::Inst(Inst { op: Op::Rts, .. })) {
                sum += 1;
            }
        }
        counts.push(sum);
        counts
    };
    struct Candidate {
        length: usize,
        starts: Vec<usize>,
        bytes: usize,
        tail: bool,
        benefit: i64,
    }
    let mut candidates = Vec::new();
    for (length, starts) in tree.repeats() {
        if length < 2 || starts.len() < 2 {
            continue;
        }
        let first = starts[0];
        // every line of the string is an instruction that may be in a body
        if first + length > lines.len() || mapped.legal[first..first + length].iter().any(|legal| !legal) {
            continue;
        }
        // an `rts` only as the last instruction
        let last_is_rts = matches!(
            lines[first + length - 1],
            Line::Inst(Inst { op: Op::Rts, .. })
        );
        let rts_inside = rts_before[first + length - 1] - rts_before[first];
        if rts_inside > 0 {
            continue;
        }
        let bytes = bytes_of(&lines[first..first + length]);
        let mut count = 0;
        let mut end = 0;
        for &start in &starts {
            if start >= end {
                count += 1;
                end = start + length;
            }
        }
        let benefit = benefit(bytes, count, last_is_rts);
        if benefit > 0 {
            candidates.push(Candidate { length, starts, bytes, tail: last_is_rts, benefit });
        }
    }
    // the pin keeps the order the tree found them in among equal benefits
    candidates
        .sort_by(|a, b| b.benefit.cmp(&a.benefit).then(b.length.cmp(&a.length)).then(a.starts[0].cmp(&b.starts[0])));
    let mut taken = vec![false; lines.len()];
    let mut chosen = Vec::new();
    for candidate in candidates {
        let mut places = Vec::new();
        let mut end = 0;
        for &start in &candidate.starts {
            if start >= end && taken[start..start + candidate.length].iter().all(|taken| !taken) {
                places.push(start);
                end = start + candidate.length;
            }
        }
        // a place in a loop is given up where its bytes cost more clocks than
        // the rate allows; outside a loop it stays
        if let Some(rate) = clocks_per_byte {
            let per_run = if candidate.tail { JUMP_CLOCKS } else { CALL_CLOCKS };
            let saved = candidate.bytes.saturating_sub(CALL) as u64;
            places.retain(|&place| weights[place] == 1 || weights[place] * per_run <= rate * saved);
        }
        // a place outside every loop is given up where the clocks its calls add
        // are a larger share of the module's clocks than the bytes it
        // saves are of the module's bytes
        if let Some((percent, module_bytes, module_clocks)) = share {
            let per_run = if candidate.tail { JUMP_CLOCKS } else { CALL_CLOCKS };
            let outside = places.iter().filter(|&&place| weights[place] == 1).count() as u64;
            let saved = benefit(candidate.bytes, places.len(), candidate.tail).max(0) as u64;
            if u128::from(outside * per_run) * u128::from(module_bytes)
                > u128::from(percent) * u128::from(saved) * u128::from(module_clocks)
            {
                places.retain(|&place| weights[place] != 1);
            }
            // and a place in a loop where its own runs add a larger share of
            // the clocks than its bytes are of the module
            let saved = candidate.bytes.saturating_sub(CALL) as u64;
            places.retain(|&place| {
                weights[place] == 1
                    || u128::from(weights[place] * per_run) * u128::from(module_bytes)
                        <= u128::from(percent) * u128::from(saved) * u128::from(module_clocks)
            });
        }
        if places.len() < 2 || benefit(candidate.bytes, places.len(), candidate.tail) <= 0 {
            continue;
        }
        for &place in &places {
            taken[place..place + candidate.length].fill(true);
        }
        chosen.push(Chosen { body: places[0], length: candidate.length, places, tail: candidate.tail });
    }
    chosen
}

/// The bytes saved by `count` calls of a body of `bytes`
/// (`OutlinedFunction::getBenefit`: the cost of the sequences not outlined less
/// the cost of the calls, the body and its frame).
fn benefit(
    bytes: usize,
    count: usize,
    tail: bool,
) -> i64 {
    let frame = if tail { 0 } else { FRAME };
    (count * bytes) as i64 - (count * CALL + bytes + frame) as i64
}

fn rewrite(
    lines: &[Line],
    chosen: &[Chosen],
    newline: bool,
) -> String {
    let mut replace: HashMap<usize, usize> = HashMap::new();
    for (index, one) in chosen.iter().enumerate() {
        for &place in &one.places {
            replace.insert(place, index);
        }
    }
    let name = |index: usize| format!("__outl_{index}");
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut at = 0;
    let mut segment: Option<&str> = None;
    while at < lines.len() {
        if let Some(&index) = replace.get(&at) {
            let one = &chosen[index];
            let op = if one.tail { "jmp" } else { "jsr" };
            out.push(format!("  {op} {}", name(index)));
            at += one.length;
            continue;
        }
        if let Line::Raw(text) = &lines[at]
            && text.trim_start().starts_with(".segment")
        {
            segment = Some(text);
        }
        out.push(lines[at].to_string());
        at += 1;
    }
    out.push(segment.unwrap_or(".segment \"CODE\"").to_owned());
    for (index, one) in chosen.iter().enumerate() {
        out.push(format!("{}:", name(index)));
        for line in &lines[one.body..one.body + one.length] {
            out.push(line.to_string());
        }
        if !one.tail {
            out.push("  rts".to_owned());
        }
    }
    let mut text = out.join("\n");
    if newline {
        text.push('\n');
    }
    text
}
