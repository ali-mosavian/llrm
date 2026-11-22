//! `IndirectBrExpandPass`: a computed goto as a `switch` over block numbers.

use llrm_mir::passes::PassManager;
use llrm_mir::program::Program;

use super::indirectbrexpand::IndirectBrExpand;
use crate::testing::{parsed, printed};

const LAYOUT: &str = "target datalayout = \"e-p:16:16-i16:16-n8:16\"\n";

fn expanded(body: &str) -> String {
    let mut program =
        Program::new(vec![parsed(&format!("{LAYOUT}{body}"))], std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add_module(IndirectBrExpand);
    manager.run(&mut program).unwrap();
    printed(&program.modules[0])
}

/// Before: a target with no jump through a register refused the function
/// ("a jump through no table", "CannotSelect(BrInd)"); now it is a switch over
/// the numbers the table holds, and no block address is left.
#[test]
fn an_indirectbr_is_a_switch_over_the_numbers_its_table_holds() {
    let text = expanded(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]

define i16 @f(i16 %x) {
entry:
  %p = getelementptr [2 x ptr], ptr @t, i16 0, i16 %x
  %a = load ptr, ptr %p
  indirectbr ptr %a, [label %one, label %two]

one:
  ret i16 1

two:
  ret i16 2
}
",
    );
    assert!(!text.contains("indirectbr") && !text.contains("blockaddress"), "{text}");
    assert!(text.contains("switch i16 %switch_cast, label %one [\n    i16 2, label %two\n  ]"), "{text}");
    assert!(text.contains("inttoptr (i16 1 to ptr), ptr inttoptr (i16 2 to ptr)"), "{text}");
}

/// Two jumps share one switch, the address a phi (`switch_bb`); a phi of a
/// target takes what each jump gave it, and `undef` from a jump that did not
/// list the block.
#[test]
fn two_jumps_share_a_switch_and_a_target_phi_takes_undef_from_the_other() {
    let text = expanded(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]

define i16 @f(i16 %x, ptr %a, ptr %b) {
entry:
  %c = icmp eq i16 %x, 0
  br i1 %c, label %left, label %right

left:
  indirectbr ptr %a, [label %one, label %two]

right:
  indirectbr ptr %b, [label %one]

one:
  %r = phi i16 [ 5, %left ], [ 6, %right ]
  ret i16 %r

two:
  %s = phi i16 [ 7, %left ]
  ret i16 %s
}
",
    );
    assert!(!text.contains("indirectbr") && !text.contains("blockaddress"), "{text}");
    assert!(text.contains("switch_bb:") && text.contains("%switch_value = phi i16"), "{text}");
    assert!(text.contains("undef"), "{text}");
}

/// A jump to blocks whose address nothing takes can reach none: `unreachable`,
/// and the phis of the blocks it listed lose the edge.
#[test]
fn a_jump_to_blocks_no_address_names_is_unreachable() {
    let text = expanded(
        "define i16 @f(ptr %a) {
entry:
  indirectbr ptr %a, [label %one]

one:
  %r = phi i16 [ 1, %entry ], [ 2, %other ]
  ret i16 %r

other:
  br label %one
}
",
    );
    assert!(text.contains("unreachable") && !text.contains("indirectbr"), "{text}");
    assert!(!text.contains("[ 1, %entry ]"), "{text}");
}

/// A table whose `indirectbr` folded away still named blocks by address; a
/// target with no label to take (the 6502's data writer) refused the table.
/// The addresses become numbers.
#[test]
fn a_block_address_nothing_jumps_to_is_a_number() {
    let text = expanded(
        "@t = internal constant [1 x ptr] [ptr blockaddress(@f, %one)]

define i16 @f(i16 %x) {
entry:
  br label %one

one:
  ret i16 1
}
",
    );
    assert!(!text.contains("blockaddress"), "{text}");
    assert!(text.contains("inttoptr (i16 1 to ptr)"), "{text}");
}

/// A function with no `indirectbr` is not touched: no switch, no numbers.
#[test]
fn a_function_without_an_indirectbr_is_left_as_it_is() {
    let body = "define i16 @f(i16 %x) {
entry:
  %c = icmp eq i16 %x, 0
  br i1 %c, label %one, label %two

one:
  ret i16 1

two:
  ret i16 2
}
";
    let before = printed(&parsed(&format!("{LAYOUT}{body}")));
    assert_eq!(expanded(body), before);
}

/// Block numbers start at one in each function: the pass numbers the blocks
/// of a function, not of the module.
#[test]
fn block_numbers_start_at_one_in_each_function() {
    let text = expanded(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %a), ptr blockaddress(@g, %b)]

define i16 @f(ptr %p) {
entry:
  indirectbr ptr %p, [label %a]

a:
  ret i16 1
}

define i16 @g(ptr %p) {
entry:
  indirectbr ptr %p, [label %b]

b:
  ret i16 2
}
",
    );
    assert!(!text.contains("blockaddress"), "{text}");
    assert_eq!(text.matches("inttoptr (i16 1 to ptr)").count(), 2, "{text}");
}

/// A block listed twice is one case, not two (a switch with a repeated value
/// does not verify).
#[test]
fn a_block_listed_twice_is_one_case() {
    let text = expanded(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %one), ptr blockaddress(@f, %two)]

define i16 @f(ptr %p) {
entry:
  indirectbr ptr %p, [label %one, label %two, label %one]

one:
  ret i16 1

two:
  ret i16 2
}
",
    );
    assert!(!text.contains("indirectbr"), "{text}");
    assert_eq!(text.matches("i16 1, label %one").count() + text.matches("i16 2, label %two").count(), 1, "{text}");
}
