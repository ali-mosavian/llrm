//! `combine_sink` against InstCombine's `tryToSinkInstruction` at the pin:
//! where an instruction goes, and what keeps it where it is.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::valuetracking::Query;

use crate::combine_sink::{droppable_uses, sink, sink_block, sinkable};
use crate::testing::{parsed, printed};

/// What the driver does with the instruction defining `name` in `@f`: the
/// function's text after, and the block it is in (`None` if it did not move).
fn sunk(
    text: &str,
    name: &str,
) -> (String, Option<String>) {
    let mut module = parsed(text);
    let layout = DataLayout::default();
    let (inst, dest, drops) = {
        let function = module
            .globals
            .iter()
            .find(|one| one.name.as_deref() == Some("f"))
            .and_then(|one| one.function())
            .expect("@f");
        let dt = DomTree::of(function);
        let mut q = Query::new(&module, function, &layout);
        q.dt = Some(&dt);
        let inst = function
            .walk()
            .map(|(_, one)| one)
            .find(|&one| {
                function.instruction(one).result.is_some_and(|v| function.value(v).name.as_deref() == Some(name))
            })
            .expect("a named instruction");
        match sink_block(&q, inst).filter(|&dest| sinkable(&q, inst, dest)) {
            Some(dest) => (inst, dest, droppable_uses(&q, inst, dest)),
            None => return (printed(&module), None),
        }
    };
    let (context, function) = module.function_mut("f").expect("@f");
    sink(context, function, inst, dest, &drops);
    let text = printed(&module);
    let block = block_of(&text, &format!("%{name} ="));
    (text, Some(block))
}

/// The label of the block whose lines include `line`.
fn block_of(
    text: &str,
    line: &str,
) -> String {
    let mut label = "";
    for one in text.lines() {
        if let Some(name) = one.strip_suffix(':') {
            label = name;
        }
        if one.trim_start().starts_with(line) {
            return label.to_owned();
        }
    }
    panic!("no {line} in {text}");
}

const DIAMOND: &str = "define i32 @f(i32 %a, i1 %c) {
b0:
  %x = add i32 %a, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i32 %x

b2:
  ret i32 0
}
";

/// A value read only in a successor whose only predecessor is its block moves
/// there, ahead of that block's other instructions.
#[test]
fn a_value_read_in_one_successor_moves_there() {
    let (text, block) = sunk(DIAMOND, "x");
    assert_eq!(block.as_deref(), Some("b1"), "{text}");
    assert_eq!(block_of(&text, "ret i32 %x"), "b1");
}

/// Read in both successors: no one block, it stays.
#[test]
fn a_value_read_in_two_blocks_stays() {
    let text = DIAMOND.replace("ret i32 0", "ret i32 %x");
    assert_eq!(sunk(&text, "x").1, None);
}

/// Read in its own block: nothing to sink to.
#[test]
fn a_value_read_in_its_own_block_stays() {
    let text =
        DIAMOND.replace("br i1 %c, label %b1, label %b2", "%y = add i32 %x, 1\n  br i1 %c, label %b1, label %b2");
    assert_eq!(sunk(&text, "x").1, None);
}

const JOIN: &str = "define i32 @f(i32 %a, i1 %c) {
b0:
  %x = add i32 %a, 1
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %r = add i32 %x, 1
  ret i32 %r
}
";

/// A user in a join (two predecessors) that goes on to other blocks would need
/// its edges split: it stays.
#[test]
fn a_value_read_in_a_join_that_continues_stays() {
    let text = JOIN.replace("ret i32 %r", "br label %b4\n\nb4:\n  ret i32 %r");
    assert_eq!(sunk(&text, "x").1, None);
}

/// A join that ends the function runs once whichever way it is reached:
/// `succ_empty` lets the value sink into it.
#[test]
fn a_value_read_in_a_returning_join_moves_there() {
    assert_eq!(sunk(JOIN, "x").1.as_deref(), Some("b3"));
}

/// A phi reads its value in the block that value comes from.
#[test]
fn a_phi_reads_in_its_incoming_block() {
    let text = "define i32 @f(i32 %a, i1 %c) {
b0:
  %x = add i32 %a, 1
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i32 [ %x, %b1 ], [ 0, %b2 ]
  ret i32 %p
}
";
    assert_eq!(sunk(text, "x").1.as_deref(), Some("b1"));
}

const LOAD: &str = "define i32 @f(ptr %p, i1 %c) {
b0:
  %x = load i32, ptr %p
  store i32 1, ptr %p
  br i1 %c, label %b1, label %b2

b1:
  ret i32 %x

b2:
  ret i32 0
}
";

/// A load with a store after it in its block would read another value if
/// moved.
#[test]
fn a_load_with_a_store_after_it_stays() {
    assert_eq!(sunk(LOAD, "x").1, None);
}

/// With nothing after it that writes, the load moves to a block whose only
/// predecessor is its own.
#[test]
fn a_load_with_no_write_after_it_moves() {
    let text = LOAD.replace("  store i32 1, ptr %p\n", "");
    assert_eq!(sunk(&text, "x").1.as_deref(), Some("b1"));
}

/// A load into a join cannot be checked on the other path: it stays, even in a
/// returning join.
#[test]
fn a_load_into_a_join_stays() {
    let text = JOIN.replace("%x = add i32 %a, 1", "%x = load i32, ptr %q").replace("i32 %a, i1", "i32 %a, ptr %q, i1");
    assert_eq!(sunk(&text, "x").1, None);
}

/// `!invariant.load` reads what no store changes: the store does not matter.
#[test]
fn an_invariant_load_moves_past_a_store() {
    let text = LOAD.replace("load i32, ptr %p", "load i32, ptr %p, !invariant.load !0") + "\n!0 = !{}\n";
    assert_eq!(sunk(&text, "x").1.as_deref(), Some("b1"));
}

/// A call that may write (or throw) is not moved.
#[test]
fn a_call_with_unknown_effects_stays() {
    let text = "declare i32 @g()

define i32 @f(i32 %a, i1 %c) {
b0:
  %x = call i32 @g()
  br i1 %c, label %b1, label %b2

b1:
  ret i32 %x

b2:
  ret i32 0
}
";
    assert_eq!(sunk(text, "x").1, None);
}

/// An alloca stays in the entry block.
#[test]
fn an_alloca_stays() {
    let text = DIAMOND
        .replace("%x = add i32 %a, 1", "%x = alloca i32")
        .replace("ret i32 %x", "%v = load i32, ptr %x\n  ret i32 %v");
    assert_eq!(sunk(&text, "x").1, None);
}

/// An `llvm.assume` of the value is droppable: it does not stop the sink, and
/// outside the destination it reads `true` after.
#[test]
fn an_assume_of_the_value_does_not_stop_the_sink_and_reads_true() {
    let text = "declare void @llvm.assume(i1)

define i32 @f(i32 %a, i1 %c) {
b0:
  %x = icmp eq i32 %a, 1
  call void @llvm.assume(i1 %x)
  br i1 %c, label %b1, label %b2

b1:
  %r = zext i1 %x to i32
  ret i32 %r

b2:
  ret i32 0
}
";
    let (after, block) = sunk(text, "x");
    assert_eq!(block.as_deref(), Some("b1"), "{after}");
    assert!(after.contains("call void @llvm.assume(i1 true)"), "{after}");
}

/// Old: `sink_block` looked up the callee of every user of every visited
/// instruction to tell an assume from the rest (11,159 visits in d_faces -O2),
/// and built a predecessor list for the user's block. `isDroppable` reads a
/// callee only of a call.
#[test]
fn test_a_user_that_is_not_a_call_costs_no_callee_lookup() {
    use llrm_mir::counts::{Event, get};
    let plain = "define i32 @f(i32 %x) {\nentry:\n  %a = add i32 %x, 1\n  br label %b\nb:\n  ret i32 %a\n}\n";
    let before = get(Event::SinkAssumeLookups);
    let (_, moved) = sunk(plain, "a");
    assert_eq!(moved.as_deref(), Some("b"));
    assert_eq!(get(Event::SinkAssumeLookups), before);

    let call = "declare void @use(i32)\ndefine void @f(i32 %x) {\nentry:\n  %a = add i32 %x, 1\n  br label %b\nb:\n  call void @use(i32 %a)\n  ret void\n}\n";
    let before = get(Event::SinkAssumeLookups);
    let (_, moved) = sunk(call, "a");
    assert_eq!(moved.as_deref(), Some("b"));
    assert_eq!(get(Event::SinkAssumeLookups), before + 1);
}

/// `IF a$ < b$ THEN ... WHILE a$(j%) < "C" AND ...`: InstCombine sank the
/// compare of B$SCMP's flags into its user's block, and isel refused the
/// function ("a call's flags read apart from its compare with zero") at -O2
/// (`string_compare_shapes.bas`). A compare of a flags result stays by its
/// call.
#[test]
fn a_compare_of_a_flags_result_is_not_sunk() {
    let text = "declare i16 @llrm.qb.B$SCMP(ptr, ptr) flagsresult
define i16 @f(ptr %a, ptr %b, i1 %p) {
b0:
  %s = call i16 @llrm.qb.B$SCMP(ptr %a, ptr %b)
  %c = icmp slt i16 %s, 0
  br i1 %p, label %b1, label %b2

b1:
  %r = zext i1 %c to i16
  ret i16 %r

b2:
  ret i16 0
}
";
    assert_eq!(sunk(text, "c").1, None);
}
