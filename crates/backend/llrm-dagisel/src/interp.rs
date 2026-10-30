//! `SelectCodeCommon`: walks the matcher table over one DAG node.
//!
//! # How it works
//! A translation of [`SelectCodeCommon`][common] over [`Matcher`] items.
//! `Scope` pushes a [`MatchScope`]; a failed check pops back to the last
//! scope's next alternative, restoring the node stack, the recorded nodes, the
//! matched memory operands, and the input chain and glue.
//! `IsPredicateKnownToFail` ([L3103][known]) skips alternatives without pushing
//! a scope. A match ends in `MorphNodeTo` (the root node becomes the machine
//! node) or `CompleteMatch`.
//!
//! Numbered deviations from the pin (reason / owner / exit / review):
//! - **SD20** Jumps count items, not bytes, and `ByHwMode` operands are
//!   resolved to a type when the table is generated. Reason: the table is
//!   decoded once; the hardware mode is fixed per target. Owner sdag-c. Exit:
//!   none. Review 2026-12-10.
//! - **SD21** `mayRaiseFPException`/`NoFPExcept` are not carried. Reason: MIR
//!   has no strict FP. Owner sdag-c. Exit: when the builder lowers constrained
//!   FP. Review 2026-12-10.
//! - **SD22** The `MatchStateUpdater` is not installed around complex patterns;
//!   a complex pattern that CSEs a node while matching is a C1 question. Owner
//!   sdag-c. Exit: C1. Review 2026-12-10.
//! - **SD23** `CheckImmAllOnesV`/`CheckImmAllZerosV` never match: no vectors
//!   before 32-bit parity. Owner sdag-c. Exit: vector MIR. Review 2026-12-10.
//!
//! # Used by
//! `X86DAGToDAG` (C1) and the shadow harness.
//!
//! [common]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3351
//! [known]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3103

use crate::dag::{Dag, Hooks, MemRef, SDValue};
use crate::generated::ids::{isd, mvt, target_opcode};
use crate::matcher::{Emit, Matcher, Opcode, Slice, Table, Vt, opfl};

/// How a node left the interpreter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The node is selected, or needs no selection (`EntryToken`, `Register`,
    /// ...).
    Selected,
    /// No alternative matched (`CannotYetSelect`).
    CannotSelect,
}

/// The table and the `OpcodeOffset` index the pin builds on the first
/// selection.
pub struct Selector {
    table: Table,
    /// Item index of each opcode's case body, 0 when the table has none.
    opcode_offset: Vec<u32>,
}

/// `MatchScope`. The node stack at the scope is kept in
/// [`Scratch::saved_stacks`].
struct MatchScope {
    fail_index: u32,
    saved_stack: usize,
    saved_stack_len: usize,
    num_recorded_nodes: usize,
    num_matched_mem_refs: usize,
    input_chain: Option<SDValue>,
    input_glue: Option<SDValue>,
    has_chain_nodes_matched: bool,
}

type Recorded = Vec<(SDValue, Option<u32>)>;

/// The buffers one selection fills, kept between selections so each node reuses
/// them.
#[derive(Default)]
pub struct Scratch {
    node_stack: Vec<SDValue>,
    match_scopes: Vec<MatchScope>,
    saved_stacks: Vec<SDValue>,
    recorded: Recorded,
    matched_mem_refs: Vec<MemRef>,
    chain_nodes_matched: Vec<Option<u32>>,
}

thread_local! {
    /// The buffers the last [`Selector::select`] on this thread left.
    static KEPT: std::cell::Cell<Option<Box<Scratch>>> = const { std::cell::Cell::new(None) };
}

/// The capacity the buffers kept for the next selection hold.
#[cfg(test)]
pub(crate) fn kept_capacity() -> usize {
    let kept = KEPT.take();
    let capacity = kept.as_ref().map_or(0, |one| one.node_stack.capacity() + one.match_scopes.capacity());
    KEPT.set(kept);
    capacity
}

impl Selector {
    pub fn new(table: Table) -> Selector {
        let mut opcode_offset = Vec::new();
        if matches!(table.items.first(), Some(Matcher::SwitchOpcode)) {
            let mut at = 1;
            while let Matcher::OpcodeCase { size, opcode } = table.items[at] {
                if size == 0 {
                    break;
                }
                let opcode = opcode as usize;
                if opcode >= opcode_offset.len() {
                    opcode_offset.resize(opcode + 1, 0);
                }
                opcode_offset[opcode] = at as u32 + 1;
                at += 1 + size as usize;
            }
        }
        Selector { table, opcode_offset }
    }

    fn operands(
        &self,
        slice: Slice,
    ) -> &[u16] {
        &self.table.operands[slice.start as usize..slice.start as usize + slice.len as usize]
    }

    /// Selects `node_to_match`. Nodes that are selected already or never need
    /// it return `Selected` untouched; the pin's `Select_*` special cases
    /// (inline asm, `UNDEF`, ...) are the caller's.
    pub fn select<D: Dag, H: Hooks<D>>(
        &self,
        dag: &mut D,
        hooks: &mut H,
        node_to_match: u32,
    ) -> Outcome {
        // The buffers are the thread's, taken for the selection and put back:
        // a fresh set per node cost five allocations and their growth each
        // (d_faces -O2: 5,336 selections).
        let mut scratch = KEPT.take().unwrap_or_default();
        let outcome = self.select_with(&mut scratch, dag, hooks, node_to_match);
        KEPT.set(Some(scratch));
        outcome
    }

    /// [`Selector::select`] with the caller's buffers.
    pub fn select_with<D: Dag, H: Hooks<D>>(
        &self,
        scratch: &mut Scratch,
        dag: &mut D,
        hooks: &mut H,
        node_to_match: u32,
    ) -> Outcome {
        if needs_no_selection(dag.opcode(node_to_match)) {
            return Outcome::Selected;
        }
        if select_special(dag, node_to_match) {
            return Outcome::Selected;
        }
        let Scratch { node_stack, match_scopes, saved_stacks, recorded, matched_mem_refs, chain_nodes_matched } =
            scratch;
        node_stack.clear();
        match_scopes.clear();
        saved_stacks.clear();
        recorded.clear();
        matched_mem_refs.clear();
        chain_nodes_matched.clear();
        let mut n = SDValue::new(node_to_match, 0);
        node_stack.push(n);
        let mut input_chain: Option<SDValue> = None;
        let mut input_glue: Option<SDValue> = None;
        let items = self.table.items;

        let mut at = 0usize;
        if let Some(&offset) = self.opcode_offset.get(dag.opcode(n.node) as usize) {
            at = offset as usize;
        }

        loop {
            let item = items[at];
            at += 1;
            let ok = match item {
                Matcher::Scope => {
                    let mut fail_index;
                    loop {
                        let Matcher::Alt(skip) = items[at] else { unreachable!("Scope not followed by Alt") };
                        at += 1;
                        if skip == 0 {
                            fail_index = 0;
                            break;
                        }
                        fail_index = at as u32 + skip;
                        let (next, failed) = self.known_to_fail(dag, hooks, at, n, recorded);
                        at = next;
                        if !failed {
                            break;
                        }
                        at = fail_index as usize;
                    }
                    if fail_index == 0 {
                        false
                    } else {
                        let saved_stack = saved_stacks.len();
                        saved_stacks.extend_from_slice(node_stack);
                        match_scopes.push(MatchScope {
                            fail_index,
                            saved_stack,
                            saved_stack_len: node_stack.len(),
                            num_recorded_nodes: recorded.len(),
                            num_matched_mem_refs: matched_mem_refs.len(),
                            input_chain,
                            input_glue,
                            has_chain_nodes_matched: !chain_nodes_matched.is_empty(),
                        });
                        continue;
                    }
                }
                Matcher::Alt(_) | Matcher::OpcodeCase { .. } | Matcher::TypeCase { .. } => {
                    unreachable!("a case header outside its switch")
                }
                Matcher::RecordNode => {
                    let parent = (node_stack.len() > 1).then(|| node_stack[node_stack.len() - 2].node);
                    recorded.push((n, parent));
                    continue;
                }
                Matcher::RecordChild(child) => {
                    if usize::from(child) >= dag.num_operands(n.node) {
                        false
                    } else {
                        recorded.push((dag.operand(n.node, child as usize), Some(n.node)));
                        continue;
                    }
                }
                Matcher::RecordMemRef => {
                    if let Some(mems) = dag.mem_operands(n.node) {
                        matched_mem_refs.extend(mems);
                    }
                    continue;
                }
                Matcher::CaptureGlueInput => {
                    let count = dag.num_operands(n.node);
                    if count != 0 {
                        let last = dag.operand(n.node, count - 1);
                        if dag.value_type(last) == mvt::Glue {
                            input_glue = Some(last);
                        }
                    }
                    continue;
                }
                Matcher::MoveChild(child) => {
                    if usize::from(child) >= dag.num_operands(n.node) {
                        false
                    } else {
                        n = dag.operand(n.node, child as usize);
                        node_stack.push(n);
                        continue;
                    }
                }
                Matcher::MoveSibling(sibling) => {
                    node_stack.pop();
                    n = *node_stack.last().expect("node stack imbalance");
                    if usize::from(sibling) >= dag.num_operands(n.node) {
                        false
                    } else {
                        n = dag.operand(n.node, sibling as usize);
                        node_stack.push(n);
                        continue;
                    }
                }
                Matcher::MoveParent => {
                    node_stack.pop();
                    n = *node_stack.last().expect("node stack imbalance");
                    continue;
                }
                Matcher::CheckSame(_)
                | Matcher::CheckChildSame { .. }
                | Matcher::CheckPatternPredicate(_)
                | Matcher::CheckPredicate(_)
                | Matcher::CheckOpcode(_)
                | Matcher::CheckType(_)
                | Matcher::CheckTypeRes { .. }
                | Matcher::CheckChildType { .. }
                | Matcher::CheckInteger(_)
                | Matcher::CheckChildInteger { .. }
                | Matcher::CheckCondCode(_)
                | Matcher::CheckChild2CondCode(_)
                | Matcher::CheckValueType(_)
                | Matcher::CheckAndImm(_)
                | Matcher::CheckOrImm(_) => match self.eval_check(dag, hooks, item, n, recorded) {
                    Some(true) => continue,
                    _ => false,
                },
                Matcher::CheckComplexPat { pattern, record } => {
                    let (value, parent) = recorded[record as usize];
                    if hooks.check_complex_pattern(dag, node_to_match, parent, value, pattern, recorded) {
                        continue;
                    }
                    false
                }
                Matcher::SwitchOpcode => {
                    let current = dag.opcode(n.node);
                    let mut found = false;
                    loop {
                        let Matcher::OpcodeCase { size, opcode } = items[at] else { unreachable!("SwitchOpcode case") };
                        at += 1;
                        if size == 0 {
                            break;
                        }
                        if opcode == current {
                            found = true;
                            break;
                        }
                        at += size as usize;
                    }
                    if found {
                        continue;
                    }
                    false
                }
                Matcher::SwitchType => {
                    let current = dag.value_type(n);
                    let mut found = false;
                    loop {
                        let Matcher::TypeCase { size, vt } = items[at] else { unreachable!("SwitchType case") };
                        at += 1;
                        if size == 0 {
                            break;
                        }
                        let vt = if vt == mvt::iPTR { hooks.pointer_type() } else { vt };
                        if vt == current {
                            found = true;
                            break;
                        }
                        at += size as usize;
                    }
                    if found {
                        continue;
                    }
                    false
                }
                Matcher::CheckImmAllOnesV | Matcher::CheckImmAllZerosV => false,
                Matcher::CheckUndef => {
                    if dag.is_undef(n) {
                        continue;
                    }
                    false
                }
                Matcher::CheckFoldableChainNode => {
                    assert!(node_stack.len() != 1, "no parent node");
                    let mut multiple_uses = false;
                    for stacked in &node_stack[1..node_stack.len() - 1] {
                        if dag.count_non_chain_uses(stacked.node, 1) > 1 {
                            multiple_uses = true;
                            break;
                        }
                    }
                    let parent = node_stack[node_stack.len() - 2].node;
                    if multiple_uses
                        || !hooks.is_profitable_to_fold(dag, n, parent, node_to_match)
                        || !hooks.is_legal_to_fold(dag, n, parent, node_to_match)
                    {
                        false
                    } else {
                        continue;
                    }
                }
                Matcher::EmitInteger { vt, value } => {
                    let value = sign_extend(value, vt);
                    recorded.push((dag.target_constant(value, vt), None));
                    continue;
                }
                Matcher::EmitRegister { vt, register } => {
                    recorded.push((dag.register(register, vt), None));
                    continue;
                }
                Matcher::EmitConvertToTarget(record) => {
                    let (value, parent) = recorded[record as usize];
                    recorded.push((dag.convert_to_target(value), parent));
                    continue;
                }
                Matcher::EmitMergeInputChains(records) => {
                    assert!(input_chain.is_none(), "EmitMergeInputChains should be the first chain producing node");
                    assert!(chain_nodes_matched.is_empty(), "one EmitMergeInputChains per match");
                    let mut failed = false;
                    for &record in self.operands(records) {
                        let value = recorded[record as usize].0;
                        chain_nodes_matched.push(Some(value.node));
                        // A chained node that is not the root must not have
                        // other users.
                        if value.node != node_to_match && !dag.has_one_use(value) {
                            chain_nodes_matched.clear();
                            failed = true;
                            break;
                        }
                    }
                    if failed {
                        false
                    } else {
                        input_chain = handle_merge_input_chains(dag, chain_nodes_matched, input_glue);
                        if input_chain.is_none() {
                            false
                        } else {
                            continue;
                        }
                    }
                }
                Matcher::EmitCopyToReg { record, register } => {
                    let chain = match input_chain {
                        Some(chain) => chain,
                        None => dag.entry_node(),
                    };
                    let copy = dag.copy_to_reg(chain, register, recorded[record as usize].0, input_glue);
                    let values = dag.num_values(copy);
                    input_chain = Some(SDValue::new(copy, 0));
                    input_glue = Some(SDValue::new(copy, (values - 1) as u8));
                    continue;
                }
                Matcher::EmitNodeXForm { xform, record } => {
                    let value = hooks.run_xform(dag, recorded[record as usize].0, xform);
                    recorded.push((value, None));
                    continue;
                }
                Matcher::EmitNode(emit) | Matcher::MorphNodeTo(emit) => {
                    let morph = matches!(item, Matcher::MorphNodeTo(_));
                    if let Some(done) = self.emit_node(
                        dag,
                        hooks,
                        emit,
                        morph,
                        node_to_match,
                        recorded,
                        matched_mem_refs,
                        &mut input_chain,
                        &mut input_glue,
                        chain_nodes_matched,
                    ) {
                        return done;
                    }
                    continue;
                }
                Matcher::CompleteMatch(results) => {
                    for (i, &slot) in self.operands(results).iter().enumerate() {
                        let result = recorded[slot as usize].0;
                        dag.replace_uses(SDValue::new(node_to_match, i as u8), result);
                    }
                    update_chains(dag, node_to_match, input_chain, chain_nodes_matched, false);
                    let values = dag.num_values(node_to_match);
                    if dag.value_type(SDValue::new(node_to_match, (values - 1) as u8)) == mvt::Glue {
                        if let Some(glue) = input_glue {
                            dag.replace_uses(SDValue::new(node_to_match, (values - 1) as u8), glue);
                        }
                    }
                    assert!(dag.use_empty(node_to_match), "didn't replace all uses of the node");
                    dag.remove_dead_node(node_to_match);
                    return Outcome::Selected;
                }
            };
            debug_assert!(!ok);

            // The match failed: continue at the last scope's next alternative.
            loop {
                let Some(last) = match_scopes.last_mut() else {
                    return Outcome::CannotSelect;
                };
                recorded.truncate(last.num_recorded_nodes);
                node_stack.clear();
                node_stack.extend_from_slice(&saved_stacks[last.saved_stack..last.saved_stack + last.saved_stack_len]);
                n = *node_stack.last().unwrap();
                matched_mem_refs.truncate(last.num_matched_mem_refs);
                at = last.fail_index as usize;
                input_chain = last.input_chain;
                input_glue = last.input_glue;
                if !last.has_chain_nodes_matched {
                    chain_nodes_matched.clear();
                }
                let Matcher::Alt(skip) = items[at] else { unreachable!("scope alternative") };
                at += 1;
                if skip != 0 {
                    last.fail_index = at as u32 + skip;
                    break;
                }
                let popped = match_scopes.pop().unwrap();
                saved_stacks.truncate(popped.saved_stack);
            }
        }
    }

    /// `IsPredicateKnownToFail`: the index to go on at, and whether the check
    /// at `at` failed.
    fn known_to_fail<D: Dag, H: Hooks<D>>(
        &self,
        dag: &D,
        hooks: &H,
        at: usize,
        n: SDValue,
        recorded: &Recorded,
    ) -> (usize, bool) {
        let item = self.table.items[at];
        match item {
            Matcher::CheckSame(_)
            | Matcher::CheckChildSame { .. }
            | Matcher::CheckPatternPredicate(_)
            | Matcher::CheckPredicate(_)
            | Matcher::CheckOpcode(_)
            | Matcher::CheckType(_)
            | Matcher::CheckTypeRes { .. }
            | Matcher::CheckChildType { .. }
            | Matcher::CheckInteger(_)
            | Matcher::CheckChildInteger { .. }
            | Matcher::CheckCondCode(_)
            | Matcher::CheckChild2CondCode(_)
            | Matcher::CheckValueType(_)
            | Matcher::CheckAndImm(_)
            | Matcher::CheckOrImm(_) => {
                let passed = self.eval_check(dag, hooks, item, n, recorded) == Some(true);
                (at + 1, !passed)
            }
            _ => (at, false),
        }
    }

    /// The checks that read the current node and nothing else; `None` for any
    /// other item.
    fn eval_check<D: Dag, H: Hooks<D>>(
        &self,
        dag: &D,
        hooks: &H,
        item: Matcher,
        n: SDValue,
        recorded: &Recorded,
    ) -> Option<bool> {
        let check_type = |vt: Vt, value: SDValue| {
            let have = dag.value_type(value);
            have == vt || (vt == mvt::iPTR && have == hooks.pointer_type())
        };
        let child =
            |index: u8| (usize::from(index) < dag.num_operands(n.node)).then(|| dag.operand(n.node, index as usize));
        Some(match item {
            Matcher::CheckSame(record) => n == recorded[record as usize].0,
            Matcher::CheckChildSame { child: index, record } => {
                child(index).is_some_and(|c| c == recorded[record as usize].0)
            }
            Matcher::CheckPatternPredicate(predicate) => hooks.check_pattern_predicate(predicate),
            Matcher::CheckPredicate(predicate) => hooks.check_node_predicate(dag, n, predicate),
            Matcher::CheckOpcode(opcode) => dag.opcode(n.node) == opcode,
            Matcher::CheckType(vt) => check_type(vt, n),
            Matcher::CheckTypeRes { res, vt } => check_type(vt, SDValue::new(n.node, res)),
            Matcher::CheckChildType { child: index, vt } => child(index).is_some_and(|c| check_type(vt, c)),
            Matcher::CheckInteger(value) => dag.constant(n) == Some(value),
            Matcher::CheckChildInteger { child: index, value } => {
                child(index).is_some_and(|c| dag.constant(c) == Some(value))
            }
            Matcher::CheckCondCode(code) => dag.cond_code(n) == Some(code),
            Matcher::CheckChild2CondCode(code) => child(2).is_some_and(|c| dag.cond_code(c) == Some(code)),
            Matcher::CheckValueType(vt) => dag
                .value_type_node(n)
                .is_some_and(|have| have == vt || (vt == mvt::iPTR && have == hooks.pointer_type())),
            Matcher::CheckAndImm(desired) => {
                dag.opcode(n.node) == isd::AND
                    && dag
                        .constant(dag.operand(n.node, 1))
                        .is_some_and(|rhs| hooks.check_and_mask(dag, dag.operand(n.node, 0), rhs, desired as i64))
            }
            Matcher::CheckOrImm(desired) => {
                dag.opcode(n.node) == isd::OR
                    && dag
                        .constant(dag.operand(n.node, 1))
                        .is_some_and(|rhs| hooks.check_or_mask(dag, dag.operand(n.node, 0), rhs, desired as i64))
            }
            _ => return None,
        })
    }

    /// `OPC_EmitNode` and `OPC_MorphNodeTo`. `Some` when a morph ended the
    /// match.
    #[allow(clippy::too_many_arguments)]
    fn emit_node<D: Dag, H: Hooks<D>>(
        &self,
        dag: &mut D,
        hooks: &H,
        emit: Emit,
        morph: bool,
        node_to_match: u32,
        recorded: &mut Recorded,
        matched_mem_refs: &[MemRef],
        input_chain: &mut Option<SDValue>,
        input_glue: &mut Option<SDValue>,
        chain_nodes_matched: &mut [Option<u32>],
    ) -> Option<Outcome> {
        let flags = emit.flags;
        let mut vts: Vec<Vt> = emit.vts[..emit.num_vts as usize]
            .iter()
            .map(|&vt| if vt == mvt::iPTR { hooks.pointer_type() } else { vt })
            .collect();
        if flags & opfl::CHAIN != 0 {
            vts.push(mvt::Other);
        }
        if flags & opfl::GLUE_OUTPUT != 0 {
            vts.push(mvt::Glue);
        }

        let mut ops: Vec<SDValue> = self.operands(emit.operands).iter().map(|&r| recorded[r as usize].0).collect();
        if flags & opfl::VARIADIC_INFO != 0 {
            let mut first = opfl::num_fixed_from_variadic_info(flags);
            first += i32::from(flags & opfl::CHAIN != 0);
            let first = first as usize;
            assert!(dag.num_operands(node_to_match) >= first, "invalid variadic node");
            for i in first..dag.num_operands(node_to_match) {
                let v = dag.operand(node_to_match, i);
                if dag.value_type(v) == mvt::Glue {
                    break;
                }
                ops.push(v);
            }
        }
        if flags & opfl::CHAIN != 0 {
            ops.push(input_chain.expect("a chained node without an input chain"));
        }
        if flags & opfl::GLUE_INPUT != 0 {
            if let Some(glue) = *input_glue {
                ops.push(glue);
            }
        }

        let res = if !morph {
            let node = dag.machine_node(emit.opcode, &vts, &ops);
            for (i, &vt) in vts.iter().enumerate() {
                if vt == mvt::Other || vt == mvt::Glue {
                    break;
                }
                recorded.push((SDValue::new(node, i as u8), None));
            }
            node
        } else {
            let (node, deleted) = self.morph_node(dag, node_to_match, emit.opcode, &vts, &ops, flags);
            for gone in deleted {
                for slot in chain_nodes_matched.iter_mut() {
                    if *slot == Some(gone) {
                        *slot = None;
                    }
                }
            }
            node
        };

        if flags & opfl::GLUE_OUTPUT != 0 {
            *input_glue = Some(SDValue::new(res, (vts.len() - 1) as u8));
            if flags & opfl::CHAIN != 0 {
                *input_chain = Some(SDValue::new(res, (vts.len() - 2) as u8));
            }
        } else if flags & opfl::CHAIN != 0 {
            *input_chain = Some(SDValue::new(res, (vts.len() - 1) as u8));
        }

        if flags & opfl::MEM_REFS != 0 {
            let (may_load, may_store) = hooks.may_load_store(emit.opcode);
            let kept: Vec<MemRef> = matched_mem_refs
                .iter()
                .copied()
                .filter(|&m| {
                    if dag.mem_ref_is_load(m) {
                        may_load
                    } else if dag.mem_ref_is_store(m) {
                        may_store
                    } else {
                        true
                    }
                })
                .collect();
            dag.set_node_mem_refs(res, &kept);
        }

        if morph {
            update_chains(dag, res, *input_chain, chain_nodes_matched, true);
            return Some(Outcome::Selected);
        }
        None
    }

    /// `SelectionDAGISel::MorphNode` ([L2896][morph]): morph in place and move
    /// the chain and glue uses of the old node to the new one's last
    /// results.
    ///
    /// [morph]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L2896
    fn morph_node<D: Dag>(
        &self,
        dag: &mut D,
        node: u32,
        opcode: u16,
        vts: &[Vt],
        ops: &[SDValue],
        flags: u8,
    ) -> (u32, Vec<u32>) {
        let old_results = dag.num_values(node);
        let (mut old_glue, mut old_chain) = (None, None);
        if dag.value_type(SDValue::new(node, (old_results - 1) as u8)) == mvt::Glue {
            old_glue = Some(old_results - 1);
            if old_results != 1 && dag.value_type(SDValue::new(node, (old_results - 2) as u8)) == mvt::Other {
                old_chain = Some(old_results - 2);
            }
        } else if dag.value_type(SDValue::new(node, (old_results - 1) as u8)) == mvt::Other {
            old_chain = Some(old_results - 1);
        }

        let (res, deleted) = dag.morph_node_to(node, opcode, vts, ops);
        let mut results = dag.num_values(res);
        if flags & opfl::GLUE_OUTPUT != 0 {
            if let Some(old) = old_glue {
                if old != results - 1 {
                    dag.replace_uses(SDValue::new(node, old as u8), SDValue::new(res, (results - 1) as u8));
                }
            }
            results -= 1;
        }
        if flags & opfl::CHAIN != 0 {
            if let Some(old) = old_chain {
                if old != results - 1 {
                    dag.replace_uses(SDValue::new(node, old as u8), SDValue::new(res, (results - 1) as u8));
                }
            }
        }
        if res != node {
            dag.replace_node(node, res);
        }
        (res, deleted)
    }
}

/// Nodes the pin's `SelectCodeCommon` leaves as they are ([L3351][common]).
///
/// [common]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3351
fn needs_no_selection(opcode: Opcode) -> bool {
    matches!(
        opcode,
        isd::EntryToken
            | isd::BasicBlock
            | isd::Register
            | isd::RegisterMask
            | isd::HANDLENODE
            | isd::MDNODE_SDNODE
            | isd::TargetConstant
            | isd::TargetConstantFP
            | isd::TargetConstantPool
            | isd::TargetFrameIndex
            | isd::TargetExternalSymbol
            | isd::MCSymbol
            | isd::TargetBlockAddress
            | isd::TargetJumpTable
            | isd::TargetGlobalTLSAddress
            | isd::TargetGlobalAddress
            | isd::TokenFactor
            | isd::CopyFromReg
            | isd::CopyToReg
            | isd::EH_LABEL
            | isd::ANNOTATION_LABEL
            | isd::LIFETIME_START
            | isd::LIFETIME_END
            | isd::PSEUDO_PROBE
            | isd::DEACTIVATION_SYMBOL
    )
}

/// The nodes `SelectCodeCommon` makes machine nodes of before the table
/// ([L3362][special]): `UNDEF`, `FREEZE` and the fences, and the asserts, which
/// only mark a value. `false` for the rest.
///
/// [special]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L3362
fn select_special<D: Dag>(
    dag: &mut D,
    node: u32,
) -> bool {
    let result = SDValue::new(node, 0);
    let operand = |dag: &D| dag.operand(node, 0);
    let (opcode, ops): (u16, Vec<SDValue>) = match dag.opcode(node) {
        // Assertions say something about a value and are not code: its users
        // read the operand.
        isd::AssertSext | isd::AssertZext | isd::AssertAlign | isd::AssertNoFPClass => {
            let source = operand(dag);
            dag.replace_uses(result, source);
            dag.remove_dead_node(node);
            return true;
        }
        isd::UNDEF | isd::POISON => (target_opcode::IMPLICIT_DEF, Vec::new()),
        isd::FREEZE => (target_opcode::COPY, vec![operand(dag)]),
        isd::ARITH_FENCE => (target_opcode::ARITH_FENCE, vec![operand(dag)]),
        isd::MEMBARRIER => (target_opcode::MEMBARRIER, vec![operand(dag)]),
        _ => return false,
    };
    // `SelectNodeTo(N, Opc, VT, Ops)`: the node becomes the machine node.
    let vt = dag.value_type(result);
    let (selected, _) = dag.morph_node_to(node, opcode, &[vt], &ops);
    if selected != node {
        dag.replace_node(node, selected);
    }
    true
}

/// `SignExtend64(Val, MVT(VT).getFixedSizeInBits())`.
fn sign_extend(
    value: i64,
    vt: Vt,
) -> i64 {
    let bits = if vt == mvt::i8 {
        8
    } else if vt == mvt::i16 {
        16
    } else if vt == mvt::i32 {
        32
    } else {
        64
    };
    let shift = 64 - bits;
    (value << shift) >> shift
}

/// `HandleMergeInputChains` ([L2834][merge]): the chain the new nodes take as
/// input, or `None` when merging the matched nodes' chains would make a cycle.
///
/// [merge]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L2834
fn handle_merge_input_chains<D: Dag>(
    dag: &mut D,
    chain_nodes_matched: &[Option<u32>],
    input_glue: Option<SDValue>,
) -> Option<SDValue> {
    const MAX: usize = 8192;
    let matched: Vec<u32> = chain_nodes_matched.iter().flatten().copied().collect();
    if matched.len() == 1 {
        return Some(dag.operand(matched[0], 0));
    }

    let mut visited = crate::dag::Visited::new();
    let mut worklist: Vec<u32> = Vec::new();
    let mut input_chains: Vec<SDValue> = Vec::new();

    fn add_chains<D: Dag>(
        dag: &D,
        v: SDValue,
        visited: &mut crate::dag::Visited,
        chains: &mut Vec<SDValue>,
    ) {
        if dag.value_type(v) != mvt::Other || dag.opcode(v.node) == isd::EntryToken || visited.contains(v.node) {
            return;
        }
        visited.insert(v.node);
        if dag.opcode(v.node) == isd::TokenFactor {
            for i in 0..dag.num_operands(v.node) {
                add_chains(dag, dag.operand(v.node, i), visited, chains);
            }
        } else {
            chains.push(v);
        }
    }

    for &n in &matched {
        worklist.push(n);
        visited.insert(n);
    }
    while let Some(n) = worklist.pop() {
        add_chains(dag, dag.operand(n, 0), &mut visited, &mut input_chains);
    }

    if input_chains.is_empty() {
        return Some(dag.entry_node());
    }

    visited.clear();
    for &v in &input_chains {
        // A token factor would defeat a glue the chain node also produces.
        let last = SDValue::new(v.node, (dag.num_values(v.node) - 1) as u8);
        if input_chains.len() != 1 && dag.value_type(last) == mvt::Glue && input_glue.is_some_and(|g| g.node == v.node)
        {
            return None;
        }
        worklist.push(v.node);
    }
    for &n in &matched {
        if dag.has_predecessor_helper(n, &mut visited, &mut worklist, MAX) {
            return None;
        }
    }

    if input_chains.len() == 1 {
        return Some(input_chains[0]);
    }
    Some(dag.token_factor(&input_chains))
}

/// `UpdateChains` ([L2767][update]): the matched nodes' chain results now come
/// from the input chain the new nodes ended with.
///
/// [update]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGISel.cpp#L2767
fn update_chains<D: Dag>(
    dag: &mut D,
    node_to_match: u32,
    input_chain: Option<SDValue>,
    chain_nodes_matched: &mut [Option<u32>],
    is_morph_node_to: bool,
) {
    let mut now_dead: Vec<u32> = Vec::new();
    if !chain_nodes_matched.is_empty() {
        let input_chain = input_chain.expect("matched input chains but didn't produce a chain");
        for chain_node in chain_nodes_matched.iter().copied() {
            let Some(chain_node) = chain_node else { continue };
            if chain_node == node_to_match && is_morph_node_to {
                continue;
            }
            let mut last = dag.num_values(chain_node) - 1;
            if dag.value_type(SDValue::new(chain_node, last as u8)) == mvt::Glue {
                last -= 1;
            }
            let chain_val = SDValue::new(chain_node, last as u8);
            debug_assert!(dag.value_type(chain_val) == mvt::Other, "not a chain");
            if dag.opcode(chain_node) != isd::TokenFactor {
                dag.replace_uses(chain_val, input_chain);
            }
            if chain_node != node_to_match && dag.use_empty(chain_node) && !now_dead.contains(&chain_node) {
                now_dead.push(chain_node);
            }
        }
    }
    if !now_dead.is_empty() {
        dag.remove_dead_nodes(&now_dead);
    }
}
