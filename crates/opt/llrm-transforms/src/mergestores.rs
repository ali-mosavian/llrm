//! Adjacent copies through registers become wider ones: LLVM's
//! `DAGCombiner::mergeConsecutiveStores` for stores of loaded values
//! (`tryStoreMergeOfLoads`).
//!
//! # How it works
//! In straight-line code, a store of a value another instruction loaded, the
//! only use of that load, is a copy of a cell. Copies of adjacent cells to
//! adjacent cells, `a[0] = b[0]; a[1] = b[1]`, are one wider load and one wider
//! store when the bytes they make are a legal integer (`getConsecutiveStores`
//! finds the run, the widest legal prefix is taken, the rest is tried again):
//!
//! ```text
//!   %x = load i16, ptr %b            %w = load i32, ptr %b
//!   store i16 %x, ptr %a      =>     store i32 %w, ptr %a
//!   %y = load i16, ptr %b2
//!   store i16 %y, ptr %a2
//! ```
//!
//! Between the first and the last of the copies of a span nothing else may
//! touch memory (a call, a store of anything else, a load whose value goes
//! elsewhere ends the span), and no object may be read in the span that is also
//! written in it, so the wider load and store can stand anywhere among the
//! narrow ones. The wider load goes before the load of the lowest address and
//! the wider store before the last of the narrow stores.
//!
//! DEVIATE M1 (no number): the DAG finds candidates through a shared chain root
//! and checks cycles through operands (`getStoreMergeCandidates`,
//! `checkMergeStoreCandidatesForDependencies`); MIR has no chains, so a span of
//! straight-line code with no other memory operation, in objects that are told
//! apart (`MemRef::object`), is the same fact. Exit: none; MemorySSA could
//! widen the span.
//!
//! DEVIATE M2 (no number): no vector types, no rotate of a reversed pair, no
//! truncating store; the target has none of them for integers here
//! (`TLI.isTypeLegal` is `largest_legal_integer`), and a load or store of
//! any alignment is fast on x86.
//!
//! # Example
//! A struct of two words copied field by field in 16-bit code is a dword load
//! and a dword store.
//!
//! # Runs
//! In `loopidioms::Merge`, after the scalar passes have settled.
//!
//! # Used by
//! `loopidioms::Merge`.

use llrm_analysis::memory::{MemRef, Unit};
use llrm_mir::context::Context;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::memory::{self, Callees};
use llrm_mir::module::{Function, InstId, Operand, ValueId};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::Outer;
use llrm_support::hash::IndexMap;

/// One copy of a cell: a load whose only use is a store, and where each reads
/// and writes.
#[derive(Clone)]
struct Pair {
    load: InstId,
    store: InstId,
    /// The store's place in the block.
    store_order: usize,
    src: Operand,
    dst: Operand,
    src_pointer: Operand,
    dst_pointer: Operand,
    src_low: i64,
    dst_low: i64,
    /// Bytes copied.
    width: i64,
}

/// The cell `pointer` names: its object and the displacement in it.
fn cell(
    unit: &Unit,
    pointer: Operand,
    width: u32,
) -> Option<(Operand, i64)> {
    let reference = MemRef::at(unit, pointer, width);
    let root =
        reference.root.filter(|_| reference.object && reference.base.is_none() && reference.segment.is_none())?;
    Some((root, reference.disp))
}

/// The wider copies the span's pairs make: runs of adjacent cells of one
/// width, each cut into the widest legal integers.
fn chunks(
    unit: &Unit,
    span: &[Pair],
    reads: &[Operand],
    legal: u32,
) -> Vec<Vec<Pair>> {
    // An object read in the span that is also written in it: the wider copy
    // could not stand anywhere among the narrow ones.
    if span.iter().any(|one| reads.contains(&one.dst)) {
        return Vec::new();
    }
    let mut groups: Vec<Vec<Pair>> = Vec::new();
    for pair in span {
        let space = (unit.space(pair.src_pointer), unit.space(pair.dst_pointer));
        match groups.iter_mut().find(|group| {
            group[0].src == pair.src
                && group[0].dst == pair.dst
                && (unit.space(group[0].src_pointer), unit.space(group[0].dst_pointer)) == space
        }) {
            Some(group) => group.push(pair.clone()),
            None => groups.push(vec![pair.clone()]),
        }
    }
    let mut made = Vec::new();
    for mut group in groups {
        group.sort_by_key(|one| one.dst_low);
        // Two stores to one byte keep their order: leave the object alone.
        if group.windows(2).any(|two| two[1].dst_low < two[0].dst_low + two[0].width) {
            continue;
        }
        let mut run: Vec<Pair> = Vec::new();
        let mut cut = |run: &mut Vec<Pair>| {
            let mut rest = std::mem::take(run);
            while rest.len() >= 2 {
                let width = rest[0].width as u32 * 8;
                // The widest prefix whose bytes are a legal integer.
                let take = (2..=rest.len())
                    .rev()
                    .find(|&count| matches!(width * count as u32, 16 | 32 | 64) && width * count as u32 <= legal);
                let Some(take) = take else {
                    rest.remove(0);
                    continue;
                };
                let tail = rest.split_off(take);
                made.push(std::mem::replace(&mut rest, tail));
            }
        };
        for pair in group {
            if run.last().is_some_and(|last| {
                last.dst_low + last.width != pair.dst_low
                    || last.src_low + last.width != pair.src_low
                    || last.width != pair.width
            }) {
                cut(&mut run);
            }
            run.push(pair);
        }
        cut(&mut run);
    }
    made
}

/// `function` with each run of copies of adjacent cells to adjacent cells made
/// one wider copy; whether any was.
pub fn merged_copies(
    context: &mut Context,
    layout: &DataLayout,
    callees: &Callees,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let legal = layout.largest_legal_integer();
    let mut plan: Vec<Vec<Pair>> = Vec::new();
    {
        let unit = Unit::within(context, layout, function, outer);
        for block in function.layout() {
            let mut loads: IndexMap<ValueId, InstId> = IndexMap::default();
            let mut span: Vec<Pair> = Vec::new();
            let mut reads: Vec<Operand> = Vec::new();
            let mut flush = |span: &mut Vec<Pair>, reads: &mut Vec<Operand>, loads: &mut IndexMap<ValueId, InstId>| {
                plan.extend(chunks(&unit, span, reads, legal));
                span.clear();
                reads.clear();
                loads.clear();
            };
            for (order, &inst) in function.block(*block).instructions().iter().enumerate() {
                let op = function.instruction(inst);
                match op.opcode {
                    Opcode::Load { volatile: false, .. } => {
                        let result = op.result.expect("a load's value");
                        let width = unit.int_bits(Operand::Value(result)).filter(|bits| matches!(bits, 8 | 16 | 32));
                        let users = function.users(result);
                        let single_store = users.len() == 1
                            && matches!(
                                function.instruction(users[0].user).opcode,
                                Opcode::Store { volatile: false, .. }
                            )
                            && users[0].index == 0;
                        match (width, single_store, width.and_then(|bits| cell(&unit, op.operands[0], bits / 8))) {
                            (Some(_), true, Some((root, _))) => {
                                reads.push(root);
                                loads.insert(result, inst);
                            }
                            _ => flush(&mut span, &mut reads, &mut loads),
                        }
                    }
                    Opcode::Store { volatile: false, .. } => {
                        let paired = match op.operands[0] {
                            Operand::Value(value) => loads.get(&value).copied().map(|load| (value, load)),
                            _ => None,
                        };
                        let Some((value, load)) = paired else {
                            flush(&mut span, &mut reads, &mut loads);
                            continue;
                        };
                        let bits = unit.int_bits(Operand::Value(value)).expect("an integer");
                        let width = bits / 8;
                        let source = function.instruction(load).operands[0];
                        let (Some((src, src_low)), Some((dst, dst_low))) =
                            (cell(&unit, source, width), cell(&unit, op.operands[1], width))
                        else {
                            flush(&mut span, &mut reads, &mut loads);
                            continue;
                        };
                        span.push(Pair {
                            load,
                            store: inst,
                            store_order: order,
                            src,
                            dst,
                            src_pointer: source,
                            dst_pointer: op.operands[1],
                            src_low,
                            dst_low,
                            width: i64::from(width),
                        });
                    }
                    _ if memory::speculatable(unit.context, callees, function, inst) => {}
                    _ => flush(&mut span, &mut reads, &mut loads),
                }
            }
            flush(&mut span, &mut reads, &mut loads);
        }
    }
    for chunk in &plan {
        let bits = (chunk[0].width * chunk.len() as i64 * 8) as u32;
        let ty = context.types.int(bits);
        let lowest = chunk.iter().min_by_key(|one| one.dst_low).expect("a run has copies");
        let last = chunk.iter().max_by_key(|one| one.store_order).expect("a run has copies");
        let load = function.create_instruction(
            Opcode::Load { align: None, volatile: false },
            ty,
            vec![lowest.src_pointer],
            Flags::default(),
            None,
        );
        function.insert(load, Position::Before(lowest.load)).expect("a placed load");
        let void = context.types.void();
        let wide = function.instruction(load).result.expect("a load's value");
        let store = function.create_instruction(
            Opcode::Store { align: None, volatile: false },
            void,
            vec![Operand::Value(wide), lowest.dst_pointer],
            Flags::default(),
            None,
        );
        function.insert(store, Position::Before(last.store)).expect("a placed store");
        for one in chunk {
            function.erase(one.store).expect("a store has no result");
        }
        for one in chunk {
            function.erase(one.load).expect("a load with no use left");
        }
    }
    !plan.is_empty()
}

#[cfg(test)]
mod tests {
    use llrm_analysis::testing::{DOS, layout};
    use llrm_mir::passes::Outer;

    use super::merged_copies;
    use crate::testing::{parsed, printed, results};

    /// `text` with its copies merged: the printed form, and whether any were.
    /// The function computes the same on `inputs` before and after.
    fn merge(
        text: &str,
        inputs: &[&[i128]],
    ) -> (String, bool) {
        let before = parsed(&format!("{DOS}{text}"));
        let mut module = before.clone();
        let (layout, outer) =
            (layout(&module), Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default()))));
        let callees = llrm_mir::memory::callees(&module);
        let (context, function) = module.function_mut("f").expect("@f");
        let changed = merged_copies(context, &layout, &callees, function, &outer);
        let after = printed(&module);
        assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
        (after, changed)
    }

    const INPUTS: &[&[i128]] = &[&[0], &[1]];

    /// Two word fields copied one by one from `@b` to `@a`.
    fn two_words(order: [usize; 2]) -> String {
        let steps = order
            .iter()
            .map(|at| {
                format!(
                    "  %s{at} = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 {at}\n  %d{at} = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 {at}\n  %v{at} = load i16, ptr %s{at}\n  store i16 %v{at}, ptr %d{at}\n"
                )
            })
            .collect::<String>();
        format!(
            "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {{\n{steps}  %r0 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 0\n  %r1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %a0 = load i16, ptr %r0\n  %a1 = load i16, ptr %r1\n  %t = shl i16 %a1, 8\n  %u = add i16 %a0, %t\n  %w = add i16 %u, %x\n  ret i16 %w\n}}\n"
        )
    }

    /// `a[0] = b[0]; a[1] = b[1]` is a dword load and a dword store.
    #[test]
    fn test_two_adjacent_word_copies_are_one_dword_copy() {
        let (after, changed) = merge(&two_words([0, 1]), INPUTS);
        assert!(changed, "{after}");
        assert!(after.contains("load i32") && after.contains("store i32"), "{after}");
        assert_eq!(after.matches("store").count(), 1, "{after}");
    }

    /// The copies are found by address, whichever is written first.
    #[test]
    fn test_the_copies_are_merged_whichever_is_written_first() {
        let (after, changed) = merge(&two_words([1, 0]), INPUTS);
        assert!(changed && after.contains("store i32"), "{after}");
    }

    /// A call between the copies may read or write either cell: nothing is
    /// merged.
    #[test]
    fn test_a_call_between_the_copies_keeps_them_apart() {
        let text = "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\ndefine void @g() {\n  store i16 5, ptr @b\n  ret void\n}\n\ndefine i16 @f(i16 %x) {\n  %s1 = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 1\n  %d1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %v0 = load i16, ptr @b\n  store i16 %v0, ptr @a\n  call void @g()\n  %v1 = load i16, ptr %s1\n  store i16 %v1, ptr %d1\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed && !after.contains("load i32"), "{after}");
    }

    /// Cells of one object copied within it (`a[1] = a[0]`) may overlap what is
    /// read: nothing is merged.
    #[test]
    fn test_copies_within_one_object_are_kept() {
        let text = "@a = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {\n  %p1 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 1\n  %p2 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 2\n  %p3 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 3\n  %v0 = load i16, ptr @a\n  store i16 %v0, ptr %p2\n  %v1 = load i16, ptr %p1\n  store i16 %v1, ptr %p3\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed, "{after}");
    }

    /// Cells that are not next to each other are not one copy.
    #[test]
    fn test_copies_with_a_gap_between_the_cells_are_kept() {
        let text = "@a = global [4 x i16] zeroinitializer\n@b = global [4 x i16] [i16 11, i16 22, i16 33, i16 44]\n\ndefine i16 @f(i16 %x) {\n  %s2 = getelementptr inbounds [4 x i16], ptr @b, i16 0, i16 2\n  %d2 = getelementptr inbounds [4 x i16], ptr @a, i16 0, i16 2\n  %v0 = load i16, ptr @b\n  store i16 %v0, ptr @a\n  %v2 = load i16, ptr %s2\n  store i16 %v2, ptr %d2\n  ret i16 %x\n}\n";
        let (after, changed) = merge(text, INPUTS);
        assert!(!changed, "{after}");
    }

    /// Four byte copies are one dword copy (the widest legal integer), not two
    /// words.
    #[test]
    fn test_four_byte_copies_are_one_dword_copy() {
        let steps: String = (0..4)
            .map(|at| {
                format!(
                    "  %s{at} = getelementptr inbounds [8 x i8], ptr @b, i16 0, i16 {at}\n  %d{at} = getelementptr inbounds [8 x i8], ptr @a, i16 0, i16 {at}\n  %v{at} = load i8, ptr %s{at}\n  store i8 %v{at}, ptr %d{at}\n"
                )
            })
            .collect();
        let text = format!(
            "@a = global [8 x i8] zeroinitializer\n@b = global [8 x i8] [i8 1, i8 2, i8 3, i8 4, i8 5, i8 6, i8 7, i8 8]\n\ndefine i8 @f(i8 %x) {{\n{steps}  %p = getelementptr inbounds [8 x i8], ptr @a, i16 0, i16 3\n  %r = load i8, ptr %p\n  %w = add i8 %r, %x\n  ret i8 %w\n}}\n"
        );
        let (after, changed) = merge(&text, &[&[0], &[3]]);
        assert!(changed && after.contains("load i32") && !after.contains("store i8"), "{after}");
        assert_eq!(after.matches("store i32").count(), 1, "{after}");
    }
}
