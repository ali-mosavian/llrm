//! Which 32-bit register roots hold zero in their upper half, and the `movzx`
//! that makes it so where a 32-bit address needs it.
//!
//! # How it works
//! In 16-bit code a 32-bit effective address through a register is the prefixed
//! form of the 16-bit one. It names the same byte only if the register's upper
//! half is zero. A forward dataflow over the allocated body keeps, before each
//! instruction, a set of roots (one bit each) known to be zero above bit 15.
//!
//! ```text
//!  entry            nothing known (empty set)
//!  meet             intersection over predecessors; unreached blocks are top
//!  movzx r32,r/m    sets r       xor r,r / sub r,r        sets r
//!  mov r32,imm      sets r when imm is 0..0xFFFF and has no relocation
//!  and r32,imm      sets r when imm is 0..0xFFFF and the source is r
//!  mov a,b          a is zero when b is
//!  xchg a,b         swaps the two bits
//!  word/byte write  keeps the bit; any write of lanes 2 or 3 clears it
//!  unknown effects  clear every bit (jumps and branches write nothing)
//! ```
//!
//! `established` makes the facts true where the code needs them. A cell that
//! reads a root 32 bits wide, whether the register holds a value or not (the
//! frame pointer), needs that root's upper half zero. Where the analysis does
//! not prove it, the pass inserts `movzx r32,r16`: in the preheader of the
//! outermost loop around the cell in which nothing disturbs the root, when the
//! loop's entry is one block with one successor, or else before the first such
//! cell in the block. It repeats until no cell lacks the fact.
//!
//! # Example
//! `mov ecx, 2; xchg esi, ecx; nop` in one block, nothing known on entry.
//!
//! Schematic, from the fixture of test
//! `test_an_exchange_swaps_the_zero_upper_halves` (not printer output):
//! ```lir
//! mov ecx, 2          ; ecx is zero above bit 15
//! xchg esi, ecx       ; the two bits swap: esi zero, ecx not
//! nop                 ; before here: esi in the set, ecx not
//! ```
//!
//! The exchange moves the zero upper half from `ecx` to `esi`, so a `movzx` of
//! `esi` before it indexes would be redundant.
//!
//! From unit test `test_an_exchange_swaps_the_zero_upper_halves` in
//! `upperzero.rs`.
//!
//! `hoisted` (m32 and other targets with the rule `zero_extensions`) puts the
//! same `movzx` in front of a loop for a word register whose widening `movzx`
//! the loop repeats, where the loop writes the register's low word only: the
//! widening is then a copy (`peephole.peep`, `zero_extended_copy`). The frame
//! register is zeroed when the body writes it (no frame pointer: its cells are
//! esp-relative later) and no instruction reads it whole by name.
//!
//! # Provides
//! `before`, the zero roots before each instruction, keyed by instruction
//! identity. The answer is kept between asks and brought up to date from the
//! blocks that changed ([`crate::backend::incremental`]);
//! `LLRM_CHECK_ZEROROOTS=1` solves the body from scratch after every answer and
//! asserts they agree. `preheaded` places `movzx` in loop preheaders for roots
//! a caller asks for.
//!
//! # Runs
//! `established` is the last sub-pass of [`crate::backend::peephole`], run when
//! the body has such a cell (`needs::UNHELD`) and not on a native target, whose
//! addresses read whole registers.
//! `hoisted` runs before the zero-extension rules of the same phase.
//!
//! # Used by
//! [`crate::backend::exactaddress`] (`before`, `preheaded`), and the rewrite
//! guards of the peephole (`upper_zero`).

use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::registers::Regs;

use crate::analysis::dataflow::{self, Direction};
use crate::analysis::loops;
use crate::backend::incremental::{Kept, Problem, Solution};
use crate::backend::liveness;
use crate::backend::peephole::{_register_effects, id};
use crate::model::ir::{Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::{HashMap, IndexMap};

/// The roots a general register names, one bit each.
pub const ROOTS: [RegId; 7] = llrm_x86::registers::ROOTS;

/// A set of roots, one bit per `ROOTS` entry.
pub type Roots = u8;

const ALL: Roots = (1 << ROOTS.len()) - 1;

/// The bit of `register`'s root, if it has one here.
pub fn bit(register: RegId) -> Option<Roots> {
    let root = crate::model::ir::root(register);
    ROOTS.iter().position(|one| *one == root).map(|index| 1 << index)
}

/// Whether a dword with this value has nothing above its low word.
fn is_word(value: i64) -> bool {
    (0..=0xFFFF).contains(&value)
}

/// The roots whose upper half `one` sets to zero.
fn zeroing(one: &Insn) -> Roots {
    let Some(what) = &one.what else {
        return 0;
    };
    let wide = |loc: &Loc| match loc {
        Loc::Reg(Reg { register, width: 4 }) => bit(*register),
        _ => None,
    };
    let [destination] = what.dests.as_slice() else {
        return 0;
    };
    let Some(root) = wide(destination) else {
        return 0;
    };
    let zeroes = match (what.op, what.name.as_deref(), what.sources.as_slice()) {
        (Operation::Extend, Some("movzx"), [Loc::Reg(Reg { width: 1 | 2, .. }) | Loc::Mem(_)]) => true,
        (_, Some("xor" | "sub"), [left, right]) => left == destination && right == destination,
        (Operation::Move, Some("mov"), [Loc::Imm(constant)]) => constant.address.is_none() && is_word(constant.value),
        // A mask of a word clears what lies above it.
        (Operation::Binary, Some("and"), [left, Loc::Imm(constant)]) => {
            left == destination && constant.address.is_none() && is_word(constant.value)
        }
        _ => false,
    };
    if zeroes { root } else { 0 }
}

/// The roots whose upper half `one` may leave other than it found it.
fn disturbed(
    regs: Regs,
    bits: u32,
    one: &Insn,
) -> Roots {
    // A jump or branch writes no register; no decoder answers for it.
    if liveness::_terminator(one.what.as_ref()) {
        return 0;
    }
    let effects = _register_effects(regs, bits, one, true, false).or_else(|| liveness::_declared(regs, one));
    let Some((_, writes)) = effects else {
        return ALL;
    };
    ROOTS
        .iter()
        .enumerate()
        .filter(|(_, root)| writes.contains(&(**root, 2)) || writes.contains(&(**root, 3)))
        .fold(0, |roots, (index, _)| roots | 1 << index)
}

/// What one instruction does to the roots known zero, worked out once from the
/// instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
    disturbed: Roots,
    zeroed: Roots,
    /// `(into, from)` of a whole-register copy.
    copy: Option<(Roots, Roots)>,
    /// The two roots an `xchg` of whole registers swaps.
    swap: Option<(Roots, Roots)>,
}

impl Step {
    fn of(
        regs: Regs,
        bits: u32,
        one: &Insn,
    ) -> Self {
        Self { disturbed: disturbed(regs, bits, one), zeroed: zeroing(one), copy: copy_of(one), swap: swap_of(one) }
    }

    /// The roots known zero after it, given those before it.
    fn after(
        &self,
        zero: Roots,
    ) -> Roots {
        if let Some((left, right)) = self.swap {
            let (had_left, had_right) = (zero & left != 0, zero & right != 0);
            let rest = zero & !(left | right);
            return rest | if had_right { left } else { 0 } | if had_left { right } else { 0 };
        }
        let copied = self.copy.map_or(0, |(into, from)| if zero & from != 0 { into } else { 0 });
        (zero & !self.disturbed) | self.zeroed | copied
    }
}

/// What `one` makes of `zero`, the roots known zero before it.
pub fn after(
    regs: Regs,
    bits: u32,
    one: &Insn,
    zero: Roots,
) -> Roots {
    Step::of(regs, bits, one).after(zero)
}

/// The two roots `xchg a, b` of two whole registers swaps: each now holds the
/// other's upper half.
fn swap_of(one: &Insn) -> Option<(Roots, Roots)> {
    let Some(Semantics { op: Operation::Exchange, dests, .. }) = &one.what else {
        return None;
    };
    let [Loc::Reg(Reg { register: left, width: 4 }), Loc::Reg(Reg { register: right, width: 4 })] = dests.as_slice()
    else {
        return None;
    };
    Some((bit(*left)?, bit(*right)?))
}

/// The roots a whole-register copy writes and reads: the root written has a
/// zero upper half where the root it copies from has.
fn copy_of(one: &Insn) -> Option<(Roots, Roots)> {
    let Some(Semantics { op: Operation::Move, dests, sources, .. }) = &one.what else {
        return None;
    };
    match (dests.as_slice(), sources.as_slice()) {
        ([Loc::Reg(Reg { register: into, width: 4 })], [Loc::Reg(Reg { register: from, width: 4 })]) => {
            Some((bit(*into)?, bit(*from)?))
        }
        _ => None,
    }
}

/// The roots zero above their word as a [`Problem`]: what a block does to them
/// is the steps of its instructions, and they flow on, meeting by intersection.
pub struct ZeroRoots;

impl Problem for ZeroRoots {
    type Summary = Vec<Step>;
    type Value = Roots;
    const FORWARD: bool = true;

    fn summarize(
        &self,
        body: &LirBody,
        block: &Block,
    ) -> Vec<Step> {
        let regs = body.regs();
        block.insns.iter().map(|one| Step::of(regs, body.bits, one)).collect()
    }

    fn start(&self) -> Roots {
        ALL
    }

    /// Nothing is known on entry to the body; elsewhere the roots every way in
    /// leaves zero, and a block nothing leads to keeps them all.
    fn entering(
        &self,
        at: i64,
        entry: i64,
        present: &[Roots],
        _missing: usize,
    ) -> Roots {
        if at == entry { 0 } else { present.iter().fold(ALL, |all, next| all & next) }
    }

    fn leaving(
        &self,
        steps: &Vec<Step>,
        zero: Roots,
    ) -> Roots {
        steps.iter().fold(zero, |zero, step| step.after(zero))
    }
}

/// The answer last given: the solution it came from, what it says before each
/// instruction, and which instructions it says it of, by block.
struct Given {
    solution: Rc<Solution<ZeroRoots>>,
    result: Rc<HashMap<usize, Roots>>,
    ids: IndexMap<i64, Vec<usize>>,
}

thread_local! {
    static KEPT: Kept<ZeroRoots> = const { Kept::new() };
    static GIVEN: std::cell::RefCell<Option<Given>> = const { std::cell::RefCell::new(None) };
}

/// How many times this thread has solved a body's zero roots from scratch (no
/// block left as the last answer had it), brought a held answer up to date, and
/// been asked of a body no pass changed.
pub fn asks() -> [usize; 3] {
    KEPT.with(Kept::asks)
}

/// Before each instruction, by `id`, the roots whose upper half is zero. See
/// [`crate::backend::incremental`]: the problem is the greatest fixed point, so
/// a component with a changed block is solved from every root zero.
pub fn before(body: &LirBody) -> Rc<HashMap<usize, Roots>> {
    let solution = KEPT.with(|kept| kept.ask(&ZeroRoots, body));
    let given = GIVEN.with(|given| given.borrow_mut().take());
    if let Some(same) = given.as_ref().filter(|given| Rc::ptr_eq(&given.solution, &solution)) {
        let result = Rc::clone(&same.result);
        GIVEN.with(|slot| *slot.borrow_mut() = given);
        return result;
    }
    // What was said of the blocks that are gone or not as they were is taken
    // back, and said again of the blocks that are not.
    let (mut result, mut ids) = match given {
        Some(given) => ((*given.result).clone(), given.ids),
        None => (HashMap::default(), IndexMap::default()),
    };
    ids.retain(|at, was| {
        let keep = solution.entering.contains_key(at) && !solution.refreshed.contains(at);
        if !keep {
            for one in was.iter() {
                result.remove(one);
            }
        }
        keep
    });
    for block in body.blocks.iter().filter(|block| solution.refreshed.contains(&block.at)) {
        let mut zero = solution.entering[&block.at];
        let mut said = Vec::with_capacity(block.insns.len());
        for (one, step) in block.insns.iter().zip(solution.summaries[&block.at].iter()) {
            result.insert(id(one), zero);
            said.push(id(one));
            zero = step.after(zero);
        }
        ids.insert(block.at, said);
    }
    let result = Rc::new(result);
    if llrm_support::env_flag!("LLRM_CHECK_ZEROROOTS") {
        assert!(
            *result == from_scratch(body),
            "{}: upper-half zeroes brought up to date differ from ones solved from scratch",
            body.name
        );
    }
    GIVEN.with(|slot| {
        *slot.borrow_mut() = Some(Given { solution, result: Rc::clone(&result), ids });
    });
    result
}

/// `before`, solved from nothing held: what `LLRM_CHECK_ZEROROOTS` compares an
/// answer brought up to date with.
fn from_scratch(body: &LirBody) -> HashMap<usize, Roots> {
    let regs = body.regs();
    let graph = &body.blocks;
    let predecessors = loops::predecessors(&graph);
    let nodes: Vec<&Block> = body.blocks.iter().collect();
    let blocks: IndexMap<i64, &Block> = nodes.iter().map(|block| (block.at, *block)).collect();
    let solved = dataflow::solve(
        &nodes,
        Direction::Forward,
        |_| ALL,
        |at, out| {
            let mut zero = if at == body.entry { 0 } else { ALL };
            for from in predecessors.get(&at).into_iter().flatten().filter(|from| out.contains_key(*from)) {
                zero &= out[from];
            }
            zero
        },
        |at, zero| blocks[&at].insns.iter().fold(*zero, |zero, one| after(regs, body.bits, one, zero)),
    );
    let into = solved.input;
    let mut result = HashMap::default();
    for block in &body.blocks {
        let mut zero = into[&block.at];
        for one in &block.insns {
            result.insert(id(one), zero);
            zero = after(regs, body.bits, one, zero);
        }
    }
    result
}

/// `movzx root,word` for each root in `roots`, before `block`'s terminator.
pub fn extended(
    regs: Regs,
    block: &LirBlock,
    roots: Roots,
) -> Block {
    let mut insns = block.insns.to_vec();
    let at = insns.last().map_or(block.at, |one| one.at);
    let position = if insns.last().is_some_and(|one| liveness::_leaves(one.what.as_ref())) {
        insns.len() - 1
    } else {
        insns.len()
    };
    for (index, root) in ROOTS.iter().enumerate() {
        if roots & 1 << index == 0 {
            continue;
        }
        let word = regs.named(*root, 2);
        let what = Semantics {
            name: Some("movzx".into()),
            dests: vec![Loc::Reg(Reg { register: *root, width: 4 })],
            sources: vec![Loc::Reg(Reg { register: word, width: 2 })],
            ..Semantics::new(Operation::Extend)
        };
        insns.insert(position, Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())));
    }
    block.with_insns(insns).into_inner().into()
}

/// `body` with the upper half of each loop's `roots` zeroed on entry, where
/// every way in may take it: one successor, and those lanes dead there
/// unless `held` says no value lives in them.
pub fn preheaded(
    body: &LirBody,
    wanted: &IndexMap<i64, Roots>,
    held: bool,
) -> (LirBody, IndexMap<i64, Roots>) {
    let regs = body.regs();
    let graph = &body.blocks;
    let found = loops::loops(&graph, Some(body.entry));
    let predecessors = loops::predecessors(&graph);
    let exits = liveness::dead_at_exit(body);
    let mut placed: IndexMap<i64, Roots> = IndexMap::default();
    let mut per_block: IndexMap<i64, Roots> = IndexMap::default();
    for (header, roots) in wanted {
        let Some(inside) = found.iter().find(|one| one.header == *header).map(|one| &one.body) else {
            continue;
        };
        let entries: Vec<i64> =
            predecessors.get(header).into_iter().flatten().copied().filter(|at| !inside.contains(at)).collect();
        let mut allowed = *roots;
        for (index, root) in ROOTS.iter().enumerate() {
            let upper = [(*root, 2), (*root, 3)];
            let takes = !entries.is_empty()
                && entries.iter().all(|at| {
                    body.blocks.iter().find(|block| block.at == *at).is_some_and(|block| block.succ == [*header])
                        && (!held || upper.iter().all(|lane| exits[at].contains(lane)))
                });
            if !takes {
                allowed &= !(1 << index);
            }
        }
        if allowed == 0 {
            continue;
        }
        placed.insert(*header, allowed);
        for at in entries {
            *per_block.entry(at).or_default() |= allowed;
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match per_block.get(&block.at) {
            Some(roots) => extended(regs, block, *roots),
            None => block.clone(),
        })
        .collect();
    (body.with_blocks(blocks), placed)
}

/// The roots `one`'s cells read 32 bits wide, through a register that holds a
/// value (a base or an index the allocator placed) or none (the frame pointer):
/// in 16-bit code the cell names the byte its 16-bit form does only if the
/// root's upper half is zero.
pub(crate) fn wide_roots(
    regs: Regs,
    one: &Insn,
) -> Roots {
    let Some(what) = &one.what else {
        return 0;
    };
    // A `lea` is arithmetic: the low 16 bits of `[e?x+e?x+k]` are those of the
    // 16-bit sum whatever the upper halves hold, and a 32-bit result is the
    // full sum; a cell that reads the result asks for its own upper half.
    if what.op == Operation::Address {
        return 0;
    }
    let wide = |register: RegId| if regs.width_of(register) == Some(4) { bit(register).unwrap_or(0) } else { 0 };
    what.dests
        .iter()
        .chain(&what.sources)
        .filter_map(|operand| match operand {
            Loc::Mem(cell) => Some(wide(cell.through) | wide(cell.index_through)),
            _ => None,
        })
        .fold(0, |roots, one| roots | one)
}

/// `body` with the upper half of each word register a loop's `movzx` widens
/// zeroed once, in the preheader of the outermost loop that writes the register
/// only below its upper half.
pub fn hoisted(body: &LirBody) -> LirBody {
    // The word register a `movzx` widens into another root.
    let widened = |one: &Insn| -> Option<Roots> {
        let what = one.what.as_ref()?;
        match (what.op, what.name.as_deref(), what.dests.as_slice(), what.sources.as_slice()) {
            (
                Operation::Extend,
                Some("movzx"),
                [Loc::Reg(wide @ Reg { width: 4, .. })],
                [Loc::Reg(word @ Reg { width: 2, .. })],
            ) if crate::model::ir::root(wide.register) != crate::model::ir::root(word.register) => bit(word.register),
            _ => None,
        }
    };
    if !body.blocks.iter().any(|block| block.insns.iter().any(|one| widened(one).is_some())) {
        return body.clone();
    }
    let regs = body.regs();
    let natural = loops::loops(&body.blocks, Some(body.entry));
    let zero = before(body);
    let untouched = |inside: &loops::Body, roots: Roots| {
        body.blocks
            .iter()
            .filter(|block| inside.contains(&block.at))
            .all(|block| block.insns.iter().all(|one| disturbed(regs, body.bits, one) & roots == 0))
    };
    let mut wanted: IndexMap<i64, Roots> = IndexMap::default();
    for block in &body.blocks {
        for one in &block.insns {
            let Some(roots) = widened(one).filter(|roots| zero[&id(one)] & roots == 0) else { continue };
            let outermost = natural
                .iter()
                .filter(|inside| inside.body.contains(&block.at) && untouched(&inside.body, roots))
                .max_by_key(|inside| inside.body.len());
            if let Some(found) = outermost {
                *wanted.entry(found.header).or_default() |= roots;
            }
        }
    }
    if wanted.is_empty() {
        return body.clone();
    }
    // A body that writes the frame register has no frame pointer: the cells
    // that name it as their base are resolved to the stack pointer later,
    // and read none of it. So its upper half is dead where no instruction
    // reads it by name, which liveness cannot say of a register those cells
    // keep live.
    let frame = bit(regs.frame).unwrap_or(0);
    let reads_wide = |roots: Roots| {
        body.blocks
            .iter()
            .flat_map(|block| block.insns.iter())
            .any(
                |one| one.what.as_ref().is_some_and(|what| {
                    what.sources.iter().any(|source| {
                        matches!(
                            source,
                            Loc::Reg(Reg { register, width: 4 }) if bit(*register).is_some_and(|one| one & roots != 0)
                        )
                    })
                }),
            )
    };
    let writes_frame = body
        .blocks
        .iter()
        .flat_map(|block| block.insns.iter())
        .any(
            |one| one.what
                .as_ref()
                .is_some_and(
                    |what| what.dests
                        .iter()
                        .any(
                            |dest| matches!(
                                dest,
                                Loc::Reg(Reg { register, .. }) if bit(*register) == Some(frame)
                            ),
                        ),
                ),
        );
    let symbolic = if frame != 0 && writes_frame && !reads_wide(frame) { frame } else { 0 };
    let mut placed = body.clone();
    let own: IndexMap<i64, Roots> =
        wanted.iter().map(|(header, roots)| (*header, roots & !symbolic)).filter(|(_, r)| *r != 0).collect();
    let by_cells: IndexMap<i64, Roots> =
        wanted.iter().map(|(header, roots)| (*header, roots & symbolic)).filter(|(_, r)| *r != 0).collect();
    if !own.is_empty() {
        placed = preheaded(&placed, &own, true).0;
    }
    if !by_cells.is_empty() {
        placed = preheaded(&placed, &by_cells, false).0;
    }
    placed
}

/// `body` with the upper half of every unheld root a cell reads zero: one
/// `movzx` in the preheader of the outermost loop around the cell that
/// leaves that half alone, or before the first cell of a block no
/// preheader proves.
pub fn established(body: &LirBody) -> LirBody {
    let regs = body.regs();
    let graph = &body.blocks;
    let natural = loops::loops(&graph, Some(body.entry));
    let untouched = |inside: &loops::Body, roots: Roots| {
        body.blocks
            .iter()
            .filter(|block| inside.contains(&block.at))
            .all(|block| block.insns.iter().all(|one| disturbed(regs, body.bits, one) & roots == 0))
    };
    let outermost = |at: i64, roots: Roots| {
        let around = natural.iter().filter(|one| one.body.contains(&at));
        around
            .clone()
            .filter(|one| untouched(&one.body, roots))
            .max_by_key(|one| one.body.len())
            .or_else(|| around.min_by_key(|one| one.body.len()))
            .map(|one| one.header)
    };
    let mut body = body.clone();
    let mut preheaders = true;
    loop {
        let zero = before(&body);
        let missing: Vec<(i64, usize, Roots)> = body
            .blocks
            .iter()
            .flat_map(|block| block.insns.iter().map(move |one| (block.at, one)))
            .map(|(at, one)| (at, id(one), wide_roots(regs, one) & !zero[&id(one)]))
            .filter(|(_, _, roots)| *roots != 0)
            .collect();
        if missing.is_empty() {
            return body;
        }
        if std::mem::take(&mut preheaders) {
            let mut wanted: IndexMap<i64, Roots> = IndexMap::default();
            // Each root has its own outermost loop that leaves it alone: one
            // the counter in edx disturbs must not keep ebp in the inner loop.
            for (at, _, roots) in &missing {
                for root in (0..Roots::BITS).map(|shift| 1 << shift).filter(|bit| roots & bit != 0) {
                    if let Some(header) = outermost(*at, root) {
                        *wanted.entry(header).or_default() |= root;
                    }
                }
            }
            if !wanted.is_empty() {
                body = preheaded(&body, &wanted, false).0;
                continue;
            }
        }
        let mut first: IndexMap<i64, (usize, Roots)> = IndexMap::default();
        for (at, position, roots) in missing {
            first.entry(at).or_insert((position, roots));
        }
        let blocks = body
            .blocks
            .iter()
            .map(|block| match first.get(&block.at) {
                None => block.clone(),
                Some((position, roots)) => {
                    let at = block.insns.iter().position(|one| id(one) == *position).expect("the cell");
                    let mut insns = block.insns.to_vec();
                    let zeroed = extended(regs, &LirBlock::new(block.at, Vec::new()), *roots).insns.to_vec();
                    let when = insns[at].at;
                    insns.splice(at..at, zeroed.into_iter().map(|one| Arc::new(Insn { at: when, ..(*one).clone() })));
                    block.with_insns(insns)
                }
            })
            .collect();
        body = body.with_blocks(blocks);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::{before, bit, established, hoisted};
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::Block;
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn one(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        target: Option<i64>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, target, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    /// In 16-bit code `[esi]` names the byte `[si]` does only while esi's upper
    /// half is zero. A base the allocator held in esi, written whole by an
    /// `add`, was read through the prefixed form with nothing to say its upper
    /// half was zero: past 64K the access faulted where the word form wrapped.
    #[test]
    fn test_a_held_dword_base_with_unknown_upper_half_is_zero_extended_first() {
        use crate::model::ir::{Addr, Held, Mem, Space};
        let (esi, ebx) =
            (Loc::Reg(Reg { register: RegId::ESI, width: 4 }), Loc::Reg(Reg { register: RegId::EBX, width: 4 }));
        let cell = Mem {
            through: RegId::ESI,
            base: Some(Held { value: 7, width: 4 }),
            ..Mem::new(Some(Addr::new(Space::Segment, 0)), 2)
        };
        let load = one(
            1,
            Operation::Move,
            "mov",
            vec![Loc::Reg(Reg { register: RegId::AX, width: 2 })],
            vec![Loc::Mem(cell)],
            None,
        );
        let body = LirBody::new(
            "held",
            0,
            vec![
                LirBlock::new(0, vec![one(0, Operation::Binary, "add", vec![esi.clone()], vec![esi, ebx], None), load])
                    .into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let names: Vec<_> = established(&body)
            .blocks
            .iter()
            .flat_map(|block| {
                block.insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            })
            .collect();
        assert_eq!(names, ["add", "movzx", "mov"], "esi is zero-extended between its write and the cell");
    }

    /// A `lea` is arithmetic, not an address read: `lea ecx, [eax-1]` of a
    /// dword `eax` is the dword sum, and a `movzx eax, ax` put before it by
    /// the cell rule destroyed the upper half of a `long` (the first
    /// version of the m16 conversion of a tied add to a `lea`, `g(long *p, long
    /// n)` in ~/scratch/w1-gate/xa/l32.c).
    #[test]
    fn test_a_lea_cell_does_not_ask_for_its_registers_upper_half() {
        use crate::model::ir::{Held, Mem};
        let (esi, ebx) =
            (Loc::Reg(Reg { register: RegId::ESI, width: 4 }), Loc::Reg(Reg { register: RegId::EBX, width: 4 }));
        let cell =
            Mem { through: RegId::ESI, base: Some(Held { value: 7, width: 4 }), offset: -1, ..Mem::new(None, 4) };
        let lea = one(
            1,
            Operation::Address,
            "lea",
            vec![Loc::Reg(Reg { register: RegId::ECX, width: 4 })],
            vec![Loc::Mem(cell)],
            None,
        );
        let body = LirBody::new(
            "lea",
            0,
            vec![
                LirBlock::new(0, vec![one(0, Operation::Binary, "add", vec![esi.clone()], vec![esi, ebx], None), lea])
                    .into(),
            ],
            IndexMap::default(),
            IndexMap::default(),
        );
        let names: Vec<_> = established(&body)
            .blocks
            .iter()
            .flat_map(|block| {
                block.insns.iter().filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
            })
            .collect();
        assert_eq!(names, ["add", "lea"]);
    }

    #[test]
    fn test_a_zeroed_upper_half_survives_word_writes_and_jumps() {
        // A `jmp` decoded as unknown lost every root, so no loop kept the
        // preheader's `movzx`.
        let (ebx, bx) =
            (Loc::Reg(Reg { register: RegId::EBX, width: 4 }), Loc::Reg(Reg { register: RegId::BX, width: 2 }));
        let word = || Loc::Imm(Imm { value: 5, width: 2, address: None });
        let entry = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(
                0,
                vec![
                    one(0, Operation::Extend, "movzx", vec![ebx], vec![bx.clone()], None),
                    one(1, Operation::Jump, "jmp", vec![], vec![], Some(1)),
                ],
            )
        };
        let looped = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                1,
                vec![
                    one(2, Operation::Move, "mov", vec![bx.clone()], vec![word()], None),
                    one(3, Operation::Compare, "cmp", vec![], vec![bx.clone(), word()], None),
                    one(4, Operation::Branch, "jne", vec![], vec![], Some(1)),
                ],
            )
        };
        let exit = LirBlock::new(2, vec![one(5, Operation::Move, "mov", vec![bx], vec![word()], None)]);
        let body = LirBody::new(
            "zero",
            0,
            vec![entry.into(), looped.into(), exit.into()],
            IndexMap::default(),
            IndexMap::default(),
        );

        let zero = before(&body);

        let ebx = bit(RegId::EBX).unwrap();
        for block in &body.blocks[1..] {
            for insn in &block.insns {
                assert_ne!(zero[&crate::backend::peephole::id(insn)] & ebx, 0, "{:?}", insn.what);
            }
        }
    }

    /// `and edx, 32767` left edx's upper half unknown, so the `movzx edx, dx`
    /// that followed a promoted word `and` stayed (quicksort -O2: three
    /// instructions for two).
    #[test]
    fn test_a_word_mask_leaves_the_upper_half_zero() {
        let edx = Loc::Reg(Reg { register: RegId::EDX, width: 4 });
        let mask = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let body = |value| {
            LirBody::new(
                "mask",
                0,
                vec![
                    LirBlock::new(
                        0,
                        vec![
                            one(0, Operation::Binary, "and", vec![edx.clone()], vec![edx.clone(), mask(value)], None),
                            one(1, Operation::Move, "mov", vec![edx.clone()], vec![edx.clone()], None),
                        ],
                    )
                    .into(),
                ],
                IndexMap::default(),
                IndexMap::default(),
            )
        };
        let (narrow, wide) = (body(32767), body(0x1_0000));
        let at = |body: &LirBody| before(body)[&crate::backend::peephole::id(&body.blocks[0].insns[1])];
        let bit = bit(RegId::EDX).unwrap();
        assert_ne!(at(&narrow) & bit, 0, "and with a word leaves the upper half zero");
        assert_eq!(at(&wide) & bit, 0, "and with a dword mask does not");
    }

    /// `xchg esi, ecx` lost both registers' zero upper halves, so a counter
    /// that began as `mov ecx, 2` was copied by `movzx` before it indexed
    /// (bench/sieve with EBP free: one more instruction a trip).
    #[test]
    fn test_an_exchange_swaps_the_zero_upper_halves() {
        let (ecx, esi) =
            (Loc::Reg(Reg { register: RegId::ECX, width: 4 }), Loc::Reg(Reg { register: RegId::ESI, width: 4 }));
        let two = Loc::Imm(Imm { value: 2, width: 4, address: None });
        let entry = LirBlock::new(
            0,
            vec![
                one(0, Operation::Move, "mov", vec![ecx.clone()], vec![two], None),
                one(
                    1,
                    Operation::Exchange,
                    "xchg",
                    vec![esi.clone(), ecx.clone()],
                    vec![ecx.clone(), esi.clone()],
                    None,
                ),
                one(2, Operation::Nothing, "nop", vec![], vec![], None),
            ],
        );
        let body = LirBody::new("swap", 0, vec![entry.into()], IndexMap::default(), IndexMap::default());

        let zero = before(&body);

        let id = |at: usize| crate::backend::peephole::id(&body.blocks[0].insns[at]);
        assert_ne!(zero[&id(2)] & bit(RegId::ESI).unwrap(), 0);
        assert_eq!(zero[&id(2)] & bit(RegId::ECX).unwrap(), 0);
    }

    /// sieve -O2 (m32) ran `movzx ecx, bp` 1,449 times in its inner loop, where
    /// `bp` is written only as a word: 4,350 of the row's 4,680 extra
    /// clocks over the allocation that happened to reuse a register already
    /// zero above its word. One `movzx ebp, bp` before the loops makes the half
    /// zero for every trip, and the loop's `movzx` a copy.
    #[test]
    fn test_a_loops_widening_of_a_word_written_register_is_zeroed_once_before_it() {
        let (ebx, bx) =
            (Loc::Reg(Reg { register: RegId::EBX, width: 4 }), Loc::Reg(Reg { register: RegId::BX, width: 2 }));
        let ecx = Loc::Reg(Reg { register: RegId::ECX, width: 4 });
        let word = |value| Loc::Imm(Imm { value, width: 2, address: None });
        let entry = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(
                0,
                vec![
                    one(0, Operation::Move, "mov", vec![bx.clone()], vec![word(2)], None),
                    one(1, Operation::Jump, "jmp", vec![], vec![], Some(1)),
                ],
            )
        };
        let looped = LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(
                1,
                vec![
                    one(2, Operation::Extend, "movzx", vec![ecx], vec![bx.clone()], None),
                    one(3, Operation::Binary, "add", vec![bx.clone()], vec![bx.clone(), word(3)], None),
                    one(4, Operation::Compare, "cmp", vec![], vec![bx.clone(), word(9)], None),
                    one(5, Operation::Branch, "jne", vec![], vec![], Some(1)),
                ],
            )
        };
        // A callee-saved register is restored whole before the return: its
        // upper half is dead until then.
        let restore = Loc::Imm(Imm { value: 7, width: 4, address: None });
        let exit = LirBlock::new(
            2,
            vec![
                one(6, Operation::Move, "mov", vec![ebx.clone()], vec![restore], None),
                one(7, Operation::Return, "ret", vec![], vec![], None),
            ],
        );
        let body = LirBody::new(
            "hoist",
            0,
            vec![entry.into(), looped.into(), exit.into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let widening = crate::backend::peephole::id(&body.blocks[1].insns[0]);
        let ebx_bit = bit(RegId::EBX).unwrap();
        assert_eq!(before(&body)[&widening] & ebx_bit, 0, "premise: nothing zeroes the half before the loop");

        let after = hoisted(&body);

        assert_ne!(before(&after)[&widening] & ebx_bit, 0, "the loop's widening reads a register zero above its word");
        let names = |block: &Block| {
            block
                .insns
                .iter()
                .filter_map(|one| one.what.as_ref()?.name.as_deref().map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&after.blocks[0]), ["mov", "movzx", "jmp"]);
        assert_eq!(names(&after.blocks[1]), ["movzx", "add", "cmp", "jne"]);
    }

    /// A loop wrote ECX whole each trip, so ECX was not zero above its word
    /// anywhere in it; the write goes. Solved again from the answer that was
    /// held, ECX stayed not zero round the loop (the answer is the greatest
    /// fixed point, and one started below it never rises), so the `movzx` the
    /// loop's cells needed were put in again.
    #[test]
    fn test_a_root_a_loop_no_longer_disturbs_is_zero_round_it_again() {
        let reg = |register| Loc::Reg(Reg { register, width: 4 });
        let (ecx, edx, ebx) = (reg(RegId::ECX), reg(RegId::EDX), reg(RegId::EBX));
        let word = Loc::Imm(Imm { value: 2, width: 4, address: None });
        let entry = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(0, vec![one(0, Operation::Move, "mov", vec![ecx.clone()], vec![word], None)])
        };
        let trip = |dest: Loc| LirBlock {
            succ: vec![1, 2].into(),
            ..LirBlock::new(1, vec![one(1, Operation::Move, "mov", vec![dest], vec![ebx.clone()], None)])
        };
        let exit = LirBlock::new(2, vec![one(2, Operation::Move, "mov", vec![edx.clone()], vec![ebx.clone()], None)]);
        let (entry, exit): (Block, Block) = (entry.into(), exit.into());
        let body = |trip: LirBlock| {
            LirBody::new(
                "f",
                0,
                vec![entry.clone(), trip.into(), exit.clone()],
                IndexMap::default(),
                IndexMap::default(),
            )
        };
        let ecx_bit = bit(RegId::ECX).unwrap();
        let first = body(trip(ecx.clone()));
        let before_first = before(&first);
        assert_eq!(before_first[&crate::backend::peephole::id(&first.blocks[1].insns[0])] & ecx_bit, 0, "premise");
        let second = body(trip(edx.clone()));
        let after = before(&second);
        let at = crate::backend::peephole::id(&second.blocks[1].insns[0]);
        assert_ne!(after[&at] & ecx_bit, 0, "ECX is not zero at the loop's top");
        assert!(*after == super::from_scratch(&second));
    }

    /// Asked of a body whose blocks are the same instructions, nothing is
    /// worked out; one block changed, the others' steps are kept.
    #[test]
    fn test_a_body_asked_again_is_answered_and_a_changed_block_is_the_only_one_worked_out() {
        let reg = |register| Loc::Reg(Reg { register, width: 4 });
        let word = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let first = LirBlock {
            succ: vec![1].into(),
            ..LirBlock::new(0, vec![one(0, Operation::Move, "mov", vec![reg(RegId::ECX)], vec![word(2)], None)])
        };
        let last = |value| {
            LirBlock::new(1, vec![one(1, Operation::Move, "mov", vec![reg(RegId::EDX)], vec![word(value)], None)])
        };
        let first: Block = first.into();
        let body = |last: LirBlock| {
            LirBody::new("f", 0, vec![first.clone(), last.into()], IndexMap::default(), IndexMap::default())
        };
        let kept = body(last(3));
        let [whole, partly, same] = super::asks();
        before(&kept);
        before(&kept);
        before(&body(last(0x12345)));
        let [whole_now, partly_now, same_now] = super::asks();
        assert_eq!((whole_now - whole, partly_now - partly, same_now - same), (1, 1, 1));
    }

    /// Only a write that leaves nothing above bit 15 counts as zeroing: a
    /// dword constant or mask past a word, `xor` of two registers, `movsx`,
    /// `or` and a word-wide destination each claimed a zero upper half that a
    /// 32-bit address then read through.
    #[test]
    fn test_only_a_write_that_clears_the_upper_half_is_counted_zero() {
        let reg = |register, width| Loc::Reg(Reg { register, width });
        let imm = |value| Loc::Imm(Imm { value, width: 4, address: None });
        let (eax, ebx, ax, bx) = (reg(RegId::EAX, 4), reg(RegId::EBX, 4), reg(RegId::AX, 2), reg(RegId::BX, 2));
        let zeroes = |op, name: &str, dest: &Loc, sources: Vec<Loc>| {
            super::zeroing(&one(0, op, name, vec![dest.clone()], sources, None)) != 0
        };
        for (op, name, sources, expect) in [
            (Operation::Move, "mov", vec![imm(0xFFFF)], true),
            (Operation::Move, "mov", vec![imm(0x10000)], false),
            (Operation::Move, "mov", vec![imm(-1)], false),
            (Operation::Binary, "and", vec![eax.clone(), imm(0xFFFF)], true),
            (Operation::Binary, "and", vec![eax.clone(), imm(0x1FFFF)], false),
            (Operation::Binary, "or", vec![eax.clone(), imm(0)], false),
            (Operation::Binary, "xor", vec![eax.clone(), eax.clone()], true),
            (Operation::Binary, "xor", vec![eax.clone(), ebx.clone()], false),
            (Operation::Binary, "sub", vec![eax.clone(), ebx.clone()], false),
            (Operation::Extend, "movzx", vec![bx.clone()], true),
            (Operation::Extend, "movsx", vec![bx.clone()], false),
        ] {
            assert_eq!(zeroes(op, name, &eax, sources.clone()), expect, "{name} {sources:?}");
        }
        assert!(!zeroes(Operation::Move, "mov", &ax, vec![imm(5)]), "a word write leaves the upper half as it was");
    }
}
