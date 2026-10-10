//! Which values are live where, adapted from llrm-core's
//! `analysis/liveness.rs` (a port of `qbopt/analysis/liveness.py`).
//!
//! That module read the old MIR's `MirBody`, so it stays here rather than in
//! `graph`; the machine phases have their own, `backend/liveness.rs`,
//! over physical register lanes.  Blocks are keyed by [`cfg::id`], as the
//! loops that `pressure`'s `inside` comes from name them.
//!
//! The old MIR's exit values (observable without being operands) and flag
//! values have no counterpart: a return's value is its operand, and every
//! value counts toward pressure.

use std::collections::{BTreeMap, BTreeSet};

use llrm_mir::dense::IdMap;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueId};
use llrm_mir::opcode::Opcode;
use llrm_support::hash::HashMap;

use crate::cfg::id;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Liveness {
    pub live_in: BTreeMap<i64, BTreeSet<ValueId>>,
    pub live_out: BTreeMap<i64, BTreeSet<ValueId>>,
}

/// The local values an instruction reads.
fn reads(instruction: &Instruction) -> impl Iterator<Item = ValueId> + '_ {
    instruction
        .operands
        .iter()
        .filter_map(
            |operand| match operand {
                Operand::Value(value) => Some(*value),
                _ => None,
            },
        )
}

fn is_phi(instruction: &Instruction) -> bool {
    instruction.opcode == Opcode::Phi
}

/// A block's instructions: its phis, then its ordinary operations.
fn split(
    function: &Function,
    block: BlockId,
) -> (Vec<&Instruction>, Vec<&Instruction>) {
    function.block(block).instructions().iter().map(|&one| function.instruction(one)).partition(|one| is_phi(one))
}

/// A phi's value on the edge from `predecessor`, when it is a local value.
fn arm(
    phi: &Instruction,
    predecessor: BlockId,
) -> Option<ValueId> {
    phi.operands
        .chunks(2)
        .find(|pair| pair[1] == Operand::Block(predecessor))
        .and_then(
            |pair| match pair[0] {
                Operand::Value(value) => Some(value),
                _ => None,
            },
        )
}

fn _defines(
    function: &Function,
    block: BlockId,
) -> BTreeSet<ValueId> {
    function.block(block).instructions().iter().filter_map(|&one| function.instruction(one).result).collect()
}

/// Values the ordinary operations read before writing.
fn _exposed(
    function: &Function,
    block: BlockId,
) -> BTreeSet<ValueId> {
    let mut live = BTreeSet::new();
    for op in split(function, block).1.into_iter().rev() {
        if let Some(one) = op.result {
            live.remove(&one);
        }
        live.extend(reads(op));
    }
    live
}

/// Values the caller supplied: used somewhere, defined by nothing here.
pub fn entry_values(function: &Function) -> BTreeSet<ValueId> {
    let defined: BTreeSet<ValueId> = function.layout().iter().flat_map(|&block| _defines(function, block)).collect();
    let used: BTreeSet<ValueId> = function.walk().flat_map(|(_, one)| reads(function.instruction(one))).collect();
    used.difference(&defined).copied().collect()
}

/// The most values live at once -- anywhere, or in `inside`.
pub fn pressure(
    function: &Function,
    found: Option<&Liveness>,
    inside: Option<&BTreeSet<i64>>,
) -> usize {
    pressure_of(function, found, inside, &|_| true)
}

/// `pressure`, counting only the values `counted` says.
pub fn pressure_of(
    function: &Function,
    found: Option<&Liveness>,
    inside: Option<&BTreeSet<i64>>,
    counted: &dyn Fn(ValueId) -> bool,
) -> usize {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut peak = 0;
    for &block in function.layout() {
        if inside.is_some_and(|inside| !inside.contains(&id(block))) {
            continue;
        }
        let mut alive = found.live_out[&id(block)].clone();
        let size = |alive: &BTreeSet<ValueId>| alive.iter().filter(|&&one| counted(one)).count();
        peak = peak.max(size(&alive));
        for op in split(function, block).1.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
            peak = peak.max(size(&alive));
        }
    }
    peak
}

/// An instruction, the values live before it and those live across it.
pub struct LivePoint {
    pub inst: InstId,
    pub before: BTreeSet<ValueId>,
    pub across: BTreeSet<ValueId>,
}

/// The values live before each instruction of `block` but its phis, and
/// those live across it, in order.
pub fn live_points(
    function: &Function,
    found: &Liveness,
    block: BlockId,
) -> Vec<LivePoint> {
    let mut alive = found.live_out[&id(block)].clone();
    let mut points = Vec::new();
    for &inst in function.block(block).instructions().iter().rev() {
        let op = function.instruction(inst);
        if is_phi(op) {
            continue;
        }
        if let Some(one) = op.result {
            alive.remove(&one);
        }
        let across = alive.clone();
        alive.extend(reads(op));
        points.push(LivePoint { inst, before: alive.clone(), across });
    }
    points.reverse();
    points
}

/// What changes in the live set across each instruction of `block` but its
/// phis, in order, and what is live before the first: `live_points` without a
/// set copied for every instruction. An instruction's `read` values are those
/// live before it that were not live across it; `made` is its result where
/// that is live after it. The live set before an instruction is the one
/// after the one before it, less what it made, plus what it read.
pub struct Steps {
    pub first: Vec<ValueId>,
    pub steps: Vec<Step>,
}

pub struct Step {
    pub inst: InstId,
    pub read: Vec<ValueId>,
    pub made: Option<ValueId>,
}

pub fn live_steps(
    function: &Function,
    found: &Liveness,
    block: BlockId,
) -> Steps {
    let mut alive = found.live_out[&id(block)].clone();
    let mut steps = Vec::new();
    for &inst in function.block(block).instructions().iter().rev() {
        let op = function.instruction(inst);
        if is_phi(op) {
            continue;
        }
        let made = op.result.filter(|one| alive.remove(one));
        let read = reads(op).filter(|one| alive.insert(*one)).collect();
        steps.push(Step { inst, read, made });
    }
    steps.reverse();
    Steps { first: alive.into_iter().collect(), steps }
}

/// How many values `counted` says are live before each instruction of
/// `block` but its phis, in order.
pub fn pressure_points(
    function: &Function,
    found: &Liveness,
    block: BlockId,
    counted: &dyn Fn(ValueId) -> bool,
) -> Vec<(InstId, usize)> {
    live_points(function, found, block)
        .into_iter()
        .map(|point| (point.inst, point.before.iter().filter(|&&one| counted(one)).count()))
        .collect()
}

/// The edge operands of phis whose results are actually live.
pub fn phi_inputs(
    function: &Function,
    found: Option<&Liveness>,
) -> BTreeSet<ValueId> {
    let owned;
    let found = match found {
        Some(found) => found,
        None => {
            owned = live(function);
            &owned
        }
    };
    let mut inputs = BTreeSet::new();
    for &block in function.layout() {
        let (phis, ops) = split(function, block);
        let mut alive = found.live_out[&id(block)].clone();
        for op in ops.into_iter().rev() {
            if let Some(one) = op.result {
                alive.remove(&one);
            }
            alive.extend(reads(op));
        }
        for phi in phis {
            if phi.result.is_some_and(|result| alive.contains(&result)) {
                inputs.extend(reads(phi));
            }
        }
    }
    inputs
}

// What is live at each block's entry and exit, to a fixed point.
thread_local! {
    static SOLVES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has solved a function's liveness, for a test that
/// a caller asks the one it holds.
pub fn solves() -> usize {
    SOLVES.with(std::cell::Cell::get)
}

// Facts `live` follows, for a test that its work is the size of the live sets.
#[cfg(test)]
thread_local! {
    pub(crate) static STEPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// What is live at each block's entry and exit: the least solution of
/// `out(B) = U in(S) + arms of S's live phis`, `after(B) = out(B) - defs(B) +
/// exposed(B)`, `in(B) = after(B) - phis(B)`. Each value is followed from
/// where it is read back to where it is made, so the work is the size of the
/// live sets and not the blocks times the values (bit sets over all of them:
/// 4.3x a doubling on `branches`). gcc's `calculate_live_on_exit` and LLVM's
/// `LiveVariables` both walk from each use.
pub fn live(function: &Function) -> Liveness {
    SOLVES.with(|solves| solves.set(solves.get() + 1));
    let layout = function.layout();
    let mut at_index = IdMap::<BlockId, usize>::new();
    for (position, &block) in layout.iter().enumerate() {
        at_index.insert(block, position);
    }
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> =
        layout.iter().map(|&block| split(function, block)).collect();
    // Where each value is made: a block's position, and whether by a phi. The
    // values the caller supplies are made at the entry.
    let mut made = IdMap::<ValueId, (usize, bool)>::new();
    for (position, (phis, ops)) in blocks.iter().enumerate() {
        for (instruction, phi) in phis.iter().map(|one| (one, true)).chain(ops.iter().map(|one| (one, false))) {
            if let Some(result) = instruction.result {
                made.insert(result, (position, phi));
            }
        }
    }
    let arriving = entry_values(function);
    for &value in &arriving {
        made.insert(value, (0, false));
    }
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); layout.len()];
    for (position, &block) in layout.iter().enumerate() {
        for successor in function.successors(block) {
            if let Some(&to) = at_index.get(&successor) {
                predecessors[to].push(position);
            }
        }
    }
    // `after` is what is live once a block's phis have run; the phis' own
    // results come out of it at the end.
    let empty = || layout.iter().map(|&block| (id(block), Default::default())).collect();
    let mut found = Liveness { live_in: empty(), live_out: empty() };
    let key = |position: usize| id(layout[position]);

    enum Reach {
        /// Live once `block`'s phis have run.
        After(usize, ValueId),
        /// Live at the end of `block`.
        Out(usize, ValueId),
    }
    let mut pending = Vec::new();
    for (position, &block) in layout.iter().enumerate() {
        for value in _exposed(function, block) {
            if position != 0 || !arriving.contains(&value) {
                pending.push(Reach::After(position, value));
            }
        }
    }
    while let Some(reach) = pending.pop() {
        #[cfg(test)]
        STEPS.with(|steps| steps.set(steps.get() + 1));
        match reach {
            Reach::Out(position, value) => {
                let by_op = made.get(&value).is_some_and(|&(at, phi)| at == position && !phi);
                if found.live_out.get_mut(&key(position)).expect("a block").insert(value) && !by_op {
                    pending.push(Reach::After(position, value));
                }
            }
            Reach::After(position, value) => {
                if !found.live_in.get_mut(&key(position)).expect("a block").insert(value) {
                    continue;
                }
                if made.get(&value).is_some_and(|&(at, phi)| at == position && phi) {
                    // Each edge brings the phi's arm from its own block.
                    for phi in blocks[position].0.iter().filter(|phi| phi.result == Some(value)) {
                        for &from in &predecessors[position] {
                            if let Some(arm) = arm(phi, layout[from]) {
                                pending.push(Reach::Out(from, arm));
                            }
                        }
                    }
                } else {
                    pending.extend(predecessors[position].iter().map(|&from| Reach::Out(from, value)));
                }
            }
        }
    }
    for (position, (phis, _)) in blocks.iter().enumerate() {
        let after = found.live_in.get_mut(&key(position)).expect("a block");
        for result in phis.iter().filter_map(|phi| phi.result) {
            after.remove(&result);
        }
    }
    #[cfg(test)]
    assert_eq!(found, live_dense(function), "liveness from the uses is not the bit-set fixed point");
    found
}

/// The fixed point on bit sets over dense value indices, which `live` replaced:
/// its oracle. `live` is held to it on every function a test of this crate
/// gives it.
#[cfg(test)]
pub(crate) fn live_dense(function: &Function) -> Liveness {
    use llrm_support::bits::Bits;
    let layout = function.layout();
    let mut index: HashMap<ValueId, usize> = HashMap::default();
    let mut values: Vec<ValueId> = Vec::new();
    for (_, one) in function.walk() {
        let instruction = function.instruction(one);
        for value in instruction.result.into_iter().chain(reads(instruction)) {
            index
                .entry(value)
                .or_insert_with(
                    || {
                        values.push(value);
                        values.len() - 1
                    },
                );
        }
    }
    let bits = |ones: &mut dyn Iterator<Item = ValueId>| {
        let mut set = Bits::new(values.len());
        for one in ones {
            set.insert(index[&one]);
        }
        set
    };
    let at_index: HashMap<BlockId, usize> =
        layout.iter().enumerate().map(|(position, &block)| (block, position)).collect();
    let blocks: Vec<(Vec<&Instruction>, Vec<&Instruction>)> =
        layout.iter().map(|&block| split(function, block)).collect();

    let mut op_defines: Vec<Bits> =
        blocks.iter().map(|(_, ops)| bits(&mut ops.iter().filter_map(|op| op.result))).collect();
    let phi_defines: Vec<Bits> =
        blocks.iter().map(|(phis, _)| bits(&mut phis.iter().filter_map(|phi| phi.result))).collect();
    let mut exposed: Vec<Bits> = layout.iter().map(|&block| bits(&mut _exposed(function, block).into_iter())).collect();
    if !layout.is_empty() {
        let arriving = bits(&mut entry_values(function).into_iter());
        op_defines[0].union_with(&arriving);
        exposed[0].subtract(&arriving);
    }
    // Each successor as its position and, per phi, (result, this block's arm).
    let successors: Vec<Vec<(usize, Vec<(usize, usize)>)>> = layout
        .iter()
        .map(|&block| {
            function
                .successors(block)
                .into_iter()
                .filter_map(|successor| at_index.get(&successor))
                .map(|&position| {
                    let arms = blocks[position]
                        .0
                        .iter()
                        .filter_map(|phi| Some((index[&phi.result?], index[&arm(phi, block)?])))
                        .collect();
                    (position, arms)
                })
                .collect()
        })
        .collect();
    let empty = Bits::new(values.len());
    let mut live_in = vec![empty.clone(); layout.len()];
    let mut live_out = vec![empty.clone(); layout.len()];
    let mut after_phis = vec![empty.clone(); layout.len()];

    let mut changing = true;
    while changing {
        changing = false;
        // Last block first: what a block makes live comes from the blocks
        // after it, so a chain settles in one round and a check, not in one
        // round per block.
        for position in (0..layout.len()).rev() {
            let mut out = empty.clone();
            for (successor, arms) in &successors[position] {
                out.union_with(&live_in[*successor]);
                for &(result, arm) in arms {
                    if after_phis[*successor].contains(result) {
                        out.insert(arm);
                    }
                }
            }
            let mut after = out.clone();
            after.subtract(&op_defines[position]);
            after.union_with(&exposed[position]);
            let mut inside = after.clone();
            inside.subtract(&phi_defines[position]);
            if out != live_out[position] || inside != live_in[position] || after != after_phis[position] {
                live_out[position] = out;
                live_in[position] = inside;
                after_phis[position] = after;
                changing = true;
            }
        }
    }
    let sets = |found: &[Bits]| -> BTreeMap<i64, BTreeSet<ValueId>> {
        layout.iter().zip(found).map(|(&block, set)| (id(block), set.iter().map(|one| values[one]).collect())).collect()
    };
    Liveness { live_in: sets(&live_in), live_out: sets(&live_out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{block, function, parsed, value};

    /// PARITYCONTROL's dead join-flag phis kept ADD/ADC live as word
    /// operations.
    #[test]
    fn test_a_dead_phi_does_not_keep_its_edge_operand_live() {
        let module = parsed(
            "define void @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret void
}
",
        );
        let function = function(&module, "f");
        let found = live(function);
        assert!(!found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")));

        let module = parsed(
            "define i16 @f(i16 %x) {
b0:
  %incoming = add i16 %x, 1
  br label %b1

b1:
  %merged = phi i16 [ %incoming, %b0 ]
  ret i16 %merged
}
",
        );
        let function = crate::testing::function(&module, "f");
        let found = live(function);
        assert!(
            found.live_out[&id(block(function, "b0"))].contains(&value(function, "incoming")),
            "a live phi keeps it"
        );
    }

    /// `@f` and its liveness.
    fn facts(text: &str) -> (llrm_mir::module::Module, Liveness) {
        let module = parsed(text);
        let found = live(function(&module, "f"));
        (module, found)
    }

    fn live_in(
        module: &llrm_mir::module::Module,
        found: &Liveness,
        at: &str,
        name: &str,
    ) -> bool {
        let f = function(module, "f");
        found.live_in[&id(block(f, at))].contains(&value(f, name))
    }

    fn live_out(
        module: &llrm_mir::module::Module,
        found: &Liveness,
        at: &str,
        name: &str,
    ) -> bool {
        let f = function(module, "f");
        found.live_out[&id(block(f, at))].contains(&value(f, name))
    }

    const LOOP: &str = "declare void @use(i16)

define i16 @f(i16 %n, i16 %x) {
pre:
  %before = add i16 %x, 1
  %across = add i16 %x, 2
  call void @use(i16 %before)
  br label %head

head:
  %i = phi i16 [ 0, %pre ], [ %next, %body ]
  %done = icmp eq i16 %i, %n
  br i1 %done, label %out, label %body

body:
  %next = add i16 %i, 1
  br label %head

out:
  %late = add i16 %across, %i
  ret i16 %late
}
";

    #[test]
    fn a_value_used_only_before_the_loop_is_not_live_inside_it() {
        let (module, found) = facts(LOOP);
        for at in ["head", "body"] {
            assert!(!live_in(&module, &found, at, "before"), "{at}");
        }
    }

    #[test]
    fn a_value_used_after_the_loop_is_live_throughout_it() {
        let (module, found) = facts(LOOP);
        for at in ["head", "body"] {
            assert!(live_in(&module, &found, at, "across") && live_out(&module, &found, at, "across"), "{at}");
        }
    }

    #[test]
    fn a_value_defined_after_the_loop_is_not_live_inside_it() {
        let (module, found) = facts(LOOP);
        for at in ["pre", "head", "body"] {
            assert!(!live_out(&module, &found, at, "late"), "{at}");
        }
    }

    #[test]
    fn a_loop_carried_value_is_live_out_of_the_latch_but_not_into_the_header() {
        let (module, found) = facts(LOOP);
        assert!(live_out(&module, &found, "body", "next"));
        assert!(!live_in(&module, &found, "head", "next"), "the phi reads it on the edge");
        assert!(!live_in(&module, &found, "head", "i"), "the phi defines it at the top");
        assert!(live_in(&module, &found, "body", "i"));
    }

    #[test]
    fn a_parameter_read_in_the_loop_is_live_across_the_loop_but_not_into_the_entry() {
        let (module, found) = facts(LOOP);
        assert!(live_out(&module, &found, "pre", "n"));
        assert!(live_out(&module, &found, "body", "n"), "the next trip compares it again");
        assert!(!live_in(&module, &found, "pre", "n"), "the caller supplies it");
        assert!(!live_out(&module, &found, "out", "n"));
    }

    #[test]
    fn a_value_read_on_one_arm_of_a_diamond_is_live_only_into_that_arm() {
        let (module, found) = facts(
            "define i16 @f(i1 %c, i16 %x) {
top:
  %y = add i16 %x, 1
  br i1 %c, label %left, label %right

left:
  br label %join

right:
  %z = add i16 %y, 1
  br label %join

join:
  %r = phi i16 [ 0, %left ], [ %z, %right ]
  ret i16 %r
}
",
        );
        assert!(live_in(&module, &found, "right", "y"));
        assert!(!live_in(&module, &found, "left", "y"));
        assert!(!live_in(&module, &found, "join", "y"));
        assert!(live_out(&module, &found, "right", "z"));
        assert!(!live_out(&module, &found, "left", "z"), "only the right edge carries it");
    }

    #[test]
    fn a_use_in_an_unreachable_block_keeps_nothing_live_in_the_reachable_ones() {
        let (module, found) = facts(
            "define i16 @f(i16 %x) {
top:
  %y = add i16 %x, 1
  ret i16 %x

dead:
  ret i16 %y
}
",
        );
        assert!(!live_out(&module, &found, "top", "y"));
    }

    #[test]
    fn a_phi_with_a_repeated_predecessor_keeps_its_input_live_on_that_edge() {
        let (module, found) = facts(
            "define i16 @f(i16 %s, i16 %x) {
top:
  %y = add i16 %x, 1
  switch i16 %s, label %join [ i16 1, label %join ]

join:
  %r = phi i16 [ %y, %top ], [ %y, %top ]
  ret i16 %r
}
",
        );
        assert!(live_out(&module, &found, "top", "y"));
    }

    #[test]
    fn a_declaration_has_nothing_live_and_no_pressure() {
        let module = parsed("declare i16 @f(i16)\n");
        let f = function(&module, "f");
        assert_eq!(live(f), Liveness { live_in: BTreeMap::new(), live_out: BTreeMap::new() });
        assert_eq!(pressure(f, None, None), 0);
    }

    #[test]
    fn pressure_counts_the_most_values_live_at_once_and_can_look_only_inside_a_loop() {
        let (module, found) = facts(LOOP);
        let f = function(&module, "f");
        let inside = BTreeSet::from([id(block(f, "head")), id(block(f, "body"))]);
        // %n, %across, %i and %done at the header's branch.
        assert_eq!(pressure(f, Some(&found), Some(&inside)), 4);
        assert_eq!(pressure(f, Some(&found), None), 4);
        assert_eq!(pressure(f, Some(&found), Some(&BTreeSet::from([id(block(f, "out"))]))), 2);
    }

    #[test]
    fn phi_inputs_are_the_edge_operands_of_live_phis_only() {
        let module = parsed(
            "define i16 @f(i1 %c, i16 %a, i16 %b) {
top:
  br i1 %c, label %left, label %join

left:
  br label %join

join:
  %used = phi i16 [ %a, %top ], [ 0, %left ]
  %unused = phi i16 [ %b, %top ], [ %b, %left ]
  ret i16 %used
}
",
        );
        let f = function(&module, "f");
        assert_eq!(phi_inputs(f, None), BTreeSet::from([value(f, "a")]));
    }

    #[test]
    fn entry_values_are_the_parameters_the_body_reads() {
        let module = parsed(
            "define i16 @f(i16 %read, i16 %ignored) {
top:
  %y = add i16 %read, 1
  ret i16 %y
}
",
        );
        let f = function(&module, "f");
        assert_eq!(entry_values(f), BTreeSet::from([value(f, "read")]));
    }

    /// The bit-set fixed point made every block's sets as wide as the
    /// function's values: `branches` at N=1024 spent 82.7 G in gvn, 39% of
    /// it in liveness (4.3x, 5.2x, 5.7x per doubling). The work is now the
    /// live sets': a chain of N blocks that each make a value for the next
    /// costs steps in N, and doubling the chain doubles them.
    #[test]
    fn test_liveness_work_follows_the_live_sets_not_blocks_times_values() {
        let steps = |blocks: usize| {
            let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  %v0 = add i16 %x, 1\n  br label %b1\n\n");
            for at in 1..blocks {
                text += &format!("b{at}:\n  %v{at} = add i16 %v{}, 1\n  br label %b{}\n\n", at - 1, at + 1);
            }
            text += &format!("b{blocks}:\n  ret i16 %v{}\n}}\n", blocks - 1);
            let module = parsed(&text);
            STEPS.with(|steps| steps.set(0));
            live(function(&module, "f"));
            STEPS.with(|steps| steps.get())
        };
        let (small, large) = (steps(100), steps(200));
        assert!(small > 0 && large <= 2 * small + 8, "{small} steps for 100 blocks, {large} for 200");
    }
}
